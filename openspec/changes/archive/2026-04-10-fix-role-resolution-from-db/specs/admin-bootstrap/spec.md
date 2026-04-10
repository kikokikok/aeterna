## MODIFIED Requirements

### Requirement: Declarative PlatformAdmin Bootstrap
The system SHALL support declarative seeding of an initial PlatformAdmin identity from configuration, so that a fresh deployment is operational without manual database intervention.

The bootstrap configuration SHALL include the admin email, identity provider type, and provider-specific subject identifier. When bootstrap is enabled and valid configuration is present, the server SHALL idempotently seed the admin identity and PlatformAdmin role grant on startup before accepting HTTP traffic.

The PlatformAdmin role SHALL be granted with `tenant_id = '__root__'` (the instance-scope sentinel) rather than any tenant-specific value. This makes the role explicitly instance-scoped and distinguishable from tenant-scoped roles.

On startup, the bootstrap SHALL migrate any existing `user_roles` rows where `role = 'PlatformAdmin' AND tenant_id = 'default'` to use `tenant_id = '__root__'`, ensuring backward compatibility with deployments that used the previous `'default'` sentinel.

#### Scenario: Fresh deployment with bootstrap enabled
- **WHEN** the server starts with admin bootstrap enabled and valid email, provider, and provider subject configured
- **THEN** the system SHALL create or update the admin user in the `users` table with the configured email, idp_provider, and idp_subject
- **AND** the system SHALL ensure a `__root__` company organizational unit exists for instance-scope role grants
- **AND** the system SHALL grant the PlatformAdmin role with `tenant_id = '__root__'` and `unit_id = '__root__'` for the seeded user in the `user_roles` table
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

#### Scenario: Legacy default tenant_id migrated to __root__
- **WHEN** the bootstrap runs and finds existing `user_roles` rows with `role = 'PlatformAdmin'` and `tenant_id = 'default'`
- **THEN** the system SHALL update those rows to use `tenant_id = '__root__'`
- **AND** the migration SHALL be idempotent (safe to run on every startup)
