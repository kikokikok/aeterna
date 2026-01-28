pub mod approval_workflow;
pub mod budget_storage;
pub mod events;
pub mod governance;
pub mod graph;
pub mod graph_duckdb;
pub mod meta_governance;
pub mod postgres;
pub mod query_builder;
pub mod redis;
pub mod rlm_weights;
pub mod rls_migration;

// Re-export Redis lock types for job coordination
pub use redis::{JobSkipReason, LockResult};

// Re-export budget storage types
pub use budget_storage::{BudgetStorage, BudgetStorageError, StoredBudget, StoredUsage};

// Re-export governance types
pub use governance::{
    ApprovalDecision, ApprovalMode, ApprovalRequest, AuditFilters, CreateApprovalRequest,
    CreateDecision, CreateGovernanceRole, Decision, GovernanceAuditEntry, GovernanceConfig,
    GovernanceRole, GovernanceStorage, PrincipalType, RequestFilters, RequestStatus, RequestType,
    RiskLevel
};

// Re-export approval workflow state machine
pub use approval_workflow::{
    ApprovalDecisionRecord, ApprovalEvent, ApprovalModeKind, ApprovalWorkflow,
    ApprovalWorkflowContext, RiskLevelKind, WorkflowError, WorkflowState, create_workflow
};

// Re-export meta-governance types
pub use meta_governance::{
    ActionPermission, AgentCapability, AgentDelegationConfig, AgentRateLimits, AuthorizationResult,
    ConfirmationReason, ConfirmationStatus, EscalationConfig, EscalationFallback, EscalationTarget,
    EscalationTier, GovernanceActionType, GovernanceLayer, HumanConfirmationRequest,
    MetaGovernancePolicy, MetaGovernanceStorage, NotificationChannel, RoleLevel,
    create_default_policies
};

// Re-export RLM weight storage types
pub use rlm_weights::{
    PostgresRlmWeightStorage, RlmWeightStorage, RlmWeightStorageError, StoredPolicyState
};
