# Testing Guide

Comprehensive step-by-step guide for validating a complete aeterna deployment locally (K8s-based) before cloud deployment. Covers build verification, infrastructure validation, all CLI commands, HTTP services, authorization stack, and supporting tools.

## Prerequisites

### Required tools
- `kubectl` (K8s cluster access via kubeconfig)
- `cargo` (Rust toolchain)
- `npm` (Node.js 18+)
- `curl`, `jq`, `nc` (for HTTP testing)
- `psql` (PostgreSQL client)
- `redis-cli` (Redis client)

### Required infrastructure
- Aeterna deployed to K8s (see [local-kubernetes.md](local-kubernetes.md))
- OpenAI-compatible embedding endpoint (Ollama, LM Studio, or cloud provider)
- LLM endpoint for reasoning features (optional)

### Environment variables

Set these in your shell before running tests:

```bash
# Aeterna backend
export AETERNA_SERVER_URL="http://localhost:8080"
export AETERNA_TOKEN="test-token"
export AETERNA_USER_ID="local-user"
export AETERNA_TEAM="local-team"
export AETERNA_ORG="local-org"

# Embedding API (required for memory operations)
export EMBEDDING_API_BASE="http://localhost:11434/v1"  # Ollama example
export EMBEDDING_MODEL="nomic-embed-text"  # or your model
export EMBEDDING_DIMENSION="768"

# LLM API (optional, for reasoning)
export LLM_API_BASE="http://localhost:11434/v1"
export LLM_API_KEY="not-needed"

# PostgreSQL (for direct DB tests)
export PG_HOST="localhost"
export PG_PORT="5432"
export PG_USER="postgres"
export PG_PASSWORD="postgres"
export PG_DATABASE="aeterna"

# Redis (for cache tests)
export REDIS_URL="redis://localhost:6379"

# Qdrant (for memory operations)
export QDRANT_URL="http://localhost:6334"  # gRPC
export QDRANT_COLLECTION="aeterna_memories"

# Local context (CLI reads from .aeterna/context.toml)
export AETERNA_CONTEXT_FILE=".aeterna/context.toml"
```

### Port-forwards (from K8s cluster)

Open these in separate terminals or background processes:

```bash
# Agent-a2a HTTP service (HTTP + metrics)
kubectl port-forward svc/aeterna 8080:8080 &

# PostgreSQL (for migrations, views, direct DB tests)
kubectl port-forward svc/aeterna-postgresql 5432:5432 &

# Qdrant REST API (for direct vector store verification)
kubectl port-forward svc/aeterna-qdrant 6333:6333 &

# Qdrant gRPC (for CLI memory operations)
kubectl port-forward svc/aeterna-qdrant 6334:6334 &

# Redis (for cache tests)
kubectl port-forward svc/aeterna-redis 6379:6379 &

# OPAL Server (authorization policy distribution)
kubectl port-forward svc/opal-server 7002:7002 &

# OPAL Client/Cedar Agent (local policy decisions, Cedar API)
kubectl port-forward svc/opal-client 8180:8180 &
kubectl port-forward svc/opal-client 7000:7000 &

# OPAL Fetcher (serves Cedar entities from PostgreSQL)
kubectl port-forward svc/opal-fetcher 8085:8080 &

# pgAdmin (optional, for DB inspection)
kubectl port-forward svc/aeterna-pgadmin 5050:5050 &

# Redis Commander (optional, for cache inspection)
kubectl port-forward svc/redis-commander 8081:8081 &
```

---

## Phase 0: Build Verification

Before deploying, verify that all components build and tests pass locally.

### 0.1 Build the Rust workspace

```bash
cd /opt/workspace/git/aeterna
cargo build --workspace
```

**Expected:** All 17 crates compile without errors.

**Common issues:**
| Error | Cause | Fix |
|-------|-------|-----|
| `could not compile 'aeterna'` | Missing dependencies or outdated lockfile | `cargo update && cargo build` |
| `RUSTFLAGS` mismatch | Conflicting nightly/stable toolchain | `rustup update` |

### 0.2 Run Rust tests

```bash
cargo test --workspace
```

**Expected:** All tests pass. Some tests may skip if services (Qdrant, PostgreSQL) are unavailable.

Tests are organized by crate:
- `memory::tests` -- Memory layer operations
- `storage::tests` -- PostgreSQL, Redis, DuckDB persistence
- `knowledge::tests` -- Git-based knowledge repo
- `sync::tests` -- Memory-Knowledge sync bridge
- `tools::tests` -- MCP tool interface
- `cli::tests` -- CLI command logic

### 0.3 Check formatting and linting

```bash
cargo fmt --check
cargo clippy --workspace
```

**Expected:** No warnings or format violations.

### 0.4 Build the OpenCode plugin

```bash
cd packages/opencode-plugin
npm install
npm run build
```

**Expected:** TypeScript compiles to `dist/index.js` without errors.

### 0.5 Test the OpenCode plugin

```bash
npm test              # Single run
npm run typecheck     # Type check without emit
```

**Expected:** All vitest suites pass.

### 0.6 Build the website

```bash
cd website
npm install
npm run build
```

**Expected:** Docusaurus site builds to `build/` directory.

### 0.7 Build Docker images

```bash
# Default image (aeterna CLI)
docker build -t aeterna:local .

# Agent-a2a image (uses same Dockerfile with PACKAGE build-arg)
docker build --build-arg PACKAGE=agent-a2a -t agent-a2a:local .

# OPAL Fetcher
docker build -f opal-fetcher/Dockerfile -t aeterna-opal-fetcher:local opal-fetcher/
```

**Expected:** All images build without errors.

---

## Phase 1: Infrastructure Validation

Verify that the K8s deployment is healthy and all databases are accessible.

### 1.1 Check K8s cluster

```bash
kubectl cluster-info
kubectl get nodes
```

**Expected:** Cluster is running and at least one node is `Ready`.

### 1.2 Check Aeterna deployment

```bash
kubectl get all -l app=aeterna
kubectl get pods -l app=aeterna
```

**Expected:** All aeterna pods are `Running`. Check pod logs for startup errors:

```bash
kubectl logs -l app=aeterna -c aeterna
```

### 1.3 Verify PostgreSQL connectivity

```bash
# Via port-forward
psql -h localhost -U postgres -d aeterna -c "SELECT version();"
```

**Expected:** PostgreSQL 16+ output showing pgvector extension.

```bash
psql -h localhost -U postgres -d aeterna -c "SELECT * FROM pg_available_extensions WHERE name = 'vector';"
```

**Expected:** One row with `vector | installed` or `vector | <version>`.

### 1.4 Verify PostgreSQL migrations

```bash
psql -h localhost -U postgres -d aeterna -c "
  SELECT version, description, success FROM schema_migrations ORDER BY version;
"
```

**Expected:** All 14 migrations complete with `success = true`.

Verify key tables exist:

```bash
# Organizational units
psql -h localhost -U postgres -d aeterna -c "SELECT COUNT(*) FROM companies;"

# Memory storage
psql -h localhost -U postgres -d aeterna -c "SELECT COUNT(*) FROM memories;"

# Knowledge
psql -h localhost -U postgres -d aeterna -c "SELECT COUNT(*) FROM knowledge_items;"

# Governance
psql -h localhost -U postgres -d aeterna -c "SELECT COUNT(*) FROM governance_requests;"
```

### 1.5 Verify pgvector extension and views

```bash
psql -h localhost -U postgres -d aeterna -c "
  SELECT table_name FROM information_schema.views 
  WHERE table_schema = 'public' 
  ORDER BY table_name;
"
```

**Expected:** Views including:
- `v_hierarchy` (Company/Org/Team/Project hierarchy)
- `v_user_permissions` (User roles and team memberships)
- `v_agent_permissions` (Agent delegation chains)
- `v_code_search_repositories` (Code search repos)
- `v_code_search_requests` (Approval requests)
- `v_code_search_identities` (IdP integrations)

### 1.6 Verify Qdrant connectivity

```bash
# REST API (6333)
curl -s http://localhost:6333/health | jq .

# Expected: {"status": "ok"}

# Check collections
curl -s http://localhost:6333/collections | jq '.result.collections'
```

**Expected:** Collections exist (or will be created on first use).

### 1.7 Verify Redis connectivity

```bash
redis-cli -h localhost -p 6379 PING
```

**Expected:** `PONG`

Test basic operations:

```bash
redis-cli -h localhost -p 6379 SET test_key "hello"
redis-cli -h localhost -p 6379 GET test_key
redis-cli -h localhost -p 6379 DEL test_key
```

**Expected:** SET returns `OK`, GET returns `"hello"`, DEL returns `1`.

---

## Phase 2: Agent-a2a HTTP Service

Tests the A2A protocol HTTP server (stub implementation).

### 2.1 Health check

```bash
curl -s http://localhost:8080/health
```

**Expected:** `OK` (HTTP 200)

### 2.2 Agent card (skills discovery)

```bash
curl -s http://localhost:8080/.well-known/agent.json | jq .
```

**Expected:** JSON with agent metadata and 3 skills:

```json
{
  "name": "Aeterna A2A Agent",
  "version": "0.1.0",
  "skills": [
    {
      "name": "memory",
      "description": "Manage ephemeral memories",
      "tools": ["memory_add", "memory_search", "memory_delete"]
    },
    {
      "name": "knowledge",
      "description": "Query knowledge base",
      "tools": ["knowledge_query", "knowledge_show", "knowledge_check"]
    },
    {
      "name": "governance",
      "description": "Validate policies and check drift",
      "tools": ["governance_validate", "governance_drift_check"]
    }
  ]
}
```

