## 1. Storage Migration (Iceberg)
- [x] 1.1 Add `iceberg` catalog extension loading commands to `storage/src/graph_duckdb.rs`
- [x] 1.2 Replace `COPY TO FORMAT PARQUET` methods with `INSERT INTO iceberg.tables` logic
- [x] 1.3 Implement optimistic concurrency and retries for Iceberg write conflicts
- [x] 1.4 Write Rust integration tests verifying DuckDB <-> Iceberg transactional behavior

## 2. Infrastructure as Code (OpenTofu)
- [x] 2.1 Scaffold `infrastructure/modules/providers/gcp/` (GKE, Cloud SQL, Memorystore, GCS, KMS)
- [x] 2.2 Scaffold `infrastructure/modules/providers/aws/` (EKS, RDS, ElastiCache, S3, KMS)
- [x] 2.3 Scaffold `infrastructure/modules/providers/azure/` (AKS, Azure DB, Azure Redis, Blob, KeyVault)
- [x] 2.4 Create `infrastructure/modules/application/aeterna-helm/` module to wrap standard Helm deployments

## 3. High Availability & Governance Resiliency
- [x] 3.1 Refactor OPAL Server Helm values (`deploy/helm/aeterna-opal/values.yaml`) for HA (replicas: 3, Redis backend)
- [x] 3.2 Implement Cedar local policy caching with TTL in `policies/src/evaluator.rs`
- [x] 3.3 Create PostgreSQL schema (`sync_states` table equivalent) for Radkit thread persistence in `agent-a2a/`
- [x] 3.4 Wire Radkit SDK initialization to pull from PostgreSQL instead of in-memory maps

## 4. Security & Compliance
- [x] 4.1 Bind GCP Workload Identity (and AWS IRSA) annotations to Aeterna K8s Service Accounts
- [x] 4.2 Validate Cloud KMS / CMEK enforcement on all data-at-rest buckets/databases
- [x] 4.3 Implement cascading delete for GDPR across memory edges and DuckDB Iceberg tables

## 5. SOTA AI Reasoning (GraphRAG & Evolution)
- [x] 5.1 Implement Leiden community detection algorithm over DuckDB nodes/edges in `storage/src/graph.rs`
- [x] 5.2 Implement hierarchical community LLM summarization async workers (Microsoft GraphRAG pattern)
- [x] 5.3 Replace static Vector DB memory TTLs with dynamic LRU/LFU usage-based decay scores
- [x] 5.4 Implement Positional Index Encoding pointers from L2 context summaries to raw memory vectors