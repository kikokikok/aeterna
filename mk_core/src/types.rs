use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use validator::Validate;

/// Knowledge types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeType {
    /// Architecture Decision Records
    Adr,

    /// Policy documents
    Policy,

    /// Design patterns
    Pattern,

    /// Specifications
    Spec
}

/// Knowledge status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeStatus {
    /// Initial draft state
    Draft,

    /// Proposed change
    Proposed,

    /// Accepted/Active state
    Accepted,

    /// Deprecated but still present
    Deprecated,

    /// Superseded by a newer item
    Superseded
}

/// Knowledge layers for hierarchical organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeLayer {
    /// Company-wide knowledge
    Company,

    /// Organization-level knowledge
    Org,

    /// Team-specific knowledge
    Team,

    /// Project-specific knowledge
    Project
}

/// Constraint severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintSeverity {
    /// Informational only
    Info,

    /// Warning level
    Warn,

    /// Blocking violation
    Block
}

/// Constraint operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintOperator {
    /// Must use this item
    MustUse,

    /// Must not use this item
    MustNotUse,

    /// Must match pattern
    MustMatch,

    /// Must not match pattern
    MustNotMatch,

    /// Must exist
    MustExist,

    /// Must not exist
    MustNotExist
}

/// Constraint targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintTarget {
    /// File-based constraint
    File,

    /// Code-based constraint
    Code,

    /// Dependency-based constraint
    Dependency,

    /// Import-based constraint
    Import,

    /// Config-based constraint
    Config
}

/// Memory layers for hierarchical storage
///
/// 7-layer hierarchy with precedence rules:
/// - Priority 1 (highest): agent
/// - Priority 2: user
/// - Priority 3: session
/// - Priority 4: project
/// - Priority 5: team
/// - Priority 6: org
/// - Priority 7 (lowest): company
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    strum::EnumString,
    strum::Display,
)]
#[serde(rename_all = "camelCase")]
pub enum MemoryLayer {
    /// Per-agent instance (most specific)
    Agent,

    /// Cross-session user data
    User,

    /// Single conversation context
    Session,

    /// Project-wide persistent data
    Project,

    /// Team-shared knowledge
    Team,

    /// Organization-level policies
    Org,

    /// Company-wide standards
    Company
}

impl MemoryLayer {
    /// Returns precedence value (1=highest, 7=lowest)
    #[must_use]
    pub fn precedence(&self) -> u8 {
        match self {
            MemoryLayer::Agent => 1,
            MemoryLayer::User => 2,
            MemoryLayer::Session => 3,
            MemoryLayer::Project => 4,
            MemoryLayer::Team => 5,
            MemoryLayer::Org => 6,
            MemoryLayer::Company => 7
        }
    }

