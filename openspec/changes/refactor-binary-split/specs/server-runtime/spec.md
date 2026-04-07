## ADDED Requirements

### Requirement: Dedicated Migration Binary
The system SHALL provide a standalone `aeterna-migrate` binary that embeds all SQL migrations and connects directly to PostgreSQL to apply them. This binary MUST be the only artifact that holds direct database credentials for migration purposes.

#### Scenario: Run migrations via dedicated binary
- **WHEN** an operator executes `aeterna-migrate up`
- **THEN** all pending migrations are applied to the target database
- **AND** the binary exits with code 0 on success

#### Scenario: Migration status via dedicated binary
- **WHEN** an operator executes `aeterna-migrate status`
- **THEN** a report of applied and pending migrations is displayed

#### Scenario: Helm Job uses dedicated binary
- **WHEN** a Helm upgrade triggers the migration Job
- **THEN** the Job container runs `aeterna-migrate up` instead of `aeterna admin migrate up`

### Requirement: CLI Binary Without Server Dependencies
The system SHALL produce an `aeterna-cli` binary that includes all CLI commands except `serve`. This binary MUST NOT depend on axum, tower, hyper, sqlx, or the server module.

#### Scenario: CLI binary excludes serve command
- **WHEN** a user runs `aeterna-cli serve`
- **THEN** the command is not recognized

#### Scenario: CLI binary includes all other commands
- **WHEN** a user runs `aeterna-cli admin health`
- **THEN** the command executes via API call to the server

#### Scenario: CLI admin migrate calls API
- **WHEN** a user runs `aeterna-cli admin migrate status`
- **THEN** the CLI calls `GET /api/v1/admin/migrate/status` on the server
- **AND** does NOT connect directly to the database

## MODIFIED Requirements

### Requirement: Full Binary Composition
The `aeterna` binary SHALL continue to include all functionality: CLI commands, HTTP server (`serve`), and migration logic. It MUST remain backward-compatible for existing Docker images and server deployments.

#### Scenario: Full binary retains serve command
- **WHEN** a server operator runs `aeterna serve`
- **THEN** the HTTP API server starts as before

#### Scenario: Full binary retains admin migrate
- **WHEN** a server operator runs `aeterna admin migrate up`
- **THEN** migrations are applied directly via sqlx as before
