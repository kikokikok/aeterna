# Local Mode Quick Start

This guide helps you set up Aeterna on a local Kubernetes cluster for development and testing.

## Prerequisites

- Kubernetes 1.25+ (minikube, kind, k3s, Docker Desktop)
- Helm 3.10+
- kubectl configured
- At least 4GB RAM available for the cluster
- Storage provisioner for PersistentVolumeClaims (PVCs)

## Step 1: Create Cluster

Choose your preferred local Kubernetes tool:

### Minikube
```bash
minikube start --memory 8192 --cpus 4 --addons=storage-provisioner,default-storageclass
```

### Kind
```bash
cat <<EOF | kind create cluster --config=-
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  extraPortMappings:
  - containerPort: 30080
    hostPort: 8080
EOF
```

### K3s (K3d)
```bash
k3d cluster create aeterna-dev --port "8080:80@loadbalancer"
```

## Step 2: Install Aeterna

Add the Helm repository or use the local source:

### From Repo
```bash
helm repo add aeterna https://kikokikok.github.io/aeterna
helm repo update
helm install aeterna aeterna/aeterna -f examples/values-local.yaml
```

### From Source
```bash
helm install aeterna ./charts/aeterna -f charts/aeterna/examples/values-local.yaml
```

## Step 3: Verify Installation

Check if the pods are running:
```bash
kubectl get pods -l app.kubernetes.io/instance=aeterna
```

Run Helm tests to verify connectivity:
```bash
helm test aeterna
```

## Step 4: Configure LLM Provider

Aeterna requires an LLM provider to function. Create a secret with your API key:

### OpenAI
```bash
kubectl create secret generic aeterna-llm \
  --from-literal=OPENAI_API_KEY=your-api-key-here
```

### Anthropic
```bash
kubectl create secret generic aeterna-llm \
  --from-literal=ANTHROPIC_API_KEY=your-api-key-here
```

Upgrade the installation to use the secret:
```bash
helm upgrade aeterna aeterna/aeterna \
  --reuse-values \
  --set aeterna.secrets.llm=aeterna-llm
```

## Step 5: Access Aeterna

Port-forward the server to your local machine:
```bash
kubectl port-forward svc/aeterna-server 8080:8080
```

Test the health endpoint:
```bash
curl http://localhost:8080/health
```

## Step 6: Enable Code Search (Optional)

Enable the codesearch sidecar for vector-based search:
```bash
helm upgrade aeterna aeterna/aeterna \
  --reuse-values \
  --set codesearch.enabled=true
```

## What's Included

| Component | Purpose | Replicas |
|-----------|---------|----------|
| Aeterna Server | Core API and logic | 1 |
| PostgreSQL (CNPG) | Metadata storage | 1 |
| Qdrant | Vector database | 1 |
| Dragonfly | In-memory cache | 1 |
| OPAL Stack | Policy orchestration | 3 (server, fetcher, agent) |

## Resource Usage

Typical resource consumption for local mode:
- **CPU**: 1-2 cores (idle)
- **Memory**: ~2.5GB total
- **Storage**: ~5GB for PVCs

## Next Steps

- [Hybrid Mode Guide](hybrid-mode.md): Offload databases to cloud services.
- [Production Checklist](production-checklist.md): Prepare for a real deployment.
- [Security Guide](security.md): Configure TLS and auth.

## Troubleshooting

### ImagePullBackOff
Check your internet connection or if the registry is reachable.
```bash
kubectl describe pod <pod-name>
```

### PVC Pending
Ensure your cluster has a default storage class.
```bash
kubectl get storageclass
```

### OOMKilled
Increase the memory allocated to your local cluster (Minikube/Docker Desktop).

### Pod Not Ready
Check logs for specific errors:
```bash
kubectl logs <pod-name>
```