### 2.3 Prometheus metrics

```bash
curl -s http://localhost:8080/metrics
```

**Expected:** Prometheus-format text containing:
- `a2a_requests_total` (counter)
- `a2a_active_connections` (gauge)
- `a2a_up` (gauge, value 1)

### 2.4 Task submission (stub)

```bash
curl -s -X POST http://localhost:8080/tasks/send \
  -H "Content-Type: application/json" \
  -d '{"task": "test"}' | jq .
```

**Expected:** HTTP 200 with stub response:

```json
{
  "status": "completed",
  "result": {}
}
```

---

## Phase 3: OPAL Authorization Stack

Tests the Open Policy Administration Layer (OPAL) that distributes Cedar authorization policies.

### 3.1 OPAL Server health

```bash
curl -s http://localhost:7002/healthcheck | jq .
```

**Expected:** HTTP 200 with `{"status": "ok"}` or similar.

### 3.2 OPAL Client/Cedar Agent health

```bash
curl -s http://localhost:7000/healthcheck | jq .
```

**Expected:** HTTP 200 with health status.

### 3.3 Cedar policy validation

```bash
# Test a simple policy
curl -s -X POST http://localhost:8180/access/allow \
  -H "Content-Type: application/json" \
  -d '{
    "principal": "User::\"alice\"",
    "action": "Action::\"read\"",
    "resource": "Document::\"doc1\""
  }' | jq .
```

**Expected:** Cedar decision (allow/deny) based on loaded policies.

---

## Phase 4: OPAL Fetcher

Tests the custom OPAL data fetcher that serves organizational hierarchy and Cedar entities from PostgreSQL.

### 4.1 Fetcher health

```bash
curl -s http://localhost:8085/health | jq .
```

**Expected:** HTTP 200 with:

```json
{
  "status": "healthy",
  "database": "connected"
}
```

### 4.2 Fetcher metrics

```bash
curl -s http://localhost:8085/metrics
```

**Expected:** Prometheus-format metrics stub.

### 4.3 Get organizational hierarchy

```bash
curl -s http://localhost:8085/v1/hierarchy | jq .
```

**Expected:** JSON `CedarEntitiesResponse` with entities:

```json
{
  "entities": [
    {
      "uid": {"type": "Aeterna::Company", "id": "..."},
      "attrs": {"slug": "acme", "name": "ACME Corp"},
      "parents": []
    },
    {
      "uid": {"type": "Aeterna::Organization", "id": "..."},
      "attrs": {"slug": "engineering", "name": "Engineering"},
      "parents": [{"type": "Aeterna::Company", "id": "..."}]
    }
  ],
  "timestamp": "2026-03-13T...",
  "count": 4
}
```

### 4.4 Get users

```bash
curl -s http://localhost:8085/v1/users | jq .
```

**Expected:** Cedar User entities with roles and team memberships.

### 4.5 Get agents

```bash
curl -s http://localhost:8085/v1/agents | jq .
```

**Expected:** Cedar Agent entities with delegation chains and capabilities.

### 4.6 Get all entities (combined)

```bash
curl -s http://localhost:8085/v1/all | jq '.count'
```

**Expected:** Count of all entities (should be larger than individual endpoints).

---

## Phase 5: CLI Commands

Tests all 19 aeterna CLI subcommands. These can be run with `cargo run -p aeterna -- <command>` or via a built Docker image.

### CLI setup

```bash
# Option A: Build and run from repo
cargo build -p aeterna --release
export AETERNA_BIN="./target/release/aeterna"

# Option B: Use cargo run directly
export AETERNA_BIN="cargo run -p aeterna --"

# Option C: Docker (if image built)
export AETERNA_BIN="docker run --rm aeterna:local"
```

Test with:

```bash
$AETERNA_BIN --version
```

**Expected:** Version output (e.g., `aeterna 0.1.0`).

### 5.1 Init

Initialize aeterna in a new directory:

```bash
mkdir -p /tmp/test-aeterna
cd /tmp/test-aeterna
$AETERNA_BIN init
```

**Expected:** Creates `.aeterna/` directory with config files and outputs:
- `.aeterna/config.toml`
- `.aeterna/context.toml`

Verify:

```bash
cat .aeterna/context.toml
```

**Expected:** TOML with `tenant_id`, `user_id`, `org_id` fields.

### 5.2 Context

Manage tenant/user context:

```bash
# Show current context
$AETERNA_BIN context show

# Expected: JSON or table with tenant, user, org, project

# Set context
$AETERNA_BIN context set --tenant my-tenant --user alice --org acme

# Clear context
$AETERNA_BIN context clear

# Show again
$AETERNA_BIN context show
```

### 5.3 Hints

List and explain operation hints (presets, toggles):

