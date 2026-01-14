# Aeterna Production Readiness Gaps

This document identifies production-grade gaps across all active OpenSpec change proposals. Each gap is categorized by severity and includes recommended solutions.

**Document Version**: 2026-01-14
**Analyzed Changes**: 8 active proposals
**Target Deployment**: 300+ engineers, enterprise-scale

---

## Summary by Change

| Change ID | Critical | High | Medium | Total |
|-----------|----------|------|--------|-------|
| `add-r1-graph-memory` | 4 | 9 | 4 | 17 |
| `add-ux-first-governance` | 3 | 7 | 3 | 13 |
| `add-cca-capabilities` | 2 | 6 | 4 | 12 |
| `add-radkit-integration` | 2 | 5 | 3 | 10 |
| `add-opencode-plugin` | 2 | 5 | 3 | 10 |
| `add-helm-chart` | 2 | 6 | 4 | 12 |
| `add-multi-tenant-governance` | 3 | 5 | 3 | 11 |
| `add-reflective-reasoning` | 1 | 4 | 3 | 8 |
| **TOTAL** | **19** | **47** | **27** | **93** |

---

## 1. add-r1-graph-memory (DuckDB Graph Store)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| R1-C1 | **Cascading deletion missing** | Memory deletion doesn't cascade to graph nodes/edges/entities. Orphaned graph data accumulates. | Implement soft-delete with cascade cleanup job. Add `DELETE FROM memory_edges WHERE source_id = ?` triggers. |
| R1-C2 | **No FK enforcement** | DuckDB doesn't enforce foreign key constraints. Invalid references can exist. | Add application-level referential integrity checks. Validate node existence before edge creation. |
| R1-C3 | **Single-writer contention** | DuckDB supports single-writer only. Concurrent Lambda invocations will deadlock. | Implement write-ahead queue in Redis. Serialize writes through coordinator process. |
| R1-C4 | **S3 partial failure** | Parquet export can partially fail (3/4 tables exported). No transactional consistency. | Implement two-phase commit: write to temp path, then atomic rename. Validate checksums before swap. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| R1-H1 | **No composite indexes** | Missing `(tenant_id, source_id)` and `(tenant_id, target_id)` indexes. Queries scan entire tables. | Add `CREATE INDEX idx_edges_tenant_source ON memory_edges(tenant_id, source_id)`. |
| R1-H2 | **No query observability** | No metrics for query latency, cache hits, graph traversal depth. | Add OpenTelemetry spans around `find_related()`, `shortest_path()`. Export to Prometheus. |
| R1-H3 | **Tenant isolation unverified** | SQL injection or query manipulation could bypass `WHERE tenant_id = ?` filter. | Add query validation layer. Parameterize all queries. Add tenant context middleware. |
| R1-H4 | **No automated backups** | S3 persistence is checkpoint-only. No point-in-time recovery capability. | Add scheduled S3 snapshots with versioning. Implement WAL shipping for continuous backup. |
| R1-H5 | **Transaction isolation weak** | Multi-table operations (node + edges + entities) not atomic. | Wrap related operations in `BEGIN TRANSACTION`. Implement saga pattern for S3 persistence. |
| R1-H6 | **Lambda cold start lock contention** | Concurrent Lambda cold starts all try to initialize DuckDB from S3 simultaneously. | Add distributed lock (Redis SETNX) during cold start. Implement warm pool strategy. |
| R1-H7 | **Large graph cold start too slow** | Loading large Parquet files exceeds 3-second cold start budget. | Implement lazy loading. Load only accessed partitions. Add pre-warming API. |
| R1-H8 | **No health checks** | No connectivity verification for DuckDB or S3. Silent failures possible. | Add `/health` endpoint checking DuckDB connection and S3 access. |
| R1-H9 | **No schema migrations** | Schema changes require manual intervention. No versioning. | Add `schema_version` table. Implement migration runner on startup. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| R1-M1 | **No corruption detection** | Silent data corruption in Parquet files goes undetected. | Add checksum validation on load. Implement periodic integrity scans. |
| R1-M2 | **No graceful degradation** | Graph unavailability cascades to memory operations. | Add circuit breaker. Fallback to vector-only search when graph unavailable. |
| R1-M3 | **No audit logging** | Graph operations not tracked for compliance. | Add audit log table. Log all mutations with actor and timestamp. |
| R1-M4 | **Large graph OOM risk** | Complex traversals can consume unbounded memory. | Add query timeout (30s). Limit max hops (5). Implement pagination for large result sets. |

