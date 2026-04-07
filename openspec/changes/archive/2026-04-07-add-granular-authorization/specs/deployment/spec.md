## ADDED Requirements

### Requirement: Plugin Auth Helm Configuration
The Helm chart SHALL expose a `pluginAuth` values block for configuring Aeterna's authentication system in Kubernetes deployments, including enable/disable toggle, JWT secret, and GitHub OAuth App credentials.

#### Scenario: Auth disabled by default
- **WHEN** the Helm chart is installed with default values
- **THEN** `pluginAuth.enabled` SHALL default to `false`
- **AND** the deployed server SHALL operate in backward-compatible no-auth mode

#### Scenario: Auth enabled via values
- **WHEN** the Helm chart is installed with `pluginAuth.enabled: true` and required credentials
- **THEN** the configmap SHALL include `AETERNA_PLUGIN_AUTH_ENABLED=true`
- **AND** the deployment SHALL inject all `AETERNA_PLUGIN_AUTH_*` environment variables into the Aeterna container
- **AND** the server SHALL enforce authentication on protected routes

#### Scenario: Missing auth credentials fail chart validation
- **WHEN** the Helm chart is installed with `pluginAuth.enabled: true` but missing required fields (JWT secret, GitHub client ID)
- **THEN** the chart SHALL fail during template rendering or values validation
- **AND** the error message SHALL indicate which required auth fields are missing
