## ADDED Requirements

### Requirement: Memory Feedback Route Alias
The system MUST accept memory feedback at both `POST /api/v1/memory/feedback` (existing) and `POST /api/v1/memory/{id}/feedback` (alias for admin UI compatibility).

#### Scenario: Feedback via path-based route
- **WHEN** a client calls `POST /api/v1/memory/{id}/feedback` with `{ "layer": "User", "rewardType": "positive", "score": 1.0 }`
- **THEN** the system MUST record the feedback for the memory identified by `{id}`
- **AND** the `memoryId` field in the body is optional when the ID is in the path

#### Scenario: Feedback via body-based route (existing)
- **WHEN** a client calls `POST /api/v1/memory/feedback` with `{ "memoryId": "abc", "layer": "User", "rewardType": "positive", "score": 1.0 }`
- **THEN** the system MUST record the feedback for the specified memory

### Requirement: Memory Search Pagination
The `POST /api/v1/memory/search` endpoint MUST support `offset` and return a true grand total.

#### Scenario: Search with offset
- **WHEN** a client sends `POST /api/v1/memory/search` with `{ "query": "auth patterns", "limit": 10, "offset": 20 }`
- **THEN** the response MUST skip the first 20 results
- **AND** `total` MUST be the grand total of matching entries, not `items.len()`

### Requirement: Memory List SQL-Level Pagination
The `POST /api/v1/memory/list` endpoint MUST push LIMIT and OFFSET to the database query, not fetch all entries and truncate in Rust.

#### Scenario: List with SQL-level limit
- **WHEN** a client calls `POST /api/v1/memory/list` with `{ "layer": "User", "limit": 20, "offset": 40 }`
- **THEN** the SQL query MUST include `LIMIT 20 OFFSET 40`
- **AND** the response `total` MUST be the count of all entries in that layer
