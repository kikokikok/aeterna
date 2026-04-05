// Aeterna Forbid Policies
// Explicit deny rules that take precedence over permit policies.
// These establish security boundaries that CANNOT be bypassed.
//
// Cedar evaluation order:
// 1. If ANY forbid policy matches -> DENY
// 2. If ANY permit policy matches -> ALLOW
// 3. Default -> DENY
//
// NOTE: Many agent restrictions are enforced at the SCHEMA level.
// Actions like ApproveGovernanceRequest, AssignRoles, etc. only apply
// to User principals in aeterna.cedarschema - agents can never even
// attempt these actions. The forbid policies below handle runtime
// conditions that can't be expressed in the schema.

// =============================================================================
// AGENT STATUS RESTRICTIONS - Runtime Conditions
// =============================================================================

// FORBID: Revoked agents cannot perform any action
@id("forbid-revoked-agent")
forbid (
    principal is Aeterna::Agent,
    action,
    resource
)
when {
    principal.status == "revoked"
};

// FORBID: Expired agents cannot perform any action
@id("forbid-expired-agent")
forbid (
    principal is Aeterna::Agent,
    action,
    resource
)
when {
    principal.status == "expired"
};

// FORBID: Agents that exceed delegation depth cannot act
@id("forbid-agent-exceeded-delegation")
forbid (
    principal is Aeterna::Agent,
    action,
    resource
)
when {
    principal.delegation_depth > principal.max_delegation_depth
};

// =============================================================================
// USER STATUS RESTRICTIONS - Runtime Conditions
// =============================================================================

// FORBID: Inactive users cannot perform any action
@id("forbid-inactive-user")
forbid (
    principal is Aeterna::User,
    action,
    resource
)
when {
    principal.status == "inactive"
};

// FORBID: Suspended users cannot perform any action
@id("forbid-suspended-user")
forbid (
    principal is Aeterna::User,
    action,
    resource
)
when {
    principal.status == "suspended"
};

// =============================================================================
// GOVERNANCE MODE RESTRICTIONS - Resource-Level Conditions
// =============================================================================

// FORBID: Direct edits to approved knowledge are blocked
// Must go through governance request process
@id("forbid-direct-edit-approved-knowledge")
forbid (
    principal,
    action == Aeterna::Action::"EditKnowledge",
    resource is Aeterna::Knowledge
)
when {
    resource.status == "approved"
};

// FORBID: Direct edits to active policies requiring approval are blocked
@id("forbid-direct-edit-protected-policy")
forbid (
    principal,
    action == Aeterna::Action::"EditPolicy",
    resource is Aeterna::Policy
)
when {
    resource.status == "active" &&
    resource.requires_approval == true
};

// =============================================================================
// CROSS-TENANT ACCESS RESTRICTIONS - PlatformAdmin Only
// =============================================================================
// These actions operate across tenant boundaries and are restricted to
// PlatformAdmin role only. The rbac.cedar permit rules only grant these to
// platformadmin, but forbid rules provide defense-in-depth: even if a
// misconfigured permit rule were added, the forbid would still block.

// FORBID: Non-PlatformAdmin users cannot list tenants (cross-tenant visibility)
@id("forbid-cross-tenant-list-tenants")
forbid (
    principal is Aeterna::User,
    action == Aeterna::Action::"ListTenants",
    resource
)
unless {
    principal in Aeterna::Role::"PlatformAdmin"
};

// FORBID: Non-PlatformAdmin users cannot create tenants
@id("forbid-cross-tenant-create-tenant")
forbid (
    principal is Aeterna::User,
    action == Aeterna::Action::"CreateTenant",
    resource
)
unless {
    principal in Aeterna::Role::"PlatformAdmin"
};

// FORBID: Non-PlatformAdmin users cannot manage shared git provider connections
@id("forbid-cross-tenant-manage-git-providers")
forbid (
    principal is Aeterna::User,
    action == Aeterna::Action::"ManageGitProviderConnections",
    resource
)
unless {
    principal in Aeterna::Role::"PlatformAdmin"
};

// FORBID: Non-PlatformAdmin users cannot trigger admin-level GitHub sync
@id("forbid-cross-tenant-admin-sync-github")
forbid (
    principal is Aeterna::User,
    action == Aeterna::Action::"AdminSyncGitHub",
    resource
)
unless {
    principal in Aeterna::Role::"PlatformAdmin"
};

// =============================================================================
// PRIVILEGE ESCALATION RESTRICTIONS
// =============================================================================
// Prevent users from assigning roles with higher precedence than their own.
// This is enforced at the policy level as defense-in-depth; the API layer
// should also validate role precedence before processing grant requests.

// FORBID: Non-admin users cannot assign roles (only Admin+ can)
// Developers, Viewers, TechLeads, and Architects lack AssignRoles in their
// permit rules, but this forbid provides an explicit safety net.
@id("forbid-privilege-escalation-assign-roles")
forbid (
    principal is Aeterna::User,
    action == Aeterna::Action::"AssignRoles",
    resource
)
when {
    !(principal in Aeterna::Role::"Admin") &&
    !(principal in Aeterna::Role::"TenantAdmin") &&
    !(principal in Aeterna::Role::"PlatformAdmin")
};

// =============================================================================
// SCHEMA-ENFORCED RESTRICTIONS (Documentation)
// =============================================================================
// The following restrictions are enforced at the SCHEMA level in aeterna.cedarschema.
// Agents are NOT included in the principal list for these actions, so they
// can never be attempted by agents - no forbid policy needed.
//
// Agent CANNOT (schema-enforced):
// - ApproveGovernanceRequest (only User principal)
// - RejectGovernanceRequest (only User principal)
// - ApproveKnowledge (only User principal)
// - DeprecateKnowledge (only User principal)
// - ApprovePolicy (only User principal)
// - EditPolicy (only User principal)
// - AssignRoles (only User principal)
// - ManageMembers (only User principal)
// - RevokeAgent (only User principal)
// - ConfigureGovernance (only User principal)
// - ViewAuditLog (only User principal)
// - ExportData (only User principal)
// - ImportData (only User principal)
// - CreateOrganization (only User principal)
// - CreateTeam (only User principal)
// - CreateProject (only User principal)
//
// Cross-tenant actions also schema-enforced for agents (User-only principal):
// - ListTenants (only User principal)
// - CreateTenant (only User principal)
// - ManageGitProviderConnections (only User principal)
// - AdminSyncGitHub (only User principal)
//
// This is the preferred approach - enforce at schema level when possible,
// use forbid policies only for runtime conditions (status, delegation depth, etc.)
