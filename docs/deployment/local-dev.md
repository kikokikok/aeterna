# Local Development: K8s Dependencies + Bare Metal Services

Run infrastructure (PostgreSQL, Qdrant, Dragonfly/Redis) in local Kubernetes via Helm, while running aeterna services directly with `cargo run` on your host machine.

Best for: active development with fast iteration (no image rebuilds needed).

## Prerequisites

- **Rancher Desktop** (or any local K8s): <https://rancherdesktop.io/>
- **Helm 3**: `brew install helm`
- **kubectl**: bundled with Rancher Desktop
- **Rust toolchain**: `rustup` with stable channel
- **An OpenAI-compatible LLM endpoint**: any server exposing `/v1/embeddings` and `/v1/chat/completions` (e.g., Ollama, LM Studio, vLLM, llama.cpp, or OpenAI itself)

## 1. Deploy Infrastructure

```bash
# Add required Helm repos (first time only)
helm repo add cnpg https://cloudnative-pg.github.io/charts
helm repo add qdrant https://qdrant.github.io/qdrant-helm
helm repo update

# Install dependencies using the dev values file
helm install aeterna charts/aeterna \
  -f charts/aeterna/examples/values-dev.yaml \
  -n aeterna --create-namespace
```

Wait for all pods to become ready:

```bash
kubectl get pods -n aeterna -w
```

## 2. Port-Forward Infrastructure Services

In separate terminals (or use `&` to background):

```bash
# PostgreSQL
kubectl port-forward -n aeterna svc/aeterna-postgresql 5432:5432 &

# Qdrant (gRPC for the Rust client)
kubectl port-forward -n aeterna svc/aeterna-qdrant 6334:6334 &

# Dragonfly (Redis-compatible)
kubectl port-forward -n aeterna svc/aeterna-dragonfly 6379:6379 &
```

> **Tip**: Use a tool like [kubefwd](https://github.com/txn2/kubefwd) to forward all services at once.

## 3. Configure Environment

Export the required environment variables. Adjust the LLM endpoint to match your setup:

```bash
# --- Infrastructure (pointing to port-forwarded K8s services) ---
export DATABASE_URL="postgresql://aeterna:aeterna@localhost:5432/aeterna"
export QDRANT_URL="http://localhost:6334"
export QDRANT_COLLECTION="aeterna_memories"
export RD_URL="redis://localhost:6379"

# --- LLM / Embedding (your OpenAI-compatible endpoint) ---
export EMBEDDING_API_BASE="http://localhost:11434/v1"   # Example: Ollama
export EMBEDDING_API_KEY="not-needed"                    # Most local servers ignore this
export EMBEDDING_MODEL="nomic-embed-text"
export EMBEDDING_DIMENSION="768"

export LLM_API_BASE="http://localhost:11434/v1"          # Same or different endpoint
export LLM_API_KEY="not-needed"
export LLM_MODEL="llama3"

# --- Context ---
export AETERNA_TENANT_ID="local-tenant"
export AETERNA_USER_ID="local-user"
export RUST_LOG="debug"
```

Or create a `.env` file and use `source .env` before running commands.

## 4. Run Services

### Agent A2A server

```bash
cargo run --bin agent-a2a
# Listening on http://0.0.0.0:8080
```

Verify:
```bash
curl http://localhost:8080/health
# OK
curl http://localhost:8080/.well-known/agent.json
# Agent card JSON
```

### CLI

```bash
# List memories
cargo run --bin aeterna -- memory list --layer user

# Add a memory
cargo run --bin aeterna -- memory add --layer user --content "Test memory"

# Search
cargo run --bin aeterna -- memory search --layer user --query "test"
```

## 5. Teardown

```bash
helm uninstall aeterna -n aeterna
kubectl delete namespace aeterna
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `connection refused` on 5432/6334/6379 | Port-forward not running | Restart `kubectl port-forward` |
| `EMBEDDING_API_BASE must be set` | Missing env var | Export all required env vars (see step 3) |
| `embedding API error` | LLM endpoint down or wrong model name | Verify endpoint: `curl $EMBEDDING_API_BASE/models` |
| Pods stuck in `Pending` | Insufficient cluster resources | Check `kubectl describe pod -n aeterna <pod>` |
| PG connection fails with auth error | Wrong credentials | Check CNPG secret: `kubectl get secret -n aeterna` |

See also: [Environment Variables Reference](environment-variables.md)
