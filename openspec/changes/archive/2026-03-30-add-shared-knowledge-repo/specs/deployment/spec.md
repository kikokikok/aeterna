## MODIFIED Requirements

### Requirement: Deployment Configuration

The system SHALL support multiple deployment modes WITH high availability options. The Helm chart SHALL include configuration for the shared knowledge repository remote, SSH authentication, GitHub API access, and webhook integration.

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

#### Scenario: Knowledge repository remote configuration
- **WHEN** deploying with a shared knowledge repository
- **THEN** the Helm chart SHALL accept values for:
  - `aeterna.knowledgeRepo.url` (SSH URL of the remote repository)
  - `aeterna.knowledgeRepo.branch` (branch name, default: main)
  - `aeterna.knowledgeRepo.sshKeySecret` (K8s secret name containing the SSH private key)
  - `aeterna.knowledgeRepo.owner` (GitHub organization or user)
  - `aeterna.knowledgeRepo.name` (repository name)
  - `aeterna.knowledgeRepo.tokenSecret` (K8s secret name containing the GitHub API token)
- **AND** the deployment template SHALL inject these as environment variables into the Aeterna container

#### Scenario: Webhook configuration
- **WHEN** deploying with webhook integration enabled
- **THEN** the Helm chart SHALL accept `aeterna.webhook.enabled` (boolean, default: false) and `aeterna.webhook.secretRef` (K8s secret name)
- **AND** the deployment template SHALL inject AETERNA_WEBHOOK_SECRET from the referenced secret
- **AND** the webhook endpoint SHALL be accessible through the existing ingress at /api/v1/webhooks/github

#### Scenario: Local-only knowledge deployment
- **WHEN** deploying without a knowledge repository remote URL
- **THEN** the knowledge repository SHALL operate in local-only mode
- **AND** no SSH key, GitHub token, or webhook secrets SHALL be required
- **AND** all knowledge operations SHALL use the local filesystem only
