use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumString};
use utoipa::ToSchema;
use validator::Validate;

#[derive(
    Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, EnumString, Display,
)]
#[serde(rename_all = "camelCase")]
#[strum(ascii_case_insensitive)]
pub enum Role {
    Developer,
    TechLead,
    Architect,
    Admin,
    Agent
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
    Project
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
    pub updated_at: i64
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
            agent_id: None
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
    pub agent_id: Option<String>
}

impl TenantContext {
    pub fn new(tenant_id: TenantId, user_id: UserId) -> Self {
        Self {
            tenant_id,
            user_id,
            agent_id: None
        }
    }

    pub fn with_agent(tenant_id: TenantId, user_id: UserId, agent_id: String) -> Self {
        Self {
            tenant_id,
            user_id,
            agent_id: Some(agent_id)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct HierarchyPath {
    pub company: String,
    pub org: Option<String>,
    pub team: Option<String>,
    pub project: Option<String>
}

impl HierarchyPath {
    pub fn company(id: String) -> Self {
        Self {
            company: id,
            org: None,
            team: None,
            project: None
        }
    }

    pub fn org(company: String, id: String) -> Self {
        Self {
            company,
            org: Some(id),
            team: None,
            project: None
        }
    }

    pub fn team(company: String, org: String, id: String) -> Self {
        Self {
            company,
            org: Some(org),
            team: Some(id),
            project: None
        }
    }

    pub fn project(company: String, org: String, team: String, id: String) -> Self {
        Self {
            company,
            org: Some(org),
            team: Some(team),
            project: Some(id)
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
            Role::Agent => 0
        }
    }

    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Role::Developer => "Developer",
            Role::TechLead => "Tech Lead",
            Role::Architect => "Architect",
            Role::Admin => "Admin",
            Role::Agent => "Agent"
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
    Spec
}

/// Knowledge status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum KnowledgeStatus {
    Draft,
    Proposed,
    Accepted,
    Deprecated,
    Superseded
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
    Project
}

/// Constraint severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintSeverity {
    Info,
    Warn,
    Block
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
    MustNotExist
}

/// Constraint targets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ConstraintTarget {
    File,
    Code,
    Dependency,
    Import,
    Config
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
    Company
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
            MemoryLayer::Company => 7
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
            MemoryLayer::Company => "Company"
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
    pub company_id: Option<String>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum SummaryDepth {
    Sentence,
    Paragraph,
    Detailed
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LayerSummary {
    pub depth: SummaryDepth,
    pub content: String,
    pub token_count: u32,
    pub generated_at: i64,
    pub source_hash: String,
    pub personalized: bool,
    pub personalization_context: Option<String>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummaryConfig {
    pub layer: MemoryLayer,
    pub update_interval_secs: Option<u64>,
    pub update_on_changes: Option<u32>,
    pub skip_if_unchanged: bool,
    pub personalized: bool,
    pub depths: Vec<SummaryDepth>
}

pub type ContextVector = Vec<f32>;

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
    ToSchema,
    Display,
    EnumString,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum ReasoningStrategy {
    Exhaustive,
    Targeted,
    SemanticOnly
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningTrace {
    pub strategy: ReasoningStrategy,
    pub thought_process: String,
    pub refined_query: Option<String>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: chrono::DateTime<chrono::Utc>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub embedding: Option<Vec<f32>>,
    pub layer: MemoryLayer,
    pub summaries: std::collections::HashMap<SummaryDepth, LayerSummary>,
    pub context_vector: Option<ContextVector>,
    pub importance_score: Option<f32>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub created_at: i64,
    pub updated_at: i64
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum MemoryOperation {
    Add,
    Update,
    Delete,
    Retrieve,
    Prune,
    Compress,
    Noop
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
    JsonSchema,
    ToSchema,
    Display,
    EnumString,
)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum RewardType {
    Helpful,
    Irrelevant,
    Outdated,
    Inaccurate,
    Duplicate
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RewardSignal {
    pub reward_type: RewardType,
    pub score: f32, // -1.0 to 1.0
    pub reasoning: Option<String>,
    pub agent_id: Option<String>,
    pub timestamp: i64
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTrajectoryEvent {
    pub operation: MemoryOperation,
    pub entry_id: String,
    pub reward: Option<RewardSignal>,
    pub reasoning: Option<String>,
    pub timestamp: i64
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
    pub embedding: Option<Vec<f32>>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub id: String,
    pub source_id: String,
    pub target_id: String,
    pub relation_type: String,
    pub weight: f32,
    pub description: Option<String>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Community {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub level: u32,
    pub entity_ids: Vec<String>,
    pub relationship_ids: Vec<String>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeEntry {
    pub path: String,
    pub content: String,
    pub layer: KnowledgeLayer,
    pub kind: KnowledgeType,
    pub status: KnowledgeStatus,
    pub summaries: std::collections::HashMap<SummaryDepth, LayerSummary>,
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    pub commit_hash: Option<String>,
    pub author: Option<String>,
    pub updated_at: i64
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum PolicyMode {
    #[default]
    Optional,
    Mandatory
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum RuleMergeStrategy {
    #[default]
    Override,
    Merge,
    Intersect
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema, Default, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub enum RuleType {
    #[default]
    Allow,
    Deny
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
    pub metadata: std::collections::HashMap<String, serde_json::Value>
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
    pub message: String
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<PolicyViolation>
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PolicyViolation {
    pub rule_id: String,
    pub policy_id: String,
    pub severity: ConstraintSeverity,
    pub message: String,
    pub context: std::collections::HashMap<String, serde_json::Value>
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
        timestamp: i64
    },

    /// Organizational unit updated
    UnitUpdated {
        unit_id: String,
        tenant_id: TenantId,
        timestamp: i64
    },

    /// Organizational unit deleted
    UnitDeleted {
        unit_id: String,
        tenant_id: TenantId,
        timestamp: i64
    },

    /// Role assigned to a user for a specific unit
    RoleAssigned {
        user_id: UserId,
        unit_id: String,
        role: Role,
        tenant_id: TenantId,
        timestamp: i64
    },

    /// Role removed from a user
    RoleRemoved {
        user_id: UserId,
        unit_id: String,
        role: Role,
        tenant_id: TenantId,
        timestamp: i64
    },

    /// Policy created or updated
    PolicyUpdated {
        policy_id: String,
        layer: KnowledgeLayer,
        tenant_id: TenantId,
        timestamp: i64
    },

    /// Policy deleted
    PolicyDeleted {
        policy_id: String,
        tenant_id: TenantId,
        timestamp: i64
    },

    /// Drift detected in a project
    DriftDetected {
        project_id: String,
        tenant_id: TenantId,
        drift_score: f32,
        timestamp: i64
    }
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
            GovernanceEvent::DriftDetected { tenant_id, .. } => tenant_id
        }
    }
}

/// Drift analysis result with confidence scoring
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DriftResult {
    pub project_id: String,
    pub tenant_id: TenantId,
    pub drift_score: f32,
    pub confidence: f32,
    pub violations: Vec<PolicyViolation>,
    pub suppressed_violations: Vec<PolicyViolation>,
    pub requires_manual_review: bool,
    pub timestamp: i64
}

impl DriftResult {
    pub fn new(project_id: String, tenant_id: TenantId, violations: Vec<PolicyViolation>) -> Self {
        let drift_score = Self::calculate_score(&violations);
        Self {
            project_id,
            tenant_id,
            drift_score,
            confidence: 1.0,
            violations,
            suppressed_violations: Vec::new(),
            requires_manual_review: false,
            timestamp: chrono::Utc::now().timestamp()
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self.requires_manual_review = self.confidence < 0.7;
        self
    }

    pub fn with_suppressions(mut self, suppressed: Vec<PolicyViolation>) -> Self {
        self.suppressed_violations = suppressed;
        self
    }

    fn calculate_score(violations: &[PolicyViolation]) -> f32 {
        if violations.is_empty() {
            return 0.0;
        }
        violations
            .iter()
            .map(|v| match v.severity {
                ConstraintSeverity::Block => 1.0,
                ConstraintSeverity::Warn => 0.5,
                ConstraintSeverity::Info => 0.1
            })
            .sum::<f32>()
            .min(1.0)
    }

    pub fn active_violation_count(&self) -> usize {
        self.violations.len()
    }

    pub fn suppressed_count(&self) -> usize {
        self.suppressed_violations.len()
    }
}

/// Drift suppression rule to ignore specific violations
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DriftSuppression {
    pub id: String,
    pub project_id: String,
    pub tenant_id: TenantId,
    pub policy_id: String,
    pub rule_pattern: Option<String>,
    pub reason: String,
    pub created_by: UserId,
    pub expires_at: Option<i64>,
    pub created_at: i64
}

impl DriftSuppression {
    pub fn new(
        project_id: String,
        tenant_id: TenantId,
        policy_id: String,
        reason: String,
        created_by: UserId
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            project_id,
            tenant_id,
            policy_id,
            rule_pattern: None,
            reason,
            created_by,
            expires_at: None,
            created_at: chrono::Utc::now().timestamp()
        }
    }

    pub fn with_pattern(mut self, pattern: String) -> Self {
        self.rule_pattern = Some(pattern);
        self
    }

    pub fn with_expiry(mut self, expires_at: i64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            chrono::Utc::now().timestamp() > expires
        } else {
            false
        }
    }

    pub fn matches(&self, violation: &PolicyViolation) -> bool {
        if self.policy_id != violation.policy_id {
            return false;
        }
        if let Some(pattern) = &self.rule_pattern {
            if let Ok(re) = regex::Regex::new(pattern) {
                return re.is_match(&violation.message);
            }
        }
        true
    }
}

/// Drift threshold configuration per project
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DriftConfig {
    pub project_id: String,
    pub tenant_id: TenantId,
    pub threshold: f32,
    pub low_confidence_threshold: f32,
    pub auto_suppress_info: bool,
    pub updated_at: i64
}

impl Default for DriftConfig {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            tenant_id: TenantId::default(),
            threshold: 0.2,
            low_confidence_threshold: 0.7,
            auto_suppress_info: false,
            updated_at: chrono::Utc::now().timestamp()
        }
    }
}

impl DriftConfig {
    pub fn new(project_id: String, tenant_id: TenantId) -> Self {
        Self {
            project_id,
            tenant_id,
            ..Default::default()
        }
    }

    pub fn for_project(project_id: String, tenant_id: TenantId) -> Self {
        Self::new(project_id, tenant_id)
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold.clamp(0.0, 1.0);
        self
    }
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
            "Session ID cannot be empty"
        ));
    }
    Ok(())
}

pub fn validate_project_id(id: &&String) -> Result<(), validator::ValidationError> {
    if id.is_empty() {
        return Err(validator::ValidationError::new(
            "Project ID cannot be empty"
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
            "Company ID cannot be empty"
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Pending,
    Published,
    Acknowledged,
    DeadLettered
}

impl std::fmt::Display for EventStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventStatus::Pending => write!(f, "pending"),
            EventStatus::Published => write!(f, "published"),
            EventStatus::Acknowledged => write!(f, "acknowledged"),
            EventStatus::DeadLettered => write!(f, "dead_lettered")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersistentEvent {
    pub id: String,
    pub event_id: String,
    pub idempotency_key: String,
    pub tenant_id: TenantId,
    pub event_type: String,
    pub payload: GovernanceEvent,
    pub status: EventStatus,
    pub retry_count: i32,
    pub max_retries: i32,
    pub last_error: Option<String>,
    pub created_at: i64,
    pub published_at: Option<i64>,
    pub acknowledged_at: Option<i64>,
    pub dead_lettered_at: Option<i64>
}

impl PersistentEvent {
    pub fn new(event: GovernanceEvent) -> Self {
        let event_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();
        let tenant_id = event.tenant_id().clone();
        let idempotency_key = Self::calculate_idempotency_key(&event_id, timestamp, &tenant_id);
        let event_type = Self::event_type_name(&event);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            event_id,
            idempotency_key,
            tenant_id,
            event_type,
            payload: event,
            status: EventStatus::Pending,
            retry_count: 0,
            max_retries: 3,
            last_error: None,
            created_at: timestamp,
            published_at: None,
            acknowledged_at: None,
            dead_lettered_at: None
        }
    }

    fn calculate_idempotency_key(event_id: &str, timestamp: i64, tenant_id: &TenantId) -> String {
        use sha2::{Digest, Sha256};
        let input = format!("{}:{}:{}", event_id, timestamp, tenant_id.as_str());
        let hash = Sha256::digest(input.as_bytes());
        hex::encode(hash)
    }

    fn event_type_name(event: &GovernanceEvent) -> String {
        match event {
            GovernanceEvent::UnitCreated { .. } => "unit_created".to_string(),
            GovernanceEvent::UnitUpdated { .. } => "unit_updated".to_string(),
            GovernanceEvent::UnitDeleted { .. } => "unit_deleted".to_string(),
            GovernanceEvent::RoleAssigned { .. } => "role_assigned".to_string(),
            GovernanceEvent::RoleRemoved { .. } => "role_removed".to_string(),
            GovernanceEvent::PolicyUpdated { .. } => "policy_updated".to_string(),
            GovernanceEvent::PolicyDeleted { .. } => "policy_deleted".to_string(),
            GovernanceEvent::DriftDetected { .. } => "drift_detected".to_string()
        }
    }

    pub fn mark_published(&mut self) {
        self.status = EventStatus::Published;
        self.published_at = Some(chrono::Utc::now().timestamp());
    }

    pub fn mark_acknowledged(&mut self) {
        self.status = EventStatus::Acknowledged;
        self.acknowledged_at = Some(chrono::Utc::now().timestamp());
    }

    pub fn mark_failed(&mut self, error: String) -> bool {
        self.retry_count += 1;
        self.last_error = Some(error);

        if self.retry_count >= self.max_retries {
            self.status = EventStatus::DeadLettered;
            self.dead_lettered_at = Some(chrono::Utc::now().timestamp());
            false
        } else {
            self.status = EventStatus::Pending;
            true
        }
    }

    pub fn is_retriable(&self) -> bool {
        self.retry_count < self.max_retries && self.status == EventStatus::Pending
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventDeliveryMetrics {
    pub tenant_id: TenantId,
    pub event_type: String,
    pub period_start: i64,
    pub period_end: i64,
    pub total_events: i64,
    pub delivered_events: i64,
    pub retried_events: i64,
    pub dead_lettered_events: i64,
    pub avg_delivery_time_ms: Option<f64>
}

impl EventDeliveryMetrics {
    pub fn new(
        tenant_id: TenantId,
        event_type: String,
        period_start: i64,
        period_end: i64
    ) -> Self {
        Self {
            tenant_id,
            event_type,
            period_start,
            period_end,
            total_events: 0,
            delivered_events: 0,
            retried_events: 0,
            dead_lettered_events: 0,
            avg_delivery_time_ms: None
        }
    }

    pub fn delivery_success_rate(&self) -> f64 {
        if self.total_events == 0 {
            return 1.0;
        }
        self.delivered_events as f64 / self.total_events as f64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsumerState {
    pub consumer_group: String,
    pub idempotency_key: String,
    pub tenant_id: TenantId,
    pub processed_at: i64
}

impl ConsumerState {
    pub fn new(consumer_group: String, idempotency_key: String, tenant_id: TenantId) -> Self {
        Self {
            consumer_group,
            idempotency_key,
            tenant_id,
            processed_at: chrono::Utc::now().timestamp()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobCoordinationMetrics {
    pub job_name: String,
    pub tenant_id: TenantId,
    pub total_runs: u64,
    pub successful_runs: u64,
    pub failed_runs: u64,
    pub skipped_runs: u64,
    pub timeout_count: u64,
    pub total_duration_ms: u64,
    pub last_run_at: Option<i64>,
    pub last_success_at: Option<i64>
}

impl JobCoordinationMetrics {
    pub fn new(job_name: String, tenant_id: TenantId) -> Self {
        Self {
            job_name,
            tenant_id,
            total_runs: 0,
            successful_runs: 0,
            failed_runs: 0,
            skipped_runs: 0,
            timeout_count: 0,
            total_duration_ms: 0,
            last_run_at: None,
            last_success_at: None
        }
    }

    pub fn record_run(&mut self, duration_ms: u64, success: bool) {
        self.total_runs += 1;
        self.total_duration_ms += duration_ms;
        self.last_run_at = Some(chrono::Utc::now().timestamp());
        if success {
            self.successful_runs += 1;
            self.last_success_at = self.last_run_at;
        } else {
            self.failed_runs += 1;
        }
    }

    pub fn record_skip(&mut self) {
        self.skipped_runs += 1;
    }

    pub fn record_timeout(&mut self) {
        self.timeout_count += 1;
        self.failed_runs += 1;
    }

    pub fn avg_duration_ms(&self) -> Option<f64> {
        if self.total_runs == 0 {
            None
        } else {
            Some(self.total_duration_ms as f64 / self.total_runs as f64)
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_runs == 0 {
            1.0
        } else {
            self.successful_runs as f64 / self.total_runs as f64
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartialJobResult {
    pub job_name: String,
    pub tenant_id: TenantId,
    pub checkpoint_id: String,
    pub processed_count: usize,
    pub total_count: Option<usize>,
    pub last_processed_id: Option<String>,
    pub partial_data: serde_json::Value,
    pub created_at: i64
}

impl PartialJobResult {
    pub fn new(job_name: String, tenant_id: TenantId) -> Self {
        Self {
            job_name,
            tenant_id,
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
            processed_count: 0,
            total_count: None,
            last_processed_id: None,
            partial_data: serde_json::Value::Null,
            created_at: chrono::Utc::now().timestamp()
        }
    }

    pub fn with_progress(mut self, processed: usize, total: Option<usize>) -> Self {
        self.processed_count = processed;
        self.total_count = total;
        self
    }

    pub fn with_last_id(mut self, id: String) -> Self {
        self.last_processed_id = Some(id);
        self
    }

    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.partial_data = data;
        self
    }

    pub fn progress_percentage(&self) -> Option<f64> {
        self.total_count
            .map(|total| (self.processed_count as f64 / total as f64) * 100.0)
    }
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
            summaries: std::collections::HashMap::new(),
            context_vector: None,
            importance_score: None,
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
            summaries: std::collections::HashMap::new(),
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
            rule_type: RuleType::Deny,
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
            mode: PolicyMode::Mandatory,
            merge_strategy: RuleMergeStrategy::Merge,
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
    fn test_reasoning_strategy_serialization() {
        let exhaustive = ReasoningStrategy::Exhaustive;
        let json = serde_json::to_string(&exhaustive).unwrap();
        assert_eq!(json, "\"exhaustive\"");

        let deserialized: ReasoningStrategy = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ReasoningStrategy::Exhaustive);
    }

    #[test]
    fn test_reasoning_strategy_display() {
        assert_eq!(format!("{}", ReasoningStrategy::Exhaustive), "exhaustive");
        assert_eq!(format!("{}", ReasoningStrategy::Targeted), "targeted");
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
            "p1".to_string()
        );
        assert_eq!(project.depth(), 4);
    }

    #[test]
    fn test_hierarchy_path_string() {
        let project = HierarchyPath::project(
            "c1".to_string(),
            "o1".to_string(),
            "t1".to_string(),
            "p1".to_string()
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

    #[test]
    fn test_governance_event_tenant_id() {
        let tenant_id = TenantId::new("tenant-1".to_string()).unwrap();
        let user_id = UserId::new("user-1".to_string()).unwrap();

        let events = vec![
            GovernanceEvent::UnitCreated {
                unit_id: "u1".to_string(),
                unit_type: UnitType::Company,
                tenant_id: tenant_id.clone(),
                parent_id: None,
                timestamp: 0
            },
            GovernanceEvent::UnitUpdated {
                unit_id: "u1".to_string(),
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            GovernanceEvent::UnitDeleted {
                unit_id: "u1".to_string(),
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            GovernanceEvent::RoleAssigned {
                user_id: user_id.clone(),
                unit_id: "u1".to_string(),
                role: Role::Admin,
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            GovernanceEvent::RoleRemoved {
                user_id: user_id.clone(),
                unit_id: "u1".to_string(),
                role: Role::Admin,
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            GovernanceEvent::PolicyUpdated {
                policy_id: "p1".to_string(),
                layer: KnowledgeLayer::Company,
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            GovernanceEvent::PolicyDeleted {
                policy_id: "p1".to_string(),
                tenant_id: tenant_id.clone(),
                timestamp: 0
            },
            GovernanceEvent::DriftDetected {
                project_id: "proj-1".to_string(),
                tenant_id: tenant_id.clone(),
                drift_score: 0.5,
                timestamp: 0
            },
        ];

        for event in events {
            assert_eq!(event.tenant_id().as_str(), "tenant-1");
        }
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
    fn test_drift_suppression_new() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let user_id = UserId::new("user1".to_string()).unwrap();

        let suppression = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id.clone(),
            "policy-1".to_string(),
            "False positive".to_string(),
            user_id.clone()
        );

        assert_eq!(suppression.project_id, "proj-1");
        assert_eq!(suppression.tenant_id, tenant_id);
        assert_eq!(suppression.policy_id, "policy-1");
        assert_eq!(suppression.reason, "False positive");
        assert!(suppression.rule_pattern.is_none());
        assert!(suppression.expires_at.is_none());
        assert!(!suppression.id.is_empty());
    }

    #[test]
    fn test_drift_suppression_with_pattern() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let user_id = UserId::new("user1".to_string()).unwrap();

        let suppression = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id,
            "policy-1".to_string(),
            "Known issue".to_string(),
            user_id
        )
        .with_pattern(".*test.*".to_string());

        assert_eq!(suppression.rule_pattern, Some(".*test.*".to_string()));
    }

    #[test]
    fn test_drift_suppression_with_expiry() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let user_id = UserId::new("user1".to_string()).unwrap();
        let future_time = chrono::Utc::now().timestamp() + 86400;

        let suppression = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id,
            "policy-1".to_string(),
            "Temporary".to_string(),
            user_id
        )
        .with_expiry(future_time);

        assert_eq!(suppression.expires_at, Some(future_time));
    }

    #[test]
    fn test_drift_suppression_is_expired() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let user_id = UserId::new("user1".to_string()).unwrap();

        let not_expired = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id.clone(),
            "policy-1".to_string(),
            "Not expired".to_string(),
            user_id.clone()
        );
        assert!(!not_expired.is_expired());

        let future_expiry = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id.clone(),
            "policy-1".to_string(),
            "Future".to_string(),
            user_id.clone()
        )
        .with_expiry(chrono::Utc::now().timestamp() + 86400);
        assert!(!future_expiry.is_expired());

        let past_expiry = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id,
            "policy-1".to_string(),
            "Expired".to_string(),
            user_id
        )
        .with_expiry(chrono::Utc::now().timestamp() - 86400);
        assert!(past_expiry.is_expired());
    }

    #[test]
    fn test_drift_suppression_matches() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let user_id = UserId::new("user1".to_string()).unwrap();

        let violation = PolicyViolation {
            rule_id: "rule-1".to_string(),
            policy_id: "policy-1".to_string(),
            severity: ConstraintSeverity::Warn,
            message: "Test violation message".to_string(),
            context: std::collections::HashMap::new()
        };

        let suppression_match = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id.clone(),
            "policy-1".to_string(),
            "Match all".to_string(),
            user_id.clone()
        );
        assert!(suppression_match.matches(&violation));

        let suppression_no_match = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id.clone(),
            "policy-2".to_string(),
            "Different policy".to_string(),
            user_id.clone()
        );
        assert!(!suppression_no_match.matches(&violation));

        let suppression_pattern_match = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id.clone(),
            "policy-1".to_string(),
            "Pattern match".to_string(),
            user_id.clone()
        )
        .with_pattern(".*violation.*".to_string());
        assert!(suppression_pattern_match.matches(&violation));

        let suppression_pattern_no_match = DriftSuppression::new(
            "proj-1".to_string(),
            tenant_id,
            "policy-1".to_string(),
            "Pattern no match".to_string(),
            user_id
        )
        .with_pattern(".*xyz.*".to_string());
        assert!(!suppression_pattern_no_match.matches(&violation));
    }

    #[test]
    fn test_drift_config_default() {
        let config = DriftConfig::default();
        assert!(config.project_id.is_empty());
        assert_eq!(config.threshold, 0.2);
        assert_eq!(config.low_confidence_threshold, 0.7);
        assert!(!config.auto_suppress_info);
    }

    #[test]
    fn test_drift_config_new() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let config = DriftConfig::new("proj-1".to_string(), tenant_id.clone());

        assert_eq!(config.project_id, "proj-1");
        assert_eq!(config.tenant_id, tenant_id);
        assert_eq!(config.threshold, 0.2);
    }

    #[test]
    fn test_drift_config_for_project() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let config = DriftConfig::for_project("proj-2".to_string(), tenant_id.clone());

        assert_eq!(config.project_id, "proj-2");
        assert_eq!(config.tenant_id, tenant_id);
    }

    #[test]
    fn test_drift_config_with_threshold() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let config = DriftConfig::new("proj-1".to_string(), tenant_id).with_threshold(0.5);

        assert_eq!(config.threshold, 0.5);
    }

    #[test]
    fn test_drift_config_with_threshold_clamped() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();

        let config_low =
            DriftConfig::new("proj-1".to_string(), tenant_id.clone()).with_threshold(-0.5);
        assert_eq!(config_low.threshold, 0.0);

        let config_high = DriftConfig::new("proj-1".to_string(), tenant_id).with_threshold(1.5);
        assert_eq!(config_high.threshold, 1.0);
    }

    #[test]
    fn test_drift_result_active_violation_count() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let violations = vec![
            PolicyViolation {
                rule_id: "r1".to_string(),
                policy_id: "p1".to_string(),
                severity: ConstraintSeverity::Warn,
                message: "Warning".to_string(),
                context: std::collections::HashMap::new()
            },
            PolicyViolation {
                rule_id: "r2".to_string(),
                policy_id: "p1".to_string(),
                severity: ConstraintSeverity::Block,
                message: "Blocking".to_string(),
                context: std::collections::HashMap::new()
            },
        ];

        let result = DriftResult::new("proj-1".to_string(), tenant_id, violations);
        assert_eq!(result.active_violation_count(), 2);
    }

    #[test]
    fn test_drift_result_suppressed_count() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut result = DriftResult::new("proj-1".to_string(), tenant_id, vec![]);
        result.suppressed_violations = vec![PolicyViolation {
            rule_id: "r1".to_string(),
            policy_id: "p1".to_string(),
            severity: ConstraintSeverity::Info,
            message: "Suppressed".to_string(),
            context: std::collections::HashMap::new()
        }];

        assert_eq!(result.suppressed_count(), 1);
    }

    #[test]
    fn test_job_coordination_metrics_new() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let metrics = JobCoordinationMetrics::new("drift_scan".to_string(), tenant_id.clone());

        assert_eq!(metrics.job_name, "drift_scan");
        assert_eq!(metrics.tenant_id, tenant_id);
        assert_eq!(metrics.total_runs, 0);
        assert_eq!(metrics.successful_runs, 0);
        assert_eq!(metrics.failed_runs, 0);
        assert_eq!(metrics.skipped_runs, 0);
        assert_eq!(metrics.timeout_count, 0);
        assert_eq!(metrics.total_duration_ms, 0);
        assert!(metrics.last_run_at.is_none());
        assert!(metrics.last_success_at.is_none());
    }

    #[test]
    fn test_job_coordination_metrics_record_run_success() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics = JobCoordinationMetrics::new("test_job".to_string(), tenant_id);
        metrics.record_run(1000, true);

        assert_eq!(metrics.total_runs, 1);
        assert_eq!(metrics.successful_runs, 1);
        assert_eq!(metrics.failed_runs, 0);
        assert_eq!(metrics.total_duration_ms, 1000);
        assert!(metrics.last_run_at.is_some());
        assert!(metrics.last_success_at.is_some());
    }

