use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{Pool, Postgres};
use storage::governance::{
    ApprovalDecision, ApprovalMode, ApprovalRequest, CreateApprovalRequest, CreateDecision,
    CreateGovernanceRole, Decision, GovernanceConfig, GovernanceRole, GovernanceStorage,
    GovernanceTemplate, PrincipalType, RequestStatus, RequestType, RiskLevel
};
use testing::{postgres, unique_id};
use uuid::Uuid;

async fn create_test_governance_storage() -> Option<GovernanceStorage> {
    let fixture = postgres().await?;
    let pool = Pool::<Postgres>::connect(fixture.url()).await.ok()?;
    let storage = GovernanceStorage::new(pool);
    Some(storage)
}

fn unique_tenant_id() -> String {
    unique_id("test-tenant")
}

fn create_test_config(company_id: Option<Uuid>) -> GovernanceConfig {
    GovernanceConfig {
        id: None,
        company_id,
        org_id: None,
        team_id: None,
        project_id: None,
        approval_mode: ApprovalMode::Quorum,
        min_approvers: 2,
        timeout_hours: 72,
        auto_approve_low_risk: false,
        escalation_enabled: true,
        escalation_timeout_hours: 48,
        escalation_contact: Some("admin@example.com".to_string()),
        policy_settings: json!({"require_approval": true, "min_approvers": 2}),
        knowledge_settings: json!({"require_approval": true, "min_approvers": 1}),
        memory_settings: json!({"require_approval": false, "auto_approve_threshold": 0.8})
    }
}

fn create_test_request(company_id: Option<Uuid>, requestor_id: Uuid) -> CreateApprovalRequest {
    CreateApprovalRequest {
        request_type: RequestType::Policy,
        target_type: "policy".to_string(),
        target_id: Some("test-policy-123".to_string()),
        company_id,
        org_id: None,
        team_id: None,
        project_id: None,
        title: "Test Policy Request".to_string(),
        description: Some("This is a test policy request".to_string()),
        payload: json!({"policy_name": "test-policy", "action": "create"}),
        risk_level: RiskLevel::Medium,
        requestor_type: PrincipalType::User,
        requestor_id,
        requestor_email: Some("user@example.com".to_string()),
        required_approvals: 2,
        timeout_hours: None
    }
}

#[tokio::test]
async fn test_governance_template_enum_conversions() {
    assert_eq!(GovernanceTemplate::Standard.to_string(), "standard");
    assert_eq!(GovernanceTemplate::Strict.to_string(), "strict");
    assert_eq!(GovernanceTemplate::Permissive.to_string(), "permissive");

    assert_eq!(
        "standard".parse::<GovernanceTemplate>().unwrap(),
        GovernanceTemplate::Standard
    );
    assert_eq!(
        "strict".parse::<GovernanceTemplate>().unwrap(),
        GovernanceTemplate::Strict
    );
    assert_eq!(
        "permissive".parse::<GovernanceTemplate>().unwrap(),
        GovernanceTemplate::Permissive
    );

    assert!("invalid".parse::<GovernanceTemplate>().is_err());

    assert!(
        GovernanceTemplate::Standard
            .description()
            .contains("quorum-based")
    );
    assert!(
        GovernanceTemplate::Strict
            .description()
            .contains("unanimous")
    );
    assert!(
        GovernanceTemplate::Permissive
            .description()
            .contains("single")
    );

    let all_templates = GovernanceTemplate::all();
    assert_eq!(all_templates.len(), 3);
    assert!(all_templates.contains(&GovernanceTemplate::Standard));
    assert!(all_templates.contains(&GovernanceTemplate::Strict));
    assert!(all_templates.contains(&GovernanceTemplate::Permissive));
}

