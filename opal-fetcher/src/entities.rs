//! Cedar entity transformation from PostgreSQL rows.
//!
//! This module transforms organizational data from PostgreSQL into Cedar entity format
//! that can be consumed by OPAL and Cedar Agent for authorization decisions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::error::Result;

// ============================================================================
// Database Row Types
// ============================================================================

/// Row from the v_hierarchy view.
#[derive(Debug, Clone, FromRow)]
pub struct HierarchyRow {
    pub company_id: Option<Uuid>,
    pub company_slug: Option<String>,
    pub company_name: Option<String>,
    pub org_id: Option<Uuid>,
    pub org_slug: Option<String>,
    pub org_name: Option<String>,
    pub team_id: Option<Uuid>,
    pub team_slug: Option<String>,
    pub team_name: Option<String>,
    pub project_id: Option<Uuid>,
    pub project_slug: Option<String>,
    pub project_name: Option<String>,
    pub git_remote: Option<String>,
}

/// Row from the v_user_permissions view.
#[derive(Debug, Clone, FromRow)]
pub struct UserPermissionRow {
    pub user_id: Uuid,
    pub email: String,
    pub user_name: Option<String>,
    pub user_status: String,
    pub team_id: Uuid,
    pub role: String,
    pub permissions: serde_json::Value,
    pub org_id: Uuid,
    pub company_id: Uuid,
    pub company_slug: String,
    pub org_slug: String,
    pub team_slug: String,
}

/// Row from the v_agent_permissions view.
#[derive(Debug, Clone, FromRow)]
pub struct AgentPermissionRow {
    pub agent_id: Uuid,
    pub agent_name: String,
    pub agent_type: String,
    pub delegated_by_user_id: Option<Uuid>,
    pub delegated_by_agent_id: Option<Uuid>,
    pub delegation_depth: i32,
    pub capabilities: serde_json::Value,
    pub allowed_company_ids: Option<Vec<Uuid>>,
    pub allowed_org_ids: Option<Vec<Uuid>>,
    pub allowed_team_ids: Option<Vec<Uuid>>,
    pub allowed_project_ids: Option<Vec<Uuid>>,
    pub agent_status: String,
    pub delegating_user_email: Option<String>,
    pub delegating_user_name: Option<String>,
}

// ============================================================================
// Cedar Entity Types
// ============================================================================

/// A Cedar entity with type, ID, attributes, and parent relationships.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CedarEntity {
    /// Entity UID in format "Type::\"id\"".
    pub uid: CedarEntityUid,
    /// Entity attributes.
    pub attrs: serde_json::Value,
    /// Parent entity UIDs (for hierarchy).
    pub parents: Vec<CedarEntityUid>,
}

/// Cedar entity UID structure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CedarEntityUid {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub id: String,
}

impl CedarEntityUid {
    /// Creates a new entity UID.
    #[must_use]
    pub fn new(entity_type: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            entity_type: entity_type.into(),
            id: id.into(),
        }
    }
}

/// Response containing Cedar entities for OPAL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedarEntitiesResponse {
    /// List of Cedar entities.
    pub entities: Vec<CedarEntity>,
    /// Timestamp of the response.
    pub timestamp: DateTime<Utc>,
    /// Number of entities.
    pub count: usize,
}

impl CedarEntitiesResponse {
    /// Creates a new response with the given entities.
    #[must_use]
    pub fn new(entities: Vec<CedarEntity>) -> Self {
        let count = entities.len();
        Self {
            entities,
            timestamp: Utc::now(),
            count,
        }
    }
}

// ============================================================================
// Entity Transformation Functions
// ============================================================================