```bash
# List all hints
$AETERNA_BIN hints list --json

# Expected: Array of hint objects with name, description, type, default value

# Explain a hint
$AETERNA_BIN hints explain MEMORY_LAYER_DECOMPOSITION

# Expected: Full description, accepted values, implications

# Parse hints (advanced)
$AETERNA_BIN hints parse --hints "MEMORY_LAYER_DECOMPOSITION=5,SEMANTIC_THRESHOLD=0.8"

# Expected: JSON with parsed values
```

### 5.4 Completion

Generate shell completions:

```bash
# Bash
$AETERNA_BIN completion bash > /tmp/aeterna.bash
source /tmp/aeterna.bash

# Verify
aeterna mem<TAB>

# Expected: Completes to "aeterna memory"
```

### 5.5 Setup

Interactive deployment setup wizard:

```bash
$AETERNA_BIN setup --non-interactive --show
```

**Expected:** Outputs current configuration without prompting.

Generate a new config:

```bash
$AETERNA_BIN setup --reconfigure
```

**Expected:** Interactive wizard with prompts for:
- PostgreSQL connection
- Redis URL
- Qdrant URL
- Embedding API endpoint
- LLM API endpoint (optional)
- OPAL Server configuration

### 5.6 Status

Check system status and health:

```bash
$AETERNA_BIN status --json
```

**Expected:** JSON with status of all backends:

```json
{
  "aeterna": "ok",
  "qdrant": "ok",
  "postgres": "ok",
  "redis": "ok",
  "cedar": "ok",
  "context": {
    "tenant_id": "local-tenant",
    "user_id": "local-user"
  }
}
```

### 5.7 Memory operations

#### 5.7.1 Add a memory

```bash
$AETERNA_BIN memory add \
  "The deployment uses Helm with K8s CNPG for PostgreSQL" \
  --layer project \
  --tags "deployment,infrastructure,k8s"
```

**Expected:** Output with UUID and confirmation. Stores to Qdrant as vector.

#### 5.7.2 List memories

```bash
$AETERNA_BIN memory list --layer project --json
```

**Expected:** JSON array of memories:

```json
[
  {
    "id": "...",
    "content": "The deployment uses Helm...",
    "layer": "project",
    "created_at": "...",
    "tags": ["deployment", "infrastructure", "k8s"]
  }
]
```

#### 5.7.3 Search memories

```bash
$AETERNA_BIN memory search "Helm deployment" \
  --layer project \
  --limit 5 \
  --json
```

**Expected:** JSON array with similarity scores > 0 for matching memories.

With reasoning (requires `LLM_API_BASE`):

```bash
$AETERNA_BIN memory search "Helm deployment" \
  --layer project \
  --reasoning true \
  --json
```

**Expected:** Same results but with additional reasoning context.

#### 5.7.4 Show a memory

Using the UUID from 5.7.1:

```bash
$AETERNA_BIN memory show <uuid> --json
```

**Expected:** Full memory details including metadata, timestamps, parent/child relationships.

#### 5.7.5 Promote a memory

Move memory to a higher layer:

```bash
$AETERNA_BIN memory promote <uuid> --from project --to team --yes
```

**Expected:** Confirmation that memory moved layers.

#### 5.7.6 Delete a memory

```bash
$AETERNA_BIN memory delete <uuid> --layer team --yes
```

**Expected:** Confirmation of deletion.

### 5.8 Knowledge operations

#### 5.8.1 Search knowledge

```bash
$AETERNA_BIN knowledge search "Cedar policies" --type policy --json
```

**Expected:** JSON array of knowledge items from git repository.

**Note:** Partially implemented; may show stub data if git backend unavailable.

#### 5.8.2 Get specific knowledge

```bash
$AETERNA_BIN knowledge get --id <knowledge-id> --json
```

**Expected:** Full knowledge item with content, tags, references.

#### 5.8.3 Check knowledge constraints

```bash
$AETERNA_BIN knowledge check --id <knowledge-id> --json
```

**Expected:** JSON with constraint validation results.

#### 5.8.4 List knowledge

```bash
$AETERNA_BIN knowledge list --scope team --json
```

**Expected:** JSON array of available knowledge items.

### 5.9 Policy operations

#### 5.9.1 Create a policy

```bash
$AETERNA_BIN policy create \
  --name "allow_agent_memory_read" \
  --scope team \
  --principal "Agent" \
  --action "read" \
  --resource "Memory"
```

**Expected:** Policy created and compiled to Cedar.

#### 5.9.2 List policies

```bash
$AETERNA_BIN policy list --scope team --json
```

**Expected:** JSON array of policies.

#### 5.9.3 Explain a policy

```bash
$AETERNA_BIN policy explain --name "allow_agent_memory_read"
```

**Expected:** Human-readable explanation of policy logic.

#### 5.9.4 Validate policies

```bash
$AETERNA_BIN policy validate --scope team
```

