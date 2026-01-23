use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    pub id: Option<Uuid>,
    pub company_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub approval_mode: ApprovalMode,
    pub min_approvers: i32,
    pub timeout_hours: i32,
    pub auto_approve_low_risk: bool,
    pub escalation_enabled: bool,
    pub escalation_timeout_hours: i32,
    pub escalation_contact: Option<String>,
    pub policy_settings: serde_json::Value,
    pub knowledge_settings: serde_json::Value,
    pub memory_settings: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalMode {
    Single,
    Quorum,
    Unanimous,
}

impl Default for ApprovalMode {
    fn default() -> Self {
        Self::Quorum
    }
}

impl std::fmt::Display for ApprovalMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalMode::Single => write!(f, "single"),
            ApprovalMode::Quorum => write!(f, "quorum"),
            ApprovalMode::Unanimous => write!(f, "unanimous"),
        }
    }
}

impl std::str::FromStr for ApprovalMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "single" => Ok(ApprovalMode::Single),
            "quorum" => Ok(ApprovalMode::Quorum),
            "unanimous" => Ok(ApprovalMode::Unanimous),
            _ => Err(format!("Invalid approval mode: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GovernanceTemplate {
    Standard,
    Strict,
    Permissive,
}

impl Default for GovernanceTemplate {
    fn default() -> Self {
        Self::Standard
    }
}

impl std::fmt::Display for GovernanceTemplate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GovernanceTemplate::Standard => write!(f, "standard"),
            GovernanceTemplate::Strict => write!(f, "strict"),
            GovernanceTemplate::Permissive => write!(f, "permissive"),
        }
    }
}

impl std::str::FromStr for GovernanceTemplate {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(GovernanceTemplate::Standard),
            "strict" => Ok(GovernanceTemplate::Strict),
            "permissive" => Ok(GovernanceTemplate::Permissive),
            _ => Err(format!(
                "Invalid governance template: {}. Use: standard, strict, permissive",
                s
            )),
        }
    }
}

impl GovernanceTemplate {
    pub fn description(&self) -> &'static str {
        match self {
            GovernanceTemplate::Standard => {
                "Balanced governance with quorum-based approvals (2 approvers, 72h timeout)"
            }
            GovernanceTemplate::Strict => {
                "Maximum control with unanimous approvals (3+ approvers, 24h timeout, no auto-approve)"
            }
            GovernanceTemplate::Permissive => {
                "Minimal friction with single approvals (1 approver, auto-approve low-risk)"
            }
        }
    }

    pub fn to_config(&self) -> GovernanceConfig {
        match self {
            GovernanceTemplate::Standard => GovernanceConfig::default(),
            GovernanceTemplate::Strict => GovernanceConfig {
                id: None,
                company_id: None,
                org_id: None,
                team_id: None,
                project_id: None,
                approval_mode: ApprovalMode::Unanimous,
                min_approvers: 3,
                timeout_hours: 24,
                auto_approve_low_risk: false,
                escalation_enabled: true,
                escalation_timeout_hours: 12,
                escalation_contact: None,
                policy_settings: serde_json::json!({
                    "require_approval": true,
                    "min_approvers": 3,
                    "require_security_review": true,
                    "block_on_conflict": true
                }),
                knowledge_settings: serde_json::json!({
                    "require_approval": true,
                    "min_approvers": 2,
                    "require_tech_lead_approval": true
                }),
                memory_settings: serde_json::json!({
                    "require_approval": true,
                    "min_approvers": 1,
                    "auto_approve_threshold": 0.0
                }),
            },
            GovernanceTemplate::Permissive => GovernanceConfig {
                id: None,
                company_id: None,
                org_id: None,
                team_id: None,
                project_id: None,
                approval_mode: ApprovalMode::Single,
                min_approvers: 1,
                timeout_hours: 168,
                auto_approve_low_risk: true,
                escalation_enabled: false,
                escalation_timeout_hours: 72,
                escalation_contact: None,
                policy_settings: serde_json::json!({
                    "require_approval": true,
                    "min_approvers": 1,
                    "auto_approve_low_risk": true
                }),
                knowledge_settings: serde_json::json!({
                    "require_approval": false,
                    "min_approvers": 0
                }),
                memory_settings: serde_json::json!({
                    "require_approval": false,
                    "auto_approve_threshold": 0.5
                }),
            },
        }
    }

    pub fn all() -> &'static [GovernanceTemplate] {
        &[
            GovernanceTemplate::Standard,
            GovernanceTemplate::Strict,
            GovernanceTemplate::Permissive,
        ]
    }
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        Self {
            id: None,
            company_id: None,
            org_id: None,
            team_id: None,
            project_id: None,
            approval_mode: ApprovalMode::Quorum,
            min_approvers: 2,
            timeout_hours: 72,
            auto_approve_low_risk: false,
            escalation_enabled: true,
            escalation_timeout_hours: 48,
            escalation_contact: None,
            policy_settings: serde_json::json!({"require_approval": true, "min_approvers": 2}),
            knowledge_settings: serde_json::json!({"require_approval": true, "min_approvers": 1}),
            memory_settings: serde_json::json!({"require_approval": false, "auto_approve_threshold": 0.8}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: Uuid,
    pub request_number: String,
    pub request_type: RequestType,
    pub target_type: String,
    pub target_id: Option<String>,
    pub company_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub payload: serde_json::Value,
    pub risk_level: RiskLevel,
    pub requestor_type: PrincipalType,
    pub requestor_id: Uuid,
    pub requestor_email: Option<String>,
    pub required_approvals: i32,
    pub current_approvals: i32,
    pub status: RequestStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_reason: Option<String>,
    pub applied_at: Option<DateTime<Utc>>,
    pub applied_by: Option<Uuid>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestType {
    Policy,
    Knowledge,
    Memory,
    Role,
    Config,
}

impl std::fmt::Display for RequestType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestType::Policy => write!(f, "policy"),
            RequestType::Knowledge => write!(f, "knowledge"),
            RequestType::Memory => write!(f, "memory"),
            RequestType::Role => write!(f, "role"),
            RequestType::Config => write!(f, "config"),
        }
    }
}

