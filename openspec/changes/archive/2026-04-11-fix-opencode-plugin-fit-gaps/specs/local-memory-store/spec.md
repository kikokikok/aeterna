## MODIFIED Requirements

### Requirement: Local Memory Store Initialization
The system SHALL initialize an embedded SQLite database compatible with the OpenCode runtime on plugin startup at the configured path (default `~/.aeterna/local.db`).

#### Scenario: First-time initialization
- **WHEN** the plugin starts and no local database file exists
- **THEN** the system SHALL create the database file with WAL journal mode
- **AND** the system SHALL create the `memories`, `sync_queue`, and `sync_cursors` tables
- **AND** the system SHALL complete initialization in under 50ms

#### Scenario: Existing database
- **WHEN** the plugin starts and a local database file already exists
- **THEN** the system SHALL open the database and verify schema version
- **AND** the system SHALL apply any pending schema migrations idempotently

#### Scenario: Disabled local store
- **WHEN** the plugin config sets `local.enabled = false`
- **THEN** the system SHALL NOT initialize the local database
- **AND** all memory operations SHALL route to the remote server exclusively

### Requirement: Shared Layer Cache
The system SHALL cache remote shared-layer memories (project, team, org, company) in the local store for offline access.

#### Scenario: Read cached shared memory
- **WHEN** a memory search targets layer `project`, `team`, `org`, or `company`
- **AND** cached entries exist in the local store with `ownership = 'cached'`
- **THEN** the system SHALL return cached results immediately when the cache is still fresh
- **AND** the returned results SHALL indicate that they came from cache

#### Scenario: Cache miss with server available
- **WHEN** a shared-layer search has no cached results
- **AND** the remote server is reachable
- **THEN** the system SHALL fetch from the remote server
- **AND** the system SHALL cache the results locally with `ownership = 'cached'`

#### Scenario: Cache miss with server unavailable
- **WHEN** a shared-layer search has no cached results
- **AND** the remote server is unreachable
- **THEN** the system SHALL return an empty result set (not an error)
- **AND** the system SHALL log a warning about offline state

#### Scenario: Stale cache fallback
- **WHEN** a shared-layer search has cached results that are older than the fresh-cache threshold
- **AND** the remote server request fails
- **THEN** the system SHALL return the stale cached results rather than failing the search
- **AND** the returned results SHALL indicate that they may be stale

#### Scenario: Cache eviction
- **WHEN** the number of cached shared-layer entries exceeds `local.max_cached_entries` (default 50,000)
- **THEN** the system SHALL evict the oldest entries by `cached_at` timestamp

### Requirement: Offline Resilience
The system SHALL function for personal layers without any network connectivity.

#### Scenario: Fully offline operation
- **WHEN** the remote Aeterna server is unreachable
- **THEN** memory add, search, get, update, and delete operations for agent/user/session layers SHALL succeed using the local store
- **AND** changes SHALL accumulate in the `sync_queue` for later push

#### Scenario: Network recovery
- **WHEN** the remote server becomes reachable after an offline period
- **THEN** the `SyncEngine` SHALL push all queued changes on the next sync cycle
- **AND** the system SHALL pull any shared layer updates missed during the offline period

#### Scenario: Offline search provenance is visible
- **WHEN** the plugin returns local, cached, or remote memory results
- **THEN** the returned results SHALL include source metadata identifying whether the result came from local storage, cache, or remote retrieval
- **AND** stale cached results SHALL be identifiable to the user-facing tool output