**Expected:** Validation results; warnings if policies conflict.

#### 5.9.5 Simulate policy

```bash
$AETERNA_BIN policy simulate \
  --principal "User::alice" \
  --action "read" \
  --resource "Memory::mem123" \
  --json
```

**Expected:** JSON with decision (allow/deny) and applicable policies.

### 5.10 Organizational hierarchy

#### 5.10.1 Create organization

```bash
$AETERNA_BIN org create --name "ACME Engineering" --slug "acme-eng"
```

**Expected:** Confirmation with org UUID.

**Note:** Stub implementation; use PostgreSQL directly for actual data.

#### 5.10.2 List organizations

```bash
$AETERNA_BIN org list --json
```

**Expected:** JSON array of organizations.

#### 5.10.3 Show organization

```bash
$AETERNA_BIN org show --id <org-id> --json
```

**Expected:** Full org details including members, teams, projects.

#### 5.10.4 List organization members

```bash
$AETERNA_BIN org members --id <org-id> --json
```

**Expected:** JSON array of users in org with roles.

#### 5.10.5 Switch context (use org)

```bash
$AETERNA_BIN org use --id <org-id>
```

**Expected:** Updates context to selected org.

### 5.11 Teams

Similar pattern to orgs:

```bash
$AETERNA_BIN team create --name "Core Team" --org <org-id>
$AETERNA_BIN team list --org <org-id> --json
$AETERNA_BIN team show --id <team-id> --json
$AETERNA_BIN team members --id <team-id> --json
$AETERNA_BIN team use --id <team-id>
```

### 5.12 Users

Manage user accounts and roles:

```bash
$AETERNA_BIN user register --email alice@example.com --name "Alice"
$AETERNA_BIN user list --json
$AETERNA_BIN user show --id <user-id> --json
$AETERNA_BIN user roles --id <user-id> --json
$AETERNA_BIN user whoami --json
$AETERNA_BIN user invite --email bob@example.com --role developer
```

### 5.13 Agents

Register and manage AI agents:

```bash
$AETERNA_BIN agent register --name "CodeReview" --type "opencode" --team <team-id>
$AETERNA_BIN agent list --json
$AETERNA_BIN agent show --id <agent-id> --json
$AETERNA_BIN agent permissions --id <agent-id> --json
$AETERNA_BIN agent revoke --id <agent-id> --permission "memory:write"
```

### 5.14 Governance

Manage approval workflows and compliance:

```bash
$AETERNA_BIN govern status --json
$AETERNA_BIN govern pending --json
$AETERNA_BIN govern approve --request-id <req-id>
$AETERNA_BIN govern reject --request-id <req-id> --reason "policy conflict"
$AETERNA_BIN govern configure --policy "require_approval_for_production"
$AETERNA_BIN govern roles --json
$AETERNA_BIN govern audit --since "2026-03-01" --json
```

**Note:** Stub implementation; real governance workflows require PostgreSQL state machine.

### 5.15 Admin

System administration and maintenance:

#### 5.15.1 Health check

```bash
$AETERNA_BIN admin health --json
```

**Expected:** Detailed health of all services.

#### 5.15.2 Run migrations

```bash
$AETERNA_BIN admin migrate --direction up
```

**Expected:** Runs pending PostgreSQL migrations.

#### 5.15.3 Check drift

```bash
$AETERNA_BIN admin drift --scope memory --json
```

**Expected:** Detects inconsistencies between vector store and PostgreSQL.

#### 5.15.4 Export data

```bash
$AETERNA_BIN admin export --output /tmp/export.json
```

**Expected:** JSON dump of all data.

#### 5.15.5 Import data

```bash
$AETERNA_BIN admin import --input /tmp/export.json
```

**Expected:** Restores from export.

### 5.16 Sync

Synchronize memory and knowledge systems:

```bash
$AETERNA_BIN sync --dry-run --json
```

**Expected:** Preview of changes to sync.

```bash
$AETERNA_BIN sync --json
```

**Expected:** Executes sync and confirms completion.

### 5.17 Check

Run constraint validation checks:

```bash
$AETERNA_BIN check --scope team --json
```

**Expected:** JSON with validation results and any violations.

### 5.18 Code search

Search and analyze code with call graphs:

#### 5.18.1 Initialize code search

```bash
$AETERNA_BIN code-search init --repo <git-url> --vendor openai
```

**Expected:** Indexing begins; confirmation with status URL.

#### 5.18.2 Search code

```bash
$AETERNA_BIN code-search search "authentication handler" --json
```

**Expected:** JSON array of code locations with semantic similarity.

#### 5.18.3 Trace call graph

Callers:

```bash
$AETERNA_BIN code-search trace --function "authenticate" --direction callers --format json
```

**Expected:** JSON call graph showing all functions calling `authenticate`.

Callees:

```bash
$AETERNA_BIN code-search trace --function "authenticate" --direction callees --format mermaid
```

**Expected:** Mermaid diagram of functions called by `authenticate`.

#### 5.18.4 Check indexing status

```bash
$AETERNA_BIN code-search status --json
```

**Expected:** JSON with repositories indexed, last update time, coverage.

#### 5.18.5 Repository management

```bash
# Request indexing
$AETERNA_BIN code-search repo request --url https://github.com/example/repo

# List requests
$AETERNA_BIN code-search repo list --json

# Approve a request
$AETERNA_BIN code-search repo approve --id <request-id>

# Reject a request
$AETERNA_BIN code-search repo reject --id <request-id>
```

---

## Phase 6: OpenCode Plugin

Tests the TypeScript plugin that integrates Aeterna into OpenCode AI sessions.

### 6.1 Build the plugin

```bash
cd packages/opencode-plugin
npm run build
```

**Expected:** Compiles to `dist/index.js` without errors.

Verify entry point:

```bash
ls -lh dist/index.js
```

### 6.2 Type check

```bash
npm run typecheck
```

**Expected:** No TypeScript errors.

### 6.3 Run tests

```bash
npm test
```

**Expected:** All vitest suites pass:
- `AeternaClient` tests (HTTP communication)
- Tool tests (memory, graph, CCA, knowledge, governance)
- Hook tests (chat, system, tool execution, permissions, session)

### 6.4 Integrate into OpenCode

Add to `opencode.jsonc`:

```json
{
  "plugins": [
    "@kiko-aeterna/opencode-plugin"
  ]
}
```

Set environment variables:

```bash
export AETERNA_SERVER_URL="http://localhost:8080"
export AETERNA_TOKEN="test-token"
export AETERNA_USER_ID="local-user"
export AETERNA_TEAM="local-team"
export AETERNA_ORG="local-org"
```

### 6.5 Test in OpenCode session

Once the plugin is loaded in an OpenCode session, you should be able to:

1. **Chat with context injection** -- The `chat.message` hook prepends relevant knowledge and memories
2. **Use memory tools** -- `aeterna_memory_add`, `aeterna_memory_search`, etc.
3. **Query knowledge** -- `aeterna_knowledge_query`, `aeterna_knowledge_propose`
4. **Analyze graphs** -- `aeterna_graph_query`, `aeterna_graph_neighbors`
5. **Check governance** -- `aeterna_governance_status`, `aeterna_sync_status`

Test by asking OpenCode a question that should trigger memory search:

```
In OpenCode: "What do we know about CI/CD pipeline?"
```

**Expected:** OpenCode injects memories with tags containing "ci", "cd", "pipeline".

---

## Phase 7: Website

Tests the Docusaurus documentation site.

### 7.1 Build

```bash
cd website
npm run build
```

**Expected:** Generates static site in `build/` directory.

### 7.2 Serve locally

```bash
npm start
```

**Expected:** Starts dev server on `http://localhost:3000`

Open a browser and verify:
- Navigation works
- Deployment guide (`docs/deployment/`) is accessible
- Code snippets render correctly
- Search functions

### 7.3 Type check (if applicable)

```bash
npm run typecheck 2>/dev/null || echo "No typecheck configured"
```

---

## Phase 8: Direct Database Verification

Detailed checks for data consistency across PostgreSQL, Qdrant, and Redis.

### 8.1 PostgreSQL data

#### Check organizational hierarchy

```bash
psql -h localhost -U postgres -d aeterna -c "
  SELECT table_name, COUNT(*) as count 
  FROM information_schema.tables 
  WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
  GROUP BY table_name 
  ORDER BY table_name;
"
```

**Expected:** Tables for companies, organizations, teams, projects, users, agents, memories, knowledge, governance, etc.

#### Sample data from views

```bash
# Organizational hierarchy
psql -h localhost -U postgres -d aeterna -c "SELECT * FROM v_hierarchy LIMIT 1 \gx"

# User permissions
psql -h localhost -U postgres -d aeterna -c "SELECT * FROM v_user_permissions LIMIT 1 \gx"

# Agent permissions
psql -h localhost -U postgres -d aeterna -c "SELECT * FROM v_agent_permissions LIMIT 1 \gx"
```

### 8.2 Qdrant data

#### Get collection stats

```bash
curl -s http://localhost:6333/collections/aeterna_memories | jq '{
  status: .result.status,
  points: .result.points_count,
  vectors: .result.vectors_count,
  indexed_vectors: .result.indexed_vectors_count
}'
```

**Expected:** Positive counts for populated collections.

#### Sample stored vectors

```bash
curl -s http://localhost:6333/collections/aeterna_memories/points/scroll \
  -H "Content-Type: application/json" \
  -d '{"limit": 1, "with_payload": true, "with_vector": false}' | jq '.result.points[0]'
```