/// Transforms hierarchy rows into Cedar entities.
///
/// Creates entities for: Company, Organization, Team, Project
/// with proper parent relationships (in Cedar format).
pub fn transform_hierarchy(rows: Vec<HierarchyRow>) -> Result<Vec<CedarEntity>> {
    let mut entities: Vec<CedarEntity> = Vec::new();
    let mut seen_companies = std::collections::HashSet::new();
    let mut seen_orgs = std::collections::HashSet::new();
    let mut seen_teams = std::collections::HashSet::new();
    let mut seen_projects = std::collections::HashSet::new();

    for row in rows {
        // Company entity
        if let (Some(id), Some(slug), Some(name)) =
            (&row.company_id, &row.company_slug, &row.company_name)
        {
            if seen_companies.insert(*id) {
                entities.push(CedarEntity {
                    uid: CedarEntityUid::new("Aeterna::Company", id.to_string()),
                    attrs: serde_json::json!({
                        "slug": slug,
                        "name": name,
                    }),
                    parents: vec![],
                });
            }
        }

        // Organization entity
        if let (Some(id), Some(slug), Some(name), Some(company_id)) =
            (&row.org_id, &row.org_slug, &row.org_name, &row.company_id)
        {
            if seen_orgs.insert(*id) {
                entities.push(CedarEntity {
                    uid: CedarEntityUid::new("Aeterna::Organization", id.to_string()),
                    attrs: serde_json::json!({
                        "slug": slug,
                        "name": name,
                    }),
                    parents: vec![CedarEntityUid::new(
                        "Aeterna::Company",
                        company_id.to_string(),
                    )],
                });
            }
        }

        // Team entity
        if let (Some(id), Some(slug), Some(name), Some(org_id)) =
            (&row.team_id, &row.team_slug, &row.team_name, &row.org_id)
        {
            if seen_teams.insert(*id) {
                entities.push(CedarEntity {
                    uid: CedarEntityUid::new("Aeterna::Team", id.to_string()),
                    attrs: serde_json::json!({
                        "slug": slug,
                        "name": name,
                    }),
                    parents: vec![CedarEntityUid::new(
                        "Aeterna::Organization",
                        org_id.to_string(),
                    )],
                });
            }
        }

        // Project entity
        if let (Some(id), Some(slug), Some(name), Some(team_id)) = (
            &row.project_id,
            &row.project_slug,
            &row.project_name,
            &row.team_id,
        ) {
            if seen_projects.insert(*id) {
                let mut attrs = serde_json::json!({
                    "slug": slug,
                    "name": name,
                });
                if let Some(git_remote) = &row.git_remote {
                    attrs["git_remote"] = serde_json::json!(git_remote);
                }
                entities.push(CedarEntity {
                    uid: CedarEntityUid::new("Aeterna::Project", id.to_string()),
                    attrs,
                    parents: vec![CedarEntityUid::new("Aeterna::Team", team_id.to_string())],
                });
            }
        }
    }

    Ok(entities)
}

/// Transforms user permission rows into Cedar entities.
///
/// Creates User entities with role and membership information.
pub fn transform_users(rows: Vec<UserPermissionRow>) -> Result<Vec<CedarEntity>> {
    let mut entities: Vec<CedarEntity> = Vec::new();
    let mut user_teams: std::collections::HashMap<Uuid, Vec<CedarEntityUid>> =
        std::collections::HashMap::new();
    let mut user_info: std::collections::HashMap<Uuid, (String, Option<String>, String)> =
        std::collections::HashMap::new();

    // Collect all team memberships per user
    for row in &rows {
        let team_uid = CedarEntityUid::new("Aeterna::Team", row.team_id.to_string());
        user_teams.entry(row.user_id).or_default().push(team_uid);
        user_info.entry(row.user_id).or_insert((
            row.email.clone(),
            row.user_name.clone(),
            row.user_status.clone(),
        ));
    }

    // Create User entities with all their team memberships as parents
    for (user_id, parents) in user_teams {
        let (email, name, status) = user_info.get(&user_id).cloned().unwrap_or_default();

        // Collect roles for this user
        let roles: Vec<String> = rows
            .iter()
            .filter(|r| r.user_id == user_id)
            .map(|r| r.role.clone())
            .collect();

        let mut attrs = serde_json::json!({
            "email": email,
            "status": status,
            "roles": roles,
        });
        if let Some(n) = name {
            attrs["name"] = serde_json::json!(n);
        }

        entities.push(CedarEntity {
            uid: CedarEntityUid::new("Aeterna::User", user_id.to_string()),
            attrs,
            parents,
        });
    }

    Ok(entities)
}

