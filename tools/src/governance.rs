use crate::tools::Tool;
use async_trait::async_trait;
use knowledge::governance::GovernanceEngine;
use mk_core::types::{GovernanceEvent, OrganizationalUnit, Role, TenantContext, UnitType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use storage::postgres::PostgresBackend;
use validator::Validate;

/// Tool to create a new organizational unit.
pub struct UnitCreateTool {
    backend: Arc<PostgresBackend>,
    governance_engine: Arc<GovernanceEngine>
}

impl UnitCreateTool {
    pub fn new(backend: Arc<PostgresBackend>, governance_engine: Arc<GovernanceEngine>) -> Self {
        Self {
            backend,
            governance_engine
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct UnitCreateParams {
    pub name: String,
    pub unit_type: String,
    pub parent_id: Option<String>,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>,
    #[serde(default)]
    pub metadata: HashMap<String, Value>
}

#[async_trait]
impl Tool for UnitCreateTool {
    fn name(&self) -> &str {
        "governance_unit_create"
    }

    fn description(&self) -> &str {
        "Create a new organizational unit (Company, Organization, Team, or Project)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Name of the unit" },
                "unit_type": {
                    "type": "string",
                    "enum": ["company", "organization", "team", "project"],
                    "description": "Type of the unit"
                },
                "parent_id": { "type": "string", "description": "Parent unit ID" },
                "metadata": { "type": "object", "description": "Optional metadata" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["name", "unit_type"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: UnitCreateParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        let unit_type = match p.unit_type.as_str() {
            "company" => UnitType::Company,
            "organization" => UnitType::Organization,
            "team" => UnitType::Team,
            "project" => UnitType::Project,
            _ => return Err("Invalid unit type".into())
        };

        let unit = OrganizationalUnit {
            id: uuid::Uuid::new_v4().to_string(),
            name: p.name,
            unit_type,
            parent_id: p.parent_id,
            tenant_id: ctx.tenant_id.clone(),
            metadata: p.metadata,
            created_at: chrono::Utc::now().timestamp(),
            updated_at: chrono::Utc::now().timestamp()
        };

        self.backend.create_unit(&unit).await?;

        if let Some(tx) = self.governance_engine.event_tx() {
            let _ = tx.send(GovernanceEvent::UnitCreated {
                unit_id: unit.id.clone(),
                unit_type: unit.unit_type,
                tenant_id: ctx.tenant_id.clone(),
                parent_id: unit.parent_id.clone(),
                timestamp: chrono::Utc::now().timestamp()
            });
        }

        Ok(json!({
            "success": true,
            "unit_id": unit.id
        }))
    }
}

/// Tool to add a policy to an organizational unit.
pub struct UnitPolicyAddTool {
    backend: Arc<PostgresBackend>,
    governance_engine: Arc<GovernanceEngine>
}

impl UnitPolicyAddTool {
    pub fn new(backend: Arc<PostgresBackend>, governance_engine: Arc<GovernanceEngine>) -> Self {
        Self {
            backend,
            governance_engine
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct UnitPolicyAddParams {
    pub unit_id: String,
    pub policy: mk_core::types::Policy,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>
}

#[async_trait]
impl Tool for UnitPolicyAddTool {
    fn name(&self) -> &str {
        "governance_policy_add"
    }

    fn description(&self) -> &str {
        "Add or update a policy for an organizational unit."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "unit_id": { "type": "string", "description": "Unit ID to attach policy to" },
                "policy": { "type": "object", "description": "Policy definition" },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["unit_id", "policy"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: UnitPolicyAddParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;

        self.backend
            .add_unit_policy(&ctx, &p.unit_id, &p.policy)
            .await?;

        if let Some(tx) = self.governance_engine.event_tx() {
            let _ = tx.send(GovernanceEvent::PolicyUpdated {
                policy_id: p.policy.id.clone(),
                layer: p.policy.layer,
                tenant_id: ctx.tenant_id.clone(),
                timestamp: chrono::Utc::now().timestamp()
            });
        }

        Ok(json!({
            "success": true,
            "policy_id": p.policy.id
        }))
    }
}

/// Tool to assign a role to a user within an organizational unit.
pub struct UserRoleAssignTool {
    backend: Arc<PostgresBackend>,
    governance_engine: Arc<GovernanceEngine>
}

impl UserRoleAssignTool {
    pub fn new(backend: Arc<PostgresBackend>, governance_engine: Arc<GovernanceEngine>) -> Self {
        Self {
            backend,
            governance_engine
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Validate)]
pub struct UserRoleAssignParams {
    pub user_id: String,
    pub unit_id: String,
    pub role: String,
    #[serde(rename = "tenantContext")]
    pub tenant_context: Option<TenantContext>
}

#[async_trait]
impl Tool for UserRoleAssignTool {
    fn name(&self) -> &str {
        "governance_role_assign"
    }

    fn description(&self) -> &str {
        "Assign a role to a user within a specific organizational unit."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "user_id": { "type": "string", "description": "User ID" },
                "unit_id": { "type": "string", "description": "Unit ID" },
                "role": {
                    "type": "string",
                    "enum": ["developer", "techlead", "architect", "admin", "agent"],
                    "description": "Role to assign"
                },
                "tenantContext": { "$ref": "#/definitions/TenantContext" }
            },
            "required": ["user_id", "unit_id", "role"]
        })
    }

    async fn call(&self, params: Value) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let p: UserRoleAssignParams = serde_json::from_value(params)?;
        p.validate()?;

        let ctx = p.tenant_context.ok_or("Missing tenant context")?;
        let user_id = mk_core::types::UserId::new(p.user_id).ok_or("Invalid user ID")?;
        let role: Role = p.role.parse()?;

        self.backend
            .assign_role(&user_id, &ctx.tenant_id, &p.unit_id, role)
            .await?;

        if let Some(tx) = self.governance_engine.event_tx() {
            let _ = tx.send(GovernanceEvent::RoleAssigned {
                user_id: user_id.clone(),
                unit_id: p.unit_id.clone(),
                role,
                tenant_id: ctx.tenant_id.clone(),
                timestamp: chrono::Utc::now().timestamp()
            });
        }

        Ok(json!({
            "success": true
        }))
    }
}
