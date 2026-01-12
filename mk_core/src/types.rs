use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use utoipa::ToSchema;
use validator::Validate;

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, EnumString, Display,
)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Developer,
    TechLead,
    Architect,
    Admin,
    Agent,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, EnumString, Display,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum UnitType {
    Company,
    Organization,
    Team,
    Project,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationalUnit {
    pub id: String,
    pub name: String,
    pub unit_type: UnitType,
    pub parent_id: Option<String>,
    pub tenant_id: TenantId,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, JsonSchema, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(id: String) -> Option<Self> {
        if id.is_empty() || id.len() > 100 {
            None
        } else {
            Some(Self(id))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for TenantId {
    fn default() -> Self {
        Self("default".to_string())
    }
}

impl std::str::FromStr for TenantId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string()).ok_or_else(|| anyhow::anyhow!("Invalid tenant ID"))
    }
}

impl Default for TenantContext {
    fn default() -> Self {
        Self {
            tenant_id: TenantId::default(),
            user_id: UserId::default(),
            agent_id: None,
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, JsonSchema, PartialOrd, Ord,
)]
#[serde(transparent)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: String) -> Option<Self> {
        if id.is_empty() || id.len() > 100 {
            None
        } else {
            Some(Self(id))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UserId {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s.to_string()).ok_or_else(|| anyhow::anyhow!("Invalid user ID"))
    }
}

impl Default for UserId {
    fn default() -> Self {
        Self("default".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
pub struct TenantContext {
    pub tenant_id: TenantId,
    pub user_id: UserId,
    pub agent_id: Option<String>,
}

impl TenantContext {
    pub fn new(tenant_id: TenantId, user_id: UserId) -> Self {
        Self {
            tenant_id,
            user_id,
            agent_id: None,
        }
    }

    pub fn with_agent(tenant_id: TenantId, user_id: UserId, agent_id: String) -> Self {
        Self {
            tenant_id,
            user_id,
            agent_id: Some(agent_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct HierarchyPath {
    pub company: String,
    pub org: Option<String>,
    pub team: Option<String>,
    pub project: Option<String>,
}

impl HierarchyPath {
    pub fn company(id: String) -> Self {
        Self {
            company: id,
            org: None,
            team: None,
            project: None,
        }
    }

    pub fn org(company: String, id: String) -> Self {
        Self {
            company,
            org: Some(id),
            team: None,
            project: None,
        }
    }

    pub fn team(company: String, org: String, id: String) -> Self {
        Self {
            company,
            org: Some(org),
            team: Some(id),
            project: None,
        }
    }

    pub fn project(company: String, org: String, team: String, id: String) -> Self {
        Self {
            company,
            org: Some(org),
            team: Some(team),
            project: Some(id),
        }
    }

    pub fn depth(&self) -> usize {
        if self.project.is_some() {
            4
        } else if self.team.is_some() {
            3
        } else if self.org.is_some() {
            2
        } else {
            1
        }
    }

    pub fn path_string(&self) -> String {
        let mut parts = vec![self.company.clone()];
        if let Some(o) = &self.org {
            parts.push(o.clone());
        }
        if let Some(t) = &self.team {
            parts.push(t.clone());
        }
        if let Some(p) = &self.project {
            parts.push(p.clone());
        }
        parts.join(" > ")
    }
}

impl Role {
    #[must_use]
    pub fn precedence(&self) -> u8 {
        match self {
            Role::Admin => 4,
            Role::Architect => 3,
            Role::TechLead => 2,
            Role::Developer => 1,
            Role::Agent => 0,
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Role::Developer => "Developer",
            Role::TechLead => "Tech Lead",
            Role::Architect => "Architect",
            Role::Admin => "Admin",
            Role::Agent => "Agent",
        }
    }
}

/// Knowledge types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeType {
    Adr,
    Policy,
    Pattern,
    Spec,
}

/// Knowledge status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeStatus {
    Draft,
    Proposed,
    Accepted,
    Deprecated,
    Superseded,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    ToSchema,
    PartialOrd,
    Ord,
    JsonSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeLayer {
    Company,
    Org,
    Team,
    Project,
}

/// Constraint severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintSeverity {
    Info,
    Warn,
    Block,
}

/// Constraint operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintOperator {
    MustUse,
    MustNotUse,
    MustMatch,
    MustNotMatch,
    MustExist,
    MustNotExist,
}

/// Constraint targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintTarget {
    File,
    Code,
    Dependency,
    Import,
    Config,
}

/// Memory layers for hierarchical storage
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
    ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum MemoryLayer {
    Agent,
    User,
    Session,
    Project,
    Team,
    Org,
    Company,
}

impl MemoryLayer {
    #[must_use]
    pub fn precedence(&self) -> u8 {
        match self {
            MemoryLayer::Agent => 1,
            MemoryLayer::User => 2,
            MemoryLayer::Session => 3,
            MemoryLayer::Project => 4,
            MemoryLayer::Team => 5,
            MemoryLayer::Org => 6,
            MemoryLayer::Company => 7,
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            MemoryLayer::Agent => "Agent",
            MemoryLayer::User => "User",
            MemoryLayer::Session => "Session",
            MemoryLayer::Project => "Project",
            MemoryLayer::Team => "Team",
            MemoryLayer::Org => "Organization",
            MemoryLayer::Company => "Company",
        }
    }
}

#[derive(
    Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Validate, Default, ToSchema,
)]
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
    pub company_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub layer: MemoryLayer,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
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
    pub updated_at: i64,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum PolicyMode {
    #[default]
    Optional,
    Mandatory,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum RuleMergeStrategy {
    #[default]
    Override,
    Merge,
    Intersect,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum RuleType {
    #[default]
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub layer: KnowledgeLayer,
    #[serde(default)]
    pub mode: PolicyMode,
    #[serde(default)]
    pub merge_strategy: RuleMergeStrategy,
    pub rules: Vec<PolicyRule>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    pub id: String,
    #[serde(default)]
    pub rule_type: RuleType,
    pub target: ConstraintTarget,
    pub operator: ConstraintOperator,
    pub value: serde_json::Value,
    pub severity: ConstraintSeverity,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<PolicyViolation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyViolation {
    pub rule_id: String,
    pub policy_id: String,
    pub severity: ConstraintSeverity,
    pub message: String,
    pub context: std::collections::HashMap<String, serde_json::Value>,
}

/// Governance event types for auditing and real-time updates
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum GovernanceEvent {
    /// New organizational unit created
    UnitCreated {
        unit_id: String,
        unit_type: UnitType,
        tenant_id: TenantId,
        parent_id: Option<String>,
        timestamp: i64,
    },

    /// Organizational unit updated
    UnitUpdated {
        unit_id: String,
        tenant_id: TenantId,
        timestamp: i64,
    },

    /// Organizational unit deleted
    UnitDeleted {
        unit_id: String,
        tenant_id: TenantId,
        timestamp: i64,
    },

    /// Role assigned to a user for a specific unit
    RoleAssigned {
        user_id: UserId,
        unit_id: String,
        role: Role,
        tenant_id: TenantId,
        timestamp: i64,
    },

    /// Role removed from a user
    RoleRemoved {
        user_id: UserId,
        unit_id: String,
        role: Role,
        tenant_id: TenantId,
        timestamp: i64,
    },

    /// Policy created or updated
    PolicyUpdated {
        policy_id: String,
        layer: KnowledgeLayer,
        tenant_id: TenantId,
        timestamp: i64,
    },

    /// Policy deleted
    PolicyDeleted {
        policy_id: String,
        tenant_id: TenantId,
        timestamp: i64,
    },

    /// Drift detected in a project
    DriftDetected {
        project_id: String,
        tenant_id: TenantId,
        drift_score: f32,
        timestamp: i64,
    },
}

impl GovernanceEvent {
    #[must_use]
    pub fn tenant_id(&self) -> &TenantId {
        match self {
            GovernanceEvent::UnitCreated { tenant_id, .. } => tenant_id,
            GovernanceEvent::UnitUpdated { tenant_id, .. } => tenant_id,
            GovernanceEvent::UnitDeleted { tenant_id, .. } => tenant_id,
            GovernanceEvent::RoleAssigned { tenant_id, .. } => tenant_id,
            GovernanceEvent::RoleRemoved { tenant_id, .. } => tenant_id,
            GovernanceEvent::PolicyUpdated { tenant_id, .. } => tenant_id,
            GovernanceEvent::PolicyDeleted { tenant_id, .. } => tenant_id,
            GovernanceEvent::DriftDetected { tenant_id, .. } => tenant_id,
        }
    }
}

/// Drift analysis result
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DriftResult {
    pub project_id: String,
    pub tenant_id: TenantId,
    pub drift_score: f32,
    pub violations: Vec<PolicyViolation>,
    pub timestamp: i64,
}

pub fn validate_user_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new("User ID cannot be empty"));
    }
    if id.len() > 100 {
        return Err(validator::ValidationError::new("User ID is too long"));
    }
    Ok(())
}

pub fn validate_session_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new(
            "Session ID cannot be empty",
        ));
    }
    Ok(())
}