/// Transforms agent permission rows into Cedar entities.
///
/// Creates Agent entities with delegation chain and capability information.
pub fn transform_agents(rows: Vec<AgentPermissionRow>) -> Result<Vec<CedarEntity>> {
    let mut entities: Vec<CedarEntity> = Vec::new();

    for row in rows {
        let mut parents = Vec::new();

        // Add allowed teams as parents for resource access
        if let Some(team_ids) = &row.allowed_team_ids {
            for team_id in team_ids {
                parents.push(CedarEntityUid::new("Aeterna::Team", team_id.to_string()));
            }
        }

        // Parse capabilities from JSON
        let capabilities: Vec<String> =
            serde_json::from_value(row.capabilities.clone()).unwrap_or_default();

        let mut attrs = serde_json::json!({
            "name": row.agent_name,
            "agent_type": row.agent_type,
            "delegation_depth": row.delegation_depth,
            "capabilities": capabilities,
            "status": row.agent_status,
        });

        // Add delegation info
        if let Some(user_id) = row.delegated_by_user_id {
            attrs["delegated_by"] = serde_json::json!({
                "type": "User",
                "id": user_id.to_string(),
            });
            if let Some(email) = &row.delegating_user_email {
                attrs["delegating_user_email"] = serde_json::json!(email);
            }
        } else if let Some(agent_id) = row.delegated_by_agent_id {
            attrs["delegated_by"] = serde_json::json!({
                "type": "Agent",
                "id": agent_id.to_string(),
            });
        }

        // Add scope arrays
        if let Some(ids) = &row.allowed_company_ids {
            if !ids.is_empty() {
                attrs["allowed_company_ids"] = serde_json::json!(ids);
            }
        }
        if let Some(ids) = &row.allowed_org_ids {
            if !ids.is_empty() {
                attrs["allowed_org_ids"] = serde_json::json!(ids);
            }
        }
        if let Some(ids) = &row.allowed_team_ids {
            if !ids.is_empty() {
                attrs["allowed_team_ids"] = serde_json::json!(ids);
            }
        }
        if let Some(ids) = &row.allowed_project_ids {
            if !ids.is_empty() {
                attrs["allowed_project_ids"] = serde_json::json!(ids);
            }
        }

        entities.push(CedarEntity {
            uid: CedarEntityUid::new("Aeterna::Agent", row.agent_id.to_string()),
            attrs,
            parents,
        });
    }

    Ok(entities)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_hierarchy_row() -> HierarchyRow {
        HierarchyRow {
            company_id: Some(Uuid::new_v4()),
            company_slug: Some("acme-corp".to_string()),
            company_name: Some("Acme Corporation".to_string()),
            org_id: Some(Uuid::new_v4()),
            org_slug: Some("platform-engineering".to_string()),
            org_name: Some("Platform Engineering".to_string()),
            team_id: Some(Uuid::new_v4()),
            team_slug: Some("api-team".to_string()),
            team_name: Some("API Team".to_string()),
            project_id: Some(Uuid::new_v4()),
            project_slug: Some("payments-service".to_string()),
            project_name: Some("Payments Service".to_string()),
            git_remote: Some("git@github.com:acme/payments.git".to_string()),
        }
    }

    #[test]
    fn test_cedar_entity_uid() {
        let uid = CedarEntityUid::new("Aeterna::User", "123");
        assert_eq!(uid.entity_type, "Aeterna::User");
        assert_eq!(uid.id, "123");
    }

    #[test]
    fn test_transform_hierarchy_single_row() {
        let row = sample_hierarchy_row();
        let rows = vec![row.clone()];

        let entities = transform_hierarchy(rows).unwrap();

        // Should create 4 entities: Company, Org, Team, Project
        assert_eq!(entities.len(), 4);

        // Verify Company
        let company = entities
            .iter()
            .find(|e| e.uid.entity_type == "Aeterna::Company")
            .unwrap();
        assert!(company.parents.is_empty());
        assert_eq!(company.attrs["slug"], "acme-corp");

        // Verify Organization
        let org = entities
            .iter()
            .find(|e| e.uid.entity_type == "Aeterna::Organization")
            .unwrap();
        assert_eq!(org.parents.len(), 1);
        assert_eq!(org.parents[0].entity_type, "Aeterna::Company");

        // Verify Team
        let team = entities
            .iter()
            .find(|e| e.uid.entity_type == "Aeterna::Team")
            .unwrap();
        assert_eq!(team.parents.len(), 1);
        assert_eq!(team.parents[0].entity_type, "Aeterna::Organization");

        // Verify Project
        let project = entities
            .iter()
            .find(|e| e.uid.entity_type == "Aeterna::Project")
            .unwrap();
        assert_eq!(project.parents.len(), 1);
        assert_eq!(project.parents[0].entity_type, "Aeterna::Team");
        assert!(project.attrs.get("git_remote").is_some());
    }

    #[test]
    fn test_transform_hierarchy_deduplication() {
        let row = sample_hierarchy_row();
        let rows = vec![row.clone(), row.clone()];

        let entities = transform_hierarchy(rows).unwrap();

        // Should still be 4 entities (deduplicated)
        assert_eq!(entities.len(), 4);
    }

    #[test]
    fn test_transform_users() {
        let user_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();

        let row = UserPermissionRow {
            user_id,
            email: "alice@acme.com".to_string(),
            user_name: Some("Alice".to_string()),
            user_status: "active".to_string(),
            team_id,
            role: "developer".to_string(),
            permissions: serde_json::json!([]),
            org_id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            company_slug: "acme-corp".to_string(),
            org_slug: "platform-engineering".to_string(),
            team_slug: "api-team".to_string(),
        };

        let entities = transform_users(vec![row]).unwrap();

        assert_eq!(entities.len(), 1);
        let user = &entities[0];
        assert_eq!(user.uid.entity_type, "Aeterna::User");
        assert_eq!(user.uid.id, user_id.to_string());
        assert_eq!(user.attrs["email"], "alice@acme.com");
        assert_eq!(user.attrs["status"], "active");
        assert_eq!(user.parents.len(), 1);
        assert_eq!(user.parents[0].entity_type, "Aeterna::Team");
    }

    #[test]
    fn test_transform_users_multiple_teams() {
        let user_id = Uuid::new_v4();
        let team1_id = Uuid::new_v4();
        let team2_id = Uuid::new_v4();

        let row1 = UserPermissionRow {
            user_id,
            email: "alice@acme.com".to_string(),
            user_name: Some("Alice".to_string()),
            user_status: "active".to_string(),
            team_id: team1_id,
            role: "developer".to_string(),
            permissions: serde_json::json!([]),
            org_id: Uuid::new_v4(),
            company_id: Uuid::new_v4(),
            company_slug: "acme-corp".to_string(),
            org_slug: "platform-engineering".to_string(),
            team_slug: "api-team".to_string(),
        };

        let row2 = UserPermissionRow {
            team_id: team2_id,
            role: "tech_lead".to_string(),
            team_slug: "data-team".to_string(),
            ..row1.clone()
        };

        let entities = transform_users(vec![row1, row2]).unwrap();

        assert_eq!(entities.len(), 1);
        let user = &entities[0];
        // Should have 2 parent teams
        assert_eq!(user.parents.len(), 2);
        // Should have both roles
        let roles: Vec<String> = serde_json::from_value(user.attrs["roles"].clone()).unwrap();
        assert!(roles.contains(&"developer".to_string()));
        assert!(roles.contains(&"tech_lead".to_string()));
    }

    #[test]
    fn test_transform_agents() {
        let agent_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();

        let row = AgentPermissionRow {
            agent_id,
            agent_name: "OpenCode Assistant".to_string(),
            agent_type: "opencode".to_string(),
            delegated_by_user_id: Some(user_id),
            delegated_by_agent_id: None,
            delegation_depth: 1,
            capabilities: serde_json::json!(["memory:read", "memory:write"]),
            allowed_company_ids: None,
            allowed_org_ids: None,
            allowed_team_ids: Some(vec![team_id]),
            allowed_project_ids: None,
            agent_status: "active".to_string(),
            delegating_user_email: Some("alice@acme.com".to_string()),
            delegating_user_name: Some("Alice".to_string()),
        };

        let entities = transform_agents(vec![row]).unwrap();

        assert_eq!(entities.len(), 1);
        let agent = &entities[0];
        assert_eq!(agent.uid.entity_type, "Aeterna::Agent");
        assert_eq!(agent.uid.id, agent_id.to_string());
        assert_eq!(agent.attrs["name"], "OpenCode Assistant");
        assert_eq!(agent.attrs["agent_type"], "opencode");
        assert_eq!(agent.attrs["delegation_depth"], 1);
        assert_eq!(agent.attrs["status"], "active");

        // Should have team as parent
        assert_eq!(agent.parents.len(), 1);
        assert_eq!(agent.parents[0].entity_type, "Aeterna::Team");

        // Should have delegated_by info
        let delegated_by = &agent.attrs["delegated_by"];
        assert_eq!(delegated_by["type"], "User");
        assert_eq!(delegated_by["id"], user_id.to_string());
    }

    #[test]
    fn test_transform_agents_delegated_by_agent() {
        let agent_id = Uuid::new_v4();
        let delegating_agent_id = Uuid::new_v4();

        let row = AgentPermissionRow {
            agent_id,
            agent_name: "Sub-Agent".to_string(),
            agent_type: "custom".to_string(),
            delegated_by_user_id: None,
            delegated_by_agent_id: Some(delegating_agent_id),
            delegation_depth: 2,
            capabilities: serde_json::json!(["knowledge:read"]),
            allowed_company_ids: None,
            allowed_org_ids: None,
            allowed_team_ids: None,
            allowed_project_ids: None,
            agent_status: "active".to_string(),
            delegating_user_email: None,
            delegating_user_name: None,
        };

        let entities = transform_agents(vec![row]).unwrap();

        assert_eq!(entities.len(), 1);
        let agent = &entities[0];
        let delegated_by = &agent.attrs["delegated_by"];
        assert_eq!(delegated_by["type"], "Agent");
        assert_eq!(delegated_by["id"], delegating_agent_id.to_string());
    }

    #[test]
    fn test_cedar_entities_response() {
        let entities = vec![CedarEntity {
            uid: CedarEntityUid::new("Aeterna::User", "123"),
            attrs: serde_json::json!({"email": "test@test.com"}),
            parents: vec![],
        }];

        let response = CedarEntitiesResponse::new(entities);

        assert_eq!(response.count, 1);
        assert_eq!(response.entities.len(), 1);
    }
}
