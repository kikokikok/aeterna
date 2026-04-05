use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

use adapters::auth::matrix::role_permission_matrix;
use tools::server::tool_to_cedar_action;

const MEMORY_ACTIONS: &[&str] = &[
    "ViewMemory",
    "CreateMemory",
    "UpdateMemory",
    "DeleteMemory",
    "PromoteMemory",
    "SearchMemory",
    "ListMemory",
    "OptimizeMemory",
    "ReasonMemory",
    "CloseMemory",
    "FeedbackMemory",
];

const KNOWLEDGE_ACTIONS: &[&str] = &[
    "ViewKnowledge",
    "ProposeKnowledge",
    "EditKnowledge",
    "ApproveKnowledge",
    "DeprecateKnowledge",
    "ListKnowledge",
    "SearchKnowledge",
    "BatchKnowledge",
];

const POLICY_ACTIONS: &[&str] = &[
    "ViewPolicy",
    "CreatePolicy",
    "EditPolicy",
    "ApprovePolicy",
    "SimulatePolicy",
];

const GOVERNANCE_ACTIONS: &[&str] = &[
    "ViewGovernanceRequest",
    "SubmitGovernanceRequest",
    "ApproveGovernanceRequest",
    "RejectGovernanceRequest",
];

const ORGANIZATION_ACTIONS: &[&str] = &[
    "ViewOrganization",
    "CreateOrganization",
    "CreateTeam",
    "CreateProject",
    "ManageMembers",
    "AssignRoles",
];

const AGENT_ACTIONS: &[&str] = &["RegisterAgent", "RevokeAgent", "DelegateToAgent"];

const ADMIN_ACTIONS: &[&str] = &[
    "ViewAuditLog",
    "ExportData",
    "ImportData",
    "ConfigureGovernance",
];

const TENANT_MANAGEMENT_ACTIONS: &[&str] = &[
    "ListTenants",
    "CreateTenant",
    "ViewTenant",
    "UpdateTenant",
    "DeactivateTenant",
];

const TENANT_CONFIG_ACTIONS: &[&str] = &[
    "ViewTenantConfig",
    "UpdateTenantConfig",
    "ManageTenantSecrets",
];

const REPOSITORY_BINDING_ACTIONS: &[&str] = &["ViewRepositoryBinding", "UpdateRepositoryBinding"];

const GIT_PROVIDER_ACTIONS: &[&str] =
    &["ManageGitProviderConnections", "ViewGitProviderConnections"];

const SESSION_ACTIONS: &[&str] = &["CreateSession", "ViewSession", "EndSession"];

const SYNC_ACTIONS: &[&str] = &["TriggerSync", "ViewSyncStatus", "ResolveConflict"];

const GRAPH_ACTIONS: &[&str] = &["QueryGraph", "ModifyGraph"];

const CCA_MCP_ACTIONS: &[&str] = &["InvokeCCA", "InvokeMcpTool"];

const USER_MANAGEMENT_ACTIONS: &[&str] =
    &["ViewUser", "RegisterUser", "UpdateUser", "DeactivateUser"];

const ADMIN_SYNC_ACTIONS: &[&str] = &["AdminSyncGitHub"];

const DOMAIN_GROUPS: &[(&str, &[&str])] = &[
    ("Memory", MEMORY_ACTIONS),
    ("Knowledge", KNOWLEDGE_ACTIONS),
    ("Policy", POLICY_ACTIONS),
    ("Governance", GOVERNANCE_ACTIONS),
    ("Organization", ORGANIZATION_ACTIONS),
    ("Agent", AGENT_ACTIONS),
    ("Admin", ADMIN_ACTIONS),
    ("Tenant Management", TENANT_MANAGEMENT_ACTIONS),
    ("Tenant Config", TENANT_CONFIG_ACTIONS),
    ("Repository Binding", REPOSITORY_BINDING_ACTIONS),
    ("Git Provider", GIT_PROVIDER_ACTIONS),
    ("Session", SESSION_ACTIONS),
    ("Sync", SYNC_ACTIONS),
    ("Graph", GRAPH_ACTIONS),
    ("CCA & MCP", CCA_MCP_ACTIONS),
    ("User Management", USER_MANAGEMENT_ACTIONS),
    ("Admin Sync", ADMIN_SYNC_ACTIONS),
];

