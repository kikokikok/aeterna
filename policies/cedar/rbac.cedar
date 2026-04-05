// Aeterna RBAC Policies
// Role-based permit policies for organizational access control.
// 
// Role Hierarchy (higher precedence = more permissions):
//   TenantAdmin (6)    - Explicit tenant-scoped administration
//   PlatformAdmin (5)  - Cross-tenant admin control-plane access
//   Admin (4)          - Full tenant-scoped system access (legacy alias for TenantAdmin)
//   Architect (3)      - Design policies, manage knowledge
//   TechLead (2)       - Manage team resources
//   Developer (1)      - Standard development access
//   Viewer (0)         - Read-only access
//   Agent (0)          - Delegated permissions from user context

// =============================================================================
// PLATFORM ADMIN ROLE - Cross-Tenant Control Plane Access
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory",
        Aeterna::Action::"OptimizeMemory",
        Aeterna::Action::"ReasonMemory",
        Aeterna::Action::"CloseMemory",
        Aeterna::Action::"FeedbackMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge",
        Aeterna::Action::"BatchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"CreatePolicy",
        Aeterna::Action::"EditPolicy",
        Aeterna::Action::"ApprovePolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest",
        Aeterna::Action::"ApproveGovernanceRequest",
        Aeterna::Action::"RejectGovernanceRequest"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateOrganization",
        Aeterna::Action::"CreateTeam",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers",
        Aeterna::Action::"AssignRoles",
        Aeterna::Action::"ManageTeamAssignments"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"RevokeAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewAuditLog",
        Aeterna::Action::"ExportData",
        Aeterna::Action::"ImportData",
        Aeterna::Action::"ConfigureGovernance"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ListTenants",
        Aeterna::Action::"CreateTenant",
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"UpdateTenant",
        Aeterna::Action::"DeactivateTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"UpdateTenantConfig",
        Aeterna::Action::"ManageTenantSecrets",
        Aeterna::Action::"ManageGitProviderConnections",
        Aeterna::Action::"ViewGitProviderConnections"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewRepositoryBinding",
        Aeterna::Action::"UpdateRepositoryBinding"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"CreateSession",
        Aeterna::Action::"ViewSession",
        Aeterna::Action::"EndSession"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"TriggerSync",
        Aeterna::Action::"ViewSyncStatus",
        Aeterna::Action::"ResolveConflict"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"QueryGraph",
        Aeterna::Action::"ModifyGraph"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"InvokeCCA",
        Aeterna::Action::"InvokeMcpTool"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewUser",
        Aeterna::Action::"RegisterUser",
        Aeterna::Action::"UpdateUser",
        Aeterna::Action::"DeactivateUser"
    ],
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"AdminSyncGitHub",
    resource
)
when {
    principal in Aeterna::Role::"PlatformAdmin" &&
    principal.status == "active"
};

// =============================================================================
// TENANT ADMIN ROLE - Full Tenant-Scoped Administration
// Identical to Admin permissions plus explicit tenant config/secrets management.
// Excludes cross-tenant actions: ListTenants, CreateTenant, ManageGitProviderConnections, AdminSyncGitHub
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory",
        Aeterna::Action::"OptimizeMemory",
        Aeterna::Action::"ReasonMemory",
        Aeterna::Action::"CloseMemory",
        Aeterna::Action::"FeedbackMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge",
        Aeterna::Action::"BatchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"CreatePolicy",
        Aeterna::Action::"EditPolicy",
        Aeterna::Action::"ApprovePolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest",
        Aeterna::Action::"ApproveGovernanceRequest",
        Aeterna::Action::"RejectGovernanceRequest"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateOrganization",
        Aeterna::Action::"CreateTeam",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers",
        Aeterna::Action::"AssignRoles",
        Aeterna::Action::"ManageTeamAssignments"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"RevokeAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewAuditLog",
        Aeterna::Action::"ExportData",
        Aeterna::Action::"ImportData",
        Aeterna::Action::"ConfigureGovernance"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"UpdateTenant",
        Aeterna::Action::"DeactivateTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"UpdateTenantConfig",
        Aeterna::Action::"ManageTenantSecrets",
        Aeterna::Action::"ViewGitProviderConnections"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewRepositoryBinding",
        Aeterna::Action::"UpdateRepositoryBinding"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"CreateSession",
        Aeterna::Action::"ViewSession",
        Aeterna::Action::"EndSession"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"TriggerSync",
        Aeterna::Action::"ViewSyncStatus",
        Aeterna::Action::"ResolveConflict"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"QueryGraph",
        Aeterna::Action::"ModifyGraph"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"InvokeCCA",
        Aeterna::Action::"InvokeMcpTool"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewUser",
        Aeterna::Action::"RegisterUser",
        Aeterna::Action::"UpdateUser",
        Aeterna::Action::"DeactivateUser"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TenantAdmin" &&
    principal.status == "active"
};

