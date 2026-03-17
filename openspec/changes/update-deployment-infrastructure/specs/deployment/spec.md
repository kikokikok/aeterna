## REMOVED Requirements

### Requirement: Deployment Configuration
**Reason**: References fictional architecture (Letta, Mem0, OpenMemory, memory-service, knowledge-service, sync-service). None of these services exist in the codebase. Replaced by requirements matching the actual two-binary architecture.
**Migration**: Replaced by Local Development Deployment, Local Kubernetes Deployment, and Deployment Configuration Management.

### Requirement: OpenTofu Multi-Cloud Provisioning
**Reason**: References fictional services and images. Terraform/OpenTofu modules for AWS are aspirational, not tested. Replaced by a Cloud Kubernetes Deployment requirement that reflects realistic scope.
**Migration**: Replaced by Cloud Kubernetes Deployment.

### Requirement: Cloud KMS Encryption
**Reason**: Encryption at rest is a cloud-provider default for managed services (RDS, Memorystore). A separate requirement for CMEK is premature given current project stage. Will be re-introduced when production deployment is active.
**Migration**: Cloud Kubernetes Deployment includes a scenario for encryption at rest via managed service defaults.

## ADDED Requirements

### Requirement: Hybrid Local Deployment
Developers MUST be able to run Aeterna services locally using the Helm chart to deploy infrastructure dependencies (PostgreSQL, Qdrant, Redis) into a local Kubernetes cluster while running application services on bare metal with `cargo run`. This provides the fastest development loop.

#### Scenario: Deploy infrastructure via Helm
- **WHEN** a developer runs `helm install aeterna charts/aeterna -f charts/aeterna/examples/values-dev.yaml`
- **THEN** PostgreSQL (CloudNativePG with pgvector), Qdrant, and Dragonfly (Redis-compatible) pods start in the local cluster
- **AND** all infrastructure pods pass health checks within 60 seconds

#### Scenario: Port-forward infrastructure services
- **WHEN** infrastructure pods are healthy
- **AND** a developer port-forwards PostgreSQL (5432), Qdrant (6334), and Dragonfly (6379)
- **THEN** the services are accessible from the host at localhost on the forwarded ports

#### Scenario: Run agent-a2a service on bare metal
- **WHEN** infrastructure is healthy and environment variables are configured
- **AND** a developer runs `cargo run --bin agent-a2a`
- **THEN** the agent-a2a HTTP server starts on the configured port
- **AND** the `/health` endpoint returns 200 OK

#### Scenario: Run CLI in direct backend mode
- **WHEN** infrastructure is healthy and environment variables are configured
- **AND** a developer runs `cargo run --bin aeterna -- memory list --layer user`
- **THEN** the CLI connects directly to Qdrant and returns results without requiring a running server

#### Scenario: OpenAI-compatible LLM integration
- **WHEN** an OpenAI-compatible LLM endpoint is running (e.g., Ollama, vLLM, LM Studio, or OpenAI)
- **AND** EMBEDDING_API_BASE and LLM_API_BASE point to that endpoint
- **THEN** memory add, search, and reasoning operations use the configured models via the OpenAI-compatible API

### Requirement: Cross-Compilation Build Pipeline
The project MUST support cross-compilation to produce statically-linked Linux binaries suitable for minimal container images. This avoids Docker multi-stage builds that OOM on memory-constrained VMs.

#### Scenario: Build static musl binaries
- **WHEN** a developer runs `cargo zigbuild --release --target aarch64-unknown-linux-musl` for each binary
- **THEN** statically-linked binaries are produced in `target/aarch64-unknown-linux-musl/release/`
- **AND** each binary has no dynamic library dependencies

#### Scenario: Build minimal container images
- **WHEN** static binaries are copied into Alpine 3.21 base images via Dockerfile
- **THEN** the resulting images are under 40MB each
- **AND** images contain only the binary, CA certificates, and a non-root user

#### Scenario: TLS compatibility
- **WHEN** the project is compiled with `--target *-musl`
- **THEN** all TLS operations use rustls (not native-tls/OpenSSL)
- **AND** git2 uses vendored-openssl for libgit2 compatibility

### Requirement: Local Kubernetes Deployment
The project MUST deploy the full stack to a local Kubernetes cluster (k3s via Rancher Desktop) using the Helm chart. All services including aeterna run in a dedicated namespace.

#### Scenario: Deploy full stack via Helm
- **WHEN** a developer runs `helm install aeterna charts/aeterna -f charts/aeterna/examples/values-local.yaml -f my-overrides.yaml`
- **THEN** all infrastructure and aeterna pods are created in the target namespace
- **AND** all pods reach Running/Ready state

#### Scenario: Image distribution to k3s containerd
- **WHEN** Docker images are built locally
- **AND** the developer exports and loads them via `nerdctl --namespace k8s.io load`
- **THEN** k3s can pull images with `imagePullPolicy: Never`

#### Scenario: Service connectivity
- **WHEN** all pods are Running in the aeterna namespace
- **THEN** agent-a2a can reach PostgreSQL, Qdrant, and Redis via cluster DNS
- **AND** agent-a2a is accessible from the host via NodePort 30080

#### Scenario: Persistent storage
- **WHEN** postgres or qdrant pods are restarted
- **THEN** data is preserved via PersistentVolumeClaims
- **AND** PostgreSQL migrations are applied automatically on first start

### Requirement: Cloud Kubernetes Deployment
The project MUST provide guidance and templates for deploying to managed Kubernetes services (EKS, GKE, AKS) with managed backing services.

#### Scenario: Managed infrastructure provisioning
- **WHEN** an operator provisions cloud infrastructure using the provided Terraform modules
- **THEN** a managed Kubernetes cluster is created
- **AND** managed PostgreSQL (with pgvector), managed Redis, and a Qdrant node pool are provisioned
- **AND** all data stores have encryption at rest enabled by default

#### Scenario: Helm-based service deployment
- **WHEN** an operator installs the Aeterna Helm chart with environment-specific values
- **THEN** agent-a2a deploys with appropriate resource limits, health probes, and HPA
- **AND** services connect to managed backing stores via injected configuration

#### Scenario: Container registry integration
- **WHEN** CI/CD builds and pushes images to a container registry (ECR, GCR, ACR)
- **THEN** Kubernetes deployments can pull images using registry credentials or workload identity

### Requirement: Deployment Configuration Management
All Aeterna services MUST be configurable via environment variables, managed through Kubernetes ConfigMaps for cluster deployments and `.env` files or shell exports for local development.

#### Scenario: ConfigMap-based configuration
- **WHEN** services are deployed to Kubernetes
- **THEN** all configuration is sourced from the `aeterna-config` ConfigMap via `envFrom`
- **AND** no configuration is hardcoded in container images

#### Scenario: Required environment variables
- **WHEN** a service starts without required environment variables (DATABASE_URL, QDRANT_URL)
- **THEN** the service fails fast with a clear error message listing missing variables

#### Scenario: OpenAI-compatible endpoint configuration
- **WHEN** EMBEDDING_API_BASE and LLM_API_BASE are set to any OpenAI-compatible endpoint
- **AND** EMBEDDING_MODEL and LLM_MODEL are set to available model names
- **THEN** all AI operations route to the specified endpoint
- **AND** REASONING_TIMEOUT_MS controls the maximum wait time for LLM responses
