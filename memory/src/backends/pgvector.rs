#![cfg(feature = "pgvector")]

use super::factory::PgvectorConfig;
use super::{
    BackendCapabilities, BackendError, DeleteResult, HealthStatus, SearchQuery, SearchResult,
    UpsertResult, VectorBackend, VectorRecord,
};
use async_trait::async_trait;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use std::collections::HashMap;
use std::time::Instant;

pub struct PgvectorBackend {
    pool: PgPool,
    config: PgvectorConfig,
    embedding_dimension: usize,
}

impl PgvectorBackend {
    pub async fn new(
        config: PgvectorConfig,
        embedding_dimension: usize,
    ) -> Result<Self, BackendError> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(&config.connection_string)
            .await
            .map_err(|e| BackendError::ConnectionFailed(format!("PostgreSQL: {}", e)))?;

        let backend = Self {
            pool,
            config,
            embedding_dimension,
        };

        backend.ensure_extension().await?;
        backend.ensure_table().await?;

        Ok(backend)
    }

    async fn ensure_extension(&self) -> Result<(), BackendError> {
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                BackendError::Internal(format!("Failed to create vector extension: {}", e))
            })?;
        Ok(())
    }

    async fn ensure_table(&self) -> Result<(), BackendError> {
        let query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {schema}.{table} (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                vector vector({dim}),
                metadata JSONB DEFAULT '{{}}',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW(),
                PRIMARY KEY (tenant_id, id)
            );
            
            CREATE INDEX IF NOT EXISTS idx_{table}_vector ON {schema}.{table} 
            USING hnsw (vector vector_cosine_ops);
            
            CREATE INDEX IF NOT EXISTS idx_{table}_tenant ON {schema}.{table} (tenant_id);
            "#,
            schema = self.config.schema,
            table = self.config.table_name,
            dim = self.embedding_dimension,
        );

        sqlx::query(&query)
            .execute(&self.pool)
            .await
            .map_err(|e| BackendError::Internal(format!("Failed to create table: {}", e)))?;

        Ok(())
    }

    fn vector_to_pgvector(v: &[f32]) -> String {
        let values: Vec<String> = v.iter().map(|f| f.to_string()).collect();
        format!("[{}]", values.join(","))
    }

    fn pgvector_to_vector(s: &str) -> Vec<f32> {
        s.trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect()
    }
}

