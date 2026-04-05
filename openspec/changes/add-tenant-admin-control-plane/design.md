## Context

The current system has the pieces for multi-tenant governance and an authenticated CLI, but the administrative control plane is fragmented. Tenant-scoped hierarchy and role concepts exist in storage, Cedar policies, and sync code, yet operators still cannot manage tenants, configure per-tenant knowledge repositories, or inspect effective permissions through supported CLI/API flows. Existing `org`, `team`, `user roles`, and `govern roles` commands mostly emit previews or explicit unsupported errors instead of completing real admin work.

This change is cross-cutting because it touches the authority model, tenant onboarding, server endpoints, storage, Cedar policy alignment, CLI surfaces, docs, and end-to-end tests. It also needs to preserve the fail-closed multi-tenant posture already established by `fix-multi-tenant-fail-closed`.

## Goals / Non-Goals

**Goals:**
- Provide a real tenant administration control plane, not just a CLI shape.
- Define explicit platform-admin and tenant-admin authority boundaries without weakening tenant isolation.
- Let operators configure and validate a tenant's canonical knowledge repository binding from the CLI/API.
- Replace stubbed role-management flows with real scoped role mutation, listing, and effective-permission inspection.
- Make role-to-permission mappings inspectable from the control plane while keeping Cedar as the policy source of truth.
- Reconcile runtime tenant context, role catalog, and admin UX with the active authorization model.

**Non-Goals:**
- Replacing Cedar/OPAL with a different policy engine.
- Introducing naive tenant inference from raw email suffixes alone.
- Supporting per-org or per-team repository sharding in the first phase; this change standardizes one canonical binding per tenant.
- Removing IdP sync; manual admin and IdP sync must coexist.

## Decisions

### Introduce an explicit platform-admin role with bounded authority
Add a cross-tenant `PlatformAdmin` authority for tenant lifecycle, tenant bootstrap, tenant repository bindings, and admin inspection workflows. Platform-admin authority does not imply unrestricted read/write access to tenant memory or knowledge content; tenant-content operations still require an explicit tenant context and normal policy evaluation.

**Alternatives considered:**
- **Keep only tenant-scoped `Admin`**: rejected because tenant lifecycle and cross-tenant operator flows remain awkward and partially out of band.
- **Grant platform admins implicit access to all tenant content**: rejected because it weakens tenant isolation and auditability.

### Keep tenant onboarding explicit and verified
Tenant association must come from explicit tenant selection, IdP bootstrap, or admin-approved verified mappings. Email domain mapping can be used only as an admin-managed verified hint and must fail closed on ambiguity or absence.

**Alternatives considered:**
- **Infer tenant from raw email domain automatically**: rejected because shared domains, contractors, and aliases make it unsafe.
- **Require manual tenant selection for every onboarding path**: rejected because verified bootstrap flows from sync/approved mappings are operationally useful.

### Standardize one canonical knowledge repository binding per tenant
Each tenant gets one canonical repository binding describing how knowledge operations resolve storage for that tenant. The hierarchy (company/org/team/project) remains within that tenant binding rather than requiring multiple repositories in the first phase.

**Alternatives considered:**
- **Per-scope repository sharding immediately**: rejected as too complex for the initial admin-plane completion and unnecessary for the primary tenant-level use case.
- **Global shared repository across tenants**: rejected because it complicates tenant isolation and operational ownership.

### Store credential references in tenant bindings, not raw secrets
Tenant repository bindings may reference credentials, tokens, deploy keys, or secret handles, but the binding record itself must store references or handles rather than raw secret material. Validation can dereference or test those handles through the configured secret source without making the binding table the secret store.

**Alternatives considered:**
- **Persist raw secrets in the binding model**: rejected because it broadens secret exposure and complicates audit and compliance boundaries.
- **Force only anonymous/local repositories**: rejected because remote Git-backed tenant repositories are a core use case.

### Derive permission inspection from the active policy bundle
The role-permission matrix exposed via API/CLI will be derived from the same canonical role catalog and Cedar policy bundle used for authorization decisions. The control plane should inspect the active policy model, not maintain a second hard-coded permission table.

**Alternatives considered:**
- **Hard-code role permissions in CLI output**: rejected because it drifts from real authorization behavior.
- **Expose Cedar files only**: rejected because operators need a supported, queryable admin surface.

### Finish existing admin CLI surfaces before adding parallel ones
Add a dedicated `tenant` command group, but also wire the existing `org`, `team`, `user roles`, and `govern roles` paths to real server endpoints instead of leaving them as shells. The control plane should converge rather than split into duplicate command trees.

### Require explicit tenant context switching for platform-admin tenant-scoped operations
Platform administrators may inspect tenant lifecycle records across tenants, but any tenant-scoped hierarchy, membership, repository, or content-affecting operation must carry an explicit tenant target selected through the request or CLI context override. Platform-admin sessions must not silently inherit access to tenant-scoped content across commands.

### Preserve sync-owned and admin-owned state separately
IdP sync remains authoritative for sync-owned hierarchy fields and memberships it manages, while tenant-admin-owned configuration such as repository bindings, verified domain mappings, and local metadata must survive sync runs unless an explicit admin policy says otherwise.

### Mirror supported admin journeys in Newman as black-box API coverage
Every supported end-to-end admin/operator workflow added by this change must also be represented in the Postman/Newman collection under `e2e/` so the control plane can be validated as a black-box deployed service, not only through Rust integration tests.

**Alternatives considered:**
- **Rust-only E2E coverage**: rejected because it does not validate the shipped HTTP surface the same way operators and CI smoke tests will exercise it.
- **Manual Postman examples only**: rejected because examples without Newman assertions are not reliable regression coverage.

## Risks / Trade-offs

- **[Risk] Platform-admin flows accidentally bypass tenant isolation** → Mitigation: require explicit tenant context for tenant-content operations and add audit/negative tests for cross-tenant denial.
- **[Risk] Role catalog drift between Rust and Cedar continues** → Mitigation: define one canonical role catalog and add compatibility/inspection tests.
- **[Risk] Repository binding validation becomes environment-specific** → Mitigation: validate structure, credential references, and remote reachability separately with explicit error categories.
- **[Risk] CLI scope gets too broad** → Mitigation: focus first on tenant lifecycle, repo binding, role mutation, and permission inspection; defer broader per-scope repo sharding.
- **[Risk] Sync runs overwrite admin-managed tenant metadata** → Mitigation: define source ownership rules and add conflict/regression tests for sync-managed tenants.
- **[Risk] HTTP admin flows pass Rust tests but regress in deployed API behavior** → Mitigation: require Newman/Postman scenarios with assertions for each supported admin/operator journey.

## Migration Plan

1. Define the admin-plane contract and authority model in specs.
2. Add tenant records, platform-admin role support, and verified tenant resolution rules.
3. Add server endpoints for tenant lifecycle, hierarchy/member management, role admin, and permission inspection.
4. Add tenant knowledge repository bindings and route knowledge operations through them.
5. Add source-ownership rules so sync-managed tenants and manual admin configuration can coexist safely.
6. Wire the CLI command groups to the new endpoints and remove unsupported admin stubs.
7. Add docs plus Rust and Newman end-to-end coverage for platform-admin and tenant-admin workflows.

## Open Questions

- Should `Viewer` become part of the canonical Rust role enum in the first phase, or should the first phase only reconcile existing admin-facing roles plus `PlatformAdmin`?
- Should permission inspection expose only the static role matrix, or also an effective-permissions view for a specific principal and scope?
- Do we want tenant bootstrap to allow pre-provisioned repository bindings at tenant creation time, or only as a follow-up admin step?
