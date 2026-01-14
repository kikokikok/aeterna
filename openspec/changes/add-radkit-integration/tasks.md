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

---

## 8. Production Gap Requirements

### 8.1 Radkit SDK Version Stability (RAD-C1) - CRITICAL
- [ ] 8.1.1 Pin `radkit` dependency to exact version in `Cargo.toml` (e.g., `radkit = "=0.0.4"`)
- [ ] 8.1.2 Create `agent-a2a/src/sdk_abstraction.rs` module
- [ ] 8.1.3 Define `RadkitAdapter` trait abstracting SDK operations (create_runtime, register_skill, start_server)
- [ ] 8.1.4 Implement `RadkitV0Adapter` implementing the trait for current SDK version
- [ ] 8.1.5 Add SDK version to Agent Card metadata field `sdkVersion`
- [ ] 8.1.6 Create comprehensive integration test suite in `agent-a2a/tests/sdk_integration.rs`
- [ ] 8.1.7 Add SDK migration guide documentation for future version updates

### 8.2 Thread State Persistence (RAD-C2) - CRITICAL
- [ ] 8.2.1 Create PostgreSQL migration for `a2a_threads` table (thread_id, tenant_id, created_at, updated_at, context_json, state)
- [ ] 8.2.2 Implement `ThreadRepository` struct in `agent-a2a/src/persistence/threads.rs`
- [ ] 8.2.3 Add `create_thread`, `update_thread`, `get_thread`, `list_threads` methods
- [ ] 8.2.4 Implement thread state serialization/deserialization (JSON or bincode)
- [ ] 8.2.5 Add thread recovery logic on service startup
- [ ] 8.2.6 Implement thread expiration (default 24h TTL, configurable)
- [ ] 8.2.7 Add `/threads` admin endpoint for thread management
- [ ] 8.2.8 Write integration tests for thread persistence and recovery

### 8.3 A2A Spec Compliance Monitoring (RAD-H1) - HIGH
- [ ] 8.3.1 Add A2A spec version field to Agent Card (`a2aSpecVersion: "draft-01"`)
- [ ] 8.3.2 Create `agent-a2a/tests/compliance/` directory for compliance tests
- [ ] 8.3.3 Implement Agent Card schema validation test
- [ ] 8.3.4 Implement task message format compliance tests
- [ ] 8.3.5 Implement error response format compliance tests
- [ ] 8.3.6 Add version detection and warning for newer spec versions
- [ ] 8.3.7 Document supported A2A spec version in README

### 8.4 Error Mapping Completeness (RAD-H2) - HIGH
- [ ] 8.4.1 Define exhaustive `A2AErrorCode` enum in `agent-a2a/src/errors.rs`
- [ ] 8.4.2 Implement `From<MemoryError>` for `A2AError`
- [ ] 8.4.3 Implement `From<KnowledgeError>` for `A2AError`
- [ ] 8.4.4 Implement `From<GovernanceError>` for `A2AError`
- [ ] 8.4.5 Add catch-all `INTERNAL_ERROR` mapping for unexpected errors
- [ ] 8.4.6 Add error detail sanitization (remove sensitive info before returning)
- [ ] 8.4.7 Implement error logging with full context for debugging
- [ ] 8.4.8 Write tests for all error mapping scenarios

### 8.5 A2A Rate Limiting (RAD-H3) - HIGH
- [ ] 8.5.1 Add `RateLimiter` struct in `agent-a2a/src/middleware/rate_limit.rs`
- [ ] 8.5.2 Implement Redis-backed sliding window rate limiting
- [ ] 8.5.3 Add per-tenant rate limit configuration (config file + env vars)
- [ ] 8.5.4 Implement rate limit middleware for Axum router
- [ ] 8.5.5 Add `Retry-After` header computation
- [ ] 8.5.6 Implement per-skill rate limit overrides
- [ ] 8.5.7 Add rate limit metrics (rate_limit_hits_total, rate_limit_remaining gauge)
- [ ] 8.5.8 Write tests for rate limiting scenarios

### 8.6 LLM Cost Optimization (RAD-H4) - HIGH
- [ ] 8.6.1 Add `llm_routing_mode` config option: `always`, `ambiguous_only`, `never`
- [ ] 8.6.2 Implement tool detection: check if request contains explicit tool invocation
- [ ] 8.6.3 Add direct tool routing path bypassing LLM for explicit invocations
- [ ] 8.6.4 Configure minimal LLM model for routing (e.g., gpt-3.5-turbo-instruct)
- [ ] 8.6.5 Implement LLM context minimization (only skill descriptions, no history)
- [ ] 8.6.6 Add metrics for LLM invocations vs direct routing
- [ ] 8.6.7 Write tests for both routing paths

### 8.7 Thread State Memory Management (RAD-H5) - HIGH
- [ ] 8.7.1 Add TTL column to `a2a_threads` table (default: 3600 seconds)
- [ ] 8.7.2 Implement state size calculation before update
- [ ] 8.7.3 Add `max_thread_state_size` config option (default: 1MB)
- [ ] 8.7.4 Implement state size limit enforcement (return error on exceed)
- [ ] 8.7.5 Create background cleanup job in `agent-a2a/src/jobs/cleanup.rs`
- [ ] 8.7.6 Add cleanup job scheduling (default: every 5 minutes)
- [ ] 8.7.7 Implement cleanup metrics (threads_cleaned_total, cleanup_duration_seconds)
- [ ] 8.7.8 Write tests for TTL enforcement and cleanup job

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 3 | Preparation |
| 2 | 3 | Core Infrastructure |
| 3 | 14 | Skill Implementations |
| 4 | 5 | Runtime Setup |
| 5 | 3 | Observability |
| 6 | 7 | Integration Testing |
| 7 | 3 | Documentation |
| 8 | 57 | Production Gap Requirements (RAD-C1 to RAD-H5) |
| **Total** | **95** | |

**Estimated effort**: 4-5 weeks with 80% test coverage target

---

## Production Gap Tracking

| Gap ID | Priority | Requirement | Tasks |
|--------|----------|-------------|-------|
| RAD-C1 | Critical | Radkit SDK Version Stability | 8.1.1-8.1.7 |
| RAD-C2 | Critical | Thread State Persistence | 8.2.1-8.2.8 |
| RAD-H1 | High | A2A Spec Compliance Monitoring | 8.3.1-8.3.7 |
| RAD-H2 | High | Error Mapping Completeness | 8.4.1-8.4.8 |
| RAD-H3 | High | A2A Rate Limiting | 8.5.1-8.5.8 |
| RAD-H4 | High | LLM Cost Optimization | 8.6.1-8.6.7 |
| RAD-H5 | High | Thread State Memory Management | 8.7.1-8.7.8 |
