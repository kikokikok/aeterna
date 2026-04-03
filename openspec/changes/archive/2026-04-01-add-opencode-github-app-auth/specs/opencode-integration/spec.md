## MODIFIED Requirements

### Requirement: NPM Plugin Package

The system SHALL provide an NPM package `@aeterna-org/opencode-plugin` that integrates with OpenCode using the official `@opencode-ai/plugin` SDK.

The plugin MUST:
- Export a default Plugin function conforming to OpenCode's Plugin type
- Register all Aeterna tools as OpenCode tools
- Implement lifecycle hooks for deep integration
- Support configuration via `.aeterna/config.toml`
- Initialize a `LocalMemoryManager` on startup for local-first personal layer access
- Run a background `SyncEngine` for bidirectional memory synchronization
- Support an interactive authentication flow for end-user plugin access without requiring a static `AETERNA_TOKEN` for normal sign-in

#### Scenario: Plugin installation
- **WHEN** a user configures `@aeterna-org/opencode-plugin` in OpenCode using the supported plugin configuration flow
- **THEN** the plugin SHALL be available for OpenCode initialization
- **AND** the package SHALL expose the metadata required by the OpenCode plugin loader

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

#### Scenario: Plugin configuration via opencode.jsonc
- **WHEN** the user adds `"plugin": ["@aeterna-org/opencode-plugin"]` to opencode.jsonc
- **THEN** OpenCode SHALL load and initialize the Aeterna plugin
- **AND** all Aeterna tools SHALL be available to the AI

#### Scenario: Interactive authentication replaces static token for normal sign-in
- **WHEN** a user launches OpenCode with the Aeterna plugin configured for remote access
- **THEN** the plugin SHALL use the supported interactive authentication flow to obtain Aeterna credentials
- **AND** the user SHALL NOT need to manually set a static `AETERNA_TOKEN` for normal interactive plugin usage

### Requirement: Credential Security (OC-C2)
The system SHALL implement secure credential handling to prevent token exposure.

#### Scenario: Credential Masking in Logs
- **WHEN** debug logging is enabled
- **THEN** the system MUST mask `AETERNA_TOKEN` and other credentials
- **AND** masked values MUST use format `[REDACTED:...last4chars]`

#### Scenario: Secure Credential Storage
- **WHEN** credentials are persisted
- **THEN** the system MUST use secure storage appropriate to the OpenCode plugin runtime
- **AND** credentials MUST NOT be stored in plain text project files

#### Scenario: Token Rotation Support
- **WHEN** tokens are rotated
- **THEN** the system MUST support seamless token refresh
- **AND** ongoing operations MUST NOT be interrupted during rotation

#### Scenario: Interactive plugin session storage
- **WHEN** the plugin persists credentials for interactive authenticated use
- **THEN** it MUST persist only the credentials required for session continuation and refresh
- **AND** it MUST use the supported OpenCode/plugin credential persistence mechanism rather than requiring users to manually manage static secrets
