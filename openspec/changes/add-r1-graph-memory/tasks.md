## 1. Memory-R1 Implementation
- [x] 1.1 Implement `RewardFunction` for memory trajectory evaluation
- [x] 1.2 Add `prune()` and `compress()` methods to `MemoryManager`
- [x] 1.3 Create `MemoryR1Trainer` for outcome-driven policy updates

## 2. Dynamic Knowledge Graph (DuckDB + DuckPGQ)
- [ ] 2.1 Add `duckdb` crate to `storage/Cargo.toml` with bundled feature
- [ ] 2.2 Implement `GraphStore` struct with DuckDB connection management
- [ ] 2.3 Create SQL schema for memory_nodes, memory_edges, entities, entity_edges
- [ ] 2.4 Define SQL/PGQ property graphs (memory_graph, entity_graph)
- [ ] 2.5 Implement `find_related()` - neighbor traversal within N hops
- [ ] 2.6 Implement `shortest_path()` - path finding between memories
- [ ] 2.7 Add S3 persistence support (load_from_s3, persist_to_s3)
- [ ] 2.8 Create `EntityExtractor` trait with LLM-based implementation
- [ ] 2.9 Implement community detection for memory clustering
- [ ] 2.10 Write unit tests for GraphStore (>80% coverage)

## 3. Integration & Tooling
- [ ] 3.1 Add `graph_query` tool to `tools/` with MCP interface
- [ ] 3.2 Add `graph_neighbors` tool for related memory lookup
- [ ] 3.3 Add `memory_optimize` tool for R1-led pruning
- [ ] 3.4 Integrate GraphStore with MemoryManager (auto-update on add/delete)
- [ ] 3.5 Write E2E tests for graph-based reasoning
- [ ] 3.6 Add GraphStore configuration to `config/` module

## 4. Data Integrity (Critical Gaps R1-C1, R1-C2)
- [ ] 4.1 Implement cascading deletion for memory_nodes, memory_edges, entities, entity_edges
- [ ] 4.2 Add soft-delete with `deleted_at` timestamp and deferred cleanup job
- [ ] 4.3 Implement application-level FK validation on edge creation
- [ ] 4.4 Add periodic integrity scan job with orphan detection
- [ ] 4.5 Write tests for cascade deletion (>80% coverage)
- [ ] 4.6 Write tests for referential integrity validation

## 5. Concurrency & Write Coordination (Critical Gap R1-C3)
- [ ] 5.1 Implement Redis-backed write queue for serialized writes
- [ ] 5.2 Add distributed lock (SETNX) for Lambda cold start coordination
- [ ] 5.3 Implement exponential backoff for lock acquisition
- [ ] 5.4 Add write contention metrics (queue depth, wait time, timeout rate)
- [ ] 5.5 Write tests for concurrent write scenarios
- [ ] 5.6 Add alerting threshold configuration for contention

## 6. Transactional S3 Persistence (Critical Gap R1-C4)
- [ ] 6.1 Implement two-phase commit for Parquet export (temp prefix → atomic rename)
- [ ] 6.2 Add checksum validation for exported files
- [ ] 6.3 Implement export failure recovery with temp file cleanup
- [ ] 6.4 Add checksum validation on S3 load
- [ ] 6.5 Implement fallback to previous snapshot on corruption
- [ ] 6.6 Write tests for partial export failure scenarios

## 7. Performance Optimization (High Gaps R1-H1, R1-H7)
- [ ] 7.1 Create composite indexes: `idx_edges_tenant_source`, `idx_edges_tenant_target`
- [ ] 7.2 Create single-column indexes: `idx_nodes_tenant`, `idx_entities_tenant`
- [ ] 7.3 Implement lazy partition loading for Lambda cold start
- [ ] 7.4 Add cold start budget enforcement (3s limit)
- [ ] 7.5 Implement partition access tracking for pre-warming
- [ ] 7.6 Add warm pool strategy for provisioned concurrency
- [ ] 7.7 Write benchmark tests for query performance with indexes

## 8. Observability (High Gap R1-H2)
- [ ] 8.1 Add OpenTelemetry spans for `find_related()` and `shortest_path()`
- [ ] 8.2 Record span attributes: query type, tenant_id, hop count, result count, duration_ms
- [ ] 8.3 Implement Prometheus metrics: `graph_query_duration_seconds`, `graph_query_result_count`
- [ ] 8.4 Add `graph_cache_hit_ratio` and `graph_traversal_depth` metrics
- [ ] 8.5 Write tests for telemetry emission

## 9. Security & Tenant Isolation (High Gap R1-H3)
- [ ] 9.1 Implement parameterized tenant filter for all queries
- [ ] 9.2 Add query validation layer to reject tenant filter bypass attempts
- [ ] 9.3 Add security audit logging for rejected queries
- [ ] 9.4 Write penetration tests for tenant isolation
- [ ] 9.5 Add tenant context validation middleware

## 10. Backup & Recovery (High Gap R1-H4)
- [ ] 10.1 Implement scheduled S3 snapshot job (configurable interval)
- [ ] 10.2 Add snapshot versioning with retention policy
- [ ] 10.3 Implement point-in-time recovery from snapshots
- [ ] 10.4 Add backup duration and size metrics
- [ ] 10.5 Write tests for backup and recovery workflows

## 11. Transaction Atomicity (High Gap R1-H5)
- [ ] 11.1 Wrap multi-table inserts in single transaction
- [ ] 11.2 Configure SERIALIZABLE isolation level
- [ ] 11.3 Implement transaction rollback on partial failure
- [ ] 11.4 Write tests for atomic multi-table operations

## 12. Health Checks (High Gap R1-H8)
- [ ] 12.1 Implement `/health/graph` endpoint with DuckDB connectivity check
- [ ] 12.2 Add S3 bucket accessibility check to health endpoint
- [ ] 12.3 Implement `/ready/graph` endpoint for readiness probe
- [ ] 12.4 Add latency measurements to health response
- [ ] 12.5 Write tests for health check endpoints

## 13. Schema Migrations (High Gap R1-H9)
- [ ] 13.1 Create `schema_version` table for version tracking
- [ ] 13.2 Implement migration runner on startup
- [ ] 13.3 Add migration rollback on failure
- [ ] 13.4 Ensure migrations are backward compatible (additive only)
- [ ] 13.5 Write tests for migration scenarios