impl std::str::FromStr for RequestType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "policy" => Ok(RequestType::Policy),
            "knowledge" => Ok(RequestType::Knowledge),
            "memory" => Ok(RequestType::Memory),
            "role" => Ok(RequestType::Role),
            "config" => Ok(RequestType::Config),
            _ => Err(format!("Invalid request type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl Default for RiskLevel {
    fn default() -> Self {
        Self::Medium
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
            RiskLevel::Critical => write!(f, "critical"),
        }
    }
}

impl std::str::FromStr for RiskLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(RiskLevel::Low),
            "medium" => Ok(RiskLevel::Medium),
            "high" => Ok(RiskLevel::High),
            "critical" => Ok(RiskLevel::Critical),
            _ => Err(format!("Invalid risk level: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PrincipalType {
    User,
    Agent,
    System,
}

impl std::fmt::Display for PrincipalType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrincipalType::User => write!(f, "user"),
            PrincipalType::Agent => write!(f, "agent"),
            PrincipalType::System => write!(f, "system"),
        }
    }
}

impl std::str::FromStr for PrincipalType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "user" => Ok(PrincipalType::User),
            "agent" => Ok(PrincipalType::Agent),
            "system" => Ok(PrincipalType::System),
            _ => Err(format!("Invalid principal type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RequestStatus {
    Pending,
    Approved,
    Rejected,
    Expired,
    Cancelled,
}

impl std::fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestStatus::Pending => write!(f, "pending"),
            RequestStatus::Approved => write!(f, "approved"),
            RequestStatus::Rejected => write!(f, "rejected"),
            RequestStatus::Expired => write!(f, "expired"),
            RequestStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for RequestStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(RequestStatus::Pending),
            "approved" => Ok(RequestStatus::Approved),
            "rejected" => Ok(RequestStatus::Rejected),
            "expired" => Ok(RequestStatus::Expired),
            "cancelled" => Ok(RequestStatus::Cancelled),
            _ => Err(format!("Invalid request status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalDecision {
    pub id: Uuid,
    pub request_id: Uuid,
    pub approver_type: PrincipalType,
    pub approver_id: Uuid,
    pub approver_email: Option<String>,
    pub decision: Decision,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Approve,
    Reject,
    Abstain,
}

impl std::fmt::Display for Decision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Decision::Approve => write!(f, "approve"),
            Decision::Reject => write!(f, "reject"),
            Decision::Abstain => write!(f, "abstain"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceRole {
    pub id: Uuid,
    pub principal_type: PrincipalType,
    pub principal_id: Uuid,
    pub role: String,
    pub company_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub granted_by: Uuid,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revoked_by: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceAuditEntry {
    pub id: Uuid,
    pub action: String,
    pub request_id: Option<Uuid>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub actor_type: PrincipalType,
    pub actor_id: Option<Uuid>,
    pub actor_email: Option<String>,
    pub details: serde_json::Value,
    pub old_values: Option<serde_json::Value>,
    pub new_values: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ConfigRow {
    pub id: Option<Uuid>,
    pub scope_level: String,
    pub approval_mode: String,
    pub min_approvers: i32,
    pub timeout_hours: i32,
    pub auto_approve_low_risk: bool,
    pub escalation_enabled: bool,
    pub escalation_timeout_hours: i32,
    pub escalation_contact: Option<String>,
    pub policy_settings: serde_json::Value,
    pub knowledge_settings: serde_json::Value,
    pub memory_settings: serde_json::Value,
}

#[derive(Debug, Clone, FromRow)]
struct RequestRow {
    id: Uuid,
    request_number: String,
    request_type: String,
    target_type: String,
    target_id: Option<String>,
    company_id: Option<Uuid>,
    org_id: Option<Uuid>,
    team_id: Option<Uuid>,
    project_id: Option<Uuid>,
    title: String,
    description: Option<String>,
    payload: serde_json::Value,
    risk_level: String,
    requestor_type: String,
    requestor_id: Uuid,
    requestor_email: Option<String>,
    required_approvals: i32,
    current_approvals: i32,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    resolved_at: Option<DateTime<Utc>>,
    resolution_reason: Option<String>,
    applied_at: Option<DateTime<Utc>>,
    applied_by: Option<Uuid>,
}

impl From<RequestRow> for ApprovalRequest {
    fn from(row: RequestRow) -> Self {
        Self {
            id: row.id,
            request_number: row.request_number,
            request_type: row.request_type.parse().unwrap_or(RequestType::Policy),
            target_type: row.target_type,
            target_id: row.target_id,
            company_id: row.company_id,
            org_id: row.org_id,
            team_id: row.team_id,
            project_id: row.project_id,
            title: row.title,
            description: row.description,
            payload: row.payload,
            risk_level: row.risk_level.parse().unwrap_or_default(),
            requestor_type: row.requestor_type.parse().unwrap_or(PrincipalType::User),
            requestor_id: row.requestor_id,
            requestor_email: row.requestor_email,
            required_approvals: row.required_approvals,
            current_approvals: row.current_approvals,
            status: row.status.parse().unwrap_or(RequestStatus::Pending),
            created_at: row.created_at,
            updated_at: row.updated_at,
            expires_at: row.expires_at,
            resolved_at: row.resolved_at,
            resolution_reason: row.resolution_reason,
            applied_at: row.applied_at,
            applied_by: row.applied_by,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
struct DecisionRow {
    id: Uuid,
    request_id: Uuid,
    approver_type: String,
    approver_id: Uuid,
    approver_email: Option<String>,
    decision: String,
    comment: Option<String>,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct AuditRow {
    id: Uuid,
    action: String,
    request_id: Option<Uuid>,
    target_type: Option<String>,
    target_id: Option<String>,
    actor_type: String,
    actor_id: Option<Uuid>,
    actor_email: Option<String>,
    details: serde_json::Value,
    old_values: Option<serde_json::Value>,
    new_values: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

pub struct GovernanceStorage {
    pool: PgPool,
}

impl GovernanceStorage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_effective_config(
        &self,
        company_id: Option<Uuid>,
        org_id: Option<Uuid>,
        team_id: Option<Uuid>,
        project_id: Option<Uuid>,
    ) -> Result<GovernanceConfig, sqlx::Error> {
        let row: ConfigRow =
            sqlx::query_as("SELECT * FROM get_effective_governance_config($1, $2, $3, $4)")
                .bind(company_id)
                .bind(org_id)
                .bind(team_id)
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?;

        Ok(GovernanceConfig {
            id: row.id,
            company_id,
            org_id,
            team_id,
            project_id,
            approval_mode: row.approval_mode.parse().unwrap_or_default(),
            min_approvers: row.min_approvers,
            timeout_hours: row.timeout_hours,
            auto_approve_low_risk: row.auto_approve_low_risk,
            escalation_enabled: row.escalation_enabled,
            escalation_timeout_hours: row.escalation_timeout_hours,
            escalation_contact: row.escalation_contact,
            policy_settings: row.policy_settings,
            knowledge_settings: row.knowledge_settings,
            memory_settings: row.memory_settings,
        })
    }

    pub async fn upsert_config(&self, config: &GovernanceConfig) -> Result<Uuid, sqlx::Error> {
        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO governance_configs (
                company_id, org_id, team_id, project_id,
                approval_mode, min_approvers, timeout_hours,
                auto_approve_low_risk, escalation_enabled,
                escalation_timeout_hours, escalation_contact,
                policy_settings, knowledge_settings, memory_settings
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (company_id) WHERE company_id IS NOT NULL
            DO UPDATE SET
                approval_mode = EXCLUDED.approval_mode,
                min_approvers = EXCLUDED.min_approvers,
                timeout_hours = EXCLUDED.timeout_hours,
                auto_approve_low_risk = EXCLUDED.auto_approve_low_risk,
                escalation_enabled = EXCLUDED.escalation_enabled,
                escalation_timeout_hours = EXCLUDED.escalation_timeout_hours,
                escalation_contact = EXCLUDED.escalation_contact,
                policy_settings = EXCLUDED.policy_settings,
                knowledge_settings = EXCLUDED.knowledge_settings,
                memory_settings = EXCLUDED.memory_settings,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(config.company_id)
        .bind(config.org_id)
        .bind(config.team_id)
        .bind(config.project_id)
        .bind(config.approval_mode.to_string())
        .bind(config.min_approvers)
        .bind(config.timeout_hours)
        .bind(config.auto_approve_low_risk)
        .bind(config.escalation_enabled)
        .bind(config.escalation_timeout_hours)
        .bind(&config.escalation_contact)
        .bind(&config.policy_settings)
        .bind(&config.knowledge_settings)
        .bind(&config.memory_settings)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    pub async fn create_request(
        &self,
        request: &CreateApprovalRequest,
    ) -> Result<ApprovalRequest, sqlx::Error> {
        let expires_at = request
            .timeout_hours
            .map(|h| Utc::now() + chrono::Duration::hours(h as i64));

        let row: RequestRow = sqlx::query_as(
            r#"
            INSERT INTO approval_requests (
                request_type, target_type, target_id,
                company_id, org_id, team_id, project_id,
                title, description, payload, risk_level,
                requestor_type, requestor_id, requestor_email,
                required_approvals, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
            RETURNING *
            "#,
        )
        .bind(request.request_type.to_string())
        .bind(&request.target_type)
        .bind(&request.target_id)
        .bind(request.company_id)
        .bind(request.org_id)
        .bind(request.team_id)
        .bind(request.project_id)
        .bind(&request.title)
        .bind(&request.description)
        .bind(&request.payload)
        .bind(request.risk_level.to_string())
        .bind(request.requestor_type.to_string())
        .bind(request.requestor_id)
        .bind(&request.requestor_email)
        .bind(request.required_approvals)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    pub async fn get_request(
        &self,
        request_id: Uuid,
    ) -> Result<Option<ApprovalRequest>, sqlx::Error> {
        let row: Option<RequestRow> =
            sqlx::query_as("SELECT * FROM approval_requests WHERE id = $1")
                .bind(request_id)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(Into::into))
    }

    pub async fn get_request_by_number(
        &self,
        request_number: &str,
    ) -> Result<Option<ApprovalRequest>, sqlx::Error> {
        let row: Option<RequestRow> =
            sqlx::query_as("SELECT * FROM approval_requests WHERE request_number = $1")
                .bind(request_number)
                .fetch_optional(&self.pool)
                .await?;

        Ok(row.map(Into::into))
    }

    pub async fn list_pending_requests(
        &self,
        filters: &RequestFilters,
    ) -> Result<Vec<ApprovalRequest>, sqlx::Error> {
        let request_type_str = filters.request_type.map(|rt| rt.to_string());
        let limit = filters.limit.unwrap_or(100) as i64;

        let rows: Vec<RequestRow> = sqlx::query_as(
            r#"
            SELECT * FROM approval_requests
            WHERE status = 'pending'
              AND ($1::text IS NULL OR request_type = $1)
              AND ($2::uuid IS NULL OR company_id = $2)
              AND ($3::uuid IS NULL OR org_id = $3)
              AND ($4::uuid IS NULL OR team_id = $4)
              AND ($5::uuid IS NULL OR project_id = $5)
              AND ($6::uuid IS NULL OR requestor_id = $6)
            ORDER BY created_at DESC
            LIMIT $7
            "#,
        )
        .bind(&request_type_str)
        .bind(filters.company_id)
        .bind(filters.org_id)
        .bind(filters.team_id)
        .bind(filters.project_id)
        .bind(filters.requestor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn add_decision(
        &self,
        decision: &CreateDecision,
    ) -> Result<ApprovalDecision, sqlx::Error> {
        let row: DecisionRow = sqlx::query_as(
            r#"
            INSERT INTO approval_decisions (
                request_id, approver_type, approver_id,
                approver_email, decision, comment
            ) VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING *
            "#,
        )
        .bind(decision.request_id)
        .bind(decision.approver_type.to_string())
        .bind(decision.approver_id)
        .bind(&decision.approver_email)
        .bind(decision.decision.to_string())
        .bind(&decision.comment)
        .fetch_one(&self.pool)
        .await?;

        Ok(ApprovalDecision {
            id: row.id,
            request_id: row.request_id,
            approver_type: row.approver_type.parse().unwrap_or(PrincipalType::User),
            approver_id: row.approver_id,
            approver_email: row.approver_email,
            decision: match row.decision.as_str() {
                "approve" => Decision::Approve,
                "reject" => Decision::Reject,
                _ => Decision::Abstain,
            },
            comment: row.comment,
            created_at: row.created_at,
        })
    }

    pub async fn reject_request(
        &self,
        request_id: Uuid,
        reason: &str,
    ) -> Result<ApprovalRequest, sqlx::Error> {
        let row: RequestRow = sqlx::query_as(
            r#"
            UPDATE approval_requests
            SET status = 'rejected',
                resolved_at = NOW(),
                resolution_reason = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(request_id)
        .bind(reason)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    pub async fn cancel_request(&self, request_id: Uuid) -> Result<ApprovalRequest, sqlx::Error> {
        let row: RequestRow = sqlx::query_as(
            r#"
            UPDATE approval_requests
            SET status = 'cancelled',
                resolved_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(request_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    pub async fn mark_applied(
        &self,
        request_id: Uuid,
        applied_by: Uuid,
    ) -> Result<ApprovalRequest, sqlx::Error> {
        let row: RequestRow = sqlx::query_as(
            r#"
            UPDATE approval_requests
            SET applied_at = NOW(),
                applied_by = $2,
                updated_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(request_id)
        .bind(applied_by)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    pub async fn get_decisions(
        &self,
        request_id: Uuid,
    ) -> Result<Vec<ApprovalDecision>, sqlx::Error> {
        let rows: Vec<DecisionRow> = sqlx::query_as(
            "SELECT * FROM approval_decisions WHERE request_id = $1 ORDER BY created_at",
        )
        .bind(request_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| ApprovalDecision {
                id: row.id,
                request_id: row.request_id,
                approver_type: row.approver_type.parse().unwrap_or(PrincipalType::User),
                approver_id: row.approver_id,
                approver_email: row.approver_email,
                decision: match row.decision.as_str() {
                    "approve" => Decision::Approve,
                    "reject" => Decision::Reject,
                    _ => Decision::Abstain,
                },
                comment: row.comment,
                created_at: row.created_at,
            })
            .collect())
    }

    pub async fn log_audit(
        &self,
        action: &str,
        request_id: Option<Uuid>,
        target_type: Option<&str>,
        target_id: Option<&str>,
        actor_type: PrincipalType,
        actor_id: Option<Uuid>,
        actor_email: Option<&str>,
        details: serde_json::Value,
    ) -> Result<Uuid, sqlx::Error> {
        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO governance_audit_log (
                action, request_id, target_type, target_id,
                actor_type, actor_id, actor_email, details
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id
            "#,
        )
        .bind(action)
        .bind(request_id)
        .bind(target_type)
        .bind(target_id)
        .bind(actor_type.to_string())
        .bind(actor_id)
        .bind(actor_email)
        .bind(details)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    pub async fn list_audit_logs(
        &self,
        filters: &AuditFilters,
    ) -> Result<Vec<GovernanceAuditEntry>, sqlx::Error> {
        let rows: Vec<AuditRow> = sqlx::query_as(
            r#"
            SELECT * FROM governance_audit_log
            WHERE ($1::text IS NULL OR action = $1)
              AND ($2::uuid IS NULL OR actor_id = $2)
              AND ($3::text IS NULL OR target_type = $3)
              AND created_at >= $4
            ORDER BY created_at DESC
            LIMIT $5
            "#,
        )
        .bind(&filters.action)
        .bind(filters.actor_id)
        .bind(&filters.target_type)
        .bind(filters.since)
        .bind(filters.limit.unwrap_or(50) as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| GovernanceAuditEntry {
                id: row.id,
                action: row.action,
                request_id: row.request_id,
                target_type: row.target_type,
                target_id: row.target_id,
                actor_type: row.actor_type.parse().unwrap_or(PrincipalType::System),
                actor_id: row.actor_id,
                actor_email: row.actor_email,
                details: row.details,
                old_values: row.old_values,
                new_values: row.new_values,
                created_at: row.created_at,
            })
            .collect())
    }

    pub async fn assign_role(&self, role: &CreateGovernanceRole) -> Result<Uuid, sqlx::Error> {
        let row: (Uuid,) = sqlx::query_as(
            r#"
            INSERT INTO governance_roles (
                principal_type, principal_id, role,
                company_id, org_id, team_id, project_id,
                granted_by, expires_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING id
            "#,
        )
        .bind(role.principal_type.to_string())
        .bind(role.principal_id)
        .bind(&role.role)
        .bind(role.company_id)
        .bind(role.org_id)
        .bind(role.team_id)
        .bind(role.project_id)
        .bind(role.granted_by)
        .bind(role.expires_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    pub async fn revoke_role(
        &self,
        principal_id: Uuid,
        role: &str,
        revoked_by: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            UPDATE governance_roles
            SET revoked_at = NOW(), revoked_by = $3
            WHERE principal_id = $1 AND role = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(principal_id)
        .bind(role)
        .bind(revoked_by)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_roles(
        &self,
        company_id: Option<Uuid>,
        org_id: Option<Uuid>,
        team_id: Option<Uuid>,
    ) -> Result<Vec<GovernanceRole>, sqlx::Error> {
        #[derive(FromRow)]
        struct RoleRow {
            id: Uuid,
            principal_type: String,
            principal_id: Uuid,
            role: String,
            company_id: Option<Uuid>,
            org_id: Option<Uuid>,
            team_id: Option<Uuid>,
            project_id: Option<Uuid>,
            granted_by: Uuid,
            granted_at: DateTime<Utc>,
            expires_at: Option<DateTime<Utc>>,
            revoked_at: Option<DateTime<Utc>>,
            revoked_by: Option<Uuid>,
        }

        let rows: Vec<RoleRow> = sqlx::query_as(
            r#"
            SELECT * FROM governance_roles
            WHERE revoked_at IS NULL
              AND ($1::uuid IS NULL OR company_id = $1)
              AND ($2::uuid IS NULL OR org_id = $2)
              AND ($3::uuid IS NULL OR team_id = $3)
            ORDER BY granted_at DESC
            "#,
        )
        .bind(company_id)
        .bind(org_id)
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| GovernanceRole {
                id: row.id,
                principal_type: row.principal_type.parse().unwrap_or(PrincipalType::User),
                principal_id: row.principal_id,
                role: row.role,
                company_id: row.company_id,
                org_id: row.org_id,
                team_id: row.team_id,
                project_id: row.project_id,
                granted_by: row.granted_by,
                granted_at: row.granted_at,
                expires_at: row.expires_at,
                revoked_at: row.revoked_at,
                revoked_by: row.revoked_by,
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct CreateApprovalRequest {
    pub request_type: RequestType,
    pub target_type: String,
    pub target_id: Option<String>,
    pub company_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub payload: serde_json::Value,
    pub risk_level: RiskLevel,
    pub requestor_type: PrincipalType,
    pub requestor_id: Uuid,
    pub requestor_email: Option<String>,
    pub required_approvals: i32,
    pub timeout_hours: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct CreateDecision {
    pub request_id: Uuid,
    pub approver_type: PrincipalType,
    pub approver_id: Uuid,
    pub approver_email: Option<String>,
    pub decision: Decision,
    pub comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateGovernanceRole {
    pub principal_type: PrincipalType,
    pub principal_id: Uuid,
    pub role: String,
    pub company_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub granted_by: Uuid,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct RequestFilters {
    pub request_type: Option<RequestType>,
    pub company_id: Option<Uuid>,
    pub org_id: Option<Uuid>,
    pub team_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub requestor_id: Option<Uuid>,
    pub limit: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct AuditFilters {
    pub action: Option<String>,
    pub actor_id: Option<Uuid>,
    pub target_type: Option<String>,
    pub since: DateTime<Utc>,
    pub limit: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_mode_display() {
        assert_eq!(ApprovalMode::Single.to_string(), "single");
        assert_eq!(ApprovalMode::Quorum.to_string(), "quorum");
        assert_eq!(ApprovalMode::Unanimous.to_string(), "unanimous");
    }

    #[test]
    fn test_approval_mode_parse() {
        assert_eq!(
            "single".parse::<ApprovalMode>().unwrap(),
            ApprovalMode::Single
        );
        assert_eq!(
            "QUORUM".parse::<ApprovalMode>().unwrap(),
            ApprovalMode::Quorum
        );
        assert!("invalid".parse::<ApprovalMode>().is_err());
    }

    #[test]
    fn test_request_type_roundtrip() {
        for rt in [
            RequestType::Policy,
            RequestType::Knowledge,
            RequestType::Memory,
            RequestType::Role,
            RequestType::Config,
        ] {
            let s = rt.to_string();
            assert_eq!(s.parse::<RequestType>().unwrap(), rt);
        }
    }

    #[test]
    fn test_risk_level_default() {
        assert_eq!(RiskLevel::default(), RiskLevel::Medium);
    }

    #[test]
    fn test_governance_config_default() {
        let config = GovernanceConfig::default();
        assert_eq!(config.approval_mode, ApprovalMode::Quorum);
        assert_eq!(config.min_approvers, 2);
        assert_eq!(config.timeout_hours, 72);
        assert!(!config.auto_approve_low_risk);
        assert!(config.escalation_enabled);
    }

    #[test]
    fn test_principal_type_display() {
        assert_eq!(PrincipalType::User.to_string(), "user");
        assert_eq!(PrincipalType::Agent.to_string(), "agent");
        assert_eq!(PrincipalType::System.to_string(), "system");
    }

    #[test]
    fn test_request_status_parse() {
        assert_eq!(
            "pending".parse::<RequestStatus>().unwrap(),
            RequestStatus::Pending
        );
        assert_eq!(
            "approved".parse::<RequestStatus>().unwrap(),
            RequestStatus::Approved
        );
        assert_eq!(
            "rejected".parse::<RequestStatus>().unwrap(),
            RequestStatus::Rejected
        );
        assert_eq!(
            "expired".parse::<RequestStatus>().unwrap(),
            RequestStatus::Expired
        );
        assert_eq!(
            "cancelled".parse::<RequestStatus>().unwrap(),
            RequestStatus::Cancelled
        );
    }

    #[test]
    fn test_decision_display() {
        assert_eq!(Decision::Approve.to_string(), "approve");
        assert_eq!(Decision::Reject.to_string(), "reject");
        assert_eq!(Decision::Abstain.to_string(), "abstain");
    }
}
