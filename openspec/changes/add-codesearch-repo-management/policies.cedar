// Code Search Repository Management Policies

// 1. Repository Request Policies

// Devs can request repositories
permit (
    principal in Aeterna::Role::"developer",
    action == CodeSearch::Action::"RequestRepository",
    resource is CodeSearch::Repository
);

// Admins and Leads can approve repository requests
permit (
    principal in Aeterna::Role::"admin",
    action == CodeSearch::Action::"ApproveRepository",
    resource is CodeSearch::Request
);

permit (
    principal in Aeterna::Role::"lead",
    action == CodeSearch::Action::"ApproveRepository",
    resource is CodeSearch::Request
)
when { principal.tenant_id == resource.tenant_id };

// 2. Identity Management Policies

// Only Admins can manage Git Identities (credentials)
permit (
    principal in Aeterna::Role::"admin",
    action in [CodeSearch::Action::"AddIdentity", CodeSearch::Action::"ListIdentities"],
    resource is CodeSearch::Identity
);

// 3. Indexing Policies

// Leads can trigger indexing manually
permit (
    principal in Aeterna::Role::"lead",
    action == CodeSearch::Action::"IndexRepository",
    resource is CodeSearch::Repository
)
when { principal.tenant_id == resource.tenant_id };

// Developers can search if repo is READY
permit (
    principal in Aeterna::Role::"developer",
    action in [CodeSearch::Action::"Search", CodeSearch::Action::"Trace"],
    resource is CodeSearch::Repository
)
when { 
    principal.tenant_id == resource.tenant_id &&
    resource.status == "ready"
};
