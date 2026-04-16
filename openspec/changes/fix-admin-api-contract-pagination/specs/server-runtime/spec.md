## ADDED Requirements

### Requirement: Standardized JSON Serialization Convention
All Rust structs that serialize to JSON API responses MUST use `#[serde(rename_all = "camelCase")]`. This applies to all response types across all API modules.

#### Scenario: Governance audit entry uses camelCase
- **WHEN** `GET /api/v1/govern/audit` returns audit entries
- **THEN** fields MUST be serialized as `targetType`, `targetId`, `actorType`, `actorId`, `actorEmail`, `createdAt` (camelCase)
- **AND** NOT as `target_type`, `target_id`, `actor_type`, `actor_id`, `actor_email`, `created_at` (snake_case)

#### Scenario: Knowledge item uses camelCase
- **WHEN** `POST /api/v1/knowledge/query` returns knowledge items
- **THEN** fields MUST be serialized as `variantRole` (camelCase)
- **AND** NOT as `variant_role` (snake_case)

#### Scenario: Approval request uses camelCase
- **WHEN** `GET /api/v1/govern/pending` returns approval requests
- **THEN** fields MUST be serialized as `requestType`, `requestorId`, `createdAt`, `updatedAt`, `riskLevel`, `requestorEmail` (camelCase)

### Requirement: Standardized Response Envelope Convention
Single-entity endpoints MUST return the entity directly (flat JSON object). List endpoints MUST use the `PaginatedResponse<T>` envelope (`{ items, total, limit, offset }`). The legacy `{ success: bool, entity: T }` wrapper pattern MUST NOT be used for new endpoints.

#### Scenario: Show-tenant returns wrapped response (existing contract)
- **WHEN** `GET /api/v1/admin/tenants/{tenant}` is called
- **THEN** the response MUST be `{ "success": true, "tenant": { ... } }` (existing contract preserved)
- **AND** clients MUST unwrap the `tenant` field to access the TenantRecord

#### Scenario: List endpoint returns paginated envelope
- **WHEN** any list endpoint is called
- **THEN** the response MUST follow the `PaginatedResponse<T>` format: `{ "items": [...], "total": N, "limit": N, "offset": N }`

## MODIFIED Requirements

### Requirement: HTTP Router Composition
The server SHALL compose a unified Axum HTTP router by merging sub-routers from each service crate, with a global authentication tower layer applied to all protected route groups.

#### Scenario: Route Registration
- **WHEN** the server builds the HTTP router
- **THEN** the following route groups SHALL be registered:
  - `/health`, `/ready`, `/live` — health endpoints (auth-exempt)
  - `/api/v1/auth/plugin/*` — auth bootstrap endpoints (auth-exempt)
  - `/api/v1/knowledge/*` — canonical knowledge API endpoints (auth-protected), including `GET /knowledge/{id}` for single-item retrieval
  - `/openspec/v1/knowledge/*` — compatibility alias for knowledge API endpoints (auth-protected)
  - `/api/v1/governance/*` — Governance Dashboard API endpoints (auth-protected)
  - `/api/v1/admin/*` — Admin control plane endpoints (auth-protected, role-enforced)
  - `/api/v1/memory/*` — Memory API endpoints (auth-protected), including `POST /memory/{id}/feedback` alias
  - `/mcp/*` — MCP HTTP/SSE transport endpoints (auth-protected)
  - `/a2a/*` — A2A agent protocol endpoints (auth-protected)
  - `/ws/*` — WebSocket sync endpoints (auth-protected)
  - `/webhooks/*` — IDP webhook endpoints (HMAC-authenticated)
  - `/admin/*` — Static asset serving for admin UI (auth-exempt)

#### Scenario: Unknown Route
- **WHEN** a request targets a path not matching any registered route
- **THEN** the server SHALL return HTTP 404 with a JSON error body