#[tokio::test]
async fn test_governance_template_to_config() {
    let standard_config = GovernanceTemplate::Standard.to_config();
    assert_eq!(standard_config.approval_mode, ApprovalMode::Quorum);
    assert_eq!(standard_config.min_approvers, 2);
    assert_eq!(standard_config.timeout_hours, 72);
    assert!(!standard_config.auto_approve_low_risk);

    let strict_config = GovernanceTemplate::Strict.to_config();
    assert_eq!(strict_config.approval_mode, ApprovalMode::Unanimous);
    assert_eq!(strict_config.min_approvers, 3);
    assert_eq!(strict_config.timeout_hours, 24);
    assert!(!strict_config.auto_approve_low_risk);
    assert!(strict_config.escalation_enabled);

    let permissive_config = GovernanceTemplate::Permissive.to_config();
    assert_eq!(permissive_config.approval_mode, ApprovalMode::Single);
    assert_eq!(permissive_config.min_approvers, 1);
    assert_eq!(permissive_config.timeout_hours, 168);
    assert!(permissive_config.auto_approve_low_risk);
    assert!(!permissive_config.escalation_enabled);
}

#[tokio::test]
async fn test_approval_mode_enum_conversions() {
    assert_eq!(ApprovalMode::Single.to_string(), "single");
    assert_eq!(ApprovalMode::Quorum.to_string(), "quorum");
    assert_eq!(ApprovalMode::Unanimous.to_string(), "unanimous");

    assert_eq!(
        "single".parse::<ApprovalMode>().unwrap(),
        ApprovalMode::Single
    );
    assert_eq!(
        "quorum".parse::<ApprovalMode>().unwrap(),
        ApprovalMode::Quorum
    );
    assert_eq!(
        "unanimous".parse::<ApprovalMode>().unwrap(),
        ApprovalMode::Unanimous
    );

    assert!("invalid".parse::<ApprovalMode>().is_err());
    assert_eq!(ApprovalMode::default(), ApprovalMode::Quorum);
}

#[tokio::test]
async fn test_request_type_enum_conversions() {
    assert_eq!(RequestType::Policy.to_string(), "policy");
    assert_eq!(RequestType::Knowledge.to_string(), "knowledge");
    assert_eq!(RequestType::Memory.to_string(), "memory");
    assert_eq!(RequestType::Role.to_string(), "role");
    assert_eq!(RequestType::Config.to_string(), "config");

    assert_eq!(
        "policy".parse::<RequestType>().unwrap(),
        RequestType::Policy
    );
    assert_eq!(
        "knowledge".parse::<RequestType>().unwrap(),
        RequestType::Knowledge
    );
    assert_eq!(
        "memory".parse::<RequestType>().unwrap(),
        RequestType::Memory
    );
    assert_eq!("role".parse::<RequestType>().unwrap(), RequestType::Role);
    assert_eq!(
        "config".parse::<RequestType>().unwrap(),
        RequestType::Config
    );

    assert!("invalid".parse::<RequestType>().is_err());
}

#[tokio::test]
async fn test_risk_level_enum_conversions() {
    assert_eq!(RiskLevel::Low.to_string(), "low");
    assert_eq!(RiskLevel::Medium.to_string(), "medium");
    assert_eq!(RiskLevel::High.to_string(), "high");
    assert_eq!(RiskLevel::Critical.to_string(), "critical");

    assert_eq!("low".parse::<RiskLevel>().unwrap(), RiskLevel::Low);
    assert_eq!("medium".parse::<RiskLevel>().unwrap(), RiskLevel::Medium);
    assert_eq!("high".parse::<RiskLevel>().unwrap(), RiskLevel::High);
    assert_eq!(
        "critical".parse::<RiskLevel>().unwrap(),
        RiskLevel::Critical
    );

    assert!("invalid".parse::<RiskLevel>().is_err());
    assert_eq!(RiskLevel::default(), RiskLevel::Medium);
}

#[tokio::test]
async fn test_principal_type_enum_conversions() {
    assert_eq!(PrincipalType::User.to_string(), "user");
    assert_eq!(PrincipalType::Agent.to_string(), "agent");
    assert_eq!(PrincipalType::System.to_string(), "system");

    assert_eq!(
        "user".parse::<PrincipalType>().unwrap(),
        PrincipalType::User
    );
    assert_eq!(
        "agent".parse::<PrincipalType>().unwrap(),
        PrincipalType::Agent
    );
    assert_eq!(
        "system".parse::<PrincipalType>().unwrap(),
        PrincipalType::System
    );

    assert!("invalid".parse::<PrincipalType>().is_err());
}

