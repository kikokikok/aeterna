## Context

The deployment spec was written aspirationally before the system was built. The actual
architecture that emerged is simpler: two Rust binaries (CLI + HTTP service) with three
backing stores (PostgreSQL, Qdrant, Redis) and an external LLM/embedding API. This design
document captures the architectural decisions made during the initial deployment work.

## Goals / Non-Goals

- **Goals**:
  - Document the actual deployment architecture
  - Define two local deployment modes (Helm deps + cargo run, full Helm K8s) plus cloud guidance
  - Establish the cross-compilation build pipeline for low-memory environments
  - Provide reproducible deployment procedures

- **Non-Goals**:
  - High-availability configurations (single-replica is sufficient for now)
  - Multi-region or federation patterns
  - CI/CD pipeline automation (manual deployment is acceptable at this stage)
  - Service mesh or advanced networking

## Decisions

### Architecture: Two binaries, not microservices

The system runs as two binaries:
1. `aeterna` — CLI tool that connects directly to backends (Qdrant, PostgreSQL, LLM API)
2. `agent-a2a` — HTTP service exposing A2A protocol endpoints

**Why**: The codebase is a single Cargo workspace. Splitting into microservices adds
operational overhead without benefit at current scale. The CLI uses a "direct backend"
pattern — it talks to Qdrant and LLM APIs directly without an intermediary service.

**Alternatives considered**: Dedicated memory-service and knowledge-service (as in the
original spec). Rejected because they don't exist in the codebase and would add unnecessary
network hops for a single-user system.

### Build: Cross-compilation with cargo-zigbuild

Docker multi-stage builds with `cargo build` inside the container require 6-8GB RAM for Rust
compilation. On memory-constrained VMs (e.g., Rancher Desktop with 8GB), this OOMs the Docker
daemon. Instead:
1. Cross-compile on the host: `cargo zigbuild --target aarch64-unknown-linux-musl --release`
2. Copy the static binary into a minimal Alpine image (no Rust toolchain needed)

**Result**: 22MB CLI binary, 5.6MB agent-a2a binary. Alpine images are 24-35MB total.

**Alternatives considered**: Docker BuildKit with memory limits (still OOMs), distroless
images (larger, less debuggable), nix builds (too complex for the benefit).

### TLS: rustls instead of native-tls

Switched all TLS dependencies from `native-tls` (OpenSSL) to `rustls` (pure Rust). This is
required for static musl cross-compilation — OpenSSL cannot be statically linked without
significant effort. `rustls` compiles cleanly to a static binary with no system dependencies.

### Image distribution: nerdctl load

K3s uses containerd, not Docker daemon. Images built with `docker build` are not visible to
k3s pods. The solution is `nerdctl --namespace k8s.io load` to import Docker-saved tarballs
into the k3s containerd namespace.

### External LLM: Any OpenAI-compatible endpoint

Embedding and LLM inference are provided by an external OpenAI-compatible API (e.g., Ollama,
LM Studio, vLLM, llama.cpp, or OpenAI itself). This keeps GPU workloads off the K8s cluster
and allows model swapping without redeployment. The endpoint is configured via
`EMBEDDING_API_BASE` and `LLM_API_BASE` environment variables — no provider is hardcoded.

## Risks / Trade-offs

- **Single replica**: No HA for any service. Acceptable for development; cloud tier should
  add replica counts and PDBs.
- **External LLM dependency**: If the LLM endpoint is unreachable, all memory operations fail.
  Mitigation: graceful error handling with actionable messages (implemented).
- **No secrets management**: Dev credentials in ConfigMap plaintext. Cloud tier must use
  K8s Secrets + external secrets operator.
- **No backup automation**: PostgreSQL and Qdrant PVCs are not backed up. Cloud tier should
  use managed database snapshots.

## Open Questions

- Should the cloud tier use managed Qdrant (Qdrant Cloud) or self-hosted?
- What CI/CD system will build and push images to a container registry?
- Should we support ARM64 and AMD64, or ARM64-only for now?