---

## 2. add-ux-first-governance (OPAL + Cedar + CLI)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| UX-C1 | **OPAL server single point of failure** | OPAL server down = no auth decisions, all operations blocked. | Deploy OPAL server in HA mode (3+ replicas). Add local policy cache with TTL. |
| UX-C2 | **Cedar policy conflict detection absent** | Conflicting policies (allow + deny same action) not detected until runtime. | Implement policy conflict analyzer in `aeterna_policy_validate`. Block conflicting proposals. |
| UX-C3 | **PostgreSQL referential consistency** | Org structure changes can orphan team/project references. | Add foreign key constraints. Implement cascading soft-delete with cleanup jobs. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| UX-H1 | **WebSocket PubSub reliability** | Cedar Agent disconnection loses policy updates. Silent policy drift. | Add reconnection with exponential backoff. Implement full resync on reconnect. Add connection health metrics. |
| UX-H2 | **IdP sync latency** | Okta/Azure AD changes can take hours to propagate. User permissions stale. | Add webhook handlers for IdP events. Implement pull+push sync strategy. |
| UX-H3 | **CLI offline mode absent** | No local operation when Aeterna server unreachable. | Add local policy cache. Queue operations for later sync. Implement conflict resolution. |
| UX-H4 | **Policy rollback undefined** | No mechanism to revert bad policy deployments. | Add policy versioning. Implement rollback command. Store policy history. |
| UX-H5 | **LLM translation non-determinism** | Same natural language input produces different Cedar policies. | Add prompt caching. Implement few-shot examples. Add deterministic fallback templates. |
| UX-H6 | **Approval workflow timeout** | Proposals stuck in pending forever if approvers unresponsive. | Add configurable timeout. Implement escalation path. Add reminder notifications. |
| UX-H7 | **Audit log retention undefined** | Unbounded growth of governance audit events. | Add retention policy (90 days default). Implement archival to cold storage. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| UX-M1 | **Meta-governance bootstrap** | Who approves the first governance rules? Chicken-egg problem. | Add bootstrap mode with initial admin. Document bootstrap procedure. |
| UX-M2 | **Policy simulation incomplete** | Simulation doesn't cover all constraint types. | Expand simulation to cover file, code, config targets. Add dry-run for all operations. |
| UX-M3 | **Natural language ambiguity** | "Block MySQL" unclear - driver, server, or both? | Add clarification prompts. Show interpretation for confirmation before proceeding. |

---

## 3. add-cca-capabilities (Context Compression + Learning)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| CCA-C1 | **LLM summarization cost unbounded** | Every layer update triggers summarization. Costs can explode. | Add summarization budget per tenant. Implement batching. Use cheaper models for low-priority layers. |
| CCA-C2 | **Meta-agent infinite loop** | Build-test-improve loop can cycle forever on unfixable issues. | Hard limit at 3 iterations (already in design). Add total time budget (5 minutes). |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| CCA-H1 | **Summary staleness detection** | Source content changed but summary not invalidated. Stale context served. | Add content hash comparison. Invalidate on hash mismatch. |
| CCA-H2 | **Hindsight note deduplication** | Same error pattern captured multiple times. Storage bloat. | Add error signature deduplication. Merge resolutions for matching patterns. |
| CCA-H3 | **Extension state memory growth** | Redis state grows unbounded during long sessions. | Add TTL per extension state key. Implement LRU eviction. |
| CCA-H4 | **Note-taking trajectory capture overhead** | Capturing every tool execution adds latency. | Make capture async. Batch writes. Add sampling for high-volume operations. |
| CCA-H5 | **Context assembly latency** | Querying multiple layers for relevance scores adds p99 latency. | Pre-compute relevance scores. Cache assembled contexts. Add timeout fallback. |
| CCA-H6 | **Summarization model failure** | LLM API failures block context updates. | Add retry with exponential backoff. Use cached summary on failure. Alert on repeated failures. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| CCA-M1 | **Personalization privacy** | User-specific summaries may leak to other users in same layer. | Add personalization isolation. Validate user context on summary retrieval. |
| CCA-M2 | **Token budget estimation inaccurate** | Actual token counts differ from estimates. Context overflow possible. | Use tiktoken for accurate counts. Add 10% safety margin. |
| CCA-M3 | **Hindsight promotion threshold unclear** | When does a resolution auto-promote to team/org? | Document promotion criteria. Make thresholds configurable. Require explicit opt-in. |
| CCA-M4 | **Extension callback ordering** | Multiple extensions modifying same content in undefined order. | Add priority system. Document callback execution order. |

