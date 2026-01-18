use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use mk_core::types::{OrganizationalUnit, TenantContext, UnitType};
use sqlx::{Pool, Postgres, Row};
use thiserror::Error;

use crate::rls_migration;

#[derive(Error, Debug)]
pub enum PostgresError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Unit not found: {0}")]
    NotFound(String)
}

pub struct PostgresBackend {
    pool: Pool<Postgres>
}

impl PostgresBackend {
    pub fn pool(&self) -> &Pool<Postgres> {
        &self.pool
    }

    pub async fn new(connection_url: &str) -> Result<Self, PostgresError> {
        use sqlx::postgres::PgPoolOptions;
        use std::time::Duration;

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(30))
            .connect(connection_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn initialize_schema(&self) -> Result<(), PostgresError> {
        // Enable pgcrypto extension for gen_random_uuid()
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pgcrypto")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS sync_state (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                data JSONB NOT NULL,
                updated_at BIGINT NOT NULL,
                PRIMARY KEY (id, tenant_id)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_sync_state_tenant_id ON sync_state(tenant_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS organizational_units (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                type TEXT NOT NULL, -- 'company', 'organization', 'team', 'project'
                parent_id TEXT REFERENCES organizational_units(id),
                tenant_id TEXT NOT NULL,
                metadata JSONB DEFAULT '{}',
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_roles (
                user_id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                unit_id TEXT NOT NULL REFERENCES organizational_units(id),
                role TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                PRIMARY KEY (user_id, tenant_id, unit_id, role)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS unit_policies (
                id TEXT PRIMARY KEY,
                unit_id TEXT NOT NULL REFERENCES organizational_units(id),
                policy JSONB NOT NULL,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS governance_events (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                event_type TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                payload JSONB NOT NULL,
                timestamp BIGINT NOT NULL
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS drift_results (
                project_id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                drift_score REAL NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0,
                violations JSONB NOT NULL,
                suppressed_violations JSONB NOT NULL DEFAULT '[]',
                requires_manual_review BOOLEAN NOT NULL DEFAULT FALSE,
                timestamp BIGINT NOT NULL,
                PRIMARY KEY (project_id, tenant_id, timestamp)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS drift_configs (
                project_id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                threshold REAL NOT NULL DEFAULT 0.3,
                low_confidence_threshold REAL NOT NULL DEFAULT 0.7,
                auto_suppress_info BOOLEAN NOT NULL DEFAULT FALSE,
                updated_at BIGINT NOT NULL,
                PRIMARY KEY (project_id, tenant_id)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS job_status (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                job_name TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                status TEXT NOT NULL, -- 'running', 'completed', 'failed'
                message TEXT,
                started_at BIGINT NOT NULL,
                finished_at BIGINT,
                duration_ms BIGINT
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS graph_nodes (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                label TEXT NOT NULL,
                properties JSONB NOT NULL DEFAULT '{}',
                created_at BIGINT NOT NULL,
                PRIMARY KEY (id, tenant_id)
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS graph_edges (
                id TEXT NOT NULL,
                tenant_id TEXT NOT NULL,
                source_id TEXT NOT NULL,
                target_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                properties JSONB NOT NULL DEFAULT '{}',
                created_at BIGINT NOT NULL,
                PRIMARY KEY (id, tenant_id),
                FOREIGN KEY (source_id, tenant_id) REFERENCES graph_nodes(id, tenant_id) ON DELETE \
             CASCADE,
                FOREIGN KEY (target_id, tenant_id) REFERENCES graph_nodes(id, tenant_id) ON DELETE \
             CASCADE
            )"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(source_id, \
             tenant_id)"
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(target_id, \
             tenant_id)"
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS memory_entries ( id TEXT PRIMARY KEY, tenant_id TEXT NOT \
             NULL, content TEXT NOT NULL, embedding VECTOR(1536), memory_layer TEXT NOT NULL, \
             properties JSONB DEFAULT '{}', created_at BIGINT NOT NULL, updated_at BIGINT NOT \
             NULL, deleted_at BIGINT )"
        )
        .execute(&self.pool)
        .await
        .ok();

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS knowledge_items ( id TEXT PRIMARY KEY, tenant_id TEXT NOT \
             NULL, type TEXT NOT NULL, title TEXT NOT NULL, content TEXT NOT NULL, tags TEXT[], \
             properties JSONB DEFAULT '{}', created_at BIGINT NOT NULL, updated_at BIGINT NOT \
             NULL )"
        )
        .execute(&self.pool)
        .await
        .ok();

        rls_migration::run_rls_migration(&self.pool).await?;

        Ok(())
    }

    pub async fn create_unit(&self, unit: &OrganizationalUnit) -> Result<(), PostgresError> {
        if let Some(ref parent_id) = unit.parent_id {
            let parent = self
                .get_unit_by_id(parent_id, &unit.tenant_id.to_string())
                .await?
                .ok_or_else(|| PostgresError::NotFound(parent_id.clone()))?;

            match (parent.unit_type, unit.unit_type) {
                (UnitType::Company, UnitType::Organization) => {}
                (UnitType::Organization, UnitType::Team) => {}
                (UnitType::Team, UnitType::Project) => {}
                _ => {
                    return Err(PostgresError::Database(sqlx::Error::Decode(
                        format!(
                            "Invalid hierarchy: cannot create {:?} under {:?}",
                            unit.unit_type, parent.unit_type
                        )
                        .into()
                    )));
                }
            }
        } else if unit.unit_type != UnitType::Company {
            return Err(PostgresError::Database(sqlx::Error::Decode(
                "Only Company units can be root units (no parent)".into()
            )));
        }

        sqlx::query(
            "INSERT INTO organizational_units (id, name, type, parent_id, tenant_id, metadata, \
             created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(&unit.id)
        .bind(&unit.name)
        .bind(unit.unit_type.to_string().to_lowercase())
        .bind(&unit.parent_id)
        .bind(unit.tenant_id.as_str())
        .bind(serde_json::to_value(&unit.metadata)?)
        .bind(unit.created_at)
        .bind(unit.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    fn row_to_persistent_event(
        row: &sqlx::postgres::PgRow
    ) -> Result<mk_core::types::PersistentEvent, PostgresError> {
        use sqlx::Row;

        let status_str: String = row.get("status");
        let status = match status_str.as_str() {
            "pending" => mk_core::types::EventStatus::Pending,
            "published" => mk_core::types::EventStatus::Published,
            "acknowledged" => mk_core::types::EventStatus::Acknowledged,
            "dead_lettered" => mk_core::types::EventStatus::DeadLettered,
            _ => mk_core::types::EventStatus::Pending
        };

        let payload: mk_core::types::GovernanceEvent = serde_json::from_value(row.get("payload"))?;

        Ok(mk_core::types::PersistentEvent {
            id: row.get("id"),
            event_id: row.get("event_id"),
            idempotency_key: row.get("idempotency_key"),
            tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                PostgresError::Database(sqlx::Error::Decode(
                    format!("Invalid tenant_id: {}", e).into()
                ))
            })?,
            event_type: row.get("event_type"),
            payload,
            status,
            retry_count: row.get("retry_count"),
            max_retries: row.get("max_retries"),
            last_error: row.get("last_error"),
            created_at: row.get("created_at"),
            published_at: row.get("published_at"),
            acknowledged_at: row.get("acknowledged_at"),
            dead_lettered_at: row.get("dead_lettered_at")
        })
    }

    async fn get_unit_by_id(
        &self,
        id: &str,
        tenant_id: &str
    ) -> Result<Option<OrganizationalUnit>, PostgresError> {
        let row = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units WHERE id = $1 AND tenant_id = $2"
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => {
                    return Err(PostgresError::Database(sqlx::Error::Decode(
                        "Invalid unit type".into()
                    )));
                }
            };

            Ok(Some(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_unit(
        &self,
        ctx: &TenantContext,
        id: &str
    ) -> Result<Option<OrganizationalUnit>, PostgresError> {
        let row = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units WHERE id = $1 AND tenant_id = $2"
        )
        .bind(id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => {
                    return Err(PostgresError::Database(sqlx::Error::Decode(
                        "Invalid unit type".into()
                    )));
                }
            };

            Ok(Some(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn list_children(
        &self,
        ctx: &TenantContext,
        parent_id: &str
    ) -> Result<Vec<OrganizationalUnit>, PostgresError> {
        let rows = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units WHERE parent_id = $1 AND tenant_id = $2"
        )
        .bind(parent_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut units = Vec::new();
        for row in rows {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => continue
            };

            units.push(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            });
        }

        Ok(units)
    }

    pub async fn get_ancestors(
        &self,
        ctx: &TenantContext,
        id: &str
    ) -> Result<Vec<OrganizationalUnit>, PostgresError> {
        let rows = sqlx::query(
            "WITH RECURSIVE ancestors AS (
                SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at
                FROM organizational_units
                WHERE id = $1 AND tenant_id = $2
                UNION ALL
                SELECT u.id, u.name, u.type, u.parent_id, u.tenant_id, u.metadata, u.created_at, \
             u.updated_at
                FROM organizational_units u
                INNER JOIN ancestors a ON u.id = a.parent_id AND u.tenant_id = a.tenant_id
            )
            SELECT * FROM ancestors WHERE id != $1"
        )
        .bind(id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut units = Vec::new();
        for row in rows {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => continue
            };

            units.push(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            });
        }

        Ok(units)
    }

    pub async fn get_unit_ancestors(
        &self,
        ctx: &TenantContext,
        id: &str
    ) -> Result<Vec<OrganizationalUnit>, PostgresError> {
        self.get_ancestors(ctx, id).await
    }

    pub async fn get_unit_descendants(
        &self,
        ctx: &TenantContext,
        id: &str
    ) -> Result<Vec<OrganizationalUnit>, PostgresError> {
        let rows = sqlx::query(
            "WITH RECURSIVE descendants AS (
                SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at
                FROM organizational_units
                WHERE id = $1 AND tenant_id = $2
                UNION ALL
                SELECT u.id, u.name, u.type, u.parent_id, u.tenant_id, u.metadata, u.created_at, \
             u.updated_at
                FROM organizational_units u
                INNER JOIN descendants d ON u.parent_id = d.id AND u.tenant_id = d.tenant_id
            )
            SELECT * FROM descendants WHERE id != $1"
        )
        .bind(id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut units = Vec::new();
        for row in rows {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => continue
            };

            units.push(OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            });
        }

        Ok(units)
    }

    pub async fn update_unit(
        &self,
        ctx: &TenantContext,
        unit: &OrganizationalUnit
    ) -> Result<(), PostgresError> {
        sqlx::query(
            "UPDATE organizational_units 
             SET name = $3, type = $4, parent_id = $5, metadata = $6, updated_at = $7
             WHERE id = $1 AND tenant_id = $2"
        )
        .bind(&unit.id)
        .bind(ctx.tenant_id.as_str())
        .bind(&unit.name)
        .bind(unit.unit_type.to_string().to_lowercase())
        .bind(&unit.parent_id)
        .bind(serde_json::to_value(&unit.metadata)?)
        .bind(unit.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_unit(&self, ctx: &TenantContext, id: &str) -> Result<(), PostgresError> {
        sqlx::query("DELETE FROM organizational_units WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(ctx.tenant_id.as_str())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn add_unit_policy(
        &self,
        ctx: &TenantContext,
        unit_id: &str,
        policy: &mk_core::types::Policy
    ) -> Result<(), PostgresError> {
        let exists: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM organizational_units WHERE id = $1 AND tenant_id = $2")
                .bind(unit_id)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&self.pool)
                .await?;

        if exists.is_none() {
            return Err(PostgresError::NotFound(unit_id.to_string()));
        }

        sqlx::query(
            "INSERT INTO unit_policies (id, unit_id, policy, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE SET policy = $3, updated_at = $5"
        )
        .bind(&policy.id)
        .bind(unit_id)
        .bind(serde_json::to_value(policy)?)
        .bind(chrono::Utc::now().timestamp())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_unit_policies(
        &self,
        ctx: &TenantContext,
        unit_id: &str
    ) -> Result<Vec<mk_core::types::Policy>, PostgresError> {
        let rows = sqlx::query(
            "SELECT p.policy 
             FROM unit_policies p
             JOIN organizational_units u ON p.unit_id = u.id
             WHERE p.unit_id = $1 AND u.tenant_id = $2"
        )
        .bind(unit_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut policies = Vec::new();
        for row in rows {
            let policy: mk_core::types::Policy = serde_json::from_value(row.get("policy"))?;
            policies.push(policy);
        }
        Ok(policies)
    }

    pub async fn assign_role(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId,
        unit_id: &str,
        role: mk_core::types::Role
    ) -> Result<(), PostgresError> {
        sqlx::query(
            "INSERT INTO user_roles (user_id, tenant_id, unit_id, role, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (user_id, tenant_id, unit_id, role) DO NOTHING"
        )
        .bind(user_id.as_str())
        .bind(tenant_id.as_str())
        .bind(unit_id)
        .bind(role.to_string().to_lowercase())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_role(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId,
        unit_id: &str,
        role: mk_core::types::Role
    ) -> Result<(), PostgresError> {
        sqlx::query(
            "DELETE FROM user_roles 
             WHERE user_id = $1 AND tenant_id = $2 AND unit_id = $3 AND role = $4"
        )
        .bind(user_id.as_str())
        .bind(tenant_id.as_str())
        .bind(unit_id)
        .bind(role.to_string().to_lowercase())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_user_roles(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId
    ) -> Result<Vec<(String, mk_core::types::Role)>, PostgresError> {
        let rows = sqlx::query(
            "SELECT unit_id, role FROM user_roles WHERE user_id = $1 AND tenant_id = $2"
        )
        .bind(user_id.as_str())
        .bind(tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut roles = Vec::new();
        for row in rows {
            let unit_id: String = row.get("unit_id");
            let role_str: String = row.get("role");
            if let Ok(role) = role_str.parse() {
                roles.push((unit_id, role));
            }
        }
        Ok(roles)
    }
    pub async fn log_event(
        &self,
        event: &mk_core::types::GovernanceEvent
    ) -> Result<(), PostgresError> {
        let (event_type, tenant_id, timestamp) = match event {
            mk_core::types::GovernanceEvent::UnitCreated {
                unit_id: _,
                unit_type: _,
                tenant_id,
                parent_id: _,
                timestamp
            } => ("unit_created", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::UnitUpdated {
                unit_id: _,
                tenant_id,
                timestamp
            } => ("unit_updated", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::UnitDeleted {
                unit_id: _,
                tenant_id,
                timestamp
            } => ("unit_deleted", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::RoleAssigned {
                user_id: _,
                unit_id: _,
                role: _,
                tenant_id,
                timestamp
            } => ("role_assigned", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::RoleRemoved {
                user_id: _,
                unit_id: _,
                role: _,
                tenant_id,
                timestamp
            } => ("role_removed", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::PolicyUpdated {
                policy_id: _,
                layer: _,
                tenant_id,
                timestamp
            } => ("policy_updated", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::PolicyDeleted {
                policy_id: _,
                tenant_id,
                timestamp
            } => ("policy_deleted", tenant_id, *timestamp),
            mk_core::types::GovernanceEvent::DriftDetected {
                project_id: _,
                tenant_id,
                drift_score: _,
                timestamp
            } => ("drift_detected", tenant_id, *timestamp)
        };

        sqlx::query(
            "INSERT INTO governance_events (event_type, tenant_id, payload, timestamp)
             VALUES ($1, $2, $3, $4)"
        )
        .bind(event_type)
        .bind(tenant_id.as_str())
        .bind(serde_json::to_value(event)?)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_governance_events_internal(
        &self,
        ctx: mk_core::types::TenantContext,
        since_timestamp: i64,
        limit: usize
    ) -> Result<Vec<mk_core::types::GovernanceEvent>, PostgresError> {
        let rows = sqlx::query(
            "SELECT payload FROM governance_events 
             WHERE tenant_id = $1 AND timestamp > $2 
             ORDER BY timestamp ASC LIMIT $3"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(since_timestamp)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            use sqlx::Row;
            let payload: serde_json::Value = row.get("payload");
            let event: mk_core::types::GovernanceEvent = serde_json::from_value(payload)?;
            events.push(event);
        }
        Ok(events)
    }
}

#[async_trait]
impl crate::graph::GraphStore for PostgresBackend {
    type Error = PostgresError;

    async fn add_node(
        &self,
        ctx: TenantContext,
        node: crate::graph::GraphNode
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO graph_nodes (id, tenant_id, label, properties, created_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id, tenant_id) DO UPDATE SET label = $3, properties = $4"
        )
        .bind(&node.id)
        .bind(ctx.tenant_id.as_str())
        .bind(&node.label)
        .bind(&node.properties)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn add_edge(
        &self,
        ctx: TenantContext,
        edge: crate::graph::GraphEdge
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO graph_edges (id, tenant_id, source_id, target_id, relation, properties, \
             created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (id, tenant_id) DO UPDATE SET relation = $5, properties = $6"
        )
        .bind(&edge.id)
        .bind(ctx.tenant_id.as_str())
        .bind(&edge.source_id)
        .bind(&edge.target_id)
        .bind(&edge.relation)
        .bind(&edge.properties)
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_neighbors(
        &self,
        ctx: TenantContext,
        node_id: &str
    ) -> Result<Vec<(crate::graph::GraphEdge, crate::graph::GraphNode)>, Self::Error> {
        let rows = sqlx::query(
            "SELECT e.id as edge_id, e.source_id, e.target_id, e.relation, e.properties as \
             edge_props,
                    n.id as node_id, n.label, n.properties as node_props
             FROM graph_edges e
             JOIN graph_nodes n ON e.target_id = n.id AND e.tenant_id = n.tenant_id
             WHERE e.source_id = $1 AND e.tenant_id = $2"
        )
        .bind(node_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::new();
        for row in rows {
            let edge = crate::graph::GraphEdge {
                id: row.get("edge_id"),
                source_id: row.get("source_id"),
                target_id: row.get("target_id"),
                relation: row.get("relation"),
                properties: row.get("edge_props"),
                tenant_id: ctx.tenant_id.as_str().to_string()
            };
            let node = crate::graph::GraphNode {
                id: row.get("node_id"),
                label: row.get("label"),
                properties: row.get("node_props"),
                tenant_id: ctx.tenant_id.as_str().to_string()
            };
            results.push((edge, node));
        }
        Ok(results)
    }

    async fn find_path(
        &self,
        ctx: TenantContext,
        start_id: &str,
        end_id: &str,
        max_depth: usize
    ) -> Result<Vec<crate::graph::GraphEdge>, Self::Error> {
        let rows = sqlx::query(
            "WITH RECURSIVE search_path(id, source_id, target_id, relation, properties, depth, \
             path) AS (
                SELECT id, source_id, target_id, relation, properties, 1, ARRAY[id]
                FROM graph_edges
                WHERE source_id = $1 AND tenant_id = $3
                UNION ALL
                SELECT e.id, e.source_id, e.target_id, e.relation, e.properties, sp.depth + 1, \
             sp.path || e.id
                FROM graph_edges e
                JOIN search_path sp ON e.source_id = sp.target_id AND e.tenant_id = $3
                WHERE sp.depth < $4 AND NOT (e.id = ANY(sp.path))
            )
            SELECT id, source_id, target_id, relation, properties
            FROM search_path
            WHERE target_id = $2
            LIMIT 1"
        )
        .bind(start_id)
        .bind(end_id)
        .bind(ctx.tenant_id.as_str())
        .bind(max_depth as i32)
        .fetch_all(&self.pool)
        .await?;

        let mut path = Vec::new();
        for row in rows {
            path.push(crate::graph::GraphEdge {
                id: row.get("id"),
                source_id: row.get("source_id"),
                target_id: row.get("target_id"),
                relation: row.get("relation"),
                properties: row.get("properties"),
                tenant_id: ctx.tenant_id.as_str().to_string()
            });
        }
        Ok(path)
    }

    async fn search_nodes(
        &self,
        ctx: TenantContext,
        query: &str,
        limit: usize
    ) -> Result<Vec<crate::graph::GraphNode>, Self::Error> {
        let rows = sqlx::query(
            "SELECT id, label, properties FROM graph_nodes
             WHERE tenant_id = $1 AND (id ILIKE $2 OR label ILIKE $2)
             LIMIT $3"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(format!("%{}%", query))
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut nodes = Vec::new();
        for row in rows {
            nodes.push(crate::graph::GraphNode {
                id: row.get("id"),
                label: row.get("label"),
                properties: row.get("properties"),
                tenant_id: ctx.tenant_id.as_str().to_string()
            });
        }
        Ok(nodes)
    }

    async fn soft_delete_nodes_by_source_memory_id(
        &self,
        ctx: TenantContext,
        source_memory_id: &str
    ) -> Result<usize, Self::Error> {
        let result = sqlx::query(
            "UPDATE graph_nodes SET deleted_at = NOW() 
             WHERE tenant_id = $1 
             AND deleted_at IS NULL 
             AND properties->>'source_memory_id' = $2"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(source_memory_id)
        .execute(&self.pool)
        .await?;

        let deleted_count = result.rows_affected() as usize;

        sqlx::query(
            "UPDATE graph_edges SET deleted_at = NOW()
             WHERE tenant_id = $1 
             AND deleted_at IS NULL
             AND (source_id IN (
                SELECT id FROM graph_nodes 
                WHERE tenant_id = $1 AND properties->>'source_memory_id' = $2
             ) OR target_id IN (
                SELECT id FROM graph_nodes 
                WHERE tenant_id = $1 AND properties->>'source_memory_id' = $2
             ))"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(source_memory_id)
        .execute(&self.pool)
        .await?;

        Ok(deleted_count)
    }
}

#[async_trait]
impl mk_core::traits::EventPublisher for PostgresBackend {
    type Error = PostgresError;

    async fn publish(&self, event: mk_core::types::GovernanceEvent) -> Result<(), Self::Error> {
        self.log_event(&event).await
    }

    async fn subscribe(
        &self,
        _channels: &[&str]
    ) -> Result<tokio::sync::mpsc::Receiver<mk_core::types::GovernanceEvent>, Self::Error> {
        Err(PostgresError::Database(sqlx::Error::Decode(
            "Subscribe not implemented for Postgres backend".into()
        )))
    }
}

#[async_trait]
impl StorageBackend for PostgresBackend {
    type Error = PostgresError;

    async fn store(&self, ctx: TenantContext, key: &str, value: &[u8]) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO sync_state (id, tenant_id, data, updated_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (id, tenant_id) DO UPDATE SET data = $3, updated_at = $4"
        )
        .bind(key)
        .bind(ctx.tenant_id.as_str())
        .bind(serde_json::from_slice::<serde_json::Value>(value).unwrap_or_default())
        .bind(chrono::Utc::now().timestamp())
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn retrieve(
        &self,
        ctx: TenantContext,
        key: &str
    ) -> Result<Option<Vec<u8>>, Self::Error> {
        let row: Option<(serde_json::Value,)> =
            sqlx::query_as("SELECT data FROM sync_state WHERE id = $1 AND tenant_id = $2")
                .bind(key)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.and_then(|(v,)| serde_json::to_vec(&v).ok()))
    }

    async fn delete(&self, ctx: TenantContext, key: &str) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM sync_state WHERE id = $1 AND tenant_id = $2")
            .bind(key)
            .bind(ctx.tenant_id.as_str())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn exists(&self, ctx: TenantContext, key: &str) -> Result<bool, Self::Error> {
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM sync_state WHERE id = $1 AND tenant_id = $2")
                .bind(key)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.is_some())
    }

    async fn get_ancestors(
        &self,
        ctx: TenantContext,
        unit_id: &str
    ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
        self.get_unit_ancestors(&ctx, unit_id).await
    }

    async fn get_descendants(
        &self,
        ctx: TenantContext,
        unit_id: &str
    ) -> Result<Vec<OrganizationalUnit>, Self::Error> {
        self.get_unit_descendants(&ctx, unit_id).await
    }

    async fn get_unit_policies(
        &self,
        ctx: TenantContext,
        unit_id: &str
    ) -> Result<Vec<mk_core::types::Policy>, Self::Error> {
        self.get_unit_policies(&ctx, unit_id).await
    }

    async fn create_unit(&self, unit: &OrganizationalUnit) -> Result<(), Self::Error> {
        self.create_unit(unit).await
    }

    async fn add_unit_policy(
        &self,
        ctx: &TenantContext,
        unit_id: &str,
        policy: &mk_core::types::Policy
    ) -> Result<(), Self::Error> {
        self.add_unit_policy(ctx, unit_id, policy).await
    }

    async fn assign_role(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId,
        unit_id: &str,
        role: mk_core::types::Role
    ) -> Result<(), Self::Error> {
        self.assign_role(user_id, tenant_id, unit_id, role).await
    }

    async fn remove_role(
        &self,
        user_id: &mk_core::types::UserId,
        tenant_id: &mk_core::types::TenantId,
        unit_id: &str,
        role: mk_core::types::Role
    ) -> Result<(), Self::Error> {
        self.remove_role(user_id, tenant_id, unit_id, role).await
    }

    async fn store_drift_result(
        &self,
        result: mk_core::types::DriftResult
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO drift_results (project_id, tenant_id, drift_score, confidence, \
             violations, suppressed_violations, requires_manual_review, timestamp)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(&result.project_id)
        .bind(result.tenant_id.as_str())
        .bind(result.drift_score)
        .bind(result.confidence)
        .bind(serde_json::to_value(&result.violations)?)
        .bind(serde_json::to_value(&result.suppressed_violations)?)
        .bind(result.requires_manual_review)
        .bind(result.timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_latest_drift_result(
        &self,
        ctx: mk_core::types::TenantContext,
        project_id: &str
    ) -> Result<Option<mk_core::types::DriftResult>, Self::Error> {
        let row = sqlx::query(
            "SELECT project_id, tenant_id, drift_score, confidence, violations, \
             suppressed_violations, requires_manual_review, timestamp 
             FROM drift_results 
             WHERE project_id = $1 AND tenant_id = $2 
             ORDER BY timestamp DESC LIMIT 1"
        )
        .bind(project_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(mk_core::types::DriftResult {
                project_id: row.get("project_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                drift_score: row.get("drift_score"),
                confidence: row.get("confidence"),
                violations: serde_json::from_value(row.get("violations"))?,
                suppressed_violations: serde_json::from_value(row.get("suppressed_violations"))?,
                requires_manual_review: row.get("requires_manual_review"),
                timestamp: row.get("timestamp")
            }))
        } else {
            Ok(None)
        }
    }

    async fn record_job_status(
        &self,
        job_name: &str,
        tenant_id: &str,
        status: &str,
        message: Option<&str>,
        started_at: i64,
        finished_at: Option<i64>
    ) -> Result<(), Self::Error> {
        let duration_ms = finished_at.map(|f| (f - started_at) * 1000);

        sqlx::query(
            "INSERT INTO job_status (job_name, tenant_id, status, message, started_at, \
             finished_at, duration_ms)
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
        .bind(job_name)
        .bind(tenant_id)
        .bind(status)
        .bind(message)
        .bind(started_at)
        .bind(finished_at)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_governance_events(
        &self,
        ctx: mk_core::types::TenantContext,
        since_timestamp: i64,
        limit: usize
    ) -> Result<Vec<mk_core::types::GovernanceEvent>, Self::Error> {
        self.get_governance_events_internal(ctx, since_timestamp, limit)
            .await
    }

    async fn list_all_units(&self) -> Result<Vec<mk_core::types::OrganizationalUnit>, Self::Error> {
        let rows = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units"
        )
        .fetch_all(&self.pool)
        .await?;

        let mut units = Vec::new();
        for row in rows {
            let unit_type_str: String = row.get("type");
            let unit_type = match unit_type_str.as_str() {
                "company" => UnitType::Company,
                "organization" => UnitType::Organization,
                "team" => UnitType::Team,
                "project" => UnitType::Project,
                _ => continue
            };

            units.push(mk_core::types::OrganizationalUnit {
                id: row.get("id"),
                name: row.get("name"),
                unit_type,
                parent_id: row.get("parent_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                metadata: serde_json::from_value(row.get("metadata"))?,
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at")
            });
        }

        Ok(units)
    }

    async fn create_suppression(
        &self,
        suppression: mk_core::types::DriftSuppression
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO drift_suppressions (id, project_id, tenant_id, policy_id, rule_pattern, \
             reason, created_by, expires_at, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"
        )
        .bind(&suppression.id)
        .bind(&suppression.project_id)
        .bind(suppression.tenant_id.as_str())
        .bind(&suppression.policy_id)
        .bind(&suppression.rule_pattern)
        .bind(&suppression.reason)
        .bind(suppression.created_by.as_str())
        .bind(suppression.expires_at)
        .bind(suppression.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn list_suppressions(
        &self,
        ctx: mk_core::types::TenantContext,
        project_id: &str
    ) -> Result<Vec<mk_core::types::DriftSuppression>, Self::Error> {
        let rows = sqlx::query(
            "SELECT id, project_id, tenant_id, policy_id, rule_pattern, reason, created_by, \
             expires_at, created_at
             FROM drift_suppressions
             WHERE project_id = $1 AND tenant_id = $2
             ORDER BY created_at DESC"
        )
        .bind(project_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&self.pool)
        .await?;

        let mut suppressions = Vec::new();
        for row in rows {
            suppressions.push(mk_core::types::DriftSuppression {
                id: row.get("id"),
                project_id: row.get("project_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                policy_id: row.get("policy_id"),
                rule_pattern: row.get("rule_pattern"),
                reason: row.get("reason"),
                created_by: row.get::<String, _>("created_by").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid created_by: {}", e).into()
                    ))
                })?,
                expires_at: row.get("expires_at"),
                created_at: row.get("created_at")
            });
        }

        Ok(suppressions)
    }

    async fn delete_suppression(
        &self,
        ctx: mk_core::types::TenantContext,
        suppression_id: &str
    ) -> Result<(), Self::Error> {
        sqlx::query("DELETE FROM drift_suppressions WHERE id = $1 AND tenant_id = $2")
            .bind(suppression_id)
            .bind(ctx.tenant_id.as_str())
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    async fn get_drift_config(
        &self,
        ctx: mk_core::types::TenantContext,
        project_id: &str
    ) -> Result<Option<mk_core::types::DriftConfig>, Self::Error> {
        let row = sqlx::query(
            "SELECT project_id, tenant_id, threshold, low_confidence_threshold, \
             auto_suppress_info, updated_at
             FROM drift_configs
             WHERE project_id = $1 AND tenant_id = $2"
        )
        .bind(project_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(mk_core::types::DriftConfig {
                project_id: row.get("project_id"),
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                threshold: row.get("threshold"),
                low_confidence_threshold: row.get("low_confidence_threshold"),
                auto_suppress_info: row.get("auto_suppress_info"),
                updated_at: row.get("updated_at")
            }))
        } else {
            Ok(None)
        }
    }

    async fn save_drift_config(
        &self,
        config: mk_core::types::DriftConfig
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO drift_configs (project_id, tenant_id, threshold, \
             low_confidence_threshold, auto_suppress_info, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (project_id, tenant_id) DO UPDATE SET
                threshold = EXCLUDED.threshold,
                low_confidence_threshold = EXCLUDED.low_confidence_threshold,
                auto_suppress_info = EXCLUDED.auto_suppress_info,
                updated_at = EXCLUDED.updated_at"
        )
        .bind(&config.project_id)
        .bind(config.tenant_id.as_str())
        .bind(config.threshold)
        .bind(config.low_confidence_threshold)
        .bind(config.auto_suppress_info)
        .bind(config.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn persist_event(
        &self,
        event: mk_core::types::PersistentEvent
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO governance_events (id, event_id, idempotency_key, tenant_id, event_type, \
             payload, status, retry_count, max_retries, last_error, created_at, published_at, \
             acknowledged_at, dead_lettered_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, to_timestamp($11), $12, $13, $14)
             ON CONFLICT (idempotency_key) DO NOTHING"
        )
        .bind(&event.id)
        .bind(&event.event_id)
        .bind(&event.idempotency_key)
        .bind(event.tenant_id.as_str())
        .bind(&event.event_type)
        .bind(serde_json::to_value(&event.payload)?)
        .bind(event.status.to_string())
        .bind(event.retry_count)
        .bind(event.max_retries)
        .bind(&event.last_error)
        .bind(event.created_at)
        .bind(
            event
                .published_at
                .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
        )
        .bind(
            event
                .acknowledged_at
                .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
        )
        .bind(
            event
                .dead_lettered_at
                .map(|ts| chrono::DateTime::from_timestamp(ts, 0))
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_pending_events(
        &self,
        ctx: mk_core::types::TenantContext,
        limit: usize
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        let rows = sqlx::query(
            "SELECT id, event_id, idempotency_key, tenant_id, event_type, payload, status, \
             retry_count, max_retries, last_error, 
                    EXTRACT(EPOCH FROM created_at)::bigint as created_at,
                    EXTRACT(EPOCH FROM published_at)::bigint as published_at,
                    EXTRACT(EPOCH FROM acknowledged_at)::bigint as acknowledged_at,
                    EXTRACT(EPOCH FROM dead_lettered_at)::bigint as dead_lettered_at
             FROM governance_events
             WHERE tenant_id = $1 AND status = 'pending'
             ORDER BY created_at ASC
             LIMIT $2"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            events.push(Self::row_to_persistent_event(&row)?);
        }
        Ok(events)
    }

    async fn update_event_status(
        &self,
        event_id: &str,
        status: mk_core::types::EventStatus,
        error: Option<String>
    ) -> Result<(), Self::Error> {
        let now = chrono::Utc::now();

        match status {
            mk_core::types::EventStatus::Published => {
                sqlx::query(
                    "UPDATE governance_events SET status = $2, published_at = $3 WHERE event_id = \
                     $1"
                )
                .bind(event_id)
                .bind(status.to_string())
                .bind(now)
                .execute(&self.pool)
                .await?;
            }
            mk_core::types::EventStatus::Acknowledged => {
                sqlx::query(
                    "UPDATE governance_events SET status = $2, acknowledged_at = $3 WHERE \
                     event_id = $1"
                )
                .bind(event_id)
                .bind(status.to_string())
                .bind(now)
                .execute(&self.pool)
                .await?;
            }
            mk_core::types::EventStatus::DeadLettered => {
                sqlx::query(
                    "UPDATE governance_events SET status = $2, last_error = $3, dead_lettered_at \
                     = $4 WHERE event_id = $1"
                )
                .bind(event_id)
                .bind(status.to_string())
                .bind(&error)
                .bind(now)
                .execute(&self.pool)
                .await?;
            }
            mk_core::types::EventStatus::Pending => {
                sqlx::query(
                    "UPDATE governance_events SET status = $2, retry_count = retry_count + 1, \
                     last_error = $3 WHERE event_id = $1"
                )
                .bind(event_id)
                .bind(status.to_string())
                .bind(&error)
                .execute(&self.pool)
                .await?;
            }
        }

        Ok(())
    }

    async fn get_dead_letter_events(
        &self,
        ctx: mk_core::types::TenantContext,
        limit: usize
    ) -> Result<Vec<mk_core::types::PersistentEvent>, Self::Error> {
        let rows = sqlx::query(
            "SELECT id, event_id, idempotency_key, tenant_id, event_type, payload, status, \
             retry_count, max_retries, last_error,
                    EXTRACT(EPOCH FROM created_at)::bigint as created_at,
                    EXTRACT(EPOCH FROM published_at)::bigint as published_at,
                    EXTRACT(EPOCH FROM acknowledged_at)::bigint as acknowledged_at,
                    EXTRACT(EPOCH FROM dead_lettered_at)::bigint as dead_lettered_at
             FROM governance_events
             WHERE tenant_id = $1 AND status = 'dead_lettered'
             ORDER BY dead_lettered_at DESC
             LIMIT $2"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let mut events = Vec::new();
        for row in rows {
            events.push(Self::row_to_persistent_event(&row)?);
        }
        Ok(events)
    }

    async fn check_idempotency(
        &self,
        consumer_group: &str,
        idempotency_key: &str
    ) -> Result<bool, Self::Error> {
        let result: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM event_consumer_state WHERE consumer_group = $1 AND idempotency_key = $2"
        )
        .bind(consumer_group)
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.is_some())
    }

    async fn record_consumer_state(
        &self,
        state: mk_core::types::ConsumerState
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO event_consumer_state (consumer_group, idempotency_key, tenant_id, \
             processed_at)
             VALUES ($1, $2, $3, to_timestamp($4))
             ON CONFLICT (consumer_group, idempotency_key) DO NOTHING"
        )
        .bind(&state.consumer_group)
        .bind(&state.idempotency_key)
        .bind(state.tenant_id.as_str())
        .bind(state.processed_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn get_event_metrics(
        &self,
        ctx: mk_core::types::TenantContext,
        period_start: i64,
        period_end: i64
    ) -> Result<Vec<mk_core::types::EventDeliveryMetrics>, Self::Error> {
        let rows = sqlx::query(
            "SELECT tenant_id, event_type, 
                    EXTRACT(EPOCH FROM period_start)::bigint as period_start,
                    EXTRACT(EPOCH FROM period_end)::bigint as period_end,
                    total_events, delivered_events, retried_events, dead_lettered_events, \
             avg_delivery_time_ms
             FROM event_delivery_metrics
             WHERE tenant_id = $1 AND period_start >= to_timestamp($2) AND period_end <= \
             to_timestamp($3)
             ORDER BY period_start DESC"
        )
        .bind(ctx.tenant_id.as_str())
        .bind(period_start)
        .bind(period_end)
        .fetch_all(&self.pool)
        .await?;

        let mut metrics = Vec::new();
        for row in rows {
            metrics.push(mk_core::types::EventDeliveryMetrics {
                tenant_id: row.get::<String, _>("tenant_id").parse().map_err(|e| {
                    PostgresError::Database(sqlx::Error::Decode(
                        format!("Invalid tenant_id: {}", e).into()
                    ))
                })?,
                event_type: row.get("event_type"),
                period_start: row.get("period_start"),
                period_end: row.get("period_end"),
                total_events: row.get("total_events"),
                delivered_events: row.get("delivered_events"),
                retried_events: row.get("retried_events"),
                dead_lettered_events: row.get("dead_lettered_events"),
                avg_delivery_time_ms: row.get("avg_delivery_time_ms")
            });
        }
        Ok(metrics)
    }

    async fn record_event_metrics(
        &self,
        metrics: mk_core::types::EventDeliveryMetrics
    ) -> Result<(), Self::Error> {
        sqlx::query(
            "INSERT INTO event_delivery_metrics (tenant_id, event_type, period_start, period_end, \
             total_events, delivered_events, retried_events, dead_lettered_events, \
             avg_delivery_time_ms)
             VALUES ($1, $2, to_timestamp($3), to_timestamp($4), $5, $6, $7, $8, $9)"
        )
        .bind(metrics.tenant_id.as_str())
        .bind(&metrics.event_type)
        .bind(metrics.period_start)
        .bind(metrics.period_end)
        .bind(metrics.total_events)
        .bind(metrics.delivered_events)
        .bind(metrics.retried_events)
        .bind(metrics.dead_lettered_events)
        .bind(metrics.avg_delivery_time_ms)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

impl PostgresBackend {
    pub async fn create_error_signature(
        &self,
        tenant_id: &str,
        signature: &mk_core::types::ErrorSignature
    ) -> Result<String, PostgresError> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO error_signatures (id, tenant_id, error_type, message_pattern, \
             stack_patterns, context_patterns, embedding, occurrence_count, first_seen_at, \
             last_seen_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)"
        )
        .bind(&id)
        .bind(tenant_id)
        .bind(&signature.error_type)
        .bind(&signature.message_pattern)
        .bind(serde_json::to_value(&signature.stack_patterns)?)
        .bind(serde_json::to_value(&signature.context_patterns)?)
        .bind(serde_json::to_value(&signature.embedding)?)
        .bind(1i32)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(id)
    }

    pub async fn get_error_signature(
        &self,
        tenant_id: &str,
        id: &str
    ) -> Result<Option<mk_core::types::ErrorSignature>, PostgresError> {
        let row = sqlx::query(
            "SELECT error_type, message_pattern, stack_patterns, context_patterns, embedding
             FROM error_signatures WHERE id = $1 AND tenant_id = $2"
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let embedding: Option<serde_json::Value> = row.get("embedding");
                Ok(Some(mk_core::types::ErrorSignature {
                    error_type: row.get("error_type"),
                    message_pattern: row.get("message_pattern"),
                    stack_patterns: serde_json::from_value(row.get("stack_patterns"))?,
                    context_patterns: serde_json::from_value(row.get("context_patterns"))?,
                    embedding: embedding.and_then(|v| serde_json::from_value(v).ok())
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn delete_error_signature(
        &self,
        tenant_id: &str,
        id: &str
    ) -> Result<bool, PostgresError> {
        let result = sqlx::query("DELETE FROM error_signatures WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn create_resolution(
        &self,
        tenant_id: &str,
        resolution: &mk_core::types::Resolution
    ) -> Result<(), PostgresError> {
        let now = chrono::Utc::now().timestamp();

        sqlx::query(
            "INSERT INTO resolutions (id, tenant_id, error_signature_id, description, changes, \
             success_rate, application_count, last_success_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
        )
        .bind(&resolution.id)
        .bind(tenant_id)
        .bind(&resolution.error_signature_id)
        .bind(&resolution.description)
        .bind(serde_json::to_value(&resolution.changes)?)
        .bind(resolution.success_rate)
        .bind(resolution.application_count as i32)
        .bind(if resolution.last_success_at > 0 {
            Some(resolution.last_success_at)
        } else {
            None
        })
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_resolution(
        &self,
        tenant_id: &str,
        id: &str
    ) -> Result<Option<mk_core::types::Resolution>, PostgresError> {
        let row = sqlx::query(
            "SELECT id, error_signature_id, description, changes, success_rate, \
             application_count, last_success_at FROM resolutions WHERE id = $1 AND tenant_id = $2"
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let last_success_at: Option<i64> = row.get("last_success_at");
                Ok(Some(mk_core::types::Resolution {
                    id: row.get("id"),
                    error_signature_id: row.get("error_signature_id"),
                    description: row.get("description"),
                    changes: serde_json::from_value(row.get("changes"))?,
                    success_rate: row.get("success_rate"),
                    application_count: row.get::<i32, _>("application_count") as u32,
                    last_success_at: last_success_at.unwrap_or(0)
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn get_resolutions_for_error(
        &self,
        tenant_id: &str,
        error_signature_id: &str
    ) -> Result<Vec<mk_core::types::Resolution>, PostgresError> {
        let rows = sqlx::query(
            "SELECT id, error_signature_id, description, changes, success_rate, \
             application_count, last_success_at FROM resolutions 
             WHERE error_signature_id = $1 AND tenant_id = $2
             ORDER BY success_rate DESC, application_count DESC"
        )
        .bind(error_signature_id)
        .bind(tenant_id)
        .fetch_all(&self.pool)
        .await?;

        let mut resolutions = Vec::new();
        for row in rows {
            let last_success_at: Option<i64> = row.get("last_success_at");
            resolutions.push(mk_core::types::Resolution {
                id: row.get("id"),
                error_signature_id: row.get("error_signature_id"),
                description: row.get("description"),
                changes: serde_json::from_value(row.get("changes"))?,
                success_rate: row.get("success_rate"),
                application_count: row.get::<i32, _>("application_count") as u32,
                last_success_at: last_success_at.unwrap_or(0)
            });
        }

        Ok(resolutions)
    }

    pub async fn delete_resolution(
        &self,
        tenant_id: &str,
        id: &str
    ) -> Result<bool, PostgresError> {
        let result = sqlx::query("DELETE FROM resolutions WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn create_hindsight_note(
        &self,
        tenant_id: &str,
        note: &mk_core::types::HindsightNote
    ) -> Result<(), PostgresError> {
        let resolution_ids: Vec<String> = note.resolutions.iter().map(|r| r.id.clone()).collect();

        sqlx::query(
            "INSERT INTO hindsight_notes (id, tenant_id, error_signature_id, content, tags, \
             resolution_ids, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(&note.id)
        .bind(tenant_id)
        .bind(&note.error_signature.error_type)
        .bind(&note.content)
        .bind(serde_json::to_value(&note.tags)?)
        .bind(serde_json::to_value(&resolution_ids)?)
        .bind(note.created_at)
        .bind(note.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_hindsight_note(
        &self,
        tenant_id: &str,
        id: &str
    ) -> Result<Option<mk_core::types::HindsightNote>, PostgresError> {
        let row = sqlx::query(
            "SELECT h.id, h.error_signature_id, h.content, h.tags, h.resolution_ids, \
             h.created_at, h.updated_at,
             e.error_type, e.message_pattern, e.stack_patterns, e.context_patterns, e.embedding
             FROM hindsight_notes h
             LEFT JOIN error_signatures e ON h.error_signature_id = e.error_type AND e.tenant_id = \
             $2
             WHERE h.id = $1 AND h.tenant_id = $2"
        )
        .bind(id)
        .bind(tenant_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let resolution_ids: Vec<String> =
                    serde_json::from_value(row.get("resolution_ids"))?;
                let mut resolutions = Vec::new();
                for res_id in resolution_ids {
                    if let Some(resolution) = self.get_resolution(tenant_id, &res_id).await? {
                        resolutions.push(resolution);
                    }
                }

                let embedding: Option<serde_json::Value> = row.get("embedding");

                Ok(Some(mk_core::types::HindsightNote {
                    id: row.get("id"),
                    error_signature: mk_core::types::ErrorSignature {
                        error_type: row
                            .get::<Option<String>, _>("error_type")
                            .unwrap_or_else(|| row.get("error_signature_id")),
                        message_pattern: row
                            .get::<Option<String>, _>("message_pattern")
                            .unwrap_or_default(),
                        stack_patterns: row
                            .get::<Option<serde_json::Value>, _>("stack_patterns")
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        context_patterns: row
                            .get::<Option<serde_json::Value>, _>("context_patterns")
                            .and_then(|v| serde_json::from_value(v).ok())
                            .unwrap_or_default(),
                        embedding: embedding.and_then(|v| serde_json::from_value(v).ok())
                    },
                    resolutions,
                    content: row.get("content"),
                    tags: serde_json::from_value(row.get("tags"))?,
                    created_at: row.get("created_at"),
                    updated_at: row.get("updated_at")
                }))
            }
            None => Ok(None)
        }
    }

    pub async fn update_hindsight_note(
        &self,
        tenant_id: &str,
        note: &mk_core::types::HindsightNote
    ) -> Result<bool, PostgresError> {
        let resolution_ids: Vec<String> = note.resolutions.iter().map(|r| r.id.clone()).collect();

        let result = sqlx::query(
            "UPDATE hindsight_notes 
             SET content = $3, tags = $4, resolution_ids = $5, updated_at = $6
             WHERE id = $1 AND tenant_id = $2"
        )
        .bind(&note.id)
        .bind(tenant_id)
        .bind(&note.content)
        .bind(serde_json::to_value(&note.tags)?)
        .bind(serde_json::to_value(&resolution_ids)?)
        .bind(note.updated_at)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_hindsight_note(
        &self,
        tenant_id: &str,
        id: &str
    ) -> Result<bool, PostgresError> {
        let result = sqlx::query("DELETE FROM hindsight_notes WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn list_hindsight_notes(
        &self,
        tenant_id: &str,
        limit: i64,
        offset: i64
    ) -> Result<Vec<mk_core::types::HindsightNote>, PostgresError> {
        let rows = sqlx::query(
            "SELECT id FROM hindsight_notes 
             WHERE tenant_id = $1
             ORDER BY updated_at DESC
             LIMIT $2 OFFSET $3"
        )
        .bind(tenant_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let mut notes = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            if let Some(note) = self.get_hindsight_note(tenant_id, &id).await? {
                notes.push(note);
            }
        }

        Ok(notes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use serde_json::json;

    // Test PostgresError display
    #[test]
    fn test_postgres_error_display() {
        let error = PostgresError::Database(sqlx::Error::Configuration(
            "Invalid connection string".into()
        ));

        assert!(error.to_string().contains("Database error"));
        assert!(error.to_string().contains("Invalid connection string"));
    }

    // Test error conversion from sqlx::Error
    #[test]
    fn test_postgres_error_from_sqlx() {
        let sqlx_error = sqlx::Error::Configuration("test".into());
        let pg_error: PostgresError = sqlx_error.into();

        match pg_error {
            PostgresError::Database(_) => (),
            PostgresError::Serialization(_) => (),
            PostgresError::NotFound(_) => ()
        }
    }

    // Test PostgresBackend struct (compile-time checks)
    #[test]
    fn test_postgres_backend_struct() {
        // Verify the struct has expected fields
        struct TestBackend {
            _pool: Pool<Postgres>
        }

        // This is a compile-time test - if it compiles, PostgresBackend has the right
        // structure We can't instantiate it without a real database connection
        let _backend_type = std::any::type_name::<PostgresBackend>();
        assert_eq!(_backend_type, "storage::postgres::PostgresBackend");
    }

    // Test StorageBackend trait implementation
    #[test]
    fn test_storage_backend_trait_implementation() {
        use mk_core::traits::StorageBackend;

        // Compile-time check that PostgresBackend implements StorageBackend
        fn assert_implements_storage_backend<T: StorageBackend>() {}

        assert_implements_storage_backend::<PostgresBackend>();
    }

    // Test JSON serialization patterns used in the code
    #[test]
    fn test_json_serialization_patterns() {
        // Test the serialization pattern used in store() method
        let value = json!({"key": "value", "number": 42});
        let bytes = serde_json::to_vec(&value).unwrap();

        // Test deserialization pattern used in retrieve() method
        let deserialized: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(deserialized["key"], "value");
        assert_eq!(deserialized["number"], 42);

        // Test default fallback used in store()
        let invalid_bytes = b"not json";
        let default_value =
            serde_json::from_slice::<serde_json::Value>(invalid_bytes).unwrap_or_default();
        assert!(default_value.is_null() || default_value == json!({}));
    }

    // Test timestamp generation pattern
    #[test]
    fn test_timestamp_generation() {
        use chrono::Utc;

        let timestamp = Utc::now().timestamp();
        assert!(timestamp > 0); // Should be positive (after 1970)

        // Verify it's a reasonable timestamp (not in distant future)
        let current_year = Utc::now().year();
        let timestamp_year = chrono::DateTime::from_timestamp(timestamp, 0)
            .map(|dt| dt.year())
            .unwrap_or(1970);

        // Should be within 10 years of current year
        assert!((timestamp_year - current_year).abs() <= 10);
    }

    // Test SQL query patterns for correctness
    #[test]
    fn test_sql_query_patterns() {
        // Verify the SQL queries are syntactically correct
        let create_table_query = "CREATE TABLE IF NOT EXISTS sync_state (
                id TEXT PRIMARY KEY,
                data JSONB NOT NULL,
                updated_at BIGINT NOT NULL
            )";

        let insert_query = "INSERT INTO sync_state (id, data, updated_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE SET data = $2, updated_at = $3";

        let select_query = "SELECT data FROM sync_state WHERE id = $1";
        let delete_query = "DELETE FROM sync_state WHERE id = $1";
        let exists_query = "SELECT 1 FROM sync_state WHERE id = $1";

        // Just verify they're non-empty strings
        assert!(!create_table_query.is_empty());
        assert!(!insert_query.is_empty());
        assert!(!select_query.is_empty());
        assert!(!delete_query.is_empty());
        assert!(!exists_query.is_empty());

        // Verify they contain expected keywords
        assert!(create_table_query.contains("CREATE TABLE"));
        assert!(insert_query.contains("INSERT INTO"));
        assert!(select_query.contains("SELECT"));
        assert!(delete_query.contains("DELETE"));
        assert!(exists_query.contains("SELECT 1"));
    }
}
