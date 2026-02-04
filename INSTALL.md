# Code Search Repository Management: Installation & Deployment Guide

This guide provides a comprehensive walkthrough for deploying the Code Search Repository Management system, including scaling, governance, and security configurations.

---

## 🏗 System Architecture

The Code Search Repository Management system follows a distributed, multi-tenant architecture designed for Kubernetes:

1.  **RepoManager**: Core service handling the repository lifecycle (Request → Clone → Index).
2.  **ShardRouter**: Ensures data locality by routing repository operations to specific "Owner" pods.
3.  **PolicyEvaluator**: Uses the Cedar Policy Engine for fine-grained access control (PBAC).
4.  **Identity Store**: Securely manages Git credentials (PATs, OAuth tokens) using AWS Secrets Manager or HashiCorp Vault.
5.  **Cold Storage**: Archives inactive repositories to S3/GCS as Git bundles.

---

## 🚀 Quick Start (Local Development)

### 1. Prerequisites
- **PostgreSQL 15+**
- **Redis 7+**
- **Rust (Nightly)**
- **Git**

### 2. Setup Environment
```bash
# Clone the repository
git clone https://github.com/aeterna/aeterna.git
cd aeterna

# Set up environment variables
export DATABASE_URL="postgres://user:pass@localhost:5432/aeterna"
export REDIS_URL="redis://localhost:6379/0"
export CODE_SEARCH_BASE_PATH="/tmp/code-search-repos"
```

### 3. Run Migrations
```bash
# Apply migrations using sqlx
sqlx migrate run --source storage/migrations
```

### 4. Build and Run
```bash
cargo build -p storage
cargo run -p storage --example simple_repo_manager
```

---

## ☸️ Kubernetes Production Deployment

For production, we use a **StatefulSet** for indexer pods and a **Service Mesh** or **Ingress Controller** for affinity routing.

### 1. Database Schema
Ensure the `code_search_indexer_shards` and `codesearch_repositories` (internal name) tables are initialized.

### 2. Identity Management & Secrets
Code Search integrates with external secret managers. Configure your provider:

**HashiCorp Vault Setup:**
```yaml
env:
  - name: VAULT_ADDR
    value: "https://vault.internal:8200"
  - name: VAULT_TOKEN
    valueFrom:
      secretKeyRef:
        name: vault-credentials
        key: token
```

### 3. Distributed Indexing (StatefulSet)
Use the `SHARD_ID` environment variable to identify pods.

```yaml
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: code-search-indexer
spec:
  serviceName: code-search-indexer
  replicas: 5
  template:
    spec:
      containers:
        - name: indexer
          image: aeterna/code-search-indexer:latest
          env:
            - name: SHARD_ID
              valueFrom:
                fieldRef:
                  fieldPath: metadata.name
            - name: POD_IP
              valueFrom:
                fieldRef:
                  fieldPath: status.podIP
          lifecycle:
            preStop:
              exec:
                # Trigger clean backup before shutdown
                command: ["/bin/sh", "-c", "curl -X POST http://localhost:8080/internal/shutdown"]
```

### 4. Ingress & Affinity Routing
To ensure requests for a repository hit the correct pod, configure your Ingress to hash based on the `X-Repo-ID` header.

**NGINX Ingress Example:**
```yaml
annotations:
  nginx.ingress.kubernetes.io/upstream-hash-by: "$http_x_repo_id"
```

---

## 🔐 Governance & Policies

Code Search uses **Cedar** for access control. Policies are defined in `storage/policies/repo_management.cedar`.

### Example Policy: Lead Approval
```cedar
// Only users with 'lead' role can approve repository requests
permit (
    principal in Aeterna::Role::"lead",
    action == CodeSearch::Action::"ApproveRepository",
    resource is CodeSearch::Request
);
```

### Customizing Policies:
You can update the policies at runtime if using an **OPAL**-style sync or by updating the `PolicyEvaluator` configuration.

---

## 🛠 Operation & Maintenance

### 1. Rebalancing Shards
If you add new indexer pods, the load won't automatically shift. Run a rebalancing job:

```bash
# Triggers reassignment of repos from unhealthy/overloaded shards
aeterna code-search shard rebalance
```

### 2. Cold Storage Archive
Repositories not searched for 30 days are automatically archived to S3. To manually archive a repo:

```bash
aeterna code-search repo archive --id <repo-uuid>
```

### 3. Monitoring
Code Search exposes metrics via Prometheus:
- `code_search_indexer_shards_active`: Number of healthy pods.
- `code_search_repo_indexing_duration_seconds`: Time taken for incremental indexing.
- `code_search_usage_metrics_total`: Search/Trace counters per tenant.

---

## ❓ FAQ

**Q: What happens if a pod dies suddenly (SIGKILL)?**
A: The consistent hashing algorithm will detect the shard as offline (via heartbeat timeout). Subsequent requests will trigger a "Cold Restore" from S3 onto a new healthy shard.

**Q: Can I use local storage instead of S3?**
A: Yes, for single-node deployments, set the `ColdStorageProvider` to `Local` and use a persistent volume.

---

## 📧 Support & Documentation
- **Specs**: See `openspec/changes/add-codesearch-repo-management/`
- **Architecture**: `docs/distributed-indexing.md`

---

## ❓ FAQ

**Q: What happens if a pod dies suddenly (SIGKILL)?**
A: The consistent hashing algorithm will detect the shard as offline (via heartbeat timeout). Subsequent requests will trigger a "Cold Restore" from S3 onto a new healthy shard.

**Q: Can I use local storage instead of S3?**
A: Yes, for single-node deployments, set the `ColdStorageProvider` to `Local` and use a persistent volume.

---

## 📧 Support & Documentation
- **Specs**: See `openspec/changes/add-codesearch-repo-management/`
- **Architecture**: `docs/distributed-indexing.md`