---

## 4. add-radkit-integration (A2A Protocol)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| RAD-C1 | **Radkit SDK instability** | v0.0.4 is pre-stable. Breaking changes likely. | Pin exact version. Add integration tests. Abstract SDK layer for easier migration. |
| RAD-C2 | **Thread state persistence absent** | In-memory threads lost on pod restart. Conversations lost mid-flow. | Add PostgreSQL thread persistence. Implement session recovery on startup. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| RAD-H1 | **A2A spec compliance drift** | A2A spec evolving rapidly. Radkit may lag behind. | Monitor A2A spec releases. Add compliance tests against official test suite. |
| RAD-H2 | **Error mapping incomplete** | Not all domain errors mapped to A2A result variants. | Add exhaustive error mapping. Add catch-all `Failed` variant for unmapped errors. |
| RAD-H3 | **Rate limiting absent** | No protection against A2A endpoint abuse. | Add per-tenant rate limiting. Integrate with API gateway. |
| RAD-H4 | **LLM requirement overhead** | Radkit requires LLM even for simple skill calls. Unnecessary cost. | Configure minimal LLM for skill routing only. Bypass for direct tool calls. |
| RAD-H5 | **State memory growth** | Multi-turn conversations accumulate state without cleanup. | Add state TTL (1 hour default). Implement periodic cleanup job. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| RAD-M1 | **Agent Card discovery static** | Agent Card generated at build time. Capability changes require redeploy. | Add dynamic Agent Card generation. Reflect current skill availability. |
| RAD-M2 | **No request tracing** | A2A requests not traced end-to-end. Hard to debug multi-agent flows. | Add OpenTelemetry trace propagation. Include trace ID in A2A responses. |
| RAD-M3 | **SSE streaming untested** | `/message:stream` endpoint not covered by tests. | Add integration tests for SSE streaming. Test connection drop/reconnect. |

---

## 5. add-opencode-plugin (NPM Plugin + MCP)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| OC-C1 | **Plugin SDK version coupling** | `@opencode-ai/plugin` SDK changes can break plugin. | Pin SDK version. Add compatibility tests. Abstract SDK layer. |
| OC-C2 | **Credential exposure risk** | `AETERNA_TOKEN` in environment could leak via debug logs. | Add credential masking. Use secure credential storage. Rotate tokens regularly. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| OC-H1 | **Hook API experimental** | `experimental.chat.system.transform` may change or disappear. | Feature-flag experimental hooks. Provide fallback behavior. |
| OC-H2 | **Session capture overhead** | `tool.execute.after` hook on every tool adds latency. | Make capture async. Implement sampling. Add debouncing. |
| OC-H3 | **Knowledge query latency** | Real-time knowledge injection adds p99 latency to chat. | Pre-fetch on session start. Cache queries. Add timeout fallback. |
| OC-H4 | **Session persistence undefined** | Where is session state stored? Redis or local file? | Define session storage strategy. Add Redis for multi-instance deployments. |
| OC-H5 | **MCP server process management** | Stdio MCP server can crash silently. No health monitoring. | Add health checks. Implement supervisor pattern. Add crash recovery. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| OC-M1 | **Package naming undefined** | `@aeterna/opencode-plugin` vs `aeterna-opencode` not decided. | Standardize on `@aeterna/opencode-plugin` for namespace consistency. |
| OC-M2 | **TypeScript client bundling** | Should client be bundled or separate `@aeterna/client`? | Separate package for reuse. Bundle minimal version in plugin. |
| OC-M3 | **Significance detection heuristics** | What makes a tool execution "significant" for promotion? | Document criteria. Make configurable. Start with conservative defaults. |