#[async_trait]
impl VectorBackend for PgvectorBackend {
    async fn health_check(&self) -> Result<HealthStatus, BackendError> {
        let start = Instant::now();

        match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => {
                let latency = start.elapsed().as_millis() as u64;
                Ok(HealthStatus::healthy("pgvector").with_latency(latency))
            }
            Err(e) => Ok(HealthStatus::unhealthy("pgvector", e.to_string())),
        }
    }

    async fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::pgvector()
    }

    async fn upsert(
        &self,
        tenant_id: &str,
        vectors: Vec<VectorRecord>,
    ) -> Result<UpsertResult, BackendError> {
        let mut count = 0;
        let mut failed_ids = Vec::new();

        for record in vectors {
            let vector_str = Self::vector_to_pgvector(&record.vector);
            let metadata = serde_json::to_value(&record.metadata)
                .map_err(|e| BackendError::Serialization(e.to_string()))?;

            let query = format!(
                r#"
                INSERT INTO {schema}.{table} (id, tenant_id, vector, metadata, updated_at)
                VALUES ($1, $2, $3::vector, $4, NOW())
                ON CONFLICT (tenant_id, id) DO UPDATE SET
                    vector = EXCLUDED.vector,
                    metadata = EXCLUDED.metadata,
                    updated_at = NOW()
                "#,
                schema = self.config.schema,
                table = self.config.table_name,
            );

            match sqlx::query(&query)
                .bind(&record.id)
                .bind(tenant_id)
                .bind(&vector_str)
                .bind(&metadata)
                .execute(&self.pool)
                .await
            {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::warn!(id = %record.id, error = %e, "Failed to upsert vector");
                    failed_ids.push(record.id);
                }
            }
        }

        Ok(UpsertResult {
            upserted_count: count,
            failed_ids,
        })
    }

    async fn search(
        &self,
        tenant_id: &str,
        query: SearchQuery,
    ) -> Result<Vec<SearchResult>, BackendError> {
        let vector_str = Self::vector_to_pgvector(&query.vector);

        let mut sql = format!(
            r#"
            SELECT id, vector::text, metadata, 1 - (vector <=> $1::vector) AS score
            FROM {schema}.{table}
            WHERE tenant_id = $2
            "#,
            schema = self.config.schema,
            table = self.config.table_name,
        );

        if let Some(threshold) = query.score_threshold {
            sql.push_str(&format!(
                " AND 1 - (vector <=> $1::vector) >= {}",
                threshold
            ));
        }

        for (key, value) in &query.filters {
            if let Some(s) = value.as_str() {
                sql.push_str(&format!(
                    " AND metadata->>'{}' = '{}'",
                    key.replace('\'', "''"),
                    s.replace('\'', "''")
                ));
            }
        }

        sql.push_str(&format!(
            " ORDER BY vector <=> $1::vector LIMIT {}",
            query.limit
        ));

        let rows = sqlx::query(&sql)
            .bind(&vector_str)
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| BackendError::Internal(format!("Search failed: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            let vector_text: String = row.get("vector");
            let metadata: serde_json::Value = row.get("metadata");
            let score: f64 = row.get("score");

            let metadata_map: HashMap<String, serde_json::Value> = metadata
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            results.push(SearchResult {
                id,
                score: score as f32,
                vector: if query.include_vectors {
                    Some(Self::pgvector_to_vector(&vector_text))
                } else {
                    None
                },
                metadata: metadata_map,
            });
        }

        Ok(results)
    }

    async fn delete(
        &self,
        tenant_id: &str,
        ids: Vec<String>,
    ) -> Result<DeleteResult, BackendError> {
        if ids.is_empty() {
            return Ok(DeleteResult::new(0));
        }

        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${}", i + 1)).collect();

        let query = format!(
            r#"
            DELETE FROM {schema}.{table}
            WHERE tenant_id = $1 AND id IN ({})
            "#,
            placeholders.join(", "),
            schema = self.config.schema,
            table = self.config.table_name,
        );

        let mut q = sqlx::query(&query).bind(tenant_id);
        for id in &ids {
            q = q.bind(id);
        }

        let result = q
            .execute(&self.pool)
            .await
            .map_err(|e| BackendError::Internal(format!("Delete failed: {}", e)))?;

        Ok(DeleteResult::new(result.rows_affected() as usize))
    }

    async fn get(&self, tenant_id: &str, id: &str) -> Result<Option<VectorRecord>, BackendError> {
        let query = format!(
            r#"
            SELECT id, vector::text, metadata
            FROM {schema}.{table}
            WHERE tenant_id = $1 AND id = $2
            "#,
            schema = self.config.schema,
            table = self.config.table_name,
        );

        let row = sqlx::query(&query)
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| BackendError::Internal(format!("Get failed: {}", e)))?;

        Ok(row.map(|r| {
            let id: String = r.get("id");
            let vector_text: String = r.get("vector");
            let metadata: serde_json::Value = r.get("metadata");

            let metadata_map: HashMap<String, serde_json::Value> = metadata
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            VectorRecord {
                id,
                vector: Self::pgvector_to_vector(&vector_text),
                metadata: metadata_map,
            }
        }))
    }

    fn backend_name(&self) -> &'static str {
        "pgvector"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_conversion() {
        let v = vec![0.1, 0.2, 0.3];
        let pgv = PgvectorBackend::vector_to_pgvector(&v);
        assert_eq!(pgv, "[0.1,0.2,0.3]");

        let back = PgvectorBackend::pgvector_to_vector(&pgv);
        assert_eq!(back.len(), 3);
        assert!((back[0] - 0.1).abs() < 0.001);
        assert!((back[1] - 0.2).abs() < 0.001);
        assert!((back[2] - 0.3).abs() < 0.001);
    }

    #[test]
    fn test_vector_conversion_empty() {
        let v: Vec<f32> = vec![];
        let pgv = PgvectorBackend::vector_to_pgvector(&v);
        assert_eq!(pgv, "[]");

        let back = PgvectorBackend::pgvector_to_vector(&pgv);
        assert!(back.is_empty());
    }
}
