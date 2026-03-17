# Change: Update Deployment Infrastructure to Match Actual Architecture

## Why

The existing deployment spec is entirely fictional — it references services that do not exist
(Letta, Mem0, OpenMemory, memory-service, knowledge-service, sync-service) and deployment
patterns (Patroni HA, Redis Sentinel clusters, ECS Fargate) that were never implemented. The
actual system architecture is a monolithic CLI binary (`aeterna`) plus a single HTTP service
(`agent-a2a`), backed by PostgreSQL/pgvector, Qdrant, and Redis, with an external
OpenAI-compatible API for embedding and LLM inference. The spec must be rewritten
to reflect reality and provide actionable deployment guidance.

## What Changes

- **BREAKING**: Replace all 3 existing deployment requirements with requirements that match
  the actual codebase architecture
- Add requirement for hybrid local deployment (K8s via Helm for dependencies +
  `cargo run` for services on bare metal)
- Add requirement for full local Kubernetes deployment (Helm chart installs everything
  including aeterna services)
- Add requirement for cloud Kubernetes deployment (managed services + Helm chart)
- Add requirement for cross-compilation build pipeline (cargo-zigbuild → musl → Alpine images)
- Remove all references to fictional services (Letta, Mem0, OpenMemory, memory-service,
  knowledge-service, sync-service)

## Impact

- Affected specs: `deployment`
- Affected code: `charts/aeterna/`, `Dockerfile`, `Dockerfile.agent-a2a`,
  `cli/`, `memory/`, `storage/`, `docs/deployment/`
- No runtime behavior changes — this is a spec-only correction to match existing implementation
