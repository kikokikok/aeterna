## 1. Foundation
- [ ] 1.1 Add `ReasoningStrategy` enum to `mk_core/src/types.rs`
- [ ] 1.2 Implement `ReflectiveReasoner` trait in `memory/src/reasoning.rs`
- [ ] 1.3 Add unit tests for reasoning strategies

## 2. Implementation
- [ ] 2.1 Implement LLM-based query expansion logic
- [ ] 2.2 Add `memory_reason` tool to `tools/src/memory_tools.rs`
- [ ] 2.3 Integrate reasoning step into `MemoryManager::search`
- [ ] 2.4 Add integration tests for reflective retrieval

## 3. Verification
- [ ] 3.1 Benchmark retrieval precision with vs without reasoning
- [ ] 3.2 Run `openspec validate add-reflective-reasoning --strict`

---

## 4. Production Gap Requirements

### 4.1 Reasoning Step Latency Control (MR-C1) - CRITICAL
- [ ] 4.1.1 Add `reasoning_timeout_ms` config option (default: 3000)
- [ ] 4.1.2 Implement timeout wrapper for reasoning LLM calls
- [ ] 4.1.3 Add partial result capture on timeout
- [ ] 4.1.4 Implement warning flag for timeout-interrupted results
- [ ] 4.1.5 Add reasoning latency metrics (p50, p95, p99)
- [ ] 4.1.6 Implement alerting when p95 exceeds threshold
- [ ] 4.1.7 Write timeout handling tests

### 4.2 Reasoning Cost Control (MR-H1) - HIGH
- [ ] 4.2.1 Create `ReasoningCache` struct with Redis backend
- [ ] 4.2.2 Implement cache key generation (query + tenant hash)
- [ ] 4.2.3 Add cache TTL configuration (default: 3600 seconds)
- [ ] 4.2.4 Implement simple query classifier
- [ ] 4.2.5 Add reasoning bypass for simple queries
- [ ] 4.2.6 Add `reasoning.enabled` feature flag
- [ ] 4.2.7 Implement cost metrics (llm_calls_total, cache_hits_total)
- [ ] 4.2.8 Write cost control tests

### 4.3 Reasoning Failure Handling (MR-H2) - HIGH
- [ ] 4.3.1 Implement fallback to non-reasoned search on LLM failure
- [ ] 4.3.2 Add reasoning failure logging with error context
- [ ] 4.3.3 Implement circuit breaker pattern for reasoning
- [ ] 4.3.4 Configure circuit breaker thresholds (5% failures in 5 minutes)
- [ ] 4.3.5 Add degradation metrics (reasoning_unavailable gauge)
- [ ] 4.3.6 Implement automatic circuit breaker recovery
- [ ] 4.3.7 Write failure handling tests

### 4.4 Query Refinement Caching (MR-H3) - HIGH
- [ ] 4.4.1 Implement query normalization (lowercase, trim, deduplicate whitespace)
- [ ] 4.4.2 Create cache key from normalized query + tenant_id
- [ ] 4.4.3 Add cache entry struct with refined_query and timestamp
- [ ] 4.4.4 Implement cache size limit with LRU eviction (default: 10,000)
- [ ] 4.4.5 Add per-tenant TTL configuration
- [ ] 4.4.6 Implement cache hit/miss metrics
- [ ] 4.4.7 Write caching tests

### 4.5 Multi-Hop Retrieval Safety (MR-H4) - HIGH
- [ ] 4.5.1 Add `max_hop_depth` config option (default: 3)
- [ ] 4.5.2 Implement hop depth tracking in retrieval context
- [ ] 4.5.3 Add relevance threshold for path continuation (default: 0.3)
- [ ] 4.5.4 Implement early termination on low relevance
- [ ] 4.5.5 Add `max_query_budget` config option (default: 50)
- [ ] 4.5.6 Implement query counting and budget enforcement
- [ ] 4.5.7 Add multi-hop metrics (depth_reached, paths_terminated)
- [ ] 4.5.8 Write multi-hop safety tests

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 3 | Foundation |
| 2 | 4 | Implementation |
| 3 | 2 | Verification |
| 4 | 37 | Production Gap Requirements (MR-C1 to MR-H4) |
| **Total** | **46** | |

**Estimated effort**: 3-4 weeks with 80% test coverage target

---

## Production Gap Tracking

| Gap ID | Priority | Requirement | Tasks |
|--------|----------|-------------|-------|
| MR-C1 | Critical | Reasoning Step Latency Control | 4.1.1-4.1.7 |
| MR-H1 | High | Reasoning Cost Control | 4.2.1-4.2.8 |
| MR-H2 | High | Reasoning Failure Handling | 4.3.1-4.3.7 |
| MR-H3 | High | Query Refinement Caching | 4.4.1-4.4.7 |
| MR-H4 | High | Multi-Hop Retrieval Safety | 4.5.1-4.5.8 |
