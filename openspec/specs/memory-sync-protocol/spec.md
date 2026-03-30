# memory-sync-protocol Specification

## Purpose
The Memory Sync Protocol defines the server-side HTTP endpoints for bidirectional memory synchronization between local plugin stores and the remote Aeterna server.

## Requirements
### Requirement: Sync Push Endpoint
The server SHALL expose a `POST /api/v1/sync/push` endpoint for clients to push local memory changes.

#### Scenario: Accept push batch
- **WHEN** a client sends a push request with `entries`, `device_id`, and `last_push_cursor`
- **THEN** the server SHALL upsert each entry into the remote memory store
- **AND** the server SHALL generate embeddings for entries missing them
- **AND** the server SHALL return a response with `cursor`, `conflicts[]`, and `embeddings{}`

#### Scenario: Conflict detection
- **WHEN** a pushed entry conflicts with an existing remote entry (same ID, different content)
- **AND** the remote entry has a more recent `updated_at`
- **THEN** the server SHALL include the conflicting entry in the `conflicts` array
- **AND** the server SHALL NOT overwrite the remote entry

#### Scenario: Push authentication
- **WHEN** a push request lacks valid authentication
- **THEN** the server SHALL return HTTP 401
- **AND** the server SHALL NOT modify any data

### Requirement: Sync Pull Endpoint
The server SHALL expose a `GET /api/v1/sync/pull` endpoint for clients to pull shared-layer updates.

#### Scenario: Pull with cursor
- **WHEN** a client sends a pull request with `since_cursor`, `layers`, and `limit`
- **THEN** the server SHALL return entries updated after the cursor position
- **AND** the response SHALL include `entries[]`, `cursor`, and `has_more`
- **AND** entries SHALL be ordered by `updated_at` ascending

#### Scenario: Pull without cursor (initial sync)
- **WHEN** a client sends a pull request without a `since_cursor`
- **THEN** the server SHALL return the most recent entries up to `limit`
- **AND** the server SHALL return a cursor for subsequent requests

#### Scenario: Layer filtering
- **WHEN** a pull request specifies `layers=project,team`
- **THEN** the server SHALL only return entries from the requested layers
- **AND** the server SHALL enforce tenant isolation (client only receives entries for their tenant context)