const MCP_TOOLS: &[&str] = &[
    "memory_add",
    "memory_search",
    "memory_delete",
    "memory_reason",
    "memory_close",
    "memory_feedback",
    "memory_optimize",
    "aeterna_memory_promote",
    "aeterna_memory_auto_promote",
    "graph_query",
    "graph_neighbors",
    "graph_path",
    "graph_link",
    "graph_unlink",
    "graph_traverse",
    "graph_find_path",
    "graph_violations",
    "graph_implementations",
    "graph_context",
    "graph_related",
    "knowledge_get",
    "knowledge_list",
    "knowledge_query",
    "aeterna_knowledge_propose",
    "aeterna_knowledge_submit",
    "aeterna_knowledge_pending",
    "sync_now",
    "sync_status",
    "knowledge_resolve_conflict",
    "context_assemble",
    "note_capture",
    "hindsight_query",
    "meta_loop_status",
    "governance_unit_create",
    "governance_policy_add",
    "governance_role_assign",
    "governance_role_remove",
    "governance_hierarchy_navigate",
    "governance_configure",
    "governance_config_get",
    "governance_request_create",
    "governance_approve",
    "governance_reject",
    "governance_request_list",
    "governance_request_get",
    "governance_audit_list",
    "governance_principal_role_assign",
    "governance_role_revoke",
    "governance_role_list",
    "aeterna_policy_propose",
    "aeterna_policy_list_pending",
    "codesearch_search",
    "codesearch_trace_callers",
    "codesearch_trace_callees",
    "codesearch_graph",
    "codesearch_index_status",
    "codesearch_repo_request",
];

const DISPLAY_ROLES: &[(&str, &str)] = &[
    ("platformAdmin", "PlatformAdmin"),
    ("tenantAdmin", "TenantAdmin"),
    ("admin", "Admin"),
    ("architect", "Architect"),
    ("techLead", "TechLead"),
    ("developer", "Developer"),
    ("viewer", "Viewer"),
];

fn all_actions() -> Vec<&'static str> {
    DOMAIN_GROUPS
        .iter()
        .flat_map(|(_, actions)| actions.iter().copied())
        .collect()
}

fn workspace_doc_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("security")
        .join("rbac-matrix.md")
}

