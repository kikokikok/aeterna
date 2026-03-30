# Secrets & Environment Variables Reference

Complete inventory of all secrets, credentials, and environment variables required to deploy and operate Aeterna.

:::tip
Never store credentials in `values.yaml`. Use `existingSecret` references, [SOPS encryption](./sops-secrets.md), or [External Secrets Operator](./external-secrets.md).
:::

## Kubernetes Secrets

### Core Infrastructure

| Secret Purpose | Values Key | K8s Secret Key(s) | Required | Notes |
|---|---|---|---|---|
| PostgreSQL credentials | `postgresql.external.existingSecret` | `postgres-password`, `username`, `host`, `port` | **Yes** | Auto-created by CNPG when bundled; required for external PG |
| Redis/Dragonfly password | `cache.external.existingSecret` | `redis-password` | When using external Redis | Not needed with bundled Dragonfly |
| OPAL master/client tokens | Auto-generated on first install | `OPAL_AUTH_MASTER_TOKEN`, `OPAL_AUTH_CLIENT_TOKEN` | **Yes** (auto) | Generated at first `helm install`; persist the secret for upgrades |
| OPAL Redis password | `opal.server.redis.existingSecret` | `redis-password` | When OPAL uses external Redis | Not needed when sharing bundled Dragonfly |
| OPAL Policy Repo SSH key | `opal.server.policyRepo.sshKey.existingSecret` | `ssh-key` | When Cedar policies are in a private Git repo | Only if policy repo requires SSH auth |

### LLM Provider Credentials

| Provider | Values Key | K8s Secret Key | Required |
|---|---|---|---|
| OpenAI | `llm.openai.existingSecret` | `api-key` | When `llm.provider: openai` |
| Anthropic | `llm.anthropic.existingSecret` | `api-key` | When `llm.provider: anthropic` |
| Google Vertex AI | `llm.google.existingSecret` | Mounted as `credentials.json` (ADC) | When `llm.provider: google` |
| AWS Bedrock | N/A (IAM role) | N/A | When `llm.provider: bedrock` — uses pod IAM role or IRSA |
| Ollama | N/A | N/A | Local, no auth needed |

**Create example (OpenAI):**
```bash
kubectl create secret generic aeterna-llm-openai \
  --from-literal=api-key='sk-...'
```

**Create example (Google Vertex AI):**
```bash
kubectl create secret generic aeterna-google-llm \
  --from-file=credentials.json=/path/to/service-account.json
```

### Vector Backend Credentials

| Backend | Values Key | K8s Secret Key | Required |
|---|---|---|---|
| Qdrant (external) | `vectorBackend.qdrant.external.existingSecret` | `api-key` | When using external Qdrant |
| Pinecone | `vectorBackend.pinecone.existingSecret` | `api-key` | When `vectorBackend.type: pinecone` |
| pgvector | Uses PostgreSQL secret | Same as PostgreSQL | Shares PG credentials |
| Weaviate | N/A (internal) | N/A | Bundled subchart, no auth by default |
| MongoDB Atlas | N/A | N/A | Configure via connection string |
| Vertex AI Vector | Uses Google ADC | Same as Google LLM | Shares Vertex AI credentials |
| Databricks | N/A | N/A | Configure via workspace URL + PAT |

**Create example (Pinecone):**
```bash
kubectl create secret generic aeterna-pinecone \
  --from-literal=api-key='pc-...'
```

### Knowledge Repository & GitHub Integration

| Secret Purpose | Values Key | K8s Secret Key | Required |
|---|---|---|---|
| Knowledge repo SSH key | Mounted as volume | `ssh-privatekey` | When `knowledgeRepo.enabled: true` with SSH remote |
| GitHub API token | `knowledgeRepo.github.tokenSecret` | `token` | When using token-based GitHub auth |
| GitHub App PEM key | `knowledgeRepo.github.pemSecret` | `pem-key` | When using GitHub App auth (recommended) |
| Webhook secret | `knowledgeRepo.webhook.secretName` | `webhook-secret` | When receiving GitHub webhooks |

**Create example (GitHub App PEM):**
```bash
kubectl create secret generic aeterna-github-app-pem \
  --from-file=pem-key=/path/to/github-app-private-key.pem
```

**Create example (Webhook):**
```bash
kubectl create secret generic aeterna-webhook-secret \
  --from-literal=webhook-secret="$(openssl rand -hex 32)"
```

### Okta / OAuth2 Proxy

| Secret Purpose | Values Key | K8s Secret Key | Required |
|---|---|---|---|
| OAuth2 client ID | `okta.existingSecret` | `client-id` | When `okta.enabled: true` |
| OAuth2 client secret | `okta.existingSecret` | `client-secret` | When `okta.enabled: true` |
| Cookie secret | `okta.existingSecret` | `cookie-secret` | When `okta.enabled: true` |

**Create example:**
```bash
kubectl create secret generic aeterna-okta \
  --from-literal=client-id='0oa...' \
  --from-literal=client-secret='...' \
  --from-literal=cookie-secret="$(openssl rand -base64 32 | tr -- '+/' '-_')"
```

