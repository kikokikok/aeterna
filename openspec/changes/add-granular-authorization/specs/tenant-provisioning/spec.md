## ADDED Requirements

### Requirement: Single-File Tenant Provisioning Manifest
The system SHALL accept a declarative YAML manifest file that describes a complete tenant configuration, and process it in a single API call or CLI command to create and fully configure a tenant.

#### Scenario: Provision a new tenant from manifest
- **WHEN** `POST /api/v1/admin/tenants/provision` is called with a valid tenant manifest
- **AND** the caller has PlatformAdmin role
- **THEN** the system SHALL create the tenant, apply configuration fields, store secrets, set the repository binding, create the organizational hierarchy, and assign initial roles
- **AND** the response SHALL include the created tenant ID and a status for each provisioning step

#### Scenario: CLI provisioning command
- **WHEN** `aeterna admin tenant provision --file manifest.yaml` is executed
- **AND** the CLI user has PlatformAdmin credentials
- **THEN** the CLI SHALL parse the YAML manifest and send it to the provisioning API
- **AND** the CLI SHALL display progress for each provisioning step

#### Scenario: Manifest validation before processing
- **WHEN** a manifest is submitted with invalid structure (missing required fields, invalid role names, malformed hierarchy)
- **THEN** the system SHALL reject the manifest before processing any steps
- **AND** the response SHALL list all validation errors

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

#### Scenario: Manifest includes organizational hierarchy
- **WHEN** a manifest includes a `hierarchy` section
- **THEN** the system SHALL create organizations, teams, and projects in the correct order (top-down)
- **AND** each unit SHALL be created within its parent scope

#### Scenario: Manifest includes role assignments
- **WHEN** a manifest includes a `roles` section
- **THEN** the system SHALL create resource-scoped role grants for each entry after the hierarchy is created
- **AND** role scope paths SHALL reference the hierarchy units created in the same manifest

### Requirement: Idempotent Tenant Provisioning
The system SHALL support idempotent re-application of a tenant manifest, allowing operators to update an existing tenant's configuration without side effects from repeated application.

#### Scenario: Re-provision existing tenant updates config
- **WHEN** a manifest is submitted for an existing tenant slug
- **THEN** the system SHALL update configuration fields, secrets, and role assignments to match the manifest
- **AND** the system SHALL NOT create duplicate hierarchy units

#### Scenario: Provisioning step failure reports partial status
- **WHEN** a provisioning step fails (e.g., hierarchy creation error)
- **THEN** the response SHALL indicate which steps succeeded and which failed
- **AND** successfully completed steps SHALL NOT be rolled back
- **AND** the operator SHALL be able to fix the issue and re-submit the manifest
