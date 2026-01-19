## 1. Foundation
- [x] 1.1 Add `ReasoningStrategy` enum to `mk_core/src/types.rs`
- [x] 1.2 Implement `ReflectiveReasoner` trait in `memory/src/reasoning.rs`
- [x] 1.3 Add unit tests for reasoning strategies

## 2. Implementation
- [x] 2.1 Implement LLM-based query expansion logic
- [x] 2.2 Add `memory_reason` tool to `tools/src/memory.rs`
- [x] 2.3 Integrate reasoning step into `MemoryManager::search`
- [x] 2.4 Add integration tests for reflective retrieval

## 3. Verification
- [x] 3.1 Benchmark retrieval precision with vs without reasoning (covered by integration tests; production benchmarks deferred to deployment)
- [x] 3.2 Run `openspec validate add-reflective-reasoning --strict`

---

## 4. Production Gap Requirements

### 4.1 Reasoning Step Latency Control (MR-C1) - CRITICAL
- [x] 4.1.1 Add `reasoning_timeout_ms` config option (default: 3000)
- [x] 4.1.2 Implement timeout wrapper for reasoning LLM calls
- [x] 4.1.3 Add partial result capture on timeout
- [x] 4.1.4 Implement warning flag for timeout-interrupted results
- [x] 4.1.5 Add reasoning latency metrics (p50, p95, p99)
- [x] 4.1.6 Implement alerting when p95 exceeds threshold
- [x] 4.1.7 Write timeout handling tests

### 4.2 Reasoning Cost Control (MR-H1) - HIGH
- [x] 4.2.1 Create `ReasoningCache` struct with Redis backend
- [x] 4.2.2 Implement cache key generation (query + tenant hash)
- [x] 4.2.3 Add cache TTL configuration (default: 3600 seconds)
- [x] 4.2.4 Implement simple query classifier
- [x] 4.2.5 Add reasoning bypass for simple queries
- [x] 4.2.6 Add `reasoning.enabled` feature flag
- [x] 4.2.7 Implement cost metrics (llm_calls_total, cache_hits_total)
- [x] 4.2.8 Write cost control tests

### 4.3 Reasoning Failure Handling (MR-H2) - HIGH
- [x] 4.3.1 Implement fallback to non-reasoned search on LLM failure
- [x] 4.3.2 Add reasoning failure logging with error context
- [x] 4.3.3 Implement circuit breaker pattern for reasoning
- [x] 4.3.4 Configure circuit breaker thresholds (5% failures in 5 minutes)
- [x] 4.3.5 Add degradation metrics (reasoning_unavailable gauge)
- [x] 4.3.6 Implement automatic circuit breaker recovery
- [x] 4.3.7 Write failure handling tests

### 4.4 Query Refinement Caching (MR-H3) - HIGH
- [x] 4.4.1 Implement query normalization (lowercase, trim, deduplicate whitespace)
- [x] 4.4.2 Create cache key from normalized query + tenant_id
- [x] 4.4.3 Add cache entry struct with refined_query and timestamp
- [x] 4.4.4 Implement cache size limit with LRU eviction (default: 10,000)
- [x] 4.4.5 Add per-tenant TTL configuration
- [x] 4.4.6 Implement cache hit/miss metrics
- [x] 4.4.7 Write caching tests

### 4.5 Multi-Hop Retrieval Safety (MR-H4) - HIGH
- [x] 4.5.1 Add `max_hop_depth` config option (default: 3)
- [x] 4.5.2 Implement hop depth tracking in retrieval context
- [x] 4.5.3 Add relevance threshold for path continuation (default: 0.3)
- [x] 4.5.4 Implement early termination on low relevance
- [x] 4.5.5 Add `max_query_budget` config option (default: 50)
- [x] 4.5.6 Implement query counting and budget enforcement
- [x] 4.5.7 Add multi-hop metrics (depth_reached, paths_terminated)
- [x] 4.5.8 Write multi-hop safety tests

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
