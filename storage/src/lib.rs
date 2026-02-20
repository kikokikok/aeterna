pub mod approval_workflow;
pub mod budget_storage;
pub mod code_graph;
pub mod encryption;
pub mod events;
pub mod gdpr;
pub mod governance;
pub mod graph;
pub mod graph_duckdb;
pub mod graphrag;
pub mod kms_integration;
pub mod meta_governance;
pub mod policy_evaluator;
pub mod positional_index;
pub mod postgres;
pub mod query_builder;
pub mod redis;
pub mod repo_manager;
pub mod rlm_weights;
pub mod rls_migration;
pub mod secret_provider;
pub mod shard_manager;
pub mod shard_router;
pub mod tenant_router;

// Re-export Redis lock types for job coordination
pub use redis::{JobSkipReason, LockResult};

// Re-export budget storage types
pub use budget_storage::{BudgetStorage, BudgetStorageError, StoredBudget, StoredUsage};

// Re-export governance types
pub use governance::{
    ApprovalDecision, ApprovalMode, ApprovalRequest, AuditFilters, CreateApprovalRequest,
    CreateDecision, CreateGovernanceRole, Decision, GovernanceAuditEntry, GovernanceConfig,
    GovernanceRole, GovernanceStorage, PrincipalType, RequestFilters, RequestStatus, RequestType,
    RiskLevel,
};

// Re-export approval workflow state machine
pub use approval_workflow::{
    ApprovalDecisionRecord, ApprovalEvent, ApprovalModeKind, ApprovalWorkflow,
    ApprovalWorkflowContext, RiskLevelKind, WorkflowError, WorkflowState, create_workflow,
};

// Re-export meta-governance types
pub use meta_governance::{
    ActionPermission, AgentCapability, AgentDelegationConfig, AgentRateLimits, AuthorizationResult,
    ConfirmationReason, ConfirmationStatus, EscalationConfig, EscalationFallback, EscalationTarget,
    EscalationTier, GovernanceActionType, GovernanceLayer, HumanConfirmationRequest,
    MetaGovernancePolicy, MetaGovernanceStorage, NotificationChannel, RoleLevel,
    create_default_policies,
};

// Re-export RLM weight storage types
pub use rlm_weights::{
    PostgresRlmWeightStorage, RlmWeightStorage, RlmWeightStorageError, StoredPolicyState,
};

// Re-export encryption types
pub use encryption::{EncryptedData, EncryptionConfig, EncryptionError, EncryptionManager};

// Re-export KMS types
pub use kms_integration::{
    AwsKmsProvider, KmsClient, KmsConfig, KmsError, KmsKeyMetadata, KmsProvider, LocalKmsProvider,
};

// Re-export GDPR types
pub use gdpr::{
    AnonymizationStrategy, GdprAuditLog, GdprConsent, GdprError, GdprOperations,
    PostgresGdprStorage, UserDataExport,
};

// Re-export tenant sharding types
pub use repo_manager::{
    CleanupLog, CreateRepository, CreateRequest, IndexMetadata, RepoRequest, RepoRequestStatus,
    RepoStorage, Repository, RepositoryStatus, RepositoryType, SyncStrategy, UsageMetrics,
};
pub use shard_manager::{ShardError, ShardInfo, ShardManager, ShardStatistics, ShardStatus};
pub use tenant_router::{TenantRouter, TenantShard, TenantSize};
