## ADDED Requirements

### Requirement: Single Knowledge Item Retrieval
The system MUST provide a `GET /api/v1/knowledge/{id}` endpoint that returns a complete knowledge item by its ID, including content, metadata, tags, relations, and layer information.

#### Scenario: Retrieve knowledge item by ID
- **WHEN** a client calls `GET /api/v1/knowledge/{id}` with a valid ID
- **THEN** the system MUST return the full `KnowledgeItem` object with all fields
- **AND** the response MUST use HTTP 200

#### Scenario: Knowledge item not found
- **WHEN** a client calls `GET /api/v1/knowledge/{id}` with a non-existent ID
- **THEN** the system MUST return HTTP 404 with `{ "error": "not_found" }`

### Requirement: Knowledge Query Pagination
The `POST /api/v1/knowledge/query` endpoint MUST support `offset` in addition to the existing `limit` parameter, and MUST return a true grand total count.

#### Scenario: Knowledge query with offset
- **WHEN** a client sends `POST /api/v1/knowledge/query` with `{ "query": "auth", "limit": 10, "offset": 20 }`
- **THEN** the response MUST skip the first 20 matches and return up to 10 items
- **AND** `total` MUST be the grand total of matching items, not `items.len()`

### Requirement: Knowledge Promotions Pagination
The `GET /api/v1/knowledge/promotions` endpoint MUST accept `limit` and `offset` query parameters.

#### Scenario: Paginated promotions list
- **WHEN** a client calls `GET /api/v1/knowledge/promotions?limit=20&offset=0`
- **THEN** the response MUST return at most 20 promotions in a `PaginatedResponse` envelope

### Requirement: Direct Knowledge ID Lookup
The internal `find_entry_by_id` helper MUST use a direct ID-based query instead of iterating through all layers with `list()`.

#### Scenario: Efficient ID lookup
- **WHEN** the system needs to find a knowledge entry by ID
- **THEN** it MUST issue a single SQL query with `WHERE id = $1`
- **AND** MUST NOT call `list()` on each of the 4 knowledge layers sequentially
