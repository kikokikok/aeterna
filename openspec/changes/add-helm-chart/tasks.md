# Tasks: Helm Chart + CLI Setup Wizard

## 1. CLI Setup Wizard (`aeterna setup`)

### 1.1 Core CLI Infrastructure
- [x] 1.1.1 Create `cli/` crate with Cargo.toml (dependencies: dialoguer, console, clap, serde, toml)
- [x] 1.1.2 Implement main entry point with subcommands (setup, status, validate)
- [x] 1.1.3 Add --non-interactive mode with CLI flags for all options
- [x] 1.1.4 Add --reconfigure flag to modify existing configuration
- [x] 1.1.5 Add --validate flag to check configuration validity
- [x] 1.1.6 Add --show flag to display current configuration
- [ ] 1.1.7 Write unit tests for CLI argument parsing

### 1.2 Wizard Flow Implementation
- [x] 1.2.1 Implement deployment mode selector (Local / Hybrid / Remote)
- [x] 1.2.2 Implement vector backend selector (Qdrant, pgvector, Pinecone, Weaviate, MongoDB, Vertex AI, Databricks)
- [x] 1.2.3 Implement cache selector (Dragonfly, Valkey, External Redis)
- [x] 1.2.4 Implement PostgreSQL selector (CloudNativePG, External)
- [x] 1.2.5 Implement OPAL authorization toggle with explanation
- [x] 1.2.6 Implement LLM provider selector (OpenAI, Anthropic, Ollama, Skip)
- [x] 1.2.7 Implement OpenCode integration toggle
- [x] 1.2.8 Implement advanced options (Ingress, ServiceMonitor, NetworkPolicy, HPA, PDB)
- [ ] 1.2.9 Write integration tests for wizard flow

### 1.3 Hybrid Mode Configuration
- [x] 1.3.1 Add central server URL prompt (for Hybrid/Remote modes)
- [x] 1.3.2 Add authentication method selector (API Key, OAuth2, Service Account)
- [ ] 1.3.3 Add local cache size selector for Hybrid mode
- [ ] 1.3.4 Add offline Cedar Agent toggle for Hybrid mode
- [ ] 1.3.5 Add sync interval configuration
- [ ] 1.3.6 Validate central server connectivity before proceeding
- [ ] 1.3.7 Write tests for Hybrid mode configuration

### 1.4 Configuration Generators
- [x] 1.4.1 Implement `values.yaml` generator for Helm deployments
- [x] 1.4.2 Implement `docker-compose.yaml` generator for local development
- [x] 1.4.3 Implement `.aeterna/config.toml` generator for runtime configuration
- [x] 1.4.4 Implement `~/.config/opencode/mcp.json` generator for OpenCode MCP
- [ ] 1.4.5 Add template validation before writing files
- [x] 1.4.6 Add backup of existing files before overwriting
- [ ] 1.4.7 Write unit tests for each generator

### 1.5 External Service Prompts
- [ ] 1.5.1 Add Pinecone configuration prompts (API key, environment, index name)
- [ ] 1.5.2 Add Weaviate configuration prompts (host, API key)
- [ ] 1.5.3 Add MongoDB Atlas configuration prompts (connection URI)
- [ ] 1.5.4 Add Vertex AI configuration prompts (project, region, endpoint, service account)
- [ ] 1.5.5 Add Databricks configuration prompts (workspace URL, token, catalog)
- [x] 1.5.6 Add external PostgreSQL configuration prompts (host, port, database, credentials)
- [x] 1.5.7 Add external Redis configuration prompts (host, port, password)
- [x] 1.5.8 Add OpenAI API key prompt with validation
- [x] 1.5.9 Add Anthropic API key prompt with validation
- [x] 1.5.10 Add Ollama host configuration prompt
- [ ] 1.5.11 Write tests for external service configuration

---

## 2. Helm Chart Structure

