# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### BREAKING CHANGES

- **Governance wire format: snake_case → camelCase.** All governance types
  (`GovernanceEvent`, `RemediationRequest`, `ApprovalRecord`, audit entries)
  now serialize to camelCase on the wire. Clients that parse these JSON
  responses must update field names (e.g. `resource_type` → `resourceType`,
  `created_at` → `createdAt`). This affects:
  - `GET /api/v1/govern/audit`
  - `GET /api/v1/govern/pending`
  - `GET /api/v1/admin/lifecycle/remediations`
  - All governance webhook payloads

- **Paginated list endpoints return `{ items, total, limit, offset }` envelopes**
  instead of bare arrays. Affected endpoints:
  - `GET /api/v1/admin/tenants`
  - `GET /api/v1/user`
  - `GET /api/v1/govern/policies`
  - `GET /api/v1/govern/audit`
  - `GET /api/v1/govern/pending`
  - `GET /api/v1/govern/roles`
  - `GET /api/v1/knowledge/promotions`
  - `GET /api/v1/admin/exports`
  - `GET /api/v1/admin/imports`
  - `GET /api/v1/admin/git-provider-connections`
  - `POST /api/v1/knowledge/query` (response shape unchanged but `offset`
    added to request body)
  - `POST /api/v1/memory/search` (`offset` added to request body)
  - `POST /api/v1/memory/list` (`offset` added to request body)

### Added

- Shared `<PaginationBar>` React component for admin-ui list pages.
- `offset` field on `POST /memory/search`, `POST /memory/list`, and
  `POST /knowledge/query` request bodies.
- `limit` and `offset` query parameters on `GET /admin/exports`,
  `GET /admin/imports`, `GET /govern/pending`,
  `GET /knowledge/promotions`, and `GET /admin/git-provider-connections`.
- Max cap of 500 on `GET /sync/pull` limit parameter.
- `LIMIT` clauses on `list_project_team_assignments`,
  `list_suppressions`, and other unbounded storage queries.

### Fixed

- `GET /govern/pending` no longer hard-codes `limit: 100`; respects
  caller-supplied `limit`/`offset` query params.
- `POST /memory/list` uses `list_from_layer` with proper offset/limit
  instead of fetching all then truncating.
- Admin-UI `TenantDetailPage` unwraps `{ tenant: ... }` envelope from
  `GET /admin/tenants/:id`.
- Admin-UI `KnowledgeSearchPage` uses `KnowledgeItem` type and correct
  camelCase field references.
- Admin-UI `MemorySearchPage` sends correct feedback request body
  (`rewardType`, `score`, `layer`) to `POST /memory/:id/feedback`.
- Admin-UI `LifecyclePage` handles both bare array and `{ items }`
  envelope for remediations.

### Migration Notes

Clients consuming the admin API must update:
1. Parse camelCase fields in governance responses.
2. Read list results from `response.items` instead of the top-level array.
3. Use `response.total` for pagination UI; pass `limit`/`offset` query
   params (or body fields for POST endpoints) to page through results.