    /// Returns layer display name
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryLayer::Agent => "Agent",
            MemoryLayer::User => "User",
            MemoryLayer::Session => "Session",
            MemoryLayer::Project => "Project",
            MemoryLayer::Team => "Team",
            MemoryLayer::Org => "Organization",
            MemoryLayer::Company => "Company"
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate, Default)]
#[serde(rename_all = "camelCase")]
pub struct LayerIdentifiers {
    #[validate(custom(function = "validate_agent_id"))]
    pub agent_id: Option<String>,
    #[validate(custom(function = "validate_user_id"))]
    pub user_id: Option<String>,
    #[validate(custom(function = "validate_session_id"))]
    pub session_id: Option<String>,
    #[validate(custom(function = "validate_project_id"))]
    pub project_id: Option<String>,
    #[validate(custom(function = "validate_team_id"))]
    pub team_id: Option<String>,
    #[validate(custom(function = "validate_org_id"))]
    pub org_id: Option<String>,
    #[validate(custom(function = "validate_company_id"))]
    pub company_id: Option<String>
}
#[allow(dead_code)]
fn validate_agent_id(agent_id: &&String) -> Result<(), validator::ValidationError> {
    if agent_id.is_empty() {
        return Err(validator::ValidationError::new("agent_id cannot be empty"));
    }
    if agent_id.len() > 100 {
        return Err(validator::ValidationError::new("agent_id too long"));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_user_id(user_id: &&String) -> Result<(), validator::ValidationError> {
    if user_id.is_empty() {
        return Err(validator::ValidationError::new("user_id cannot be empty"));
    }
    if user_id.len() > 100 {
        return Err(validator::ValidationError::new("user_id too long"));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_session_id(session_id: &&String) -> Result<(), validator::ValidationError> {
    if session_id.is_empty() {
        return Err(validator::ValidationError::new(
            "session_id cannot be empty"
        ));
    }
    if session_id.len() > 100 {
        return Err(validator::ValidationError::new("session_id too long"));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_project_id(project_id: &&String) -> Result<(), validator::ValidationError> {
    if project_id.is_empty() {
        return Err(validator::ValidationError::new(
            "project_id cannot be empty"
        ));
    }
    if project_id.len() > 100 {
        return Err(validator::ValidationError::new("project_id too long"));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_team_id(team_id: &&String) -> Result<(), validator::ValidationError> {
    if team_id.is_empty() {
        return Err(validator::ValidationError::new("team_id cannot be empty"));
    }
    if team_id.len() > 100 {
        return Err(validator::ValidationError::new("team_id too long"));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_org_id(org_id: &&String) -> Result<(), validator::ValidationError> {
    if org_id.is_empty() {
        return Err(validator::ValidationError::new("org_id cannot be empty"));
    }
    if org_id.len() > 100 {
        return Err(validator::ValidationError::new("org_id too long"));
    }
    Ok(())
}

#[allow(dead_code)]
fn validate_company_id(company_id: &&String) -> Result<(), validator::ValidationError> {
    if company_id.is_empty() {
        return Err(validator::ValidationError::new(
            "company_id cannot be empty"
        ));
    }
    if company_id.len() > 100 {
        return Err(validator::ValidationError::new("company_id too long"));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub layer: MemoryLayer,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeEntry {
    pub path: String,
    pub content: String,
    pub layer: KnowledgeLayer,
    pub kind: KnowledgeType,
    pub status: KnowledgeStatus,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub commit_hash: Option<String>,
    pub author: Option<String>,
    pub updated_at: i64
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub layer: KnowledgeLayer,
    pub rules: Vec<PolicyRule>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    pub target: ConstraintTarget,
    pub operator: ConstraintOperator,
    pub value: serde_json::Value,
    pub severity: ConstraintSeverity,
    pub message: String
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<PolicyViolation>
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyViolation {
    pub rule_id: String,
    pub policy_id: String,
    pub severity: ConstraintSeverity,
    pub message: String,
    pub context: std::collections::HashMap<String, serde_json::Value>
}

#[cfg(test)]
mod tests {
    use super::*;
    use validator::Validate;

    #[test]
    fn test_knowledge_type_serialization() {
        let adr = KnowledgeType::Adr;
        let json = serde_json::to_string(&adr).unwrap();
        assert_eq!(json, "\"adr\"");

        let deserialized: KnowledgeType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, KnowledgeType::Adr);
    }

    #[test]
    fn test_knowledge_layer_serialization() {
        let company = KnowledgeLayer::Company;
        let json = serde_json::to_string(&company).unwrap();
        assert_eq!(json, "\"company\"");

        let deserialized: KnowledgeLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, KnowledgeLayer::Company);
    }

    #[test]
    fn test_memory_layer_precedence() {
        assert_eq!(MemoryLayer::Agent.precedence(), 1);
        assert_eq!(MemoryLayer::User.precedence(), 2);
        assert_eq!(MemoryLayer::Session.precedence(), 3);
        assert_eq!(MemoryLayer::Project.precedence(), 4);
        assert_eq!(MemoryLayer::Team.precedence(), 5);
        assert_eq!(MemoryLayer::Org.precedence(), 6);
        assert_eq!(MemoryLayer::Company.precedence(), 7);
    }

    #[test]
    fn test_memory_layer_serialization() {
        let agent = MemoryLayer::Agent;
        let json = serde_json::to_string(&agent).unwrap();
        assert_eq!(json, "\"agent\"");

        let deserialized: MemoryLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, MemoryLayer::Agent);
    }

    #[test]
    fn test_constraint_severity_serialization() {
        let block = ConstraintSeverity::Block;
        let json = serde_json::to_string(&block).unwrap();
        assert_eq!(json, "\"block\"");

        let deserialized: ConstraintSeverity = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ConstraintSeverity::Block);
    }

    #[test]
    fn test_constraint_operator_serialization() {
        let must_use = ConstraintOperator::MustUse;
        let json = serde_json::to_string(&must_use).unwrap();
        assert_eq!(json, "\"mustUse\"");

        let deserialized: ConstraintOperator = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ConstraintOperator::MustUse);
    }

    #[test]
    fn test_constraint_target_serialization() {
        let file = ConstraintTarget::File;
        let json = serde_json::to_string(&file).unwrap();
        assert_eq!(json, "\"file\"");

        let deserialized: ConstraintTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ConstraintTarget::File);
    }

    #[test]
    fn test_memory_entry_creation() {
        let entry = MemoryEntry {
            id: "test_id".to_string(),
            content: "Test content".to_string(),
            embedding: Some(vec![0.1, 0.2, 0.3]),
            layer: MemoryLayer::User,
            metadata: std::collections::HashMap::new(),
            created_at: 1234567890,
            updated_at: 1234567890
        };

        assert_eq!(entry.id, "test_id");
        assert_eq!(entry.content, "Test content");
        assert_eq!(entry.layer, MemoryLayer::User);
        assert_eq!(entry.embedding.unwrap().len(), 3);
    }

    #[test]
    fn test_knowledge_entry_creation() {
        let entry = KnowledgeEntry {
            path: "docs/adr/001.md".to_string(),
            content: "# ADR 001: Use Rust".to_string(),
            layer: KnowledgeLayer::Project,
            kind: KnowledgeType::Adr,
            metadata: std::collections::HashMap::new(),
            commit_hash: Some("abc123".to_string()),
            author: Some("Alice".to_string()),
            status: KnowledgeStatus::Accepted,
            updated_at: 1234567890
        };

        assert_eq!(entry.path, "docs/adr/001.md");
        assert_eq!(entry.layer, KnowledgeLayer::Project);
        assert_eq!(entry.kind, KnowledgeType::Adr);
        assert_eq!(entry.commit_hash.unwrap(), "abc123");
    }

    #[test]
    fn test_policy_creation() {
        let rule = PolicyRule {
            id: "rule_1".to_string(),
            target: ConstraintTarget::Dependency,
            operator: ConstraintOperator::MustNotUse,
            value: serde_json::json!("unsafe-lib"),
            severity: ConstraintSeverity::Block,
            message: "Do not use unsafe libraries".to_string()
        };

        let policy = Policy {
            id: "policy_1".to_string(),
            name: "Security Policy".to_string(),
            description: Some("Security constraints".to_string()),
            layer: KnowledgeLayer::Company,
            rules: vec![rule],
            metadata: std::collections::HashMap::new()
        };

        assert_eq!(policy.id, "policy_1");
        assert_eq!(policy.layer, KnowledgeLayer::Company);
        assert_eq!(policy.rules.len(), 1);
        assert_eq!(policy.rules[0].target, ConstraintTarget::Dependency);
    }

    #[test]
    fn test_validation_result_creation() {
        let violation = PolicyViolation {
            rule_id: "rule_1".to_string(),
            policy_id: "policy_1".to_string(),
            severity: ConstraintSeverity::Warn,
            message: "Warning message".to_string(),
            context: std::collections::HashMap::new()
        };

        let result = ValidationResult {
            is_valid: false,
            violations: vec![violation]
        };

        assert!(!result.is_valid);
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].severity, ConstraintSeverity::Warn);
    }

    #[test]
    fn test_validate_user_id_valid() {
        let user_id = "user_123".to_string();
        let result = validate_user_id(&&user_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_user_id_empty() {
        let user_id = "".to_string();
        let result = validate_user_id(&&user_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_user_id_too_long() {
        let user_id = "a".repeat(101);
        let result = validate_user_id(&&user_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_session_id_valid() {
        let session_id = "session_456".to_string();
        let result = validate_session_id(&&session_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_project_id_valid() {
        let project_id = "project_789".to_string();
        let result = validate_project_id(&&project_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_team_id_valid() {
        let team_id = "team_abc".to_string();
        let result = validate_team_id(&&team_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_org_id_valid() {
        let org_id = "org_xyz".to_string();
        let result = validate_org_id(&&org_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_company_id_valid() {
        let company_id = "company_123".to_string();
        let result = validate_company_id(&&company_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_layer_identifiers_validation() {
        let identifiers = LayerIdentifiers {
            agent_id: Some("agent_1".to_string()),
            user_id: Some("user_123".to_string()),
            session_id: Some("session_456".to_string()),
            project_id: Some("project_789".to_string()),
            team_id: Some("team_abc".to_string()),
            org_id: Some("org_xyz".to_string()),
            company_id: Some("company_123".to_string())
        };

        let result = identifiers.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_layer_identifiers_invalid_user_id() {
        let identifiers = LayerIdentifiers {
            agent_id: Some("agent_1".to_string()),
            user_id: Some("".to_string()),
            session_id: None,
            project_id: None,
            team_id: None,
            org_id: None,
            company_id: None
        };

        let result = identifiers.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_layer_display_name() {
        assert_eq!(MemoryLayer::Agent.display_name(), "Agent");
        assert_eq!(MemoryLayer::User.display_name(), "User");
        assert_eq!(MemoryLayer::Session.display_name(), "Session");
        assert_eq!(MemoryLayer::Project.display_name(), "Project");
        assert_eq!(MemoryLayer::Team.display_name(), "Team");
        assert_eq!(MemoryLayer::Org.display_name(), "Organization");
        assert_eq!(MemoryLayer::Company.display_name(), "Company");
    }

    #[test]
    fn test_validate_agent_id_valid() {
        let agent_id = "agent_123".to_string();
        let result = validate_agent_id(&&agent_id);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_agent_id_empty() {
        let agent_id = "".to_string();
        let result = validate_agent_id(&&agent_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_agent_id_too_long() {
        let agent_id = "a".repeat(101);
        let result = validate_agent_id(&&agent_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_session_id_empty() {
        let id = "".to_string();
        assert!(validate_session_id(&&id).is_err());
    }

    #[test]
    fn test_validate_project_id_empty() {
        let id = "".to_string();
        assert!(validate_project_id(&&id).is_err());
    }

    #[test]
    fn test_validate_team_id_empty() {
        let id = "".to_string();
        assert!(validate_team_id(&&id).is_err());
    }

    #[test]
    fn test_validate_org_id_empty() {
        let id = "".to_string();
        assert!(validate_org_id(&&id).is_err());
    }

    #[test]
    fn test_validate_company_id_empty() {
        let id = "".to_string();
        assert!(validate_company_id(&&id).is_err());
    }

    #[test]
    fn test_memory_layer_from_str() {
        use std::str::FromStr;
        assert_eq!(MemoryLayer::from_str("Agent").unwrap(), MemoryLayer::Agent);
        assert_eq!(
            MemoryLayer::from_str("Session").unwrap(),
            MemoryLayer::Session
        );
        assert!(MemoryLayer::from_str("Invalid").is_err());
    }

    #[test]
    fn test_memory_layer_display() {
        assert_eq!(format!("{}", MemoryLayer::Agent), "Agent");
        assert_eq!(format!("{}", MemoryLayer::User), "User");
        assert_eq!(format!("{}", MemoryLayer::Session), "Session");
        assert_eq!(format!("{}", MemoryLayer::Project), "Project");
        assert_eq!(format!("{}", MemoryLayer::Team), "Team");
        assert_eq!(format!("{}", MemoryLayer::Org), "Org");
        assert_eq!(format!("{}", MemoryLayer::Company), "Company");
    }

    #[test]
    fn test_memory_layer_from_str_comprehensive() {
        use std::str::FromStr;
        assert_eq!(MemoryLayer::from_str("Agent").unwrap(), MemoryLayer::Agent);
        assert_eq!(MemoryLayer::from_str("User").unwrap(), MemoryLayer::User);
        assert_eq!(
            MemoryLayer::from_str("Session").unwrap(),
            MemoryLayer::Session
        );
        assert_eq!(
            MemoryLayer::from_str("Project").unwrap(),
            MemoryLayer::Project
        );
        assert_eq!(MemoryLayer::from_str("Team").unwrap(), MemoryLayer::Team);
        assert_eq!(MemoryLayer::from_str("Org").unwrap(), MemoryLayer::Org);
        assert_eq!(
            MemoryLayer::from_str("Company").unwrap(),
            MemoryLayer::Company
        );
        assert!(MemoryLayer::from_str("Invalid").is_err());
        assert!(MemoryLayer::from_str("").is_err());
    }
}
