## ADDED Requirements

### Requirement: User List Pagination
The `GET /api/v1/user` endpoint MUST accept `limit` and `offset` query parameters and apply them at the SQL level.

#### Scenario: Paginated user list
- **WHEN** a client calls `GET /api/v1/user?limit=20&offset=0`
- **THEN** the SQL query MUST include `LIMIT 20 OFFSET 0`
- **AND** the response MUST use the standard `PaginatedResponse` envelope
- **AND** the response MUST NOT return every user in the tenant as a plain unbounded array

### Requirement: User Role Response Field Alignment
The `GET /api/v1/user/{user_id}/roles` endpoint MUST return role entries with consistent camelCase field names matching the standard serde convention.

#### Scenario: Role response fields
- **WHEN** a client calls `GET /api/v1/user/{user_id}/roles`
- **THEN** each role entry MUST contain `role` (string), `scope` (string), and `unitId` (string)
- **AND** the admin UI MUST use `scope` and `unitId` to display role scope, not `resource_type` and `resource_id`

### Requirement: Grant Role Request Body Contract
The `POST /api/v1/user/{user_id}/roles` endpoint MUST accept `{ "role": "...", "scope": "..." }` where scope is a slash-separated string (e.g., `"org/uuid"`).

#### Scenario: Grant role with scope
- **WHEN** a client sends `POST /api/v1/user/{user_id}/roles` with `{ "role": "Developer", "scope": "org/550e8400" }`
- **THEN** the system MUST grant the Developer role scoped to the org unit `550e8400`

#### Scenario: Frontend composes scope string
- **WHEN** the admin UI grants a role with resource_type="org" and resource_id="550e8400"
- **THEN** it MUST compose `scope = "org/550e8400"` and send `{ "role": "Developer", "scope": "org/550e8400" }`
- **AND** MUST NOT send `{ "role": "Developer", "resource_type": "org", "resource_id": "550e8400" }`

### Requirement: Role Grants Pagination
The `GET /api/v1/roles/grants` endpoint MUST accept pagination parameters and fix the N+1 unit type lookup.

#### Scenario: Paginated role grants
- **WHEN** a client calls `GET /api/v1/roles/grants?limit=50&offset=0`
- **THEN** the response MUST return at most 50 role grants in a `PaginatedResponse` envelope
- **AND** unit type resolution MUST use a JOIN, not N+1 individual queries
