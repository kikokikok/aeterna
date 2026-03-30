## MODIFIED Requirements

### Requirement: NPM Plugin Package

The system SHALL provide an NPM package `@kiko-aeterna/opencode-plugin` that integrates with OpenCode using the official `@opencode-ai/plugin` SDK.

The plugin MUST:
- Export a default Plugin function conforming to OpenCode's Plugin type
- Register all Aeterna tools as OpenCode tools
- Implement lifecycle hooks for deep integration
- Support configuration via `.aeterna/config.toml`
- Initialize a `LocalMemoryManager` on startup for local-first personal layer access
- Run a background `SyncEngine` for bidirectional memory synchronization

#### Scenario: Plugin installation
- **WHEN** a user runs `npm install -D @kiko-aeterna/opencode-plugin`
- **THEN** the plugin SHALL be available for OpenCode configuration
- **AND** the plugin SHALL include `better-sqlite3` as a dependency

#### Scenario: Plugin initialization with local store
- **WHEN** OpenCode starts with the Aeterna plugin configured
- **THEN** the plugin SHALL initialize the `LocalMemoryManager` with the configured database path
- **AND** the plugin SHALL start the `SyncEngine` background loop if a server URL is configured
- **AND** the plugin SHALL register all tools and hooks
- **AND** the plugin SHALL start a session context

#### Scenario: Plugin initialization without server
- **WHEN** OpenCode starts with the Aeterna plugin configured
- **AND** no `AETERNA_SERVER_URL` is set
- **THEN** the plugin SHALL initialize the `LocalMemoryManager` for offline-only operation
- **AND** personal layer tools SHALL function normally
- **AND** shared layer tools SHALL return empty results with an informative message

#### Scenario: Plugin shutdown with pending sync
- **WHEN** OpenCode shuts down
- **AND** the `sync_queue` contains pending operations
- **THEN** the plugin SHALL attempt a final sync push (up to 5s timeout)
- **AND** the plugin SHALL close the SQLite database cleanly

## ADDED Requirements

### Requirement: Memory Layer Routing
The plugin SHALL transparently route memory operations to the local store or remote server based on the target layer.

#### Scenario: Personal layer routed locally
- **WHEN** a memory operation targets layer `agent`, `user`, or `session`
- **THEN** the plugin SHALL execute the operation against the `LocalMemoryManager`
- **AND** the plugin SHALL NOT send a synchronous HTTP request to the remote server

#### Scenario: Shared layer routed to remote with cache
- **WHEN** a memory search targets layer `project`, `team`, `org`, or `company`
- **THEN** the plugin SHALL check the local cache first
- **AND** if the cache has recent results (< 60s), the plugin SHALL return cached results
- **AND** if the cache is stale or empty, the plugin SHALL fetch from the remote server and update the cache

#### Scenario: Shared layer write routed to remote
- **WHEN** a memory write targets a shared layer
- **THEN** the plugin SHALL send the write to the remote server via HTTP
- **AND** the plugin SHALL NOT write directly to the local store
- **AND** the entry SHALL appear locally on the next pull cycle
