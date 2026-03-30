## MODIFIED Requirements

### Requirement: Deployment Configuration

The system SHALL support multiple deployment modes WITH high availability options.

When GitHub Organization sync is enabled, the Helm chart MUST configure the GitHub App credentials (app_id, installation_id, PEM secret reference) and target org name as environment variables for the Aeterna deployment.

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

#### Scenario: GitHub Org Sync Helm Configuration
- **WHEN** `aeterna.github.orgSync.enabled` is set to `true` in Helm values
- **THEN** the Helm chart SHALL inject `GITHUB_APP_ID`, `GITHUB_INSTALLATION_ID`, `GITHUB_ORG_NAME` as environment variables
- **AND** the Helm chart SHALL mount the PEM private key from the referenced Kubernetes secret
- **AND** the Helm chart SHALL configure the webhook endpoint to accept GitHub organization and team events