**Expected:** Payload with fields:
- `id` (UUID)
- `content` (text)
- `layer` (enum: agent, user, session, project, team, org, company)
- `tenant_id`
- `created_at`, `updated_at`
- `metadata` (tags, source, etc.)

### 8.3 Redis data

#### Check cache keys

```bash
redis-cli -h localhost -p 6379 KEYS "*" | head -20
```

**Expected:** Keys for cached items (if any operations have run).

#### Sample cache value

```bash
redis-cli -h localhost -p 6379 RANDOMKEY | xargs redis-cli -h localhost -p 6379 GET
```

#### Check distributed locks

```bash
redis-cli -h localhost -p 6379 KEYS "*:lock" 
```

**Expected:** Lock keys if long-running operations are in flight.

---

## Phase 9: Optional Dev Tools

If enabled in your Helm values, verify pgAdmin and Redis Commander.

### 9.1 pgAdmin

Access at `http://localhost:5050`

1. Login (default: `admin@example.com` / `admin`)
2. Register PostgreSQL server:
   - Host: `aeterna-postgresql` (or port-forward localhost)
   - Port: `5432`
   - User: `postgres`
   - Password: `postgres`
3. Verify you can browse databases and run queries

### 9.2 Redis Commander

Access at `http://localhost:8081`

1. Verify it connects to Redis
2. Browse keys and values
3. Execute commands (GET, SET, DEL, etc.)

---

## End-to-End Validation Checklist

Run through all phases and confirm:

| Phase | Test | Pass Criteria |
|-------|------|---------------|
| **0: Build** | `cargo build --workspace` | All crates compile |
| **0: Tests** | `cargo test --workspace` | Tests pass (or skip on unavailable services) |
| **0: Lint** | `cargo clippy && cargo fmt --check` | No warnings |
| **0: Plugin** | `npm run build && npm run test` | Builds and tests pass |
| **0: Website** | `npm run build` | Site builds to `build/` |
| **1: K8s** | `kubectl get pods` | All pods `Running` |
| **1: PostgreSQL** | `psql ... SELECT version()` | v16+ with pgvector |
| **1: Migrations** | `psql ... SELECT * FROM schema_migrations` | All 14 complete |
| **1: Qdrant** | `curl localhost:6333/health` | Status `ok` |
| **1: Redis** | `redis-cli PING` | Response `PONG` |
| **2: Agent-a2a health** | `curl localhost:8080/health` | Returns `OK` |
| **2: Agent card** | `curl localhost:8080/.well-known/agent.json` | JSON with 3 skills |
| **2: Metrics** | `curl localhost:8080/metrics` | Prometheus format |
| **2: Task stub** | `curl -X POST localhost:8080/tasks/send` | `{"status":"completed"}` |
| **3: OPAL Server** | `curl localhost:7002/healthcheck` | Health OK |
| **3: Cedar Agent** | `curl localhost:7000/healthcheck` | Health OK |
| **4: Fetcher health** | `curl localhost:8085/health` | `{"status":"healthy","database":"connected"}` |
| **4: Hierarchy** | `curl localhost:8085/v1/hierarchy` | Cedar entities |
| **4: Users** | `curl localhost:8085/v1/users` | Cedar User entities |
| **4: Agents** | `curl localhost:8085/v1/agents` | Cedar Agent entities |
| **4: All entities** | `curl localhost:8085/v1/all` | Combined response |
| **5.1: Init** | `aeterna init` | Creates `.aeterna/` |
| **5.2: Context** | `aeterna context show` | Shows tenant/user/org |
| **5.3: Hints** | `aeterna hints list` | Lists all hints |
| **5.4: Completion** | `aeterna completion bash` | Shell script output |
| **5.5: Setup** | `aeterna setup --show` | Current configuration |
| **5.6: Status** | `aeterna status` | All services healthy |
| **5.7: Memory add** | `aeterna memory add "test" --layer project` | Returns UUID |
| **5.7: Memory list** | `aeterna memory list` | Contains test memory |
| **5.7: Memory search** | `aeterna memory search "test"` | Returns result with score |
| **5.7: Memory show** | `aeterna memory show <uuid>` | Full details |
| **5.7: Memory promote** | `aeterna memory promote <uuid> --from project --to team` | Confirms promotion |
| **5.7: Memory delete** | `aeterna memory delete <uuid> --yes` | Confirms deletion |
| **5.8: Knowledge** | `aeterna knowledge search "..."` | Returns items |
| **5.9: Policy** | `aeterna policy list` | Returns policies |
| **5.10: Org** | `aeterna org list` | Returns organizations |
| **5.11: Team** | `aeterna team list` | Returns teams |
| **5.12: User** | `aeterna user list` | Returns users |
| **5.13: Agent** | `aeterna agent list` | Returns agents |
| **5.14: Govern** | `aeterna govern status` | Shows governance state |
| **5.15: Admin** | `aeterna admin health` | Health of all services |
| **5.16: Sync** | `aeterna sync --dry-run` | Preview of changes |
| **5.17: Check** | `aeterna check` | Constraint validation |
| **5.18: Code search** | `aeterna code-search search "..."` | Returns code locations |
| **6: Plugin build** | `npm run build` | `dist/index.js` created |
| **6: Plugin tests** | `npm test` | Tests pass |
| **7: Website build** | `npm run build` | `build/` directory created |
| **8: PostgreSQL views** | `psql ... v_hierarchy` | Returns rows |
| **8: Qdrant vectors** | `curl localhost:6333/collections/...` | Points stored |
| **8: Redis keys** | `redis-cli KEYS "*"` | Keys exist |

