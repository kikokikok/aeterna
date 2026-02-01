/// GDPR Compliance Module
/// 
/// This module provides functionality to comply with GDPR requirements:
/// - Right to be forgotten (data anonymization)
/// - Data export (data portability)
/// - Consent management
/// - Audit trail for data access

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use thiserror::Error;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum GdprError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("User not found: {0}")]
    UserNotFound(String),
    
    #[error("Export failed: {0}")]
    ExportFailed(String),
    
    #[error("Anonymization failed: {0}")]
    AnonymizationFailed(String),
    
    #[error("Consent error: {0}")]
    ConsentError(String),
}

/// GDPR consent record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GdprConsent {
    pub id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub purpose: String,
    pub granted: bool,
    pub granted_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// GDPR audit log entry
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct GdprAuditLog {
    pub id: Uuid,
    pub tenant_id: String,
    pub user_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// User data export format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserDataExport {
    pub user_id: String,
    pub tenant_id: String,
    pub exported_at: DateTime<Utc>,
    pub memories: Vec<serde_json::Value>,
    pub knowledge_items: Vec<serde_json::Value>,
    pub organizational_units: Vec<serde_json::Value>,
    pub consents: Vec<GdprConsent>,
    pub audit_logs: Vec<GdprAuditLog>,
}

/// Anonymization strategy for different data types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnonymizationStrategy {
    /// Replace with fixed value
    Replace(String),
    
    /// Hash the value
    Hash,
    
    /// Delete/null the value
    Delete,
    
    /// Redact (replace with [REDACTED])
    Redact,
}

impl Default for AnonymizationStrategy {
    fn default() -> Self {
        Self::Redact
    }
}

