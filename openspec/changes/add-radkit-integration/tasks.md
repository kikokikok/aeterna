## 1. Preparation

- [ ] 1.1 Add `radkit` and `a2a-types` to workspace dependencies in root `Cargo.toml`
- [ ] 1.2 Create `agent-a2a` crate structure with `Cargo.toml`
- [ ] 1.3 Add local crate dependencies to `agent-a2a/Cargo.toml`

## 2. Core Infrastructure

- [ ] 2.1 Implement `agent-a2a/src/config.rs` - configuration loading (bind address, port, auth settings)
- [ ] 2.2 Implement `agent-a2a/src/auth.rs` - `TenantContext` extraction from A2A requests
- [ ] 2.3 Implement `agent-a2a/src/errors.rs` - domain error to A2A error mapping

## 3. Skill Implementations

### 3.1 MemorySkill
- [ ] 3.1.1 Create `agent-a2a/src/skills/mod.rs` with skill module exports
- [ ] 3.1.2 Implement `MemorySkill` struct wrapping `MemoryManager`
- [ ] 3.1.3 Implement `memory_add` tool handler
- [ ] 3.1.4 Implement `memory_search` tool handler (calls `search_hierarchical`)
- [ ] 3.1.5 Implement `memory_delete` tool handler
- [ ] 3.1.6 Add unit tests for MemorySkill

### 3.2 KnowledgeSkill
- [ ] 3.2.1 Implement `KnowledgeSkill` struct wrapping `GitRepository`
- [ ] 3.2.2 Implement `knowledge_query` tool handler (calls `search`)
- [ ] 3.2.3 Implement `knowledge_show` tool handler (calls `get`)
- [ ] 3.2.4 Implement `knowledge_check` tool handler (policy validation)
- [ ] 3.2.5 Add unit tests for KnowledgeSkill

### 3.3 GovernanceSkill
- [ ] 3.3.1 Implement `GovernanceSkill` struct wrapping `GovernanceEngine`
- [ ] 3.3.2 Implement `governance_validate` tool handler
- [ ] 3.3.3 Implement `governance_drift_check` tool handler
- [ ] 3.3.4 Add unit tests for GovernanceSkill

## 4. Runtime Setup

- [ ] 4.1 Implement `agent-a2a/src/main.rs` - initialize managers and compose skills
- [ ] 4.2 Create Radkit `AgentDefinition` composing all three skills
- [ ] 4.3 Configure Agent Card with skill descriptions and input schemas
- [ ] 4.4 Start Radkit `Runtime` server with configured bind address
- [ ] 4.5 Implement `agent-a2a/src/lib.rs` for testing exports

## 5. Observability

- [ ] 5.1 Add `/health` endpoint checking MemoryManager, GitRepository, storage backends
- [ ] 5.2 Add `/metrics` endpoint with Prometheus-compatible metrics
- [ ] 5.3 Integrate with existing telemetry (`MemoryTelemetry`, `KnowledgeTelemetry`)

## 6. Integration Testing

- [ ] 6.1 Create `agent-a2a/tests/integration/a2a_test.rs`
- [ ] 6.2 Test Agent Card discovery at `/.well-known/agent.json`
- [ ] 6.3 Test `tasks/send` endpoint with MemorySkill tools
- [ ] 6.4 Test `tasks/send` endpoint with KnowledgeSkill tools
- [ ] 6.5 Test `tasks/send` endpoint with GovernanceSkill tools
- [ ] 6.6 Test multi-tenant isolation (different tenants see different data)
- [ ] 6.7 Test error responses for invalid parameters and unauthorized access

## 7. Documentation

- [ ] 7.1 Add usage examples to `agent-a2a/README.md`
- [ ] 7.2 Document environment variables and configuration options
- [ ] 7.3 Document A2A tool schemas in Agent Card format
