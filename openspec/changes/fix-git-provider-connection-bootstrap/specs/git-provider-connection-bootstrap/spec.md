## ADDED Requirements

### Requirement: Shared Git provider connections support stable explicit identifiers

The system SHALL allow a platform administrator to create a shared Git provider connection with an explicit stable identifier while remaining backward compatible with generated identifiers.

#### Scenario: Platform admin creates a connection with an explicit ID

- **WHEN** `POST /api/v1/admin/git-provider-connections` includes `id: "shared-github-app"`
- **AND** the caller is a PlatformAdmin
- **THEN** the system SHALL persist the connection with the exact ID `shared-github-app`
- **AND** subsequent repository bindings and tenant visibility grants SHALL reference that same ID

#### Scenario: Platform admin omits the ID

- **WHEN** `POST /api/v1/admin/git-provider-connections` omits `id`
- **AND** the caller is a PlatformAdmin
- **THEN** the system SHALL generate a unique identifier for the connection
- **AND** the create workflow SHALL otherwise behave as it did before the change

### Requirement: Shared Git provider connection IDs are validated and unique

The system SHALL reject malformed or duplicate shared Git provider connection IDs before mutating the registry.

#### Scenario: Invalid explicit ID is rejected

- **WHEN** a create request supplies an explicit ID containing unsupported characters or leading/trailing hyphens
- **THEN** the system SHALL reject the request with a validation error
- **AND** no connection record SHALL be created

#### Scenario: Duplicate explicit ID is rejected

- **WHEN** a create request supplies an explicit ID that already exists in the registry
- **THEN** the system SHALL reject the request with a validation error describing the duplicate ID
- **AND** the existing connection record SHALL remain unchanged

### Requirement: Shared Git provider connections bootstrap from chart-declared seed data

The system SHALL load chart-declared shared Git provider connections into the runtime registry during server startup before tenant repository bindings depend on them.

#### Scenario: Startup seeds missing shared connection

- **WHEN** server startup is configured with a shared connection seed file containing `shared-github-app`
- **AND** the registry does not already contain that ID
- **THEN** startup SHALL create the shared connection with the declared metadata
- **AND** startup SHALL grant tenant visibility for every tenant in the declared allow-list

#### Scenario: Startup preserves runtime tenant visibility for existing shared connection

- **WHEN** server startup loads a shared connection seed whose ID already exists with matching immutable metadata
- **THEN** startup SHALL preserve the existing tenant visibility allow-list for that connection
- **AND** the chart-declared allow-list SHALL be treated as initial bootstrap state rather than a restart-time overwrite

#### Scenario: Startup detects immutable metadata drift

- **WHEN** server startup loads a shared connection seed whose ID already exists with different provider metadata, secret reference, or app identifiers
- **THEN** startup SHALL fail fast with an actionable error
- **AND** it SHALL NOT silently overwrite the existing connection record
