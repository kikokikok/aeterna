## MODIFIED Requirements

### Requirement: Tenant Manifest Schema

The system SHALL define a versioned manifest schema (`apiVersion: aeterna.io/v1`, `kind: TenantManifest`) covering tenant identity, configuration fields, secrets, repository binding, organizational hierarchy, and role assignments.

#### Scenario: Manifest includes tenant identity

- **WHEN** a manifest is parsed
- **THEN** it SHALL require a `tenant.slug` (URL-friendly identifier) and `tenant.name` (display name)
- **AND** the slug SHALL be validated as kebab-case

#### Scenario: Manifest includes configuration fields

- **WHEN** a manifest includes a `config.fields` section
- **THEN** each key-value pair SHALL be applied as a tenant configuration field
- **AND** the fields SHALL be stored via the tenant config provider

#### Scenario: Manifest includes secrets

- **WHEN** a manifest includes a `secrets` section
- **THEN** each secret entry SHALL be stored via the tenant secrets provider
- **AND** secrets SHALL be write-only and not included in config readback

#### Scenario: Manifest includes repository binding

- **WHEN** a manifest includes a `repository` section
- **THEN** the system SHALL create or update the tenant's repository binding
- **AND** if a `git_provider_connection_id` is specified, the system SHALL validate that the connection exists and the tenant is allowed to use it
- **AND** platform-managed shared connection IDs declared through deployment bootstrap SHALL be valid inputs to that validation

#### Scenario: Manifest includes organizational hierarchy

- **WHEN** a manifest includes a `hierarchy` section
- **THEN** the system SHALL create organizations, teams, and projects in the correct order (top-down)
- **AND** each unit SHALL be created within its parent scope

#### Scenario: Manifest includes role assignments

- **WHEN** a manifest includes a `roles` section
- **THEN** the system SHALL create resource-scoped role grants for each entry after the hierarchy is created
- **AND** role scope paths SHALL reference the hierarchy units created in the same manifest