### 2.1 Project Setup
- [x] 2.1.1 Create `charts/aeterna/` directory structure
- [x] 2.1.2 Create `Chart.yaml` with metadata and subchart dependencies
- [x] 2.1.3 Create `values.yaml` with full schema (all options documented)
- [x] 2.1.4 Create `values.schema.json` for IDE validation
- [x] 2.1.5 Create `.helmignore` file
- [ ] 2.1.6 Add chart versioning strategy documentation

### 2.2 Template Helpers
- [x] 2.2.1 Create `_helpers.tpl` with naming conventions
- [x] 2.2.2 Add label generation helpers (app.kubernetes.io/*)
- [x] 2.2.3 Add selector helpers
- [x] 2.2.4 Add image reference helpers (with registry prefix support)
- [x] 2.2.5 Add secret reference helpers (inline vs existingSecret)
- [ ] 2.2.6 Add resource calculation helpers
- [ ] 2.2.7 Add validation helpers (mutual exclusivity checks)

---

## 3. Aeterna Core Services (Helm Templates)

### 3.1 Main Application Deployment
- [x] 3.1.1 Create `templates/aeterna/deployment.yaml` for Aeterna server
- [x] 3.1.2 Add environment variable injection from ConfigMap and Secrets
- [x] 3.1.3 Add deployment mode configuration (Local/Hybrid/Remote)
- [x] 3.1.4 Add feature flag environment variables (CCA, Radkit, RLM)
- [x] 3.1.5 Add liveness probe configuration
- [x] 3.1.6 Add readiness probe configuration
- [x] 3.1.7 Add startup probe configuration
- [x] 3.1.8 Add resource requests and limits
- [x] 3.1.9 Add pod anti-affinity for HA
- [x] 3.1.10 Add node selector and tolerations support

### 3.2 Service and Networking
- [x] 3.2.1 Create `templates/aeterna/service.yaml` for ClusterIP service
- [x] 3.2.2 Add metrics port to service (for Prometheus)
- [x] 3.2.3 Create `templates/aeterna/ingress.yaml` with TLS support
- [x] 3.2.4 Add Ingress class detection logic
- [x] 3.2.5 Add path-based routing for API endpoints

### 3.3 Configuration
- [x] 3.3.1 Create `templates/aeterna/configmap.yaml` for non-sensitive config
- [x] 3.3.2 Create `templates/aeterna/secret.yaml` with password generation
- [x] 3.3.3 Add checksum annotation for rolling restarts on config change
- [x] 3.3.4 Add support for existingSecret references

### 3.4 Security
- [x] 3.4.1 Create `templates/aeterna/serviceaccount.yaml`
- [x] 3.4.2 Create `templates/aeterna/rbac.yaml` (Role, RoleBinding)
- [x] 3.4.3 Add securityContext to pods (runAsNonRoot, readOnlyRootFilesystem)
- [x] 3.4.4 Create `templates/aeterna/networkpolicy.yaml` for isolation

### 3.5 Scaling and Availability
- [x] 3.5.1 Create `templates/aeterna/hpa.yaml` for horizontal pod autoscaling
- [x] 3.5.2 Create `templates/aeterna/pdb.yaml` for pod disruption budget
- [ ] 3.5.3 Add VPA support (VerticalPodAutoscaler template)

### 3.6 Database Migrations
- [x] 3.6.1 Create `templates/aeterna/job-migration.yaml` for schema migrations
- [ ] 3.6.2 Add pre-upgrade hook for migrations
- [ ] 3.6.3 Add migration job cleanup policy

---

## 4. OPAL Stack Integration (Merge from aeterna-opal)

### 4.1 OPAL Server
- [x] 4.1.1 Create `templates/opal/opal-server.yaml` (StatefulSet + Service)
- [x] 4.1.2 Add pod anti-affinity for zone-aware distribution
- [x] 4.1.3 Add PodDisruptionBudget for OPAL Server
- [x] 4.1.4 Add broadcasting configuration for multi-replica

### 4.2 Cedar Agent
- [x] 4.2.1 Create `templates/opal/cedar-agent.yaml` (DaemonSet + Service)
- [x] 4.2.2 Add tolerations for control-plane nodes
- [x] 4.2.3 Configure OPAL Client connection to Server

### 4.3 OPAL Fetcher
- [x] 4.3.1 Create `templates/opal/opal-fetcher.yaml` (Deployment + Service)
- [x] 4.3.2 Configure PostgreSQL data source connection
- [x] 4.3.3 Add health check endpoints

### 4.4 OPAL Configuration
- [x] 4.4.1 Create `templates/opal/configmap.yaml` for OPAL config
- [x] 4.4.2 Create `templates/opal/secrets.yaml` for tokens and credentials
- [x] 4.4.3 Add conditional rendering (only when opal.enabled=true)

---

## 5. Subchart Dependencies (Open-Source Only)

### 5.1 CloudNativePG (PostgreSQL)
- [x] 5.1.1 Add CloudNativePG as subchart dependency in Chart.yaml
- [x] 5.1.2 Pin to specific version (0.23.x)
- [ ] 5.1.3 Configure cluster creation template
- [ ] 5.1.4 Add pgvector extension installation
- [ ] 5.1.5 Configure backup to S3/GCS/Azure
- [x] 5.1.6 Add conditional logic for external PostgreSQL
- [ ] 5.1.7 Document CloudNativePG upgrade procedure

### 5.2 Dragonfly (Redis-Compatible Cache)
- [x] 5.2.1 Add Dragonfly operator as subchart dependency
- [x] 5.2.2 Pin to specific version
- [ ] 5.2.3 Configure Dragonfly instance template
- [ ] 5.2.4 Add HA configuration (master/replica)
- [x] 5.2.5 Add conditional logic for external Redis

### 5.3 Valkey (Alternative Redis)
- [ ] 5.3.1 Add Valkey as subchart dependency
- [ ] 5.3.2 Pin to specific version
- [ ] 5.3.3 Configure Valkey instance template
- [x] 5.3.4 Add mutual exclusivity with Dragonfly (only one enabled)

### 5.4 Qdrant (Vector Database)
- [x] 5.4.1 Add official Qdrant chart as subchart dependency
- [x] 5.4.2 Pin to specific version (0.10.x)
- [ ] 5.4.3 Configure distributed deployment (3+ nodes for production)
- [ ] 5.4.4 Add snapshot/backup configuration
- [x] 5.4.5 Add conditional logic for external Qdrant

### 5.5 Weaviate (Optional Vector Backend)
- [x] 5.5.1 Add Weaviate as optional subchart dependency
- [x] 5.5.2 Pin to specific version (17.x)
- [ ] 5.5.3 Configure multi-tenancy settings
- [x] 5.5.4 Add conditional logic (only when vectorBackend.type=weaviate)

### 5.6 Percona MongoDB (Optional Vector Backend)
- [x] 5.6.1 Add Percona MongoDB Operator as optional subchart
- [x] 5.6.2 Pin to specific version (1.21.x)
- [ ] 5.6.3 Configure replica set with Atlas Search
- [x] 5.6.4 Add conditional logic (only when vectorBackend.type=mongodb)

---

## 6. Deployment Modes

### 6.1 Local Mode
- [x] 6.1.1 Create values-local.yaml with all components enabled
- [x] 6.1.2 Configure resource limits for single-node deployment
- [x] 6.1.3 Disable HA features (single replicas)
- [ ] 6.1.4 Document local mode limitations

### 6.2 Hybrid Mode
- [x] 6.2.1 Create values-hybrid.yaml with local cache only
- [x] 6.2.2 Add central server connection configuration
- [ ] 6.2.3 Configure sync service settings
- [x] 6.2.4 Enable local Cedar Agent for offline policy evaluation
- [ ] 6.2.5 Configure memory layer sync (Working/Session → Episodic)
- [ ] 6.2.6 Document hybrid mode architecture and data flow

### 6.3 Remote Mode
- [ ] 6.3.1 Create values-remote.yaml with thin client configuration
- [ ] 6.3.2 Disable all local storage components
- [ ] 6.3.3 Configure central server authentication
- [ ] 6.3.4 Document remote mode requirements

---

## 7. Observability

### 7.1 Prometheus Integration
- [x] 7.1.1 Create `templates/aeterna/servicemonitor.yaml` for Prometheus Operator
- [ ] 7.1.2 Add metrics port configuration
- [ ] 7.1.3 Add scrape interval and timeout settings
- [x] 7.1.4 Add conditional rendering (when serviceMonitor.enabled=true)

### 7.2 Tracing
- [ ] 7.2.1 Add OpenTelemetry configuration to deployment
- [ ] 7.2.2 Configure tracing endpoint environment variable
- [ ] 7.2.3 Document Jaeger/Tempo integration

### 7.3 Logging
- [ ] 7.3.1 Configure structured JSON logging
- [ ] 7.3.2 Add log level configuration per component
- [ ] 7.3.3 Document log aggregation setup (Loki, ELK)

---

## 8. Secret Management

### 8.1 Helm Secrets (Default)
- [x] 8.1.1 Implement password generation in secret templates
- [x] 8.1.2 Add checksum annotation for secret rotation

### 8.2 SOPS Integration
- [ ] 8.2.1 Create values-sops.yaml example
- [ ] 8.2.2 Document SOPS with age/GPG keys setup
- [ ] 8.2.3 Add helm-secrets plugin documentation

### 8.3 External Secrets Operator
- [ ] 8.3.1 Create `templates/externalsecret.yaml` template
- [ ] 8.3.2 Add secretProvider configuration option
- [ ] 8.3.3 Document AWS Secrets Manager integration
- [ ] 8.3.4 Document HashiCorp Vault integration
- [ ] 8.3.5 Document Azure Key Vault integration

---

## 9. Production Hardening

### 9.1 Security
- [ ] 9.1.1 Add Pod Security Standards (restricted profile)
- [ ] 9.1.2 Configure network policies for all components
- [ ] 9.1.3 Add image pull secrets configuration
- [ ] 9.1.4 Document security best practices

### 9.2 High Availability
- [ ] 9.2.1 Configure pod anti-affinity for all stateless components
- [ ] 9.2.2 Add topology spread constraints
- [ ] 9.2.3 Configure PDB for all critical components
- [ ] 9.2.4 Document HA requirements (min 3 nodes)

### 9.3 Resource Management
- [ ] 9.3.1 Create values-small.yaml (< 1000 memories)
- [ ] 9.3.2 Create values-medium.yaml (< 100k memories)
- [ ] 9.3.3 Create values-large.yaml (> 100k memories)
- [ ] 9.3.4 Add sizing guide documentation

### 9.4 Backup and Recovery
- [ ] 9.4.1 Create backup CronJob template for PostgreSQL
- [ ] 9.4.2 Add S3/GCS/Azure backup destination configuration
- [ ] 9.4.3 Document restore procedure
- [ ] 9.4.4 Add backup verification job

---

## 10. Testing

### 10.1 Chart Testing
- [x] 10.1.1 Create `tests/` directory with helm test pods
- [x] 10.1.2 Write connection test for PostgreSQL
- [ ] 10.1.3 Write connection test for Redis/Dragonfly
- [x] 10.1.4 Write connection test for Qdrant
- [x] 10.1.5 Write health check test for Aeterna server
- [x] 10.1.6 Write OPAL connectivity test

### 10.2 Integration Testing
- [ ] 10.2.1 Test Local mode deployment (all bundled)
- [ ] 10.2.2 Test Hybrid mode deployment
- [ ] 10.2.3 Test Remote mode deployment
- [ ] 10.2.4 Test with external dependencies only
- [ ] 10.2.5 Test mixed configuration (some bundled, some external)

### 10.3 CLI Testing
- [ ] 10.3.1 Write unit tests for wizard flow logic
- [ ] 10.3.2 Write unit tests for configuration generators
- [ ] 10.3.3 Write integration tests for file generation
- [ ] 10.3.4 Write E2E test for full wizard → deploy cycle

---

## 11. Documentation

### 11.1 Chart Documentation
- [x] 11.1.1 Create `charts/aeterna/README.md` with full reference
- [x] 11.1.2 Document all values.yaml options
- [ ] 11.1.3 Add architecture diagram
- [x] 11.1.4 Add troubleshooting section

### 11.2 Deployment Guides
- [ ] 11.2.1 Create Local mode quick start guide
- [ ] 11.2.2 Create Hybrid mode deployment guide
- [ ] 11.2.3 Create Remote mode deployment guide
- [ ] 11.2.4 Create production deployment checklist

### 11.3 Example Configurations
- [x] 11.3.1 Create values-local.yaml example
- [x] 11.3.2 Create values-production.yaml example
- [x] 11.3.3 Create values-hybrid.yaml example
- [ ] 11.3.4 Create values-aws.yaml example (EKS-specific)
- [ ] 11.3.5 Create values-gke.yaml example (GKE-specific)
- [ ] 11.3.6 Create values-aks.yaml example (AKS-specific)

### 11.4 Upgrade Documentation
- [ ] 11.4.1 Document chart upgrade procedure
- [ ] 11.4.2 Add version migration notes
- [ ] 11.4.3 Document subchart upgrade process
- [ ] 11.4.4 Add rollback procedure

---

## 12. CI/CD

### 12.1 Chart Validation
- [x] 12.1.1 Add chart linting to CI (helm lint)
- [x] 12.1.2 Add helm template validation
- [x] 12.1.3 Add kubeval/kubeconform validation
- [x] 12.1.4 Add values schema validation

### 12.2 Publishing
- [x] 12.2.1 Add chart publishing workflow to GitHub Pages
- [x] 12.2.2 Add OCI registry publishing (ghcr.io)
- [x] 12.2.3 Add release automation with changelog

### 12.3 Docker Images
- [x] 12.3.1 Add Dockerfile for Aeterna server
- [x] 12.3.2 Add multi-arch build (amd64 + arm64)
- [x] 12.3.3 Add image scanning (Trivy)
- [x] 12.3.4 Add image signing (Cosign)

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 30 | CLI Setup Wizard |
| 2 | 13 | Helm Chart Structure |
| 3 | 21 | Aeterna Core Services |
| 4 | 12 | OPAL Stack Integration |
| 5 | 21 | Subchart Dependencies |
| 6 | 10 | Deployment Modes |
| 7 | 9 | Observability |
| 8 | 9 | Secret Management |
| 9 | 12 | Production Hardening |
| 10 | 14 | Testing |
| 11 | 14 | Documentation |
| 12 | 10 | CI/CD |
| **Total** | **175** | |

**Estimated effort**: 6-8 weeks with 80% test coverage target

---

## Dependencies

| Subchart | Repository | Version | License |
|----------|------------|---------|---------|
| CloudNativePG | https://cloudnative-pg.github.io/charts | 0.23.x | Apache-2.0 |
| Dragonfly | oci://ghcr.io/dragonflydb/dragonfly | 1.x | Apache-2.0 |
| Valkey | https://valkey.io/valkey-helm/ | 1.x | BSD-3 |
| Qdrant | https://qdrant.github.io/qdrant-helm | 0.10.x | Apache-2.0 |
| Weaviate | https://weaviate.github.io/weaviate-helm/ | 17.x | BSD-3 |
| Percona MongoDB | https://percona.github.io/percona-helm-charts/ | 1.21.x | Apache-2.0 |

**Note**: No Bitnami charts - all dependencies use genuinely open-source alternatives.
