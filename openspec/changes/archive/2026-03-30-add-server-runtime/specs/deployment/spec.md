## MODIFIED Requirements

### Requirement: Deployment Configuration

The system SHALL support multiple deployment modes WITH high availability options.

#### Scenario: Production HA Deployment
- **WHEN** deploying to production
- **THEN** Helm chart deploys:
  - PostgreSQL with Patroni (1 primary + 2 replicas)
  - Qdrant cluster (3 nodes, RF=2)
  - Redis Sentinel (3 nodes)
  - Memory service (3+ replicas)
  - Knowledge service (2+ replicas)
- **AND** configures PodDisruptionBudgets
- **AND** enables automatic backups
- **AND** configures monitoring

#### Scenario: Server Binary Kubernetes Probe Contract
- **WHEN** the Aeterna server pod starts in Kubernetes
- **THEN** the server binary SHALL respond to `GET /health` on port 8080 within the startup probe budget (160 seconds: 10s initial delay, 5s interval, 3s timeout, 30 failure threshold)
- **AND** the readiness probe SHALL target `GET /ready` on port 8080 (5s initial delay, 5s interval)
- **AND** the liveness probe SHALL target `GET /health` on port 8080 (30s initial delay, 10s interval)
- **AND** the metrics endpoint SHALL be available on port 9090 for Prometheus ServiceMonitor scraping