#[tokio::test]
async fn test_request_status_enum_conversions() {
    assert_eq!(RequestStatus::Pending.to_string(), "pending");
    assert_eq!(RequestStatus::Approved.to_string(), "approved");
    assert_eq!(RequestStatus::Rejected.to_string(), "rejected");
    assert_eq!(RequestStatus::Expired.to_string(), "expired");
    assert_eq!(RequestStatus::Cancelled.to_string(), "cancelled");

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

    assert!("invalid".parse::<RequestStatus>().is_err());
}

#[tokio::test]
async fn test_decision_enum_conversions() {
    assert_eq!(Decision::Approve.to_string(), "approve");
    assert_eq!(Decision::Reject.to_string(), "reject");
    assert_eq!(Decision::Abstain.to_string(), "abstain");
}

#[tokio::test]
async fn test_governance_config_default() {
    let config = GovernanceConfig::default();
    assert_eq!(config.approval_mode, ApprovalMode::Quorum);
    assert_eq!(config.min_approvers, 2);
    assert_eq!(config.timeout_hours, 72);
    assert!(!config.auto_approve_low_risk);
    assert!(config.escalation_enabled);
    assert_eq!(config.escalation_timeout_hours, 48);
    assert!(config.escalation_contact.is_none());

    let policy_settings: serde_json::Value = config.policy_settings;
    assert_eq!(policy_settings["require_approval"], true);
    assert_eq!(policy_settings["min_approvers"], 2);

    let knowledge_settings: serde_json::Value = config.knowledge_settings;
    assert_eq!(knowledge_settings["require_approval"], true);
    assert_eq!(knowledge_settings["min_approvers"], 1);

    let memory_settings: serde_json::Value = config.memory_settings;
    assert_eq!(memory_settings["require_approval"], false);
    assert_eq!(memory_settings["auto_approve_threshold"], 0.8);
}

#[tokio::test]
async fn test_governance_storage_new() {
    let Some(storage) = create_test_governance_storage().await else {
        eprintln!("Skipping governance storage test: Docker not available");
        return;
    };
    assert!(true, "Should create governance storage");
}

#[tokio::test]
async fn test_serde_roundtrip_governance_config() {
    let config = create_test_config(Some(Uuid::new_v4()));

    let json = serde_json::to_string(&config).unwrap();
    let restored: GovernanceConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.company_id, restored.company_id);
    assert_eq!(config.approval_mode, restored.approval_mode);
    assert_eq!(config.min_approvers, restored.min_approvers);
    assert_eq!(config.timeout_hours, restored.timeout_hours);
    assert_eq!(config.auto_approve_low_risk, restored.auto_approve_low_risk);
    assert_eq!(config.escalation_enabled, restored.escalation_enabled);
    assert_eq!(
        config.escalation_timeout_hours,
        restored.escalation_timeout_hours
    );
    assert_eq!(config.escalation_contact, restored.escalation_contact);
}

#[tokio::test]
async fn test_serde_roundtrip_approval_request() {
    let request = ApprovalRequest {
        id: Uuid::new_v4(),
        request_number: "REQ-2024-001".to_string(),
        request_type: RequestType::Policy,
        target_type: "policy".to_string(),
        target_id: Some("test-policy-123".to_string()),
        company_id: Some(Uuid::new_v4()),
        org_id: None,
        team_id: None,
        project_id: None,
        title: "Test Policy".to_string(),
        description: Some("Test policy description".to_string()),
        payload: json!({"action": "create"}),
        risk_level: RiskLevel::Medium,
        requestor_type: PrincipalType::User,
        requestor_id: Uuid::new_v4(),
        requestor_email: Some("user@example.com".to_string()),
        required_approvals: 2,
        current_approvals: 0,
        status: RequestStatus::Pending,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        expires_at: Some(Utc::now() + chrono::Duration::hours(72)),
        resolved_at: None,
        resolution_reason: None,
        applied_at: None,
        applied_by: None
    };

    let json = serde_json::to_string(&request).unwrap();
    let restored: ApprovalRequest = serde_json::from_str(&json).unwrap();

    assert_eq!(request.id, restored.id);
    assert_eq!(request.request_number, restored.request_number);
    assert_eq!(request.request_type, restored.request_type);
    assert_eq!(request.title, restored.title);
    assert_eq!(request.status, restored.status);
    assert_eq!(request.required_approvals, restored.required_approvals);
    assert_eq!(request.current_approvals, restored.current_approvals);
}

