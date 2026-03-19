## MODIFIED Requirements

### Requirement: Deployment Configuration

The system SHALL support multiple deployment modes WITH high availability options using deployment assets that are internally consistent, installable, and upgrade-safe.

#### Scenario: Supported deployment assets are valid
- **WHEN** operators use the documented production deployment path
- **THEN** all referenced manifests, charts, commands, and services SHALL exist in the repository
- **AND** the deployment path SHALL NOT reference missing service directories, missing manifests, or unsupported commands

#### Scenario: Helm install with defaults
- **WHEN** running `helm install` against the supported Aeterna chart with documented default values
- **THEN** the chart SHALL render only Kubernetes resources whose required APIs and dependencies are part of the supported installation path
- **AND** default dependency wiring SHALL resolve to valid in-cluster service names
- **AND** the deployment SHALL NOT rely on undocumented prerequisite operators or manually created resources

#### Scenario: Upgrade-safe secret behavior
- **WHEN** performing `helm upgrade` without changing secret inputs
- **THEN** generated credentials and OPAL tokens SHALL be reused from existing secrets
- **AND** upgrade operations SHALL NOT rotate live credentials unexpectedly
- **AND** external secret providers SHALL be supported without falling back to random secret generation

#### Scenario: Valid production example values
- **WHEN** operators use documented production example values
- **THEN** the values SHALL map to actual chart schema keys
- **AND** the rendered topology SHALL match the documented deployment intent
- **AND** invalid or contradictory production examples SHALL be rejected during validation

#### Scenario: Network isolation in production
- **WHEN** network policies are enabled for production deployment
- **THEN** ingress SHALL be limited to explicitly allowed controllers or workloads
- **AND** egress SHALL be limited to required dependencies and DNS
- **AND** same-namespace wildcard access SHALL NOT be treated as a secure default

#### Scenario: TLS ingress configuration
- **WHEN** ingress TLS is enabled with cert-manager integration
- **THEN** ingress resources SHALL support controller-specific annotations and certificate issuer configuration through chart values
- **AND** the documented example SHALL describe secret naming and certificate manager expectations
