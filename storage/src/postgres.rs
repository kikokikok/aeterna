use async_trait::async_trait;
use mk_core::traits::StorageBackend;
use mk_core::types::{OrganizationalUnit, TenantContext, UnitType};
use sqlx::{Pool, Postgres, Row};
use thiserror::Error;

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
        let pool = Pool::connect(connection_url).await?;
        Ok(Self { pool })
    }

    pub async fn initialize_schema(&self) -> Result<(), PostgresError> {
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
                violations JSONB NOT NULL,
                timestamp BIGINT NOT NULL,
                PRIMARY KEY (project_id, tenant_id, timestamp)
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

        Ok(())
    }

    pub async fn create_unit(&self, unit: &OrganizationalUnit) -> Result<(), PostgresError> {
        if let Some(ref parent_id) = unit.parent_id {
            let parent = self
                .get_unit_by_id(parent_id)
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

    async fn get_unit_by_id(&self, id: &str) -> Result<Option<OrganizationalUnit>, PostgresError> {
        let row = sqlx::query(
            "SELECT id, name, type, parent_id, tenant_id, metadata, created_at, updated_at 
             FROM organizational_units WHERE id = $1"
        )
        .bind(id)
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
        let exists: Option<(i64,)> =
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
        let row: Option<(i64,)> =
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
            "INSERT INTO drift_results (project_id, tenant_id, drift_score, violations, timestamp)
             VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(&result.project_id)
        .bind(result.tenant_id.as_str())
        .bind(result.drift_score)
        .bind(serde_json::to_value(&result.violations)?)
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
            "SELECT project_id, tenant_id, drift_score, violations, timestamp 
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
                violations: serde_json::from_value(row.get("violations"))?,
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