pub fn validate_project_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new(
            "Project ID cannot be empty",
        ));
    }
    Ok(())
}

pub fn validate_team_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new("Team ID cannot be empty"));
    }
    Ok(())
}

pub fn validate_org_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new("Org ID cannot be empty"));
    }
    Ok(())
}

pub fn validate_company_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new(
            "Company ID cannot be empty",
        ));
    }
    Ok(())
}

pub fn validate_agent_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new("Agent ID cannot be empty"));
    }
    if id.len() > 100 {
        return Err(validator::ValidationError::new("Agent ID is too long"));
    }
    Ok(())
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
            updated_at: 1234567890,
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
            updated_at: 1234567890,
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
            rule_type: RuleType::Deny,
            target: ConstraintTarget::Dependency,
            operator: ConstraintOperator::MustNotUse,
            value: serde_json::json!("unsafe-lib"),
            severity: ConstraintSeverity::Block,
            message: "Do not use unsafe libraries".to_string(),
        };

        let policy = Policy {
            id: "policy_1".to_string(),
            name: "Security Policy".to_string(),
            description: Some("Security constraints".to_string()),
            layer: KnowledgeLayer::Company,
            mode: PolicyMode::Mandatory,
            merge_strategy: RuleMergeStrategy::Merge,
            rules: vec![rule],
            metadata: std::collections::HashMap::new(),
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
            context: std::collections::HashMap::new(),
        };

        let result = ValidationResult {
            is_valid: false,
            violations: vec![violation],
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
            company_id: Some("company_123".to_string()),
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
            company_id: None,
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
    fn test_role_serialization() {
        let architect = Role::Architect;
        let json = serde_json::to_string(&architect).unwrap();
        assert_eq!(json, "\"architect\"");

        let deserialized: Role = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, Role::Architect);
    }

    #[test]
    fn test_role_precedence() {
        assert_eq!(Role::Admin.precedence(), 4);
        assert_eq!(Role::Architect.precedence(), 3);
        assert_eq!(Role::TechLead.precedence(), 2);
        assert_eq!(Role::Developer.precedence(), 1);
        assert_eq!(Role::Agent.precedence(), 0);
    }

    #[test]
    fn test_role_display_name() {
        assert_eq!(Role::Developer.display_name(), "Developer");
        assert_eq!(Role::TechLead.display_name(), "Tech Lead");
        assert_eq!(Role::Architect.display_name(), "Architect");
        assert_eq!(Role::Admin.display_name(), "Admin");
        assert_eq!(Role::Agent.display_name(), "Agent");
    }

    #[test]
    fn test_tenant_id_validation() {
        assert!(TenantId::new("comp_123".to_string()).is_some());
        assert!(TenantId::new("".to_string()).is_none());
        assert!(TenantId::new("a".repeat(101)).is_none());
    }

    #[test]
    fn test_user_id_validation() {
        assert!(UserId::new("user_456".to_string()).is_some());
        assert!(UserId::new("".to_string()).is_none());
        assert!(UserId::new("a".repeat(101)).is_none());
    }

    #[test]
    fn test_hierarchy_path_depth() {
        let company = HierarchyPath::company("c1".to_string());
        assert_eq!(company.depth(), 1);

        let org = HierarchyPath::org("c1".to_string(), "o1".to_string());
        assert_eq!(org.depth(), 2);

        let team = HierarchyPath::team("c1".to_string(), "o1".to_string(), "t1".to_string());
        assert_eq!(team.depth(), 3);

        let project = HierarchyPath::project(
            "c1".to_string(),
            "o1".to_string(),
            "t1".to_string(),
            "p1".to_string(),
        );
        assert_eq!(project.depth(), 4);
    }

    #[test]
    fn test_hierarchy_path_string() {
        let project = HierarchyPath::project(
            "c1".to_string(),
            "o1".to_string(),
            "t1".to_string(),
            "p1".to_string(),
        );
        assert_eq!(project.path_string(), "c1 > o1 > t1 > p1");
    }

    #[test]
    fn test_tenant_context_creation() {
        let tenant_id = TenantId::new("c1".to_string()).unwrap();
        let user_id = UserId::new("u1".to_string()).unwrap();
        let ctx = TenantContext::new(tenant_id, user_id);

        assert_eq!(ctx.tenant_id.as_str(), "c1");
        assert_eq!(ctx.user_id.as_str(), "u1");
        assert!(ctx.agent_id.is_none());
    }

    #[test]
    fn test_tenant_context_with_agent() {
        let tenant_id = TenantId::new("c1".to_string()).unwrap();
        let user_id = UserId::new("u1".to_string()).unwrap();
        let ctx = TenantContext::with_agent(tenant_id, user_id, "a1".to_string());

        assert_eq!(ctx.agent_id.unwrap(), "a1");
    }

    #[test]
    fn test_tenant_id_display() {
        let id = TenantId::new("c1".to_string()).unwrap();
        assert_eq!(format!("{}", id), "c1");
    }

    #[test]
    fn test_user_id_display() {
        let id = UserId::new("u1".to_string()).unwrap();
        assert_eq!(format!("{}", id), "u1");
    }

    #[test]
    fn test_tenant_id_from_str() {
        use std::str::FromStr;
        let id = TenantId::from_str("c1").unwrap();
        assert_eq!(id.as_str(), "c1");
        assert!(TenantId::from_str("").is_err());
    }

    #[test]
    fn test_user_id_from_str() {
        use std::str::FromStr;
        let id = UserId::from_str("u1").unwrap();
        assert_eq!(id.as_str(), "u1");
        assert!(UserId::from_str("").is_err());
    }

    #[test]
    fn test_tenant_id_into_inner() {
        let id = TenantId::new("c1".to_string()).unwrap();
        assert_eq!(id.into_inner(), "c1");
    }

    #[test]
    fn test_user_id_into_inner() {
        let id = UserId::new("u1".to_string()).unwrap();
        assert_eq!(id.into_inner(), "u1");
    }
}
