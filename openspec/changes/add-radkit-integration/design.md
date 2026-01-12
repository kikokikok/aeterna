# Design: Radkit A2A Integration

## Context

The Memory-Knowledge system currently exposes tools via MCP (Model Context Protocol) through the `tools` crate. To enable Agent-to-Agent (A2A) orchestration and discovery, we need an A2A-compliant interface. Radkit (https://radkit.rs) provides a Rust SDK that guarantees A2A protocol compliance at compile time.

**Stakeholders**: AI agents consuming memory/knowledge services, orchestration layers, multi-agent systems.

**Constraints**:
- Rust Edition 2024 (never 2021)
- Multi-tenant isolation via `TenantContext` must propagate through all requests
- Follow existing builder patterns in `MemoryManager`, `GitRepository`, `GovernanceEngine`

## Goals / Non-Goals

### Goals
- Expose Memory, Knowledge, and Governance operations as A2A Skills
- Serve A2A Agent Card at `/.well-known/agent-card.json`
- Support multi-turn conversations via Radkit's State and slot mechanism
- Propagate `TenantContext` through all A2A requests
- Provide health and observability endpoints

### Non-Goals
- Replace existing MCP interface (A2A is additive)
- Implement push notifications (initial version only)
- Support A2A agent discovery/federation (out of scope for v1)

## Decisions

### 1. Crate Structure: New Binary Crate `agent-a2a`

**Decision**: Create a new binary crate `agent-a2a` that composes existing crates.

**Rationale**:
- Keeps A2A-specific code separate from core logic
- Allows independent deployment of A2A endpoint
- Follows existing crate structure (tools, adapters, etc.)

**Alternatives Considered**:
- Extend `tools` crate with A2A support → Rejected: conflates MCP and A2A concerns
- Add to `adapters` crate → Rejected: adapters are trait implementations, not binaries

### 2. Skill-to-Crate Mapping

**Decision**: Three Skills implementing `SkillHandler` trait, mapping to existing crates:

| Skill | Wraps | Primary Methods |
|-------|-------|-----------------|
| `MemorySkill` | `memory::MemoryManager` | `search_hierarchical`, `add_to_layer`, `delete_from_layer` |
| `KnowledgeSkill` | `knowledge::GitRepository` | `get`, `store`, `list`, `search` |
| `GovernanceSkill` | `knowledge::GovernanceEngine` | `validate`, `validate_with_context`, `check_drift` |

Each skill uses the `#[skill]` macro for A2A metadata:

```rust
use radkit::macros::skill;
use radkit::agent::{SkillHandler, OnRequestResult};
use radkit::models::Content;
use radkit::runtime::context::{ProgressSender, State};
use radkit::runtime::Runtime;

#[skill(
    id = "memory",
    name = "Memory Manager",
    description = "Hierarchical memory storage with layer precedence",
    tags = ["memory", "search", "storage"],
    examples = [
        "Search for memories about authentication",
        "Add a memory about API design decisions"
    ],
    input_modes = ["text/plain", "application/json"],
    output_modes = ["application/json"]
)]
pub struct MemorySkill {
    manager: Arc<MemoryManager>,
}

#[async_trait]
impl SkillHandler for MemorySkill {
    async fn on_request(
        &self,
        state: &mut State,
        progress: &ProgressSender,
        runtime: &dyn Runtime,
        content: Content
    ) -> AgentResult<OnRequestResult> {
        // Parse request, delegate to MemoryManager, return result
    }
}
```

**Rationale**:
- Maps 1:1 to existing domain boundaries
- Each skill is self-contained and testable
- Follows single-responsibility principle

### 3. Tenant Context Extraction

**Decision**: Extract `TenantContext` from Radkit's `AuthService` in the runtime.

```rust
// In skill handler
async fn on_request(
    &self,
    state: &mut State,
    progress: &ProgressSender,
    runtime: &dyn Runtime,
    content: Content
) -> AgentResult<OnRequestResult> {
    // Extract tenant context from runtime's auth service
    let auth = runtime.auth();
    let tenant_ctx = TenantContext {
        tenant_id: TenantId::new(auth.tenant_id()),
        user_id: UserId::new(auth.user_id()),
        ..Default::default()
    };
    
    // Pass to underlying manager
    let results = self.manager.search_hierarchical(tenant_ctx, ...).await?;
}
```

**Rationale**:
- Consistent with existing auth patterns in `memory::MemoryManager`
- Radkit's `AuthService` provides tenant-aware context
- Enables tenant isolation without skill-specific logic

### 4. Error Handling Strategy

**Decision**: Map domain errors to `OnRequestResult` variants.

| Domain Error | OnRequestResult | Behavior |
|--------------|-----------------|----------|
| `RepositoryError::Git` | `Failed` | Return error message |
| `RepositoryError::InvalidPath` | `Failed` | Return validation error |
| Auth failure | `Rejected` | Refuse task |
| Missing input | `InputRequired` | Ask for clarification |
| Validation failure | `Completed` with violations | Return policy violations as artifact |

```rust
// Error handling in skill
match self.manager.search_hierarchical(ctx, query, limit, filters).await {
    Ok(results) => {
        let artifact = Artifact::from_json("results.json", &results)?;
        Ok(OnRequestResult::Completed {
            message: Some(Content::from_text(format!("Found {} results", results.len()))),
            artifacts: vec![artifact],
        })
    }
    Err(e) => Ok(OnRequestResult::Failed {
        message: Content::from_text(format!("Search failed: {}", e)),
    })
}
```

**Rationale**:
- Provides consistent error experience for A2A consumers
- Leverages Radkit's type-safe result variants
- Preserves error detail for debugging

### 5. Multi-turn Conversation Support

**Decision**: Use Radkit's `State` and slot mechanism for multi-turn interactions.

```rust
#[derive(Serialize, Deserialize)]
enum MemorySlot {
    AwaitingLayerSelection,
    AwaitingConfirmation,
}

async fn on_request(&self, state: &mut State, ...) -> AgentResult<OnRequestResult> {
    // Parse the request
    let request: MemoryRequest = parse_request(&content)?;
    
    // If layer not specified, ask for it
    if request.layer.is_none() {
        state.task().save("pending_request", &request)?;
        state.set_slot(MemorySlot::AwaitingLayerSelection)?;
        
        return Ok(OnRequestResult::InputRequired {
            message: Content::from_text(
                "Which layer should I add this memory to? (Personal, Team, Project, Org)"
            ),
        });
    }
    
    // Continue with operation...
}

async fn on_input_received(&self, state: &mut State, ...) -> AgentResult<OnInputResult> {
    let slot: MemorySlot = state.slot()?.ok_or(anyhow!("No pending slot"))?;
    
    match slot {
        MemorySlot::AwaitingLayerSelection => {
            let mut request: MemoryRequest = state.task().load("pending_request")?.unwrap();
            request.layer = Some(parse_layer(&content)?);
            state.clear_slot();
            // Continue with operation...
        }
        // ... other slots
    }
}
```

**Rationale**:
- Radkit handles A2A protocol compliance for multi-turn
- State persistence is built-in
- Slot mechanism provides type-safe conversation state

### 6. Runtime Configuration

**Decision**: Use `Runtime::builder()` with the `runtime` feature.

```rust
use radkit::agent::Agent;
use radkit::models::providers::AnthropicLlm;
use radkit::runtime::Runtime;

fn configure_agent() -> AgentDefinition {
    Agent::builder()
        .with_name("Aeterna Memory-Knowledge Agent")
        .with_description("A2A agent for hierarchical memory and governed knowledge management")
        .with_skill(MemorySkill::new(memory_manager))
        .with_skill(KnowledgeSkill::new(git_repository))
        .with_skill(GovernanceSkill::new(governance_engine))
        .build()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize domain managers
    let memory_manager = Arc::new(MemoryManager::new()
        .with_config(config.memory)
        .with_embedding_service(embedding_service));
    
    let git_repository = Arc::new(GitRepository::new(&config.knowledge.root_path)?);
    let governance_engine = Arc::new(GovernanceEngine::new()
        .with_storage(storage)
        .with_event_publisher(publisher));
    
    // Create LLM (for any LLM-assisted operations)
    let llm = AnthropicLlm::from_env("claude-sonnet-4-5-20250929")?;
    
    // Build and serve
    Runtime::builder(configure_agent(), llm)
        .build()
        .serve(&config.bind_address)
        .await?;
    
    Ok(())
}
```

**Endpoints served automatically by Radkit**:
- `/.well-known/agent-card.json` – Agent Card discovery
- `/rpc` – JSON-RPC entry point for A2A messages
- `/message:stream` – SSE streaming
- `/tasks/{task_id}/subscribe` – Task subscription

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     agent-a2a (binary)                      │
├─────────────────────────────────────────────────────────────┤
│  Radkit Runtime                                             │
│  ┌─────────────┬──────────────────┬──────────────────────┐  │
│  │MemorySkill  │ KnowledgeSkill   │ GovernanceSkill      │  │
│  │ #[skill]    │ #[skill]         │ #[skill]             │  │
│  │ SkillHandler│ SkillHandler     │ SkillHandler         │  │
│  └──────┬──────┴────────┬─────────┴──────────┬───────────┘  │
│         │               │                    │              │
└─────────┼───────────────┼────────────────────┼──────────────┘
          │               │                    │
          ▼               ▼                    ▼
    ┌───────────┐   ┌───────────┐   ┌─────────────────────┐
    │  memory   │   │ knowledge │   │     knowledge       │
    │   crate   │   │   crate   │   │ (GovernanceEngine)  │
    └───────────┘   └───────────┘   └─────────────────────┘
```

## File Structure

```
agent-a2a/
├── Cargo.toml
├── src/
│   ├── main.rs           # Runtime setup, configuration
│   ├── lib.rs            # Public API for testing
│   ├── config.rs         # Configuration loading
│   └── skills/
│       ├── mod.rs        # Skill exports + configure_agent()
│       ├── memory.rs     # MemorySkill with #[skill] macro
│       ├── knowledge.rs  # KnowledgeSkill with #[skill] macro
│       └── governance.rs # GovernanceSkill with #[skill] macro
└── tests/
    └── integration/
        └── a2a_test.rs   # End-to-end A2A tests
```

## Dependencies

Add to workspace `Cargo.toml`:
```toml
[workspace.dependencies]
radkit = { version = "0.0.4", features = ["runtime"] }
schemars = "1"
```

Add to `agent-a2a/Cargo.toml`:
```toml
[package]
name = "agent-a2a"
version = "0.1.0"
edition = "2024"

[dependencies]
radkit = { workspace = true }
schemars = { workspace = true }
memory = { path = "../memory" }
knowledge = { path = "../knowledge" }
mk_core = { path = "../core" }
config = { path = "../config" }
storage = { path = "../storage" }
tokio = { workspace = true, features = ["rt-multi-thread", "sync", "net", "process", "macros"] }
tracing = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
anyhow = { workspace = true }
async-trait = { workspace = true }
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| Radkit API instability (v0.0.4) | Pin specific version, add integration tests |
| Performance overhead of A2A protocol | Benchmark critical paths, optimize serialization |
| State memory growth in multi-turn | Radkit's in-memory state has TTL, add cleanup if needed |
| Breaking changes in A2A spec | Radkit abstracts spec compliance |

## Migration Plan

1. **Phase 1**: Add `agent-a2a` crate with basic MemorySkill
2. **Phase 2**: Add KnowledgeSkill and GovernanceSkill  
3. **Phase 3**: Add multi-turn support and health endpoints
4. **Phase 4**: Production deployment with monitoring

**Rollback**: Binary crate is additive; rollback is removal from deployment.

## Open Questions

1. **LLM requirement**: Radkit requires an LLM for the runtime. Should we use it for skill-internal reasoning, or just pass a dummy?
   - Recommendation: Use for optional semantic parsing of natural language requests.

2. **Thread persistence**: Should threads be persisted to PostgreSQL or kept in-memory?
   - Recommendation: Start in-memory (Radkit default), add persistence if resumption is required.

3. **Rate limiting**: Should A2A endpoint have separate rate limits?
   - Recommendation: Defer to infrastructure layer (API gateway).
