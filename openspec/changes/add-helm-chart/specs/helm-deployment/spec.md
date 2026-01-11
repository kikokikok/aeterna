## ADDED Requirements

### Requirement: Chart Installation
The Helm chart SHALL install Aeterna with a single command and sensible defaults.

#### Scenario: Install with defaults
- **WHEN** running `helm install aeterna ./charts/aeterna`
- **THEN** the chart SHALL deploy Aeterna with bundled Redis, PostgreSQL, and Qdrant
- **AND** all pods SHALL reach Ready state within 5 minutes
- **AND** the system SHALL be accessible via ClusterIP service

#### Scenario: Install with external dependencies
- **WHEN** running `helm install aeterna ./charts/aeterna --set redis.enabled=false --set redis.external.host=my-redis`
- **THEN** the chart SHALL configure Aeterna to connect to external Redis
- **AND** the chart SHALL NOT deploy Redis subchart

### Requirement: Configurable Dependencies
The chart SHALL support both bundled and external infrastructure dependencies.

#### Scenario: Use external Redis
- **WHEN** `redis.enabled=false` and `redis.external.host` is set
- **THEN** Aeterna SHALL connect to the external Redis instance
- **AND** no Redis pods SHALL be deployed

#### Scenario: Use external PostgreSQL
- **WHEN** `postgresql.enabled=false` and `postgresql.external.host` is set
- **THEN** Aeterna SHALL connect to the external PostgreSQL instance
- **AND** no PostgreSQL pods SHALL be deployed

#### Scenario: Use external Qdrant
- **WHEN** `qdrant.enabled=false` and `qdrant.external.host` is set
- **THEN** Aeterna SHALL connect to the external Qdrant instance
- **AND** no Qdrant pods SHALL be deployed

#### Scenario: Mixed deployment
- **WHEN** some dependencies are bundled and others external
- **THEN** the chart SHALL correctly configure connections for each dependency type

### Requirement: Health Probes
The chart SHALL configure health probes for all deployments.

#### Scenario: Liveness probe configuration
- **WHEN** Aeterna deployment is created
- **THEN** the deployment SHALL have liveness probe configured
- **AND** the probe SHALL use HTTP GET to `/health/live` endpoint
- **AND** the probe SHALL have appropriate timeout and failure thresholds

#### Scenario: Readiness probe configuration
- **WHEN** Aeterna deployment is created
- **THEN** the deployment SHALL have readiness probe configured
- **AND** the probe SHALL use HTTP GET to `/health/ready` endpoint
- **AND** pods SHALL not receive traffic until ready

#### Scenario: Startup probe configuration
- **WHEN** Aeterna deployment is created
- **THEN** the deployment SHALL have startup probe configured
- **AND** the probe SHALL allow sufficient time for initial startup

### Requirement: Horizontal Pod Autoscaling
The chart SHALL support automatic scaling based on resource utilization.

#### Scenario: HPA enabled
- **WHEN** `autoscaling.enabled=true`
- **THEN** HPA resource SHALL be created
- **AND** HPA SHALL scale between `minReplicas` and `maxReplicas`
- **AND** HPA SHALL target CPU utilization of 70% by default

#### Scenario: HPA disabled
- **WHEN** `autoscaling.enabled=false`
- **THEN** no HPA resource SHALL be created
- **AND** replica count SHALL be controlled by `replicas` value

### Requirement: Secret Management
The chart SHALL securely manage sensitive configuration.

#### Scenario: Auto-generated secrets
- **WHEN** no external secrets are provided
- **THEN** the chart SHALL generate random passwords for dependencies
- **AND** passwords SHALL be stored in Kubernetes Secrets
- **AND** passwords SHALL be reused on upgrades (lookup existing)

#### Scenario: External secrets
- **WHEN** `existingSecret` is specified
- **THEN** the chart SHALL use the referenced Secret
- **AND** the chart SHALL NOT create its own Secret

### Requirement: Ingress Configuration
The chart SHALL support optional Ingress for external access.

#### Scenario: Ingress enabled with TLS
- **WHEN** `ingress.enabled=true` and `ingress.tls` is configured
- **THEN** Ingress resource SHALL be created with TLS configuration
- **AND** Ingress SHALL route traffic to Aeterna service

#### Scenario: Ingress with custom annotations
- **WHEN** `ingress.annotations` is specified
- **THEN** Ingress SHALL include all custom annotations
- **AND** Ingress SHALL work with nginx, traefik, or other controllers

### Requirement: Observability Integration
The chart SHALL integrate with Prometheus monitoring stack.

#### Scenario: ServiceMonitor enabled
- **WHEN** `metrics.serviceMonitor.enabled=true`
- **THEN** ServiceMonitor resource SHALL be created
- **AND** Prometheus SHALL discover and scrape Aeterna metrics

#### Scenario: Metrics endpoint
- **WHEN** Aeterna pod is running
- **THEN** metrics SHALL be available at `/metrics` endpoint
- **AND** metrics SHALL include sync, memory, and knowledge operation counters

### Requirement: Resource Management
The chart SHALL allow fine-grained resource control.

#### Scenario: Resource requests and limits
- **WHEN** deployment is created
- **THEN** pods SHALL have resource requests and limits configured
- **AND** defaults SHALL prevent resource starvation

#### Scenario: Pod disruption budget
- **WHEN** `podDisruptionBudget.enabled=true`
- **THEN** PDB resource SHALL be created
- **AND** PDB SHALL ensure minimum availability during disruptions

### Requirement: Network Security
The chart SHALL support network isolation.

#### Scenario: Network policies enabled
- **WHEN** `networkPolicy.enabled=true`
- **THEN** NetworkPolicy resources SHALL be created
- **AND** Aeterna SHALL only allow traffic from specified sources
- **AND** dependencies SHALL only accept traffic from Aeterna

### Requirement: Configuration Validation
The chart SHALL validate configuration at install time.

#### Scenario: Missing external host
- **WHEN** `redis.enabled=false` but `redis.external.host` is empty
- **THEN** helm install SHALL fail with descriptive error
- **AND** error SHALL indicate the required configuration

#### Scenario: Invalid resource values
- **WHEN** resource limits are less than requests
- **THEN** helm install SHALL fail with descriptive error
