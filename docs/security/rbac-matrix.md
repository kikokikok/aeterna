# RBAC Permission Matrix

> **Auto-generated** — do not edit manually.
> Re-generate with: `cargo test -p aeterna-adapters --test rbac_matrix_doc_test -- --ignored update_rbac_doc`

## Role Hierarchy

| Precedence | Role | Description |
|------------|------|-------------|
| 7 | PlatformAdmin | Cross-tenant administration, Git provider connections |
| 6 | TenantAdmin | Tenant-scoped administration |
| 5 | Admin | Full tenant access |
| 4 | Architect | Knowledge management, policy design |
| 3 | TechLead | Team management, promotions |
| 2 | Developer | Standard development access |
| 1 | Viewer | Read-only access |
| 0 | Agent | Delegated agent permissions |

## Permission Matrix

### Memory

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| CreateMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| UpdateMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| DeleteMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| PromoteMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| SearchMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| ListMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| OptimizeMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| ReasonMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| CloseMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| FeedbackMemory | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |

### Knowledge

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| ProposeKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| EditKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| ApproveKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| DeprecateKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| ListKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| SearchKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| BatchKnowledge | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |

### Policy

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewPolicy | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| CreatePolicy | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| EditPolicy | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| ApprovePolicy | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| SimulatePolicy | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

### Governance

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewGovernanceRequest | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| SubmitGovernanceRequest | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| ApproveGovernanceRequest | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| RejectGovernanceRequest | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |

### Organization

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewOrganization | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| CreateOrganization | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| CreateTeam | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| CreateProject | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| ManageMembers | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| AssignRoles | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

### Agent

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| RegisterAgent | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| RevokeAgent | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| DelegateToAgent | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |

### Admin

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewAuditLog | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| ExportData | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| ImportData | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| ConfigureGovernance | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

### Tenant Management

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ListTenants | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| CreateTenant | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| ViewTenant | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| UpdateTenant | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| DeactivateTenant | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

### Tenant Config

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewTenantConfig | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| UpdateTenantConfig | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| ManageTenantSecrets | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

### Repository Binding

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewRepositoryBinding | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| UpdateRepositoryBinding | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

### Git Provider

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ManageGitProviderConnections | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| ViewGitProviderConnections | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

### Session

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| CreateSession | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| ViewSession | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| EndSession | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |

### Sync

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| TriggerSync | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| ViewSyncStatus | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| ResolveConflict | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |

### Graph

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| QueryGraph | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| ModifyGraph | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |

### CCA & MCP

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| InvokeCCA | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| InvokeMcpTool | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |

### User Management

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| ViewUser | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| RegisterUser | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| UpdateUser | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |
| DeactivateUser | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ | ❌ |

### Admin Sync

| Action | PlatformAdmin | TenantAdmin | Admin | Architect | TechLead | Developer | Viewer |
|--------|:---:|:---:|:---:|:---:|:---:|:---:|:---:|
| AdminSyncGitHub | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |

## MCP Tool → Cedar Action Mapping

| MCP Tool | Cedar Action |
|----------|-------------|
| memory_add | CreateMemory |
| memory_search | SearchMemory |
| memory_delete | DeleteMemory |
| memory_reason | ReasonMemory |
| memory_close | CloseMemory |
| memory_feedback | FeedbackMemory |
| memory_optimize | OptimizeMemory |
| aeterna_memory_promote | PromoteMemory |
| aeterna_memory_auto_promote | OptimizeMemory |
| graph_query | QueryGraph |
| graph_neighbors | QueryGraph |
| graph_path | QueryGraph |
| graph_link | ModifyGraph |
| graph_unlink | ModifyGraph |
| graph_traverse | QueryGraph |
| graph_find_path | QueryGraph |
| graph_violations | QueryGraph |
| graph_implementations | QueryGraph |
| graph_context | QueryGraph |
| graph_related | QueryGraph |
| knowledge_get | SearchKnowledge |
| knowledge_list | ListKnowledge |
| knowledge_query | SearchKnowledge |
| aeterna_knowledge_propose | BatchKnowledge |
| aeterna_knowledge_submit | BatchKnowledge |
| aeterna_knowledge_pending | ListKnowledge |
| sync_now | TriggerSync |
| sync_status | ViewSyncStatus |
| knowledge_resolve_conflict | ResolveConflict |
| context_assemble | InvokeCCA |
| note_capture | InvokeCCA |
| hindsight_query | InvokeCCA |
| meta_loop_status | InvokeCCA |
| governance_unit_create | CreateOrganization |
| governance_policy_add | EditPolicy |
| governance_role_assign | AssignRoles |
| governance_role_remove | AssignRoles |
| governance_hierarchy_navigate | ViewGovernanceRequest |
| governance_configure | EditPolicy |
| governance_config_get | ViewGovernanceRequest |
| governance_request_create | SubmitGovernanceRequest |
| governance_approve | ApproveGovernanceRequest |
| governance_reject | RejectGovernanceRequest |
| governance_request_list | ViewGovernanceRequest |
| governance_request_get | ViewGovernanceRequest |
| governance_audit_list | ViewAuditLog |
| governance_principal_role_assign | AssignRoles |
| governance_role_revoke | AssignRoles |
| governance_role_list | ViewGovernanceRequest |
| aeterna_policy_propose | EditPolicy |
| aeterna_policy_list_pending | ViewGovernance |
| codesearch_search | InvokeMcpTool |
| codesearch_trace_callers | InvokeMcpTool |
| codesearch_trace_callees | InvokeMcpTool |
| codesearch_graph | InvokeMcpTool |
| codesearch_index_status | InvokeMcpTool |
| codesearch_repo_request | InvokeMcpTool |

## Statistics

- **Total actions**: 68
- **Total roles**: 7 (+ Agent)
- **PlatformAdmin**: 68 actions (all)
- **TenantAdmin**: 64 actions
- **Admin**: 64 actions
- **Architect**: 51 actions
- **TechLead**: 47 actions
- **Developer**: 29 actions
- **Viewer**: 18 actions
