## 1. Rewrite Deployment Spec

- [x] 1.1 Remove 3 fictional requirements (Letta, Mem0, OpenMemory, fictional microservices)
- [x] 1.2 Add Hybrid Local Deployment requirement (Helm deps + cargo run)
- [x] 1.3 Add Local Kubernetes Deployment requirement (full Helm deployment)
- [x] 1.4 Add Cloud Kubernetes Deployment requirement (managed services + Helm)
- [x] 1.5 Add Cross-Compilation Build Pipeline requirement
- [x] 1.6 Add Deployment Configuration Management requirement

## 2. Code Changes

- [x] 2.1 Switch TLS from native-tls to rustls (cross-compilation requirement)
- [x] 2.2 Add git2 vendored-openssl feature (cross-compilation requirement)
- [x] 2.3 Add CLI direct backend (cli/src/backend.rs — Qdrant + OpenAI-compatible API)
- [x] 2.4 Implement CLI memory commands in direct mode (search, add, delete, list, show, feedback)
- [x] 2.5 Add memory/src/json_utils.rs (robust JSON extraction replacing fragile parsing)
- [x] 2.6 Add with_base_url() to embedding and LLM OpenAI services
- [x] 2.7 Add Qdrant provider layer scoping (with_layer_scope, scoped_filter)
- [x] 2.8 Fix memory manager timeout trace calculation
- [x] 2.9 Fix storage migrations (001-014: extensions, org units, missing tables, broken FKs, RLS)
- [x] 2.10 Fix graph_duckdb JSON extension loading fallback
- [x] 2.11 Fix postgres.rs missing drift_suppressions table
- [x] 2.12 Fix sync/presence.rs uuid path qualification
- [x] 2.13 Fix tools/codesearch test API changes

## 3. Infrastructure Files

- [x] 3.1 Create Dockerfile.agent-a2a
- [x] 3.2 Simplify Dockerfile (remove cargo-chef, add nightly support)
- [x] 3.3 Create .dockerignore
- [x] 3.4 Update docker-compose.yml (Qdrant v1.16.2, TCP healthcheck)
- [x] 3.5 Create charts/aeterna/examples/values-dev.yaml (Helm deps-only mode)

## 4. Documentation

- [x] 4.1 Write docs/deployment/local-dev.md (Helm deps + cargo run)
- [x] 4.2 Write docs/deployment/local-kubernetes.md (full Helm deployment)
- [x] 4.3 Write docs/deployment/environment-variables.md (reference table)
- [x] 4.4 Write docs/deployment/testing-guide.md (entrypoints + CLI memory + Qdrant verification)

## 5. Validate

- [ ] 5.1 Full Helm deployment to local K8s cluster (all pods Running/Ready)
- [ ] 5.2 Validate agent-a2a entrypoints (health, agent card, metrics, tasks/send)
- [ ] 5.3 Validate CLI memory operations via direct backend
- [ ] 5.4 Run openspec validate update-deployment-infrastructure --strict
