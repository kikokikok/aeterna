## ADDED Requirements

### Requirement: Helm Admin Bootstrap Values
The Helm chart SHALL expose an `adminBootstrap` values section that configures the initial PlatformAdmin identity seeding, mapping to `AETERNA_ADMIN_BOOTSTRAP_*` environment variables in the deployment template.

#### Scenario: Admin bootstrap values wired to deployment env vars
- **WHEN** the operator sets `adminBootstrap.email`, `adminBootstrap.provider`, and `adminBootstrap.providerSubject` in Helm values
- **THEN** the deployment template SHALL inject `AETERNA_ADMIN_BOOTSTRAP_EMAIL`, `AETERNA_ADMIN_BOOTSTRAP_PROVIDER`, and `AETERNA_ADMIN_BOOTSTRAP_PROVIDER_SUBJECT` as environment variables on the Aeterna container
- **AND** the env vars SHALL only be rendered when `adminBootstrap.email` is non-empty

#### Scenario: Default values do not seed an admin
- **WHEN** the chart is installed with default values (no `adminBootstrap.email` set)
- **THEN** the deployment template SHALL NOT inject admin bootstrap environment variables
- **AND** the server SHALL start without attempting bootstrap seeding

### Requirement: Helm Plugin Auth Tenant Value
The Helm chart SHALL expose a `pluginAuth.defaultTenantId` value that maps to the `AETERNA_PLUGIN_AUTH_TENANT` environment variable in the deployment template, enabling the plugin auth bootstrap endpoint to resolve a tenant.

#### Scenario: Plugin auth tenant wired to deployment env var
- **WHEN** the operator sets `pluginAuth.defaultTenantId` in Helm values
- **THEN** the deployment template SHALL inject `AETERNA_PLUGIN_AUTH_TENANT` as an environment variable on the Aeterna container
- **AND** the plugin auth bootstrap endpoint SHALL use this value for tenant resolution

#### Scenario: Plugin auth tenant not set
- **WHEN** the chart is installed without `pluginAuth.defaultTenantId`
- **THEN** the deployment template SHALL NOT inject `AETERNA_PLUGIN_AUTH_TENANT`
- **AND** the plugin auth bootstrap endpoint SHALL fail closed with an error rather than defaulting to an arbitrary tenant