---

## Troubleshooting

### Build failures

**Symptom:** `cargo build` fails with missing crates or version conflicts

**Fix:**
```bash
rm -rf Cargo.lock target/
cargo update
cargo build --workspace
```

### Service connectivity

**Symptom:** `Connection refused` on port 8080, 6333, 5432, etc.

**Fix:**
```bash
# Verify port-forwards are active
ps aux | grep port-forward

# Restart port-forwards if needed
kubectl port-forward svc/aeterna 8080:8080 &
kubectl port-forward svc/aeterna-qdrant 6333:6333 6334:6334 &
```

### PostgreSQL migrations fail

**Symptom:** `schema_migrations` table shows `success = false`

**Fix:**
1. Check migration SQL for syntax errors
2. Verify pgvector is installed: `psql ... CREATE EXTENSION IF NOT EXISTS vector;`
3. Manually rollback and retry:
   ```bash
   psql -h localhost -U postgres -d aeterna -c "DELETE FROM schema_migrations WHERE version = 14;"
   cargo run -p aeterna -- admin migrate --direction up
   ```

### Embedding API errors

**Symptom:** `404 Not Found` or `model not found` when running `memory add`

**Fix:**
```bash
# Check available models
curl -s http://localhost:11434/api/tags | jq '.models[].name'  # Ollama
curl -s http://<host>:1234/v1/models | jq '.data[].id'        # LM Studio

# Set correct model
export EMBEDDING_MODEL="nomic-embed-text"  # or your actual model name
$AETERNA_BIN memory add "test" --layer project
```

### Empty search results

**Symptom:** `memory search` returns no results despite added memories

**Causes:**
- **Tenant mismatch** -- Verify all commands use same tenant context
  ```bash
  $AETERNA_BIN context show
  ```
- **Embedding dimension mismatch** -- Verify `EMBEDDING_DIMENSION` matches model output
  ```bash
  export EMBEDDING_DIMENSION="768"  # for nomic-embed-text
  ```
- **Collection needs recreation** -- If dimension changed after collection created
  ```bash
  curl -X DELETE http://localhost:6333/collections/aeterna_memories
  $AETERNA_BIN memory add "test" --layer project  # Auto-creates collection
  ```

### OPAL not updating Cedar policies

**Symptom:** Policy changes don't appear in Cedar Agent decisions

**Fix:**
1. Verify OPAL Server is running: `curl localhost:7002/healthcheck`
2. Verify OPAL Client is connected: Check logs with `kubectl logs -l app=opal-client`
3. Force refresh:
   ```bash
   curl -X POST http://localhost:7002/data/config/trigger_update
   ```

### CLI commands hang or timeout

**Symptom:** `aeterna <command>` takes >10 seconds or never returns

**Fix:**
1. Check for long-running network calls:
   ```bash
   RUST_LOG=debug $AETERNA_BIN <command>
   ```
2. Verify backend service is responsive:
   ```bash
   curl -s http://localhost:8080/health
   curl -s http://localhost:6333/health
   ```
3. Kill hung processes and restart:
   ```bash
   pkill -f "cargo run.*aeterna"
   ```

---

## Success Criteria

A deployment is ready for cloud when ALL of the following pass:

1. ✅ All build phases complete without errors
2. ✅ All infrastructure services are healthy (K8s, PostgreSQL, Qdrant, Redis, OPAL)
3. ✅ Agent-a2a HTTP service responds to all 4 endpoints
4. ✅ OPAL authorization stack distributes policies correctly
5. ✅ OPAL Fetcher serves all entity types from PostgreSQL
6. ✅ All 19 CLI commands execute without errors
7. ✅ Memory operations store and retrieve vectors correctly
8. ✅ OpenCode plugin builds, tests, and integrates without errors
9. ✅ Website builds and renders correctly
10. ✅ PostgreSQL data is consistent and migrations complete
11. ✅ Qdrant vectors persist and search returns expected results
12. ✅ Redis cache and distributed locks function correctly
13. ✅ End-to-End Validation Checklist fully passes

Once all criteria pass, the deployment is verified and ready for cloud provisioning.
