# Admin API — Cross-Tenant Listing

> Status: **stable** for `?tenant=*` / `?tenant=all` on the endpoints listed below. The `?tenant=<slug>` path is **planned** and currently returns `501 scope_not_implemented`.
>
> Shipped in [RFC #56](https://github.com/kikokikok/aeterna/pull/56) · Tracked in [#44.d](https://github.com/kikokikok/aeterna/issues/44).

This document is the canonical reference for the `?tenant=` query parameter across admin list endpoints. It is the source of truth that a future OpenAPI schema will be generated from.

## Endpoints covered

| Endpoint         | Since PR | §     | `?tenant=*` envelope | Per-row `tenantId`/`tenantSlug` |
|------------------|----------|-------|----------------------|---------------------------------|
| `GET /admin/tenants` | #63  | §2.1 | ✅                    | N/A (items _are_ tenants)       |
| `GET /user`      | #64      | §2.2 | ✅                    | ✅                               |
| `GET /project`   | #65      | §2.3 | ✅                    | ✅                               |
| `GET /org`       | #66      | §2.4 | ✅                    | ✅                               |
| `GET /govern/audit` | #68   | §2.5 | ✅                    | ❌ (see [Audit exception](#audit-exception)) |

## The `?tenant=` grammar

| Value                      | Meaning                                                                  | Gate                    | Response shape                 |
|----------------------------|--------------------------------------------------------------------------|-------------------------|--------------------------------|
| _absent_ or `?tenant=`     | Tenant-scoped to the caller's tenant (from `X-Tenant-ID` / default).    | Existing auth gate      | Legacy bare array (unchanged)  |
| `?tenant=*`                | Cross-tenant listing — items from every active tenant.                  | **PlatformAdmin**       | Envelope (see below)           |
| `?tenant=all`              | Deprecated alias for `*`. Emits a `compat` warning log. Will be removed in a future minor. | **PlatformAdmin** | Envelope                       |
| `?tenant=<slug-or-uuid>`   | **Not yet implemented.** Reserved for single-foreign-tenant listing.   | PlatformAdmin (planned) | `501 scope_not_implemented`    |

## The cross-tenant response envelope

When `?tenant=*` (or its alias) resolves successfully, the response body uses a stable envelope that replaces the legacy bare array:

```json
{
  "success": true,
  "scope":   "all",
  "items": [
    {
      "id":          "…",
      "name":        "…",
      "tenantId":    "tenant-uuid",
      "tenantSlug":  "acme",
      "tenantName":  "Acme Corp",
      "…":           "… (endpoint-specific fields)"
    }
  ]
}
```

### Contract guarantees

For every endpoint except `/govern/audit`, the following are locked by the [§4.1 contract test](../../cli/tests/server_runtime_test.rs) (search `assert_cross_tenant_envelope_contract`):

1. HTTP status is `200`.
2. `body.success == true`.
3. `body.scope == "all"`.
4. `body.items` is an array.
5. Every item has a **non-empty string** `tenantId`.
6. Every item has a **non-empty string** `tenantSlug`.
7. When data spans multiple tenants, `items` reflects that (the contract test asserts `≥ min_tenants` distinct tenant ids — catches accidental single-tenant collapse).

Ordering is stable across pages: `(tenant_id ASC, name ASC, id ASC)`.

### Audit exception

`GET /govern/audit?tenant=*` emits the envelope but items intentionally **omit** `tenantId` / `tenantSlug`. The `governance_audit_log` table has no row-level tenant column (only a nullable `acting_as_tenant_id` from migration 023 that isn't in the current `SELECT`). Surfacing a misleading tenant would be worse than omitting one. A follow-up will graduate the audit endpoint to the full contract once the storage layer exposes `acting_as_tenant_id` — see the deferred tenant-decoration PR referenced in [#44.d tasks §2.5](../../openspec/changes/add-cross-tenant-admin-listing/tasks.md).

## Error responses

| HTTP | `error` code             | When                                                                |
|------|--------------------------|---------------------------------------------------------------------|
| `403` | `forbidden_scope`       | `?tenant=*` or `?tenant=all` called by a non-PlatformAdmin.         |
| `501` | `scope_not_implemented` | `?tenant=<slug>` on any endpoint (message identifies which one).    |
| `400` | `scope_not_allowed_for_write` | Future guard for write operations under `?tenant=*` (§5.8).   |

All error bodies follow the standard error envelope:

```json
{
  "success": false,
  "error":   "forbidden_scope",
  "message": "Human-readable description",
  "required_role": "PlatformAdmin"
}
```

## Filter composition

Endpoint-specific query parameters (e.g. `?actor=`, `?since=`, `?action=` on `/govern/audit`; `?company=` on `/org`) **compose with `?tenant=*`**. The envelope is a response-shape-only transform applied after the storage query. There is no special casing: the same filter rules apply in tenant-scoped and cross-tenant modes.

## Backward compatibility

- Omitting `?tenant=` is always equivalent to the pre-RFC behavior. No existing client is impacted by the introduction of the parameter.
- `?tenant=` with an empty string value is treated as absent (defensive for clients sending unfilled form fields).
- The `?tenant=all` alias is intentionally accepted so clients that tried the feature before it was formally stabilized don't break; they now see a `compat` warning log directing them to `?tenant=*`.

## Related

- RFC: [PR #56](https://github.com/kikokikok/aeterna/pull/56)
- Resolver implementation: [`cli/src/server/context.rs`](../../cli/src/server/context.rs) (search `resolve_list_scope`)
- Contract test: [`cli/tests/server_runtime_test.rs`](../../cli/tests/server_runtime_test.rs) (search `assert_cross_tenant_envelope_contract`)
- RLS boundary guard: [`storage/tests/rls_boundary_test.rs`](../../storage/tests/rls_boundary_test.rs)
- CLI integration (planned): [#44.d §6](../../openspec/changes/add-cross-tenant-admin-listing/tasks.md#6-cli-updates-separate-pr-tracked-here-for-cross-reference)