### Central Server (Hybrid/Remote Modes)

| Secret Purpose | Values Key | K8s Secret Key | Required |
|---|---|---|---|
| Central API key | `central.existingSecret` | `api-key` | When `deploymentMode: hybrid` or `remote` |

### Backup & Disaster Recovery

| Secret Purpose | Values Key | K8s Secret Key | Required |
|---|---|---|---|
| S3/GCS/Azure credentials | `backup.destination.credentialsSecret` | Provider-specific keys | When backups are enabled |

**S3 example:**
```bash
kubectl create secret generic aeterna-backup-s3 \
  --from-literal=aws-access-key-id='AKIA...' \
  --from-literal=aws-secret-access-key='...'
```

### Image Pull (Private Registries)

| Secret Purpose | Values Key | Required |
|---|---|---|
| Docker registry auth | `aeterna.imagePullSecrets[].name` | When pulling from private registries |

```bash
kubectl create secret docker-registry my-registry \
  --docker-server=ghcr.io \
  --docker-username=USERNAME \
  --docker-password=TOKEN
```

---

## Environment Variables (ConfigMap)

These are set via the ConfigMap and Helm values — no secrets involved:

| Variable | Values Key | Default | Purpose |
|---|---|---|---|
| `AETERNA_DEPLOYMENT_MODE` | `deploymentMode` | `local` | Deployment mode (local/hybrid/remote) |
| `AETERNA_LOG_LEVEL` | `observability.logging.level` | `info` | Log verbosity |
| `AETERNA_LOG_FORMAT` | `observability.logging.format` | `json` | Log format (json/text) |
| `AETERNA_LLM_PROVIDER` | `llm.provider` | `openai` | LLM backend |
| `AETERNA_VECTOR_BACKEND` | `vectorBackend.type` | `qdrant` | Vector storage backend |
| `AETERNA_POSTGRESQL_HOST` | Auto-resolved | — | PostgreSQL host |
| `AETERNA_POSTGRESQL_PORT` | Auto-resolved | `5432` | PostgreSQL port |
| `AETERNA_POSTGRESQL_DATABASE` | Auto-resolved | `aeterna` | PostgreSQL database |
| `AETERNA_REDIS_HOST` | Auto-resolved | — | Redis/Dragonfly host |
| `AETERNA_REDIS_PORT` | Auto-resolved | `6379` | Redis/Dragonfly port |
| `AETERNA_QDRANT_HOST` | Auto-resolved | — | Qdrant host |
| `AETERNA_QDRANT_PORT` | Auto-resolved | `6334` | Qdrant gRPC port |
| `AETERNA_OPAL_ENABLED` | `opal.enabled` | `true` | Enable OPAL authorization |
| `AETERNA_CEDAR_AGENT_HOST` | Auto-resolved | — | Cedar Agent service host |
| `AETERNA_CEDAR_AGENT_PORT` | — | `8180` | Cedar Agent service port |
| `AETERNA_FEATURE_CCA` | `aeterna.features.cca` | `true` | Enable CCA capabilities |
| `AETERNA_FEATURE_RADKIT` | `aeterna.features.radkit` | `false` | Enable Radkit A2A |
| `AETERNA_FEATURE_RLM` | `aeterna.features.rlm` | `true` | Enable RLM navigation |
| `AETERNA_FEATURE_REFLECTIVE` | `aeterna.features.reflective` | `true` | Enable reflective reasoning |
| `AETERNA_KNOWLEDGE_REPO_PATH` | — | `/tmp/knowledge-repo` | Local git clone path |
| `AETERNA_KNOWLEDGE_REPO_URL` | `knowledgeRepo.remoteUrl` | — | Remote knowledge repo SSH URL |
| `AETERNA_KNOWLEDGE_REPO_BRANCH` | `knowledgeRepo.branch` | `main` | Knowledge repo branch |
| `AETERNA_GITHUB_OWNER` | `knowledgeRepo.github.owner` | — | GitHub org/user for knowledge repo |
| `AETERNA_GITHUB_REPO` | `knowledgeRepo.github.repo` | — | GitHub repo name for knowledge |
| `AETERNA_GITHUB_APP_ID` | `knowledgeRepo.github.appId` | — | GitHub App ID |
| `AETERNA_GITHUB_INSTALLATION_ID` | `knowledgeRepo.github.installationId` | — | GitHub App installation ID |
| `AETERNA_GITHUB_ORG_NAME` | `githubOrgSync.orgName` | — | GitHub org to sync |
| `AETERNA_GITHUB_TEAM_FILTER` | `githubOrgSync.teamFilter` | — | Regex to filter synced teams |
| `AETERNA_GITHUB_SYNC_REPOS_AS_PROJECTS` | `githubOrgSync.syncReposAsProjects` | `false` | Map repos to projects |
| `AETERNA_TENANT_ID` | `githubOrgSync.tenantId` | `default` | Tenant ID for synced entities |
| `AETERNA_CENTRAL_URL` | `central.url` | — | Central server URL (hybrid/remote) |
| `AETERNA_CENTRAL_AUTH` | `central.auth` | — | Central auth method |