/// GDPR operations trait
#[async_trait]
pub trait GdprOperations: Send + Sync {
    /// Export all user data in JSON format
    async fn export_user_data(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<UserDataExport, GdprError>;
    
    /// Anonymize user data (right to be forgotten)
    async fn anonymize_user_data(
        &self,
        tenant_id: &str,
        user_id: &str,
        strategy: AnonymizationStrategy,
    ) -> Result<(), GdprError>;
    
    /// Record consent
    async fn record_consent(
        &self,
        tenant_id: &str,
        user_id: &str,
        purpose: &str,
        granted: bool,
    ) -> Result<GdprConsent, GdprError>;
    
    /// Get user consents
    async fn get_user_consents(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<Vec<GdprConsent>, GdprError>;
    
    /// Revoke consent
    async fn revoke_consent(
        &self,
        tenant_id: &str,
        user_id: &str,
        purpose: &str,
    ) -> Result<(), GdprError>;
    
    /// Log data access for audit trail
    async fn log_data_access(
        &self,
        tenant_id: &str,
        user_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(), GdprError>;
    
    /// Get audit logs for a user
    async fn get_audit_logs(
        &self,
        tenant_id: &str,
        user_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<GdprAuditLog>, GdprError>;
}

/// PostgreSQL implementation of GDPR operations
pub struct PostgresGdprStorage {
    pool: PgPool,
}

impl PostgresGdprStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    
    /// Initialize GDPR tables
    pub async fn initialize(&self) -> Result<(), GdprError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS gdpr_consents (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                purpose TEXT NOT NULL,
                granted BOOLEAN NOT NULL,
                granted_at TIMESTAMPTZ,
                revoked_at TIMESTAMPTZ,
                expires_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(tenant_id, user_id, purpose)
            );
            
            CREATE INDEX IF NOT EXISTS idx_gdpr_consents_tenant_user 
                ON gdpr_consents(tenant_id, user_id);
            
            CREATE TABLE IF NOT EXISTS gdpr_audit_logs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                tenant_id TEXT NOT NULL,
                user_id TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                ip_address TEXT,
                user_agent TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            
            CREATE INDEX IF NOT EXISTS idx_gdpr_audit_tenant_user_time
                ON gdpr_audit_logs(tenant_id, user_id, created_at DESC);
            
            -- Enable RLS for GDPR tables
            ALTER TABLE gdpr_consents ENABLE ROW LEVEL SECURITY;
            ALTER TABLE gdpr_audit_logs ENABLE ROW LEVEL SECURITY;
            
            DROP POLICY IF EXISTS tenant_isolation_gdpr_consents ON gdpr_consents;
            CREATE POLICY tenant_isolation_gdpr_consents ON gdpr_consents
                USING (tenant_id = current_setting('app.tenant_id', true));
            
            DROP POLICY IF EXISTS tenant_isolation_gdpr_audit ON gdpr_audit_logs;
            CREATE POLICY tenant_isolation_gdpr_audit ON gdpr_audit_logs
                USING (tenant_id = current_setting('app.tenant_id', true));
            "#
        )
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Anonymize memories for a user
    async fn anonymize_memories(
        &self,
        tenant_id: &str,
        user_id: &str,
        strategy: &AnonymizationStrategy,
    ) -> Result<(), GdprError> {
        let replacement = match strategy {
            AnonymizationStrategy::Replace(val) => val.clone(),
            AnonymizationStrategy::Hash => format!("ANON_{}", Uuid::new_v4()),
            AnonymizationStrategy::Delete => String::new(),
            AnonymizationStrategy::Redact => "[REDACTED]".to_string(),
        };
        
        // Anonymize memory entries
        sqlx::query(
            r#"
            UPDATE memory_entries
            SET content = $1,
                metadata = jsonb_set(
                    COALESCE(metadata, '{}'::jsonb),
                    '{anonymized}',
                    'true'::jsonb
                ),
                updated_at = NOW()
            WHERE tenant_id = $2 
                AND user_id = $3
                AND NOT (metadata->>'anonymized')::boolean IS TRUE
            "#
        )
        .bind(&replacement)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Anonymize knowledge items for a user
    async fn anonymize_knowledge(
        &self,
        tenant_id: &str,
        user_id: &str,
        strategy: &AnonymizationStrategy,
    ) -> Result<(), GdprError> {
        let replacement = match strategy {
            AnonymizationStrategy::Replace(val) => val.clone(),
            AnonymizationStrategy::Hash => format!("ANON_{}", Uuid::new_v4()),
            AnonymizationStrategy::Delete => String::new(),
            AnonymizationStrategy::Redact => "[REDACTED]".to_string(),
        };
        
        // Anonymize knowledge items
        sqlx::query(
            r#"
            UPDATE knowledge_items
            SET content = $1,
                metadata = jsonb_set(
                    COALESCE(metadata, '{}'::jsonb),
                    '{anonymized}',
                    'true'::jsonb
                ),
                updated_at = NOW()
            WHERE tenant_id = $2 
                AND created_by = $3
                AND NOT (metadata->>'anonymized')::boolean IS TRUE
            "#
        )
        .bind(&replacement)
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
}

#[async_trait]
impl GdprOperations for PostgresGdprStorage {
    async fn export_user_data(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<UserDataExport, GdprError> {
        // Set tenant context for RLS
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        
        // Export memories
        let memories: Vec<serde_json::Value> = sqlx::query_scalar(
            r#"
            SELECT jsonb_build_object(
                'id', id,
                'content', content,
                'layer', layer,
                'created_at', created_at,
                'metadata', metadata
            )
            FROM memory_entries
            WHERE tenant_id = $1 AND user_id = $2
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        
        // Export knowledge items
        let knowledge_items: Vec<serde_json::Value> = sqlx::query_scalar(
            r#"
            SELECT jsonb_build_object(
                'id', id,
                'path', path,
                'content', content,
                'created_by', created_by,
                'created_at', created_at,
                'metadata', metadata
            )
            FROM knowledge_items
            WHERE tenant_id = $1 AND created_by = $2
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        
        // Export organizational units
        let organizational_units: Vec<serde_json::Value> = sqlx::query_scalar(
            r#"
            SELECT jsonb_build_object(
                'id', id,
                'name', name,
                'layer', layer,
                'created_at', created_at,
                'metadata', metadata
            )
            FROM organizational_units
            WHERE tenant_id = $1 AND created_by = $2
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        
        // Export consents
        let consents = self.get_user_consents(tenant_id, user_id).await?;
        
        // Export audit logs (last 90 days)
        let from = Utc::now() - chrono::Duration::days(90);
        let to = Utc::now();
        let audit_logs = self.get_audit_logs(tenant_id, user_id, from, to).await?;
        
        Ok(UserDataExport {
            user_id: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            exported_at: Utc::now(),
            memories,
            knowledge_items,
            organizational_units,
            consents,
            audit_logs,
        })
    }
    
    async fn anonymize_user_data(
        &self,
        tenant_id: &str,
        user_id: &str,
        strategy: AnonymizationStrategy,
    ) -> Result<(), GdprError> {
        // Set tenant context for RLS
        sqlx::query("SELECT set_config('app.tenant_id', $1, true)")
            .bind(tenant_id)
            .execute(&self.pool)
            .await?;
        
        // Log the anonymization action
        self.log_data_access(
            tenant_id,
            user_id,
            "anonymize",
            "user_data",
            Some(user_id),
            None,
            None,
        ).await?;
        
        // Anonymize different data types
        self.anonymize_memories(tenant_id, user_id, &strategy).await?;
        self.anonymize_knowledge(tenant_id, user_id, &strategy).await?;
        
        // Revoke all consents
        sqlx::query(
            r#"
            UPDATE gdpr_consents
            SET granted = false,
                revoked_at = NOW()
            WHERE tenant_id = $1 AND user_id = $2 AND granted = true
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn record_consent(
        &self,
        tenant_id: &str,
        user_id: &str,
        purpose: &str,
        granted: bool,
    ) -> Result<GdprConsent, GdprError> {
        let consent = sqlx::query_as::<_, GdprConsent>(
            r#"
            INSERT INTO gdpr_consents 
                (tenant_id, user_id, purpose, granted, granted_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (tenant_id, user_id, purpose)
            DO UPDATE SET
                granted = $4,
                granted_at = CASE WHEN $4 THEN NOW() ELSE gdpr_consents.granted_at END,
                revoked_at = CASE WHEN NOT $4 THEN NOW() ELSE NULL END
            RETURNING *
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(purpose)
        .bind(granted)
        .bind(if granted { Some(Utc::now()) } else { None })
        .fetch_one(&self.pool)
        .await?;
        
        Ok(consent)
    }
    
    async fn get_user_consents(
        &self,
        tenant_id: &str,
        user_id: &str,
    ) -> Result<Vec<GdprConsent>, GdprError> {
        let consents = sqlx::query_as::<_, GdprConsent>(
            r#"
            SELECT * FROM gdpr_consents
            WHERE tenant_id = $1 AND user_id = $2
            ORDER BY created_at DESC
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(consents)
    }
    
    async fn revoke_consent(
        &self,
        tenant_id: &str,
        user_id: &str,
        purpose: &str,
    ) -> Result<(), GdprError> {
        sqlx::query(
            r#"
            UPDATE gdpr_consents
            SET granted = false,
                revoked_at = NOW()
            WHERE tenant_id = $1 AND user_id = $2 AND purpose = $3
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(purpose)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn log_data_access(
        &self,
        tenant_id: &str,
        user_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: Option<&str>,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<(), GdprError> {
        sqlx::query(
            r#"
            INSERT INTO gdpr_audit_logs
                (tenant_id, user_id, action, resource_type, resource_id, ip_address, user_agent)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(action)
        .bind(resource_type)
        .bind(resource_id)
        .bind(ip_address)
        .bind(user_agent)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    async fn get_audit_logs(
        &self,
        tenant_id: &str,
        user_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<GdprAuditLog>, GdprError> {
        let logs = sqlx::query_as::<_, GdprAuditLog>(
            r#"
            SELECT * FROM gdpr_audit_logs
            WHERE tenant_id = $1 
                AND user_id = $2
                AND created_at >= $3
                AND created_at <= $4
            ORDER BY created_at DESC
            "#
        )
        .bind(tenant_id)
        .bind(user_id)
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await?;
        
        Ok(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests require a PostgreSQL database
    // They are marked as integration tests and skipped in unit test runs
    
    #[test]
    fn test_anonymization_strategy() {
        let strategy = AnonymizationStrategy::default();
        assert!(matches!(strategy, AnonymizationStrategy::Redact));
    }
    
    #[test]
    fn test_user_data_export_structure() {
        let export = UserDataExport {
            user_id: "user-123".to_string(),
            tenant_id: "tenant-456".to_string(),
            exported_at: Utc::now(),
            memories: vec![],
            knowledge_items: vec![],
            organizational_units: vec![],
            consents: vec![],
            audit_logs: vec![],
        };
        
        assert_eq!(export.user_id, "user-123");
        assert_eq!(export.tenant_id, "tenant-456");
    }
}
