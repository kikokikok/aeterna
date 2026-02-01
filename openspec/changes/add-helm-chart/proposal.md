# Change: Helm Chart + CLI Setup Wizard for Kubernetes Deployment

## Why

The Aeterna Memory-Knowledge system requires a standardized, production-ready deployment mechanism for Kubernetes environments. Teams need flexibility to either deploy dependencies (Redis, PostgreSQL, Qdrant) as part of the chart or reference existing infrastructure.

**Current gaps:**
1. Only OPAL authorization stack has a Helm chart (`deploy/helm/aeterna-opal/`)
2. No Helm chart for Aeterna core services (Memory, Knowledge, MCP, CCA agents)
3. No interactive setup wizard - users must manually craft values.yaml
4. Bitnami charts are now closed-source with payment requirements
5. No unified configuration flow for local dev, Kubernetes, and OpenCode

## What Changes

### 1. CLI Setup Wizard (`aeterna setup`)

Interactive command-line wizard that:
- Asks deployment questions (target, backends, features)
- Generates `values.yaml` for Helm deployments
- Generates `docker-compose.yaml` for local development
- Configures OpenCode MCP integration
- Validates configurations before generation
- Supports non-interactive mode via flags/env vars

### 2. Unified Helm Chart (`charts/aeterna/`)

Single Helm chart with optional subcharts:
- **Aeterna Core**: Memory service, Knowledge service, MCP server, CCA agents
- **PostgreSQL**: CloudNativePG operator (Apache-2.0, CNCF project)
- **Redis Alternative**: Dragonfly (Apache-2.0) or Valkey (BSD-3, official Redis fork)
- **Qdrant**: Official chart (Apache-2.0)
- **OPAL Stack**: Merged from existing `deploy/helm/aeterna-opal/`

### 3. Open-Source Dependencies Only

Replace Bitnami charts with community/official alternatives:

| Component | Old (Bitnami) | New (Open-Source) | License |
|-----------|---------------|-------------------|---------|
| PostgreSQL | bitnami/postgresql | CloudNativePG | Apache-2.0 |
| Redis | bitnami/redis | Dragonfly / Valkey | Apache-2.0 / BSD-3 |
| Qdrant | N/A | qdrant/qdrant | Apache-2.0 |
| MongoDB | bitnami/mongodb | Percona Operator | Apache-2.0 |
| Weaviate | N/A | weaviate/weaviate | BSD-3 |

### 4. Multi-Target Configuration

Single source of truth generates configs for multiple targets:

```
aeterna setup
    │
    ├── values.yaml          (Kubernetes/Helm)
    ├── docker-compose.yaml  (Local development)
    ├── .aeterna/config.toml (Runtime configuration)
    └── ~/.config/opencode/mcp.json (OpenCode integration)
```

## Impact

- Affected specs: New `helm-deployment` capability, extends `configuration`
- Affected code: New `cli/` crate, new `charts/aeterna/` directory
- Dependencies: CloudNativePG, Dragonfly/Valkey, Qdrant, OPAL (all Apache 2.0 / open source)
- Existing `deploy/helm/aeterna-opal/` will be merged into main chart as subchart

## User Experience

### Interactive Mode

```bash
$ aeterna setup

Welcome to Aeterna Setup Wizard!

? Deployment target:
  > Local development (Docker Compose)
    Kubernetes (Helm chart)
    OpenCode configuration only

? Vector database backend:
  > Qdrant (default, self-hosted)
    pgvector (PostgreSQL extension)
    Pinecone (managed cloud)
    Weaviate (hybrid search)
    MongoDB Atlas (managed)

? Redis-compatible cache:
  > Dragonfly (recommended, 5x faster)
    Valkey (official Redis fork)
    External Redis (bring your own)

? PostgreSQL deployment:
  > CloudNativePG (production operator)
    External PostgreSQL (bring your own)

? Enable OPAL authorization stack?
  > Yes (recommended for multi-tenant)
    No (single-tenant mode)

? LLM provider for embeddings:
  > OpenAI (text-embedding-3-small)
    Ollama (local, no API key)
    Skip (configure later)

? Enable OpenCode integration?
  > Yes (configure MCP tools)
    No

Generated files:
  + values.yaml
  + docker-compose.yaml
  + .aeterna/config.toml
  + ~/.config/opencode/mcp.json

Next steps:
  # For local development:
  docker compose up -d

  # For Kubernetes:
  helm install aeterna ./charts/aeterna -f values.yaml

  # Verify installation:
  aeterna status
```

### Non-Interactive Mode

```bash
# CI/CD or scripted deployments
aeterna setup \
  --target kubernetes \
  --vector-backend qdrant \
  --cache dragonfly \
  --postgresql cloudnative-pg \
  --opal enabled \
  --llm openai \
  --opencode enabled \
  --output ./deploy/
```

### Reconfiguration

```bash
# Update existing configuration
aeterna setup --reconfigure

# Validate current configuration
aeterna setup --validate

# Show current configuration
aeterna setup --show
```

## Architecture

```
charts/aeterna/
├── Chart.yaml
├── values.yaml                    # Full schema with defaults
├── values.schema.json             # JSON Schema for IDE validation
├── templates/
│   ├── _helpers.tpl
│   ├── aeterna/
│   │   ├── deployment.yaml        # Memory, Knowledge, MCP, CCA
│   │   ├── service.yaml
│   │   ├── configmap.yaml
│   │   ├── secret.yaml
│   │   ├── hpa.yaml
│   │   ├── pdb.yaml
│   │   ├── ingress.yaml
│   │   ├── serviceaccount.yaml
│   │   ├── rbac.yaml
│   │   ├── networkpolicy.yaml
│   │   └── servicemonitor.yaml
│   ├── opal/                      # Merged from aeterna-opal
│   │   ├── opal-server.yaml
│   │   ├── cedar-agent.yaml
│   │   └── opal-fetcher.yaml
│   └── NOTES.txt
└── charts/                        # Optional subcharts
    ├── cloudnative-pg/
    ├── dragonfly/
    ├── valkey/
    ├── qdrant/
    ├── weaviate/
    └── percona-mongodb/
```

## Success Criteria

1. `aeterna setup` generates valid configs for all deployment targets
2. Helm chart deploys complete Aeterna stack with single command
3. Zero Bitnami dependencies - all open-source alternatives
4. OpenCode integration works out-of-the-box after setup
5. Local development requires only `docker compose up -d`
6. 80% test coverage on CLI and Helm templates
