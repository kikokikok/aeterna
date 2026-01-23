// Aeterna RBAC Policies
// Role-based permit policies for organizational access control.
// 
// Role Hierarchy (higher precedence = more permissions):
//   Admin (4)     - Full system access
//   Architect (3) - Design policies, manage knowledge
//   TechLead (2)  - Manage team resources
//   Developer (1) - Standard development access
//   Viewer (0)    - Read-only access

// =============================================================================
// ADMIN ROLE - Full Access
// =============================================================================

// Admin: Full access to all memory operations
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory"
    ],
    resource
)
when {
    principal.role == "admin" &&
    principal.status == "active"
};

// Admin: Full access to all knowledge operations
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge"
    ],
    resource
)
when {
    principal.role == "admin" &&
    principal.status == "active"
};

// Admin: Full access to all policy operations
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
    principal.role == "admin" &&
    principal.status == "active"
};

// Admin: Full access to governance
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
    principal.role == "admin" &&
    principal.status == "active"
};

// Admin: Full access to organization management
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateOrganization",
        Aeterna::Action::"CreateTeam",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers",
        Aeterna::Action::"AssignRoles"
    ],
    resource
)
when {
    principal.role == "admin" &&
    principal.status == "active"
};

// Admin: Full access to agent management
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
    principal.role == "admin" &&
    principal.status == "active"
};

// Admin: Full access to admin operations
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
    principal.role == "admin" &&
    principal.status == "active"
};

// =============================================================================
// ARCHITECT ROLE - Design & Knowledge Management
// =============================================================================

// Architect: Full memory access
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory"
    ],
    resource
)
when {
    principal.role == "architect" &&
    principal.status == "active"
};

// Architect: Full knowledge access
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge"
    ],
    resource
)
when {
    principal.role == "architect" &&
    principal.status == "active"
};

// Architect: Full policy access
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
    principal.role == "architect" &&
    principal.status == "active"
};

// Architect: View and submit governance requests
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
    principal.role == "architect" &&
    principal.status == "active"
};

// Architect: View organization structure and manage teams
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateTeam",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers"
    ],
    resource
)
when {
    principal.role == "architect" &&
    principal.status == "active"
};

// Architect: Manage agents
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
    principal.role == "architect" &&
    principal.status == "active"
};

// Architect: View audit logs
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewAuditLog",
    resource
)
when {
    principal.role == "architect" &&
    principal.status == "active"
};

// =============================================================================
// TECH LEAD ROLE - Team Resource Management
// =============================================================================

// TechLead: Full memory access within hierarchy
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory",
        Aeterna::Action::"UpdateMemory",
        Aeterna::Action::"DeleteMemory",
        Aeterna::Action::"PromoteMemory"
    ],
    resource
)
when {
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// TechLead: Full knowledge access (can't approve at company/org level)
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge",
        Aeterna::Action::"EditKnowledge",
        Aeterna::Action::"ApproveKnowledge",
        Aeterna::Action::"DeprecateKnowledge"
    ],
    resource
)
when {
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// TechLead: Policy view, create, simulate (not edit/approve at high levels)
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
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// TechLead: Governance participation
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
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// TechLead: Team and project management
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewOrganization",
        Aeterna::Action::"CreateProject",
        Aeterna::Action::"ManageMembers"
    ],
    resource
)
when {
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// TechLead: Agent management
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
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// TechLead: View audit logs
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewAuditLog",
    resource
)
when {
    principal.role == "tech_lead" &&
    principal.status == "active"
};

// =============================================================================
// DEVELOPER ROLE - Standard Access
// =============================================================================

// Developer: Read and create memories
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewMemory",
        Aeterna::Action::"CreateMemory"
    ],
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// Developer: Update own memories (requires additional context check in app)
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"UpdateMemory",
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// Developer: View knowledge, propose new knowledge
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewKnowledge",
        Aeterna::Action::"ProposeKnowledge"
    ],
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// Developer: View and simulate policies
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// Developer: View and submit governance requests
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewGovernanceRequest",
        Aeterna::Action::"SubmitGovernanceRequest"
    ],
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// Developer: View organization structure
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewOrganization",
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// Developer: Register and delegate to agents
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"RegisterAgent",
        Aeterna::Action::"DelegateToAgent"
    ],
    resource
)
when {
    principal.role == "developer" &&
    principal.status == "active"
};

// =============================================================================
// VIEWER ROLE - Read-Only Access
// =============================================================================

// Viewer: Read-only memory access
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewMemory",
    resource
)
when {
    principal.role == "viewer" &&
    principal.status == "active"
};

// Viewer: Read-only knowledge access
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewKnowledge",
    resource
)
when {
    principal.role == "viewer" &&
    principal.status == "active"
};

// Viewer: Read-only policy access
permit (
    principal is Aeterna::User,
    action in [
        Aeterna::Action::"ViewPolicy",
        Aeterna::Action::"SimulatePolicy"
    ],
    resource
)
when {
    principal.role == "viewer" &&
    principal.status == "active"
};

// Viewer: Read-only governance access
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewGovernanceRequest",
    resource
)
when {
    principal.role == "viewer" &&
    principal.status == "active"
};

// Viewer: Read-only organization access
permit (
    principal is Aeterna::User,
    action == Aeterna::Action::"ViewOrganization",
    resource
)
when {
    principal.role == "viewer" &&
    principal.status == "active"
};