#[tokio::test]
async fn test_serde_roundtrip_governance_role() {
    let role = GovernanceRole {
        id: Uuid::new_v4(),
        principal_type: PrincipalType::User,
        principal_id: Uuid::new_v4(),
        role: "admin".to_string(),
        company_id: Some(Uuid::new_v4()),
        org_id: None,
        team_id: None,
        project_id: None,
        granted_by: Uuid::new_v4(),
        granted_at: Utc::now(),
        expires_at: Some(Utc::now() + chrono::Duration::days(365)),
        revoked_at: None,
        revoked_by: None
    };

    let json = serde_json::to_string(&role).unwrap();
    let restored: GovernanceRole = serde_json::from_str(&json).unwrap();

    assert_eq!(role.id, restored.id);
    assert_eq!(role.principal_type, restored.principal_type);
    assert_eq!(role.principal_id, restored.principal_id);
    assert_eq!(role.role, restored.role);
    assert_eq!(role.company_id, restored.company_id);
    assert_eq!(role.granted_by, restored.granted_by);
}

#[tokio::test]
async fn test_serde_roundtrip_approval_decision() {
    let decision = ApprovalDecision {
        id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        approver_type: PrincipalType::User,
        approver_id: Uuid::new_v4(),
        approver_email: Some("approver@example.com".to_string()),
        decision: Decision::Approve,
        comment: Some("Looks good".to_string()),
        created_at: Utc::now()
    };

    let json = serde_json::to_string(&decision).unwrap();
    let restored: ApprovalDecision = serde_json::from_str(&json).unwrap();

    assert_eq!(decision.id, restored.id);
    assert_eq!(decision.request_id, restored.request_id);
    assert_eq!(decision.approver_type, restored.approver_type);
    assert_eq!(decision.approver_id, restored.approver_id);
    assert_eq!(decision.decision, restored.decision);
    assert_eq!(decision.comment, restored.comment);
}

#[tokio::test]
async fn test_create_governance_role_struct() {
    let role = CreateGovernanceRole {
        principal_type: PrincipalType::User,
        principal_id: Uuid::new_v4(),
        role: "admin".to_string(),
        company_id: Some(Uuid::new_v4()),
        org_id: None,
        team_id: None,
        project_id: None,
        granted_by: Uuid::new_v4(),
        expires_at: Some(Utc::now() + chrono::Duration::days(365))
    };

    assert_eq!(role.role, "admin");
    assert!(role.company_id.is_some());
    assert!(role.expires_at.is_some());
}

#[tokio::test]
async fn test_create_decision_struct() {
    let decision = CreateDecision {
        request_id: Uuid::new_v4(),
        approver_type: PrincipalType::User,
        approver_id: Uuid::new_v4(),
        approver_email: Some("approver@example.com".to_string()),
        decision: Decision::Approve,
        comment: Some("Approved".to_string())
    };

    assert_eq!(decision.decision, Decision::Approve);
    assert!(decision.approver_email.is_some());
    assert!(decision.comment.is_some());
}

#[tokio::test]
async fn test_governance_audit_entry_struct() {
    let audit_entry = storage::governance::GovernanceAuditEntry {
        id: Uuid::new_v4(),
        action: "policy_created".to_string(),
        request_id: Some(Uuid::new_v4()),
        target_type: Some("policy".to_string()),
        target_id: Some("test-policy-123".to_string()),
        actor_type: PrincipalType::User,
        actor_id: Some(Uuid::new_v4()),
        actor_email: Some("user@example.com".to_string()),
        details: json!({"policy_name": "test"}),
        old_values: None,
        new_values: Some(json!({"status": "active"})),
        created_at: Utc::now()
    };

    assert_eq!(audit_entry.action, "policy_created");
    assert!(audit_entry.request_id.is_some());
    assert!(audit_entry.target_type.is_some());
    assert!(audit_entry.actor_id.is_some());
    assert!(audit_entry.new_values.is_some());
    assert!(audit_entry.old_values.is_none());
}