---

## 6. add-helm-chart (Kubernetes Deployment)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| HC-C1 | **Secret management undefined** | Passwords/tokens in plain values.yaml. Not production-safe. | Add SOPS/sealed-secrets support. Document external secret managers (Vault, AWS SM). |
| HC-C2 | **Subchart version pinning absent** | Unpinned subcharts can introduce breaking changes. | Pin all subchart versions in Chart.yaml. Document upgrade procedure. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| HC-H1 | **PDB configuration missing** | No PodDisruptionBudget. Upgrades can cause complete outage. | Add PDB template with `minAvailable: 1`. Document drain procedure. |
| HC-H2 | **Network policy incomplete** | No network policies defined. All pod-to-pod traffic allowed. | Add NetworkPolicy templates. Restrict ingress to known sources. |
| HC-H3 | **Backup/restore undefined** | No CronJob for PostgreSQL backups. Data loss risk. | Add backup CronJob. Integrate with CloudNativePG backup features. |
| HC-H4 | **Resource limits too conservative** | 2Gi memory limit may be insufficient for large graphs. | Add configurable limits. Document sizing guidelines. Add VPA support. |
| HC-H5 | **Multi-region not supported** | Single cluster scope stated as non-goal. May block enterprise adoption. | Document multi-region architecture separately. Add federation support later. |
| HC-H6 | **Dragonfly/KeyDB untested** | Redis alternatives not battle-tested in production. | Add compatibility tests. Document Redis vs Dragonfly differences. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| HC-M1 | **Container registry undefined** | ghcr.io or custom registry? | Default to ghcr.io. Add configurable registry override. |
| HC-M2 | **Base image undefined** | distroless, alpine, or debian-slim? | Use distroless for security. Provide alpine variant for debugging. |
| HC-M3 | **Multi-arch builds undefined** | amd64 only? arm64 support? | Build multi-arch (amd64, arm64). Document platform requirements. |
| HC-M4 | **Ingress TLS configuration basic** | TLS setup requires manual cert management. | Add cert-manager integration. Document Let's Encrypt setup. |

---

## 7. add-multi-tenant-governance (RBAC + Drift)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| MT-C1 | **Tenant data isolation unverified** | SQL injection could expose cross-tenant data. | Add query parameterization. Implement row-level security in PostgreSQL. Add penetration testing. |
| MT-C2 | **RBAC policy testing absent** | No automated tests for role permissions. Misconfigurations go undetected. | Add RBAC integration tests. Test all role-action-resource combinations. |
| MT-C3 | **Drift detection false positives** | Semantic similarity can flag legitimate differences as drift. | Add drift threshold configuration. Implement drift suppression rules. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| MT-H1 | **Event streaming reliability** | Governance notifications can be lost if consumer down. | Add event persistence. Implement at-least-once delivery. Add dead letter queue. |
| MT-H2 | **Batch job scheduling conflict** | Drift detection jobs can overlap if running long. | Add distributed lock. Implement job deduplication. Add timeout. |
| MT-H3 | **Tenant context propagation** | Forgetting to pass TenantContext causes data leakage. | Add middleware that requires TenantContext. Fail-closed on missing context. |
| MT-H4 | **Permit.io dependency** | External SaaS dependency. What if Permit.io down? | Add OPA/Cedar fallback. Implement local policy cache. |
| MT-H5 | **Dashboard API authentication** | How are dashboard endpoints authenticated? | Add JWT validation. Integrate with OPAL auth. Add CORS configuration. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| MT-M1 | **Drift resolution workflow** | How are detected drifts resolved? Manual only? | Add auto-remediation option. Create drift resolution proposals. |
| MT-M2 | **Role hierarchy conflicts** | What if user has conflicting roles across teams? | Document role precedence. Implement most-permissive or least-permissive policy. |
| MT-M3 | **Audit log searchability** | Large audit logs hard to search efficiently. | Add full-text search. Implement log aggregation. Add filtering API. |

