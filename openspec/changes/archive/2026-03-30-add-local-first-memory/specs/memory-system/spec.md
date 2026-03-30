## ADDED Requirements

### Requirement: Sync Push Protocol
The system SHALL push locally-owned memory changes to the remote Aeterna server in batches via HTTP POST.

#### Scenario: Push queued changes
- **WHEN** the sync push interval elapses (default 30s)
- **AND** the `sync_queue` contains pending operations
- **THEN** the system SHALL send a `POST /api/v1/sync/push` request with the batch of changes
- **AND** the request body SHALL include `entries`, `device_id`, and `last_push_cursor`
- **AND** on success the system SHALL update the push cursor in `sync_cursors`
- **AND** the system SHALL remove successfully pushed entries from `sync_queue`

#### Scenario: Push on shutdown
- **WHEN** the plugin is shutting down
- **AND** the `sync_queue` contains pending operations
- **THEN** the system SHALL attempt a final push before exit
- **AND** the system SHALL wait up to 5 seconds for the push to complete

#### Scenario: Push failure
- **WHEN** a push request fails (network error or server error)
- **THEN** the system SHALL retain entries in the `sync_queue`
- **AND** the system SHALL retry on the next sync cycle
- **AND** the system SHALL use exponential backoff (30s, 60s, 120s, max 300s)

### Requirement: Sync Pull Protocol
The system SHALL pull shared-layer memory updates from the remote Aeterna server using cursor-based pagination.

#### Scenario: Pull shared layer updates
- **WHEN** the sync pull interval elapses (default 60s)
- **AND** the remote server is reachable
- **THEN** the system SHALL send a `GET /api/v1/sync/pull?since_cursor={cursor}&layers=project,team,org,company&limit=100`
- **AND** the system SHALL upsert returned entries into the local `memories` table with `ownership = 'cached'`
- **AND** the system SHALL update the pull cursor in `sync_cursors`

#### Scenario: Paginated pull
- **WHEN** a pull response includes `has_more = true`
- **THEN** the system SHALL immediately fetch the next page using the returned cursor
- **AND** the system SHALL continue until `has_more = false` or a configured page limit (default 10)

#### Scenario: Pull with no server
- **WHEN** a pull request fails due to network error
- **THEN** the system SHALL skip the pull cycle
- **AND** the system SHALL retry on the next scheduled interval
- **AND** the system SHALL NOT delete existing cached entries

### Requirement: Sync Conflict Resolution
The system SHALL resolve conflicts between local and remote versions of personal-layer memories using last-writer-wins.

#### Scenario: No conflict
- **WHEN** a pushed memory does not exist on the remote server
- **THEN** the remote server SHALL accept the memory as-is

#### Scenario: Local wins on personal layer
- **WHEN** a pushed memory conflicts with a remote version
- **AND** the local version has a more recent `updated_at` timestamp
- **THEN** the remote server SHALL accept the local version

#### Scenario: Remote wins on personal layer
- **WHEN** a pushed memory conflicts with a remote version
- **AND** the remote version has a more recent `updated_at` timestamp
- **THEN** the server SHALL return the conflict in the push response
- **AND** the local system SHALL overwrite its local copy with the remote version

### Requirement: Embedding Sync
The system SHALL receive embeddings from the remote server during push responses and store them locally.

#### Scenario: Embeddings returned on push
- **WHEN** a push response includes embeddings for pushed memories
- **THEN** the system SHALL update the local `memories` table with the received embeddings
- **AND** subsequent local searches SHALL use these embeddings for similarity scoring

#### Scenario: Memory added before embeddings available
- **WHEN** a memory is added locally and has no embedding
- **THEN** the system SHALL store the memory with `embedding = NULL`
- **AND** the system SHALL mark it as needing embedding in the push batch
- **AND** text-based search SHALL be used for this memory until embeddings arrive

### Requirement: Sync Status Reporting
The system SHALL expose sync status information to users and tools.

#### Scenario: Query sync status
- **WHEN** a user or tool requests sync status
- **THEN** the system SHALL return: pending push count, last push time, last pull time, server connectivity status, local store size

#### Scenario: Sync status in tool response
- **WHEN** a memory search returns results from the local cache for shared layers
- **THEN** the response metadata SHALL include `source: 'cache'` and `cached_at` timestamp
- **AND** the response SHALL NOT include stale warnings unless cache age exceeds 10 minutes

### Requirement: Device Identity
The system SHALL assign a stable device identifier for sync attribution.

#### Scenario: Device ID generation
- **WHEN** the local store is initialized for the first time
- **THEN** the system SHALL generate a UUID v4 as the device ID
- **AND** the system SHALL store it in the `sync_cursors` table or a dedicated metadata table
- **AND** the device ID SHALL persist across plugin restarts

#### Scenario: Device ID in push requests
- **WHEN** a push request is sent to the remote server
- **THEN** the request SHALL include the `device_id` field
- **AND** the server SHALL record the origin device on each synced memory
