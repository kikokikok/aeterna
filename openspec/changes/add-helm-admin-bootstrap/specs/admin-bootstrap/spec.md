## ADDED Requirements

### Requirement: Declarative PlatformAdmin Bootstrap
The system SHALL support declarative seeding of an initial PlatformAdmin identity from configuration, so that a fresh deployment is operational without manual database intervention.

The bootstrap configuration SHALL include the admin email, identity provider type, and provider-specific subject identifier. When bootstrap is enabled and valid configuration is present, the server SHALL idempotently seed the admin identity and PlatformAdmin role grant on startup before accepting HTTP traffic.

#### Scenario: Fresh deployment with bootstrap enabled
- **WHEN** the server starts with admin bootstrap enabled and valid email, provider, and provider subject configured
- **THEN** the system SHALL create or update the admin user in the `users` table with the configured email, idp_provider, and idp_subject
- **AND** the system SHALL ensure a `default` company organizational unit exists
- **AND** the system SHALL grant the PlatformAdmin role at instance scope for the seeded user in the `user_roles` table
- **AND** the system SHALL ensure the admin appears in the authorization view (`v_user_permissions`) via appropriate membership records
- **AND** all seeding operations SHALL complete before the HTTP server begins accepting requests

#### Scenario: Bootstrap is idempotent across restarts
- **WHEN** the server restarts with the same bootstrap configuration and the admin user already exists
- **THEN** the system SHALL NOT create duplicate rows
- **AND** the system SHALL NOT error or fail startup due to existing data
- **AND** existing role grants and memberships SHALL be preserved

#### Scenario: Bootstrap disabled by default
- **WHEN** the server starts without admin bootstrap configuration (no email set)
- **THEN** the system SHALL skip the bootstrap seeding entirely
- **AND** startup SHALL proceed normally without errors

#### Scenario: Bootstrap with incomplete configuration
- **WHEN** admin bootstrap is enabled but required fields (email) are missing
- **THEN** the system SHALL log a warning describing the missing configuration
- **AND** the system SHALL skip bootstrap seeding rather than failing startup

### Requirement: Bootstrap Configuration Loading
The system SHALL load admin bootstrap configuration from environment variables following the established `AETERNA_*` env var convention used by the config crate.

#### Scenario: Environment variables are loaded into config struct
- **WHEN** the config loader runs
- **THEN** the system SHALL read `AETERNA_ADMIN_BOOTSTRAP_EMAIL` into the admin bootstrap email field
- **AND** the system SHALL read `AETERNA_ADMIN_BOOTSTRAP_PROVIDER` into the identity provider field (default: `github`)
- **AND** the system SHALL read `AETERNA_ADMIN_BOOTSTRAP_PROVIDER_SUBJECT` into the provider subject field

#### Scenario: Config struct is accessible in bootstrap function
- **WHEN** the bootstrap function accesses the loaded config
- **THEN** the admin bootstrap configuration SHALL be available as a field on the root `Config` struct
- **AND** the bootstrap function SHALL use this config to determine whether and how to seed the admin
