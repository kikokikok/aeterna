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
// This is the preferred approach - enforce at schema level when possible,
// use forbid policies only for runtime conditions (status, delegation depth, etc.)