fn build_rbac_matrix_markdown() -> String {
    let matrix = role_permission_matrix();
    let mut role_sets: HashMap<&str, HashSet<String>> = HashMap::new();
    for (role, actions) in &matrix {
        role_sets.insert(role.as_str(), actions.iter().cloned().collect());
    }

    let mut output = String::new();

    output.push_str("# RBAC Permission Matrix\n\n");
    output.push_str("> **Auto-generated** — do not edit manually.\n");
    output.push_str("> Re-generate with: `cargo test -p aeterna-adapters --test rbac_matrix_doc_test -- --ignored update_rbac_doc`\n\n");

    output.push_str("## Role Hierarchy\n\n");
    output.push_str("| Precedence | Role | Description |\n");
    output.push_str("|------------|------|-------------|\n");
    output.push_str(
        "| 7 | PlatformAdmin | Cross-tenant administration, Git provider connections |\n",
    );
    output.push_str("| 6 | TenantAdmin | Tenant-scoped administration |\n");
    output.push_str("| 5 | Admin | Full tenant access |\n");
    output.push_str("| 4 | Architect | Knowledge management, policy design |\n");
    output.push_str("| 3 | TechLead | Team management, promotions |\n");
    output.push_str("| 2 | Developer | Standard development access |\n");
    output.push_str("| 1 | Viewer | Read-only access |\n");
    output.push_str("| 0 | Agent | Delegated agent permissions |\n\n");

    output.push_str("## Permission Matrix\n\n");

    for (domain, actions) in DOMAIN_GROUPS {
        output.push_str(&format!("### {}\n\n", domain));
        output.push_str("| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |\n");
        output.push_str("|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|\n");

        for action in *actions {
            output.push_str(&format!("| {}", action));
            for (role_key, _) in DISPLAY_ROLES {
                let allowed = role_sets
                    .get(role_key)
                    .is_some_and(|set| set.contains(*action));
                let mark = if allowed { "✅" } else { "❌" };
                output.push_str(&format!(" | {}", mark));
            }
            output.push_str(" |\n");
        }
        output.push('\n');
    }

    output.push_str("## MCP Tool → Cedar Action Mapping\n\n");
    output.push_str("| MCP Tool | Cedar Action |\n");
    output.push_str("|----------|-------------|\n");
    for tool in MCP_TOOLS {
        output.push_str(&format!("| {} | {} |\n", tool, tool_to_cedar_action(tool)));
    }
    output.push('\n');

    let total_actions = all_actions().len();

    let platform_admin_count = role_sets.get("platformAdmin").map_or(0, HashSet::len);
    let tenant_admin_count = role_sets.get("tenantAdmin").map_or(0, HashSet::len);
    let admin_count = role_sets.get("admin").map_or(0, HashSet::len);
    let architect_count = role_sets.get("architect").map_or(0, HashSet::len);
    let tech_lead_count = role_sets.get("techLead").map_or(0, HashSet::len);
    let developer_count = role_sets.get("developer").map_or(0, HashSet::len);
    let viewer_count = role_sets.get("viewer").map_or(0, HashSet::len);

    output.push_str("## Statistics\n\n");
    output.push_str(&format!("- **Total actions**: {}\n", total_actions));
    output.push_str("- **Total roles**: 7 (+ Agent)\n");
    output.push_str(&format!(
        "- **PlatformAdmin**: {} actions (all)\n",
        platform_admin_count
    ));
    output.push_str(&format!(
        "- **TenantAdmin**: {} actions\n",
        tenant_admin_count
    ));
    output.push_str(&format!("- **Admin**: {} actions\n", admin_count));
    output.push_str(&format!("- **Architect**: {} actions\n", architect_count));
    output.push_str(&format!("- **TechLead**: {} actions\n", tech_lead_count));
    output.push_str(&format!("- **Developer**: {} actions\n", developer_count));
    output.push_str(&format!("- **Viewer**: {} actions\n", viewer_count));

    output
}

#[test]
fn rbac_matrix_doc_matches_generated_output() {
    let expected = build_rbac_matrix_markdown();
    let doc_path = workspace_doc_path();
    let committed = fs::read_to_string(&doc_path)
        .unwrap_or_else(|err| panic!("Failed to read {}: {err}", doc_path.display()));

    if committed != expected {
        eprintln!(
            "RBAC matrix markdown drift detected at: {}",
            doc_path.display()
        );
        eprintln!(
            "\n--- BEGIN EXPECTED docs/security/rbac-matrix.md ---\n{expected}\n--- END EXPECTED docs/security/rbac-matrix.md ---\n"
        );
        panic!(
            "RBAC matrix doc is out of date. Run `cargo test -p aeterna-adapters --test rbac_matrix_doc_test -- --ignored update_rbac_doc` to update it."
        );
    }
}

#[test]
#[ignore]
fn update_rbac_doc() {
    let generated = build_rbac_matrix_markdown();
    let doc_path = workspace_doc_path();
    fs::write(&doc_path, generated)
        .unwrap_or_else(|err| panic!("Failed to write {}: {err}", doc_path.display()));
}