---

## 8. add-reflective-reasoning (MemR³)

### Critical Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| MR-C1 | **Reasoning step latency unbounded** | Complex queries can take 10s+ in reasoning phase. User experience degraded. | Add hard timeout (3s). Return un-refined query on timeout with warning. |

### High Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| MR-H1 | **LLM cost per query** | Every search now requires LLM call for reasoning. Costs increase 2-5x. | Add reasoning cache. Skip reasoning for simple queries. Make reasoning optional. |
| MR-H2 | **Fallback on reasoning failure** | If LLM fails, query fails entirely. | Add graceful fallback to non-reasoned search. Log reasoning failures. |
| MR-H3 | **Query refinement caching** | Same query refined repeatedly. Wasted LLM calls. | Cache query→refined_query mappings. Add TTL (1 hour). |
| MR-H4 | **Multi-hop retrieval depth limit** | Unbounded hop depth can cause exponential query explosion. | Add max hop depth (3). Implement early termination on low-relevance paths. |

### Medium Gaps

| ID | Gap | Problem | Solution |
|----|-----|---------|----------|
| MR-M1 | **Reasoning transparency** | User doesn't see why certain results ranked higher. | Add reasoning explanation to results. Show reasoning path. |
| MR-M2 | **A/B testing support** | Hard to compare reasoned vs non-reasoned performance. | Add feature flag. Implement shadow mode for comparison. |
| MR-M3 | **Reasoning model selection** | Which model for reasoning? Same as main LLM or specialized? | Make model configurable. Recommend smaller model for cost efficiency. |

---

## Cross-Cutting Gaps

These gaps affect multiple changes:

| ID | Gap | Affected Changes | Solution |
|----|-----|------------------|----------|
| CC-1 | **Observability inconsistent** | All | Standardize on OpenTelemetry. Add common tracing middleware. |
| CC-2 | **Error handling non-uniform** | All | Create shared error types. Implement consistent error responses. |
| CC-3 | **Configuration validation absent** | All | Add config schema validation on startup. Fail fast on invalid config. |
| CC-4 | **Testing coverage gaps** | All | Require 80% coverage. Add integration tests for all critical paths. |
| CC-5 | **Documentation incomplete** | All | Add runbooks for operations. Document failure modes and recovery. |

---

## Priority Matrix

### Must Fix Before v1.0 (Critical + High severity)

1. **Data Integrity**: R1-C1, R1-C2, MT-C1, UX-C3
2. **Availability**: UX-C1, R1-C3, RAD-C2
3. **Cost Control**: CCA-C1, MR-H1, CCA-H1
4. **Security**: OC-C2, MT-C1, HC-C1
5. **Stability**: RAD-C1, OC-C1, HC-C2

### Should Fix Before Production Scale

1. All High severity gaps
2. Observability gaps (CC-1)
3. Testing coverage (CC-4)

### Nice to Have for GA

1. Medium severity gaps
2. Documentation (CC-5)
3. A/B testing support

---

## Implementation Order

Based on dependencies and risk:

1. **Phase 1: Data Safety** (Week 1-2)
   - Cascading deletion (R1-C1)
   - FK enforcement (R1-C2)
   - Tenant isolation verification (MT-C1)
   - Secret management (HC-C1)

2. **Phase 2: Availability** (Week 3-4)
   - OPAL HA (UX-C1)
   - Write-ahead queue (R1-C3)
   - Thread persistence (RAD-C2)
   - PDB configuration (HC-H1)

3. **Phase 3: Cost & Performance** (Week 5-6)
   - Summarization budget (CCA-C1)
   - Query caching (MR-H3)
   - Index optimization (R1-H1)
   - Session capture optimization (OC-H2)

4. **Phase 4: Observability** (Week 7-8)
   - OpenTelemetry integration (CC-1)
   - Health checks (R1-H8)
   - Audit logging (R1-M3)
   - Error handling standardization (CC-2)

5. **Phase 5: Polish** (Week 9-10)
   - Documentation (CC-5)
   - Testing coverage (CC-4)
   - Medium severity gaps
