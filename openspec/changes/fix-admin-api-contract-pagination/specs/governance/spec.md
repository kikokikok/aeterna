## ADDED Requirements

### Requirement: Governance Audit Log Pagination
The `GET /api/v1/govern/audit` endpoint MUST accept `limit` and `offset` query parameters and return a `PaginatedResponse` envelope.

#### Scenario: Paginated audit log
- **WHEN** a client calls `GET /api/v1/govern/audit?limit=20&offset=0`
- **THEN** the response MUST be `{ "items": [...], "total": N, "limit": 20, "offset": 0 }`
- **AND** the response MUST NOT be a plain array

### Requirement: Governance Roles Pagination
The `GET /api/v1/govern/roles` endpoint MUST accept `limit` and `offset` query parameters.

#### Scenario: Paginated roles list
- **WHEN** a client calls `GET /api/v1/govern/roles?limit=50&offset=0`
- **THEN** the response MUST return at most 50 role assignments in a `PaginatedResponse` envelope

### Requirement: Governance Policies Response Format
The `GET /api/v1/govern/policies` endpoint MUST return a `PaginatedResponse` envelope with `items` key (not `policies` key).

#### Scenario: Policies in standard envelope
- **WHEN** a client calls `GET /api/v1/govern/policies?limit=20`
- **THEN** the response MUST be `{ "items": [...policies...], "total": N, "limit": 20, "offset": 0 }`
- **AND** MUST NOT use the `{ "policies": [...] }` wrapper

### Requirement: Governance Pending Requests Pagination
The `GET /api/v1/govern/pending` endpoint MUST accept client-provided `limit` and `offset` parameters instead of using a hardcoded limit of 100.

#### Scenario: Custom page size for pending requests
- **WHEN** a client calls `GET /api/v1/govern/pending?limit=25&offset=50`
- **THEN** the system MUST return at most 25 pending requests starting from offset 50
- **AND** the hardcoded internal cap of 100 MUST be removed in favor of the standard PaginationParams cap of 200
