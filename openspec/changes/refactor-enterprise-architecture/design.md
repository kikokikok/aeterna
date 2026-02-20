## Context
Aeterna's storage layer abstracted 11 vector/SQL/graph databases flawlessly but ignored Cloud Infrastructure resilience, DR, and consistency. 
1. `DuckDB`'s current behavior of directly using raw S3 Parquet `COPY` writes causes data corruption on partial failures (single-writer locks, no transaction safety). 
2. Deploying Aeterna currently relies purely on a Helm chart, which ignores the highly available managed databases required for production (PostgreSQL HA, Redis HA).
3. Security keys and OPAL deployments are SPOFs.

## Goals / Non-Goals
- Goals: Guarantee High Availability (HA) across all services. Ensure zero data loss for the DuckDB Knowledge Graph during Pod crashes. Enable instant deployment to GCP, AWS, and Azure via OpenTofu. Remove OPAL as a single point of failure.
- Non-Goals: Rewriting Aeterna's abstraction layer. We are migrating the infrastructure and adapting DuckDB to Iceberg, not rebuilding Aeterna's trait abstractions.

## Decisions
- **Decision 1: Apache Iceberg over S3 Parquet**. DuckDB 1.4+ natively supports Apache Iceberg. Writing to Iceberg automatically handles ACID guarantees across multi-table updates. We will migrate `storage/src/graph_duckdb.rs` to use `INSERT INTO iceberg.tables` instead of `COPY TO ... FORMAT PARQUET`.
- **Decision 2: SOTA AI Capabilities (GraphRAG & Memory Evolution)**. To meet 2026 academic standards, we will implement Leiden community detection in the DuckDB graph layer for hierarchical Microsoft GraphRAG summarization. We will also replace static memory TTLs with dynamic LRU/LFU memory decay based on active usage feedback (EVOLVE-MEM pattern).
- **Decision 3: OpenTofu Multi-Cloud IaC**. We will manage state and infrastructure via OpenTofu, providing 3 provider implementations: AWS, Azure, and GCP. The OpenTofu modules will inject their highly available database endpoints securely into the Aeterna Helm chart.
- **Decision 4: HA OPAL & Redis**. OPAL Server will scale to `replicas: 3` and be backed by a managed HA Redis, making the Cedar auth system highly available.

## Risks / Trade-offs
- **Iceberg Complexity**: Introducing Iceberg means introducing a catalog (REST or AWS Glue). 
  - *Mitigation*: We will use a lightweight REST catalog container for testing, and native Glue/Vertex catalog for production.
- **GraphRAG Compute Costs**: Leiden community detection and hierarchical summarization requires massive LLM token overhead compared to simple vector retrieval.
  - *Mitigation*: Run GraphRAG summarization as an asynchronous background job triggered only when threshold edge modifications occur.
- **Radkit Thread State PostgreSQL**: Adding PostgreSQL for Radkit threads adds I/O latency to A2A operations.
  - *Mitigation*: Use Redis cache in front of Postgres thread serialization.

## Migration Plan
1. Create OpenTofu `infrastructure/` directory and configure GCP GKE + Cloud SQL + GCS + Memorystore.
2. Refactor `storage/src/graph_duckdb.rs` to use DuckDB's Iceberg extension.
3. Update Aeterna Helm charts to consume dynamically generated Cloud KMS keys and IAM Workload Identity annotations.
4. Scale OPAL Server and write A2A Radkit persistence layer.