# Local Kubernetes: Full Helm Deployment

Deploy the entire aeterna stack (services + infrastructure) into local Kubernetes using Helm.

Best for: testing the full deployment, validating Helm chart changes, or running without a Rust toolchain.

## Prerequisites

- **Rancher Desktop** (or any local K8s): <https://rancherdesktop.io/>
  - Use **moby** (dockerd) as the container engine — images built with `docker build` are automatically visible to k3s
- **Helm 3**: `brew install helm`
- **kubectl**: bundled with Rancher Desktop
- **Docker CLI**: for building images
- **An OpenAI-compatible LLM endpoint** (for memory operations): any server exposing `/v1/embeddings` and `/v1/chat/completions`, reachable from inside the cluster

## 1. Install the CNPG Operator

The chart uses CloudNativePG for PostgreSQL. The CNPG operator **must be installed before** the aeterna chart because the chart's migration hook depends on a secret that the operator creates.

```bash
# Add repo and install (one-time)
helm repo add cnpg https://cloudnative-pg.github.io/charts
helm install cnpg-operator cnpg/cloudnative-pg \
  --namespace cnpg-system --create-namespace

# Wait for operator readiness
kubectl wait --for=condition=available \
  deployment/cnpg-operator-cloudnative-pg \
  -n cnpg-system --timeout=60s
```

## 2. Build the Aeterna Image

The Helm chart deploys the `agent-a2a` binary as the aeterna server. Build a container image and make it available to k3s.

### Docker build (needs 16GB+ RAM for Docker daemon)

```bash
docker build -t aeterna:local .
```

> **Note**: With Rancher Desktop using the **moby** engine, images built with `docker build` are automatically visible to k3s. No need for `nerdctl load` or `docker save`. If you use the **containerd** engine instead, you must load images manually:
> ```bash
> docker save aeterna:local | nerdctl --namespace k8s.io load
> ```

## 3. Configure Values

Start from the provided local example:

```bash
cp charts/aeterna/examples/values-local.yaml my-values.yaml
```

The defaults deploy aeterna + PostgreSQL (CNPG) + Qdrant with no cache and no OPAL. Key settings to review:

```yaml
# my-values.yaml — only override what you need

aeterna:
  image:
    repository: aeterna
    tag: local
    pullPolicy: Never           # Use locally-built image

# LLM configuration (for memory operations)
# Set these when you have an LLM endpoint available:
#   aeterna:
#     env:
#       EMBEDDING_API_BASE: "http://host.k3s.internal:11434/v1"
#       EMBEDDING_MODEL: "nomic-embed-text"
#       LLM_API_BASE: "http://host.k3s.internal:11434/v1"
#       LLM_MODEL: "llama3"
```

> **Note**: Use `host.k3s.internal` (k3s) or `host.docker.internal` (Docker Desktop) to reach services running on your host machine from inside the cluster.

## 4. Deploy

```bash
# Add Helm repos (first time only)
helm repo add qdrant https://qdrant.github.io/qdrant-helm
helm repo update

# Update chart dependencies
helm dependency update charts/aeterna

# Install (--no-hooks skips migration job that runs before CNPG provisions the database)
helm install aeterna charts/aeterna \
  -f my-values.yaml \
  -n aeterna --create-namespace \
  --no-hooks
```

Wait for all pods:

```bash
kubectl get pods -n aeterna -w
```

Expected pods (all should reach `Running 1/1`):
- `aeterna-*` — the agent-a2a server
- `aeterna-cnpg-1` — PostgreSQL (provisioned by CNPG operator)
- `aeterna-qdrant-0` — Qdrant vector store
- `aeterna-cnpg-1-initdb-*` — init job, should show `Completed`

This typically takes 1-2 minutes. The CNPG cluster status can be checked with:

```bash
kubectl get clusters.postgresql.cnpg.io -n aeterna
```

## 5. Verify

```bash
# Port-forward the aeterna server
kubectl port-forward -n aeterna svc/aeterna 8080:8080 &

# Health check
curl http://localhost:8080/health
# OK

# Agent card
curl -s http://localhost:8080/.well-known/agent.json | python3 -m json.tool

# Metrics
curl http://localhost:8080/metrics

# Kill port-forward
kill %1
```

For detailed entrypoint testing (including memory operations), see [Testing Guide](testing-guide.md).

## 6. Upgrade / Iterate

After chart or config changes:

```bash
helm upgrade aeterna charts/aeterna \
  -f my-values.yaml \
  -n aeterna --no-hooks
```

After rebuilding the aeterna image:

```bash
# Rebuild image (see step 2), then restart pods
kubectl rollout restart deployment/aeterna -n aeterna
```

## 7. Teardown

```bash
helm uninstall aeterna -n aeterna
kubectl delete namespace aeterna

# Optional: remove CNPG operator
helm uninstall cnpg-operator -n cnpg-system
kubectl delete namespace cnpg-system
```

## Helm Values Reference

| Key | Description | Default |
|-----|-------------|---------|
| `aeterna.enabled` | Deploy aeterna server | `true` |
| `aeterna.image.repository` | Container image | `ghcr.io/kikokikok/aeterna` |
| `aeterna.image.tag` | Image tag | appVersion |
| `aeterna.image.pullPolicy` | Pull policy | `IfNotPresent` |
| `aeterna.replicaCount` | Replica count | `1` |
| `postgresql.bundled` | Deploy PostgreSQL via CNPG | `true` |
| `vectorBackend.type` | Vector store backend | `qdrant` |
| `vectorBackend.qdrant.bundled` | Deploy Qdrant | `true` |
| `cache.type` | Cache backend (`valkey`/`external`) | `valkey` |
| `cache.valkey.bundled` | Deploy Valkey | `false` |
| `opal.enabled` | Deploy OPAL auth stack | `false` |
| `llm.provider` | LLM provider (none/openai/anthropic/ollama) | `none` |
| `cnpg.enabled` | Install CNPG operator as subchart | `false` |

For the full schema, see `charts/aeterna/values.yaml` or `charts/aeterna/values.schema.json`.

See also: [Environment Variables Reference](environment-variables.md)

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| Image pull `ErrImageNeverPull` | Image not in local Docker | Run `docker build -t aeterna:local ...` (see step 2) |
| `ErrImageNeverPull` with containerd engine | Image not loaded into k3s | Run `docker save aeterna:local \| nerdctl --namespace k8s.io load` |
| Migration pod `CreateContainerConfigError` | CNPG secret not created yet | Install CNPG operator first (step 1), use `--no-hooks` |
| CNPG cluster stuck provisioning | CNPG operator not running | Check `kubectl get pods -n cnpg-system` |
| `EMBEDDING_API_BASE must be set` | Missing env config in values | Add `aeterna.env.EMBEDDING_API_BASE` to your values file |
| Pods OOMKilled | Resource limits too low | Increase limits in values file |
| LLM endpoint unreachable from pod | Host networking | Use `host.k3s.internal` or `host.docker.internal` |
| `shared_preload_libraries` CNPG error | Fixed parameter set in values | Remove it from `postgresql.cloudnativepg.postgresql.parameters` |
| Dragonfly CRD not found | Dragonfly template enabled | Set `cache.dragonfly.enabled: false` under `cache:` (not top-level) |