    #[test]
    fn test_job_coordination_metrics_record_run_failure() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics = JobCoordinationMetrics::new("test_job".to_string(), tenant_id);
        metrics.record_run(500, false);

        assert_eq!(metrics.total_runs, 1);
        assert_eq!(metrics.successful_runs, 0);
        assert_eq!(metrics.failed_runs, 1);
        assert_eq!(metrics.total_duration_ms, 500);
        assert!(metrics.last_run_at.is_some());
        assert!(metrics.last_success_at.is_none());
    }

    #[test]
    fn test_job_coordination_metrics_record_skip() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics = JobCoordinationMetrics::new("test_job".to_string(), tenant_id);
        metrics.record_skip();

        assert_eq!(metrics.skipped_runs, 1);
        assert_eq!(metrics.total_runs, 0);
    }

    #[test]
    fn test_job_coordination_metrics_record_timeout() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics = JobCoordinationMetrics::new("test_job".to_string(), tenant_id);
        metrics.record_timeout();

        assert_eq!(metrics.timeout_count, 1);
        assert_eq!(metrics.failed_runs, 1);
    }

    #[test]
    fn test_job_coordination_metrics_avg_duration_ms() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics = JobCoordinationMetrics::new("test_job".to_string(), tenant_id);
        assert!(metrics.avg_duration_ms().is_none());

        metrics.record_run(1000, true);
        metrics.record_run(2000, true);
        assert_eq!(metrics.avg_duration_ms(), Some(1500.0));
    }

    #[test]
    fn test_job_coordination_metrics_success_rate() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics = JobCoordinationMetrics::new("test_job".to_string(), tenant_id);
        assert_eq!(metrics.success_rate(), 1.0);

        metrics.record_run(100, true);
        metrics.record_run(100, true);
        metrics.record_run(100, false);
        let rate = metrics.success_rate();
        assert!((rate - 0.6666666666666666).abs() < 0.0001);
    }

    #[test]
    fn test_partial_job_result_new() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let result = PartialJobResult::new("drift_scan".to_string(), tenant_id.clone());

        assert_eq!(result.job_name, "drift_scan");
        assert_eq!(result.tenant_id, tenant_id);
        assert_eq!(result.processed_count, 0);
        assert!(result.total_count.is_none());
        assert!(result.last_processed_id.is_none());
        assert_eq!(result.partial_data, serde_json::Value::Null);
        assert!(!result.checkpoint_id.is_empty());
    }

    #[test]
    fn test_partial_job_result_with_progress() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let result =
            PartialJobResult::new("drift_scan".to_string(), tenant_id).with_progress(50, Some(100));

        assert_eq!(result.processed_count, 50);
        assert_eq!(result.total_count, Some(100));
    }

    #[test]
    fn test_partial_job_result_with_last_id() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let result = PartialJobResult::new("drift_scan".to_string(), tenant_id)
            .with_last_id("item-50".to_string());

        assert_eq!(result.last_processed_id, Some("item-50".to_string()));
    }

    #[test]
    fn test_partial_job_result_with_data() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let data = serde_json::json!({"key": "value"});
        let result =
            PartialJobResult::new("drift_scan".to_string(), tenant_id).with_data(data.clone());

        assert_eq!(result.partial_data, data);
    }

    #[test]
    fn test_partial_job_result_progress_percentage() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();

        let no_total = PartialJobResult::new("drift_scan".to_string(), tenant_id.clone());
        assert!(no_total.progress_percentage().is_none());

        let with_total =
            PartialJobResult::new("drift_scan".to_string(), tenant_id).with_progress(25, Some(100));
        assert_eq!(with_total.progress_percentage(), Some(25.0));
    }

    #[test]
    fn test_persistent_event_mark_published() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let event = GovernanceEvent::UnitCreated {
            unit_id: "u1".to_string(),
            unit_type: UnitType::Company,
            tenant_id: tenant_id.clone(),
            parent_id: None,
            timestamp: 0
        };

        let mut persistent = PersistentEvent::new(event);
        assert_eq!(persistent.status, EventStatus::Pending);
        assert!(persistent.published_at.is_none());

        persistent.mark_published();
        assert_eq!(persistent.status, EventStatus::Published);
        assert!(persistent.published_at.is_some());
    }

    #[test]
    fn test_persistent_event_mark_acknowledged() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let event = GovernanceEvent::UnitCreated {
            unit_id: "u1".to_string(),
            unit_type: UnitType::Company,
            tenant_id: tenant_id.clone(),
            parent_id: None,
            timestamp: 0
        };

        let mut persistent = PersistentEvent::new(event);
        persistent.mark_acknowledged();

        assert_eq!(persistent.status, EventStatus::Acknowledged);
        assert!(persistent.acknowledged_at.is_some());
    }

    #[test]
    fn test_persistent_event_mark_failed_retriable() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let event = GovernanceEvent::UnitCreated {
            unit_id: "u1".to_string(),
            unit_type: UnitType::Company,
            tenant_id: tenant_id.clone(),
            parent_id: None,
            timestamp: 0
        };

        let mut persistent = PersistentEvent::new(event);
        let can_retry = persistent.mark_failed("Connection timeout".to_string());

        assert!(can_retry);
        assert_eq!(persistent.retry_count, 1);
        assert_eq!(
            persistent.last_error,
            Some("Connection timeout".to_string())
        );
        assert_eq!(persistent.status, EventStatus::Pending);
    }

    #[test]
    fn test_persistent_event_mark_failed_dead_lettered() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let event = GovernanceEvent::UnitCreated {
            unit_id: "u1".to_string(),
            unit_type: UnitType::Company,
            tenant_id: tenant_id.clone(),
            parent_id: None,
            timestamp: 0
        };

        let mut persistent = PersistentEvent::new(event);
        persistent.mark_failed("Error 1".to_string());
        persistent.mark_failed("Error 2".to_string());
        let can_retry = persistent.mark_failed("Error 3".to_string());

        assert!(!can_retry);
        assert_eq!(persistent.status, EventStatus::DeadLettered);
        assert!(persistent.dead_lettered_at.is_some());
    }

    #[test]
    fn test_persistent_event_is_retriable() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let event = GovernanceEvent::UnitCreated {
            unit_id: "u1".to_string(),
            unit_type: UnitType::Company,
            tenant_id: tenant_id.clone(),
            parent_id: None,
            timestamp: 0
        };

        let mut persistent = PersistentEvent::new(event);
        assert!(persistent.is_retriable());

        persistent.mark_failed("Error 1".to_string());
        assert!(persistent.is_retriable());

        persistent.mark_failed("Error 2".to_string());
        assert!(persistent.is_retriable());

        persistent.mark_failed("Error 3".to_string());
        assert!(!persistent.is_retriable());
    }

    #[test]
    fn test_event_delivery_metrics_new() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let metrics =
            EventDeliveryMetrics::new(tenant_id.clone(), "drift_detected".to_string(), 1000, 2000);

        assert_eq!(metrics.tenant_id, tenant_id);
        assert_eq!(metrics.event_type, "drift_detected");
        assert_eq!(metrics.period_start, 1000);
        assert_eq!(metrics.period_end, 2000);
        assert_eq!(metrics.total_events, 0);
        assert_eq!(metrics.delivered_events, 0);
    }

    #[test]
    fn test_event_delivery_metrics_success_rate() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let mut metrics =
            EventDeliveryMetrics::new(tenant_id, "drift_detected".to_string(), 1000, 2000);

        assert_eq!(metrics.delivery_success_rate(), 1.0);

        metrics.total_events = 10;
        metrics.delivered_events = 8;
        assert_eq!(metrics.delivery_success_rate(), 0.8);
    }

    #[test]
    fn test_consumer_state_new() {
        let tenant_id = TenantId::new("acme".to_string()).unwrap();
        let state = ConsumerState::new(
            "drift_processor".to_string(),
            "idempotency-key-123".to_string(),
            tenant_id.clone()
        );

        assert_eq!(state.consumer_group, "drift_processor");
        assert_eq!(state.idempotency_key, "idempotency-key-123");
        assert_eq!(state.tenant_id, tenant_id);
        assert!(state.processed_at > 0);
    }
}