// =============================================================================
// ADMIN ROLE - Full Tenant Access (legacy alias for TenantAdmin)
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory",
        Aeterna::Action::"OptimizeMemory",
        Aeterna::Action::"ReasonMemory",
        Aeterna::Action::"CloseMemory",
        Aeterna::Action::"FeedbackMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge",
        Aeterna::Action::"BatchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"CreatePolicy",
        Aeterna::Action::"EditPolicy",
        Aeterna::Action::"ApprovePolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest",
        Aeterna::Action::"ApproveGovernanceRequest",
        Aeterna::Action::"RejectGovernanceRequest"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateOrganization",
        Aeterna::Action::"CreateTeam",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers",
        Aeterna::Action::"AssignRoles",
        Aeterna::Action::"ManageTeamAssignments"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"RevokeAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewAuditLog",
        Aeterna::Action::"ExportData",
        Aeterna::Action::"ImportData",
        Aeterna::Action::"ConfigureGovernance"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"UpdateTenant",
        Aeterna::Action::"DeactivateTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"UpdateTenantConfig",
        Aeterna::Action::"ManageTenantSecrets",
        Aeterna::Action::"ViewGitProviderConnections"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewRepositoryBinding",
        Aeterna::Action::"UpdateRepositoryBinding"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"CreateSession",
        Aeterna::Action::"ViewSession",
        Aeterna::Action::"EndSession"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"TriggerSync",
        Aeterna::Action::"ViewSyncStatus",
        Aeterna::Action::"ResolveConflict"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"QueryGraph",
        Aeterna::Action::"ModifyGraph"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"InvokeCCA",
        Aeterna::Action::"InvokeMcpTool"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewUser",
        Aeterna::Action::"RegisterUser",
        Aeterna::Action::"UpdateUser",
        Aeterna::Action::"DeactivateUser"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Admin" &&
    principal.status == "active"
};

// =============================================================================
// ARCHITECT ROLE - Design & Knowledge Management
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory",
        Aeterna::Action::"OptimizeMemory",
        Aeterna::Action::"ReasonMemory",
        Aeterna::Action::"CloseMemory",
        Aeterna::Action::"FeedbackMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge",
        Aeterna::Action::"BatchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"CreatePolicy",
        Aeterna::Action::"EditPolicy",
        Aeterna::Action::"ApprovePolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest",
        Aeterna::Action::"ApproveGovernanceRequest",
        Aeterna::Action::"RejectGovernanceRequest"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateTeam",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers",
        Aeterna::Action::"ManageTeamAssignments"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"RevokeAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewAuditLog",
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"ViewGitProviderConnections",
        Aeterna::Action::"ViewRepositoryBinding"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"CreateSession",
        Aeterna::Action::"ViewSession",
        Aeterna::Action::"EndSession"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"TriggerSync",
        Aeterna::Action::"ViewSyncStatus",
        Aeterna::Action::"ResolveConflict"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"QueryGraph",
        Aeterna::Action::"ModifyGraph"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"InvokeCCA",
        Aeterna::Action::"InvokeMcpTool"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewUser",
    resource
)
when {
    principal in Aeterna::Role::"Architect" &&
    principal.status == "active"
};

// =============================================================================
// TECH LEAD ROLE - Team Resource Management
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory",
        Aeterna::Action::"OptimizeMemory",
        Aeterna::Action::"ReasonMemory",
        Aeterna::Action::"CloseMemory",
        Aeterna::Action::"FeedbackMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge",
        Aeterna::Action::"BatchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"CreatePolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest",
        Aeterna::Action::"ApproveGovernanceRequest",
        Aeterna::Action::"RejectGovernanceRequest"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers",
        Aeterna::Action::"ManageTeamAssignments"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"RevokeAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewAuditLog",
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"ViewRepositoryBinding",
        Aeterna::Action::"ViewGitProviderConnections"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"CreateSession",
        Aeterna::Action::"ViewSession",
        Aeterna::Action::"EndSession"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"TriggerSync",
        Aeterna::Action::"ViewSyncStatus"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"QueryGraph",
        Aeterna::Action::"ModifyGraph"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"InvokeCCA",
        Aeterna::Action::"InvokeMcpTool"
    ],
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewUser",
    resource
)
when {
    principal in Aeterna::Role::"TechLead" &&
    principal.status == "active"
};

// =============================================================================
// DEVELOPER ROLE - Standard Access
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory",
        Aeterna::Action::"FeedbackMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewOrganization",
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"CreateSession",
        Aeterna::Action::"ViewSession",
        Aeterna::Action::"EndSession"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewSyncStatus",
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"QueryGraph",
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"InvokeCCA",
        Aeterna::Action::"InvokeMcpTool"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

    permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"ViewRepositoryBinding",
        Aeterna::Action::"ViewGitProviderConnections"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewUser",
    resource
)
when {
    principal in Aeterna::Role::"Developer" &&
    principal.status == "active"
};

// =============================================================================
// VIEWER ROLE - Read-Only Access
// =============================================================================

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"SearchMemory",
        Aeterna::Action::"ListMemory"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ListKnowledge",
        Aeterna::Action::"SearchKnowledge"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewGovernanceRequest",
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewOrganization",
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewSyncStatus",
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewSession",
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewTenant",
        Aeterna::Action::"ViewTenantConfig",
        Aeterna::Action::"ViewRepositoryBinding",
        Aeterna::Action::"ViewGitProviderConnections"
    ],
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"QueryGraph",
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};

permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewUser",
    resource
)
when {
    principal in Aeterna::Role::"Viewer" &&
    principal.status == "active"
};