### LLM Provider-Specific Variables

| Provider | Variables |
|---|---|
| OpenAI | `AETERNA_OPENAI_MODEL`, `AETERNA_OPENAI_EMBEDDING_MODEL` |
| Anthropic | `AETERNA_ANTHROPIC_MODEL` |
| Ollama | `AETERNA_OLLAMA_HOST`, `AETERNA_OLLAMA_MODEL`, `AETERNA_OLLAMA_EMBEDDING_MODEL` |
| Google | `AETERNA_GOOGLE_PROJECT_ID`, `AETERNA_GOOGLE_LOCATION`, `AETERNA_GOOGLE_MODEL`, `AETERNA_GOOGLE_EMBEDDING_MODEL` |
| Bedrock | `AETERNA_BEDROCK_REGION`, `AETERNA_BEDROCK_MODEL`, `AETERNA_BEDROCK_EMBEDDING_MODEL` |

### Vector Backend-Specific Variables

| Backend | Variables |
|---|---|
| Pinecone | `AETERNA_PINECONE_ENVIRONMENT`, `AETERNA_PINECONE_INDEX_NAME` |
| pgvector | `AETERNA_PGVECTOR_ENABLED` |
| Weaviate | `AETERNA_WEAVIATE_HOST` |
| Vertex AI | `AETERNA_VERTEXAI_PROJECT`, `AETERNA_VERTEXAI_REGION`, `AETERNA_VERTEXAI_ENDPOINT` |
| Databricks | `AETERNA_DATABRICKS_WORKSPACE_URL`, `AETERNA_DATABRICKS_CATALOG` |

### Observability Variables

| Variable | Values Key | Default | Purpose |
|---|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `observability.tracing.endpoint` | — | OpenTelemetry collector endpoint |
| `OTEL_TRACES_SAMPLER_ARG` | `observability.tracing.samplingRatio` | `1.0` | Trace sampling ratio |
| `RUST_LOG` | `observability.logging.level` | `info` | Rust log filter |
| `LOG_FORMAT` | `observability.logging.format` | `json` | Log output format |

---

## GitHub Actions Secrets

These secrets must be configured in your GitHub repository settings under **Settings → Secrets and variables → Actions**.

### Required for CI/CD

| Secret | Workflow | Required | Purpose |
|---|---|---|---|
| `GITHUB_TOKEN` | `docker.yml`, `helm-release.yml`, `deploy-docs.yml` | Auto-provided | GitHub Actions built-in token for GHCR push, Pages deploy |
| `NPM_TOKEN` | `opencode-plugin-ci.yml` | For npm publish | npm registry auth token; only needed when publishing plugin via `plugin-v*` tag |

### Required for Codesearch Index (Optional)

| Secret | Workflow | Required | Purpose |
|---|---|---|---|
| `OPENAI_API_KEY` | `codesearch-index.yml` | When using OpenAI embeddings | Embedding model API key |
| `QDRANT_URL` | `codesearch-index.yml` | Yes | Qdrant endpoint URL for index storage |
| `AETERNA_API_KEY` | `codesearch-index.yml` | Yes | API key for Central Index webhook notifications |

---

## Quick Setup Cheat Sheet

Minimum secrets for a production deployment with OpenAI and bundled infrastructure:

```bash
# 1. LLM provider
kubectl create secret generic aeterna-llm-openai \
  --from-literal=api-key='sk-...'

# 2. GitHub App (for knowledge repo + org sync)
kubectl create secret generic aeterna-github-app-pem \
  --from-file=pem-key=/path/to/private-key.pem

# 3. Webhook secret (for GitHub webhook integration)
kubectl create secret generic aeterna-webhook-secret \
  --from-literal=webhook-secret="$(openssl rand -hex 32)"

# 4. Okta (if using interactive login)
kubectl create secret generic aeterna-okta \
  --from-literal=client-id='0oa...' \
  --from-literal=client-secret='...' \
  --from-literal=cookie-secret="$(openssl rand -base64 32 | tr -- '+/' '-_')"
```

Then reference them in your Helm values:

```yaml
llm:
  provider: openai
  openai:
    existingSecret: aeterna-llm-openai

knowledgeRepo:
  enabled: true
  github:
    pemSecret: aeterna-github-app-pem
  webhook:
    secretName: aeterna-webhook-secret

okta:
  enabled: true
  existingSecret: aeterna-okta
```

---

## See Also

- [External Secrets Operator](./external-secrets.md) — Sync secrets from AWS, Vault, or Azure
- [SOPS Encryption](./sops-secrets.md) — Encrypt values files at rest
- [Security Best Practices](./security.md) — Pod security, network policies, TLS
- [Production Checklist](./production-checklist.md) — Pre-deployment verification
