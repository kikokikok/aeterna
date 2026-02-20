# Change: Enterprise Multi-Cloud Architecture & Iceberg Migration

## Why
Aeterna's core engine, including the DuckDB Knowledge Graph, Multi-tenant Governance (OPAL/Cedar), and Agentic interfaces, is functionally complete. However, when evaluating the system for Tier-1 Enterprise Production (300+ engineers, mission-critical workloads), 19 "Critical" and 47 "High" severity gaps remain. These gaps primarily relate to High Availability (HA), Disaster Recovery (DR), Data consistency during S3 partial failures, Single-Point-Of-Failure (SPOF) risks in OPAL, and a lack of Infrastructure-as-Code (IaC).

We need to transition Aeterna from a "containerized application" into a "Cloud-Native Enterprise Platform" by leveraging OpenTofu for multi-cloud deployments and Apache Iceberg for transactional knowledge graph storage.

## What Changes
- ****BREAKING** Storage Engine Migration**: Replace raw S3 Parquet `COPY` writes in DuckDB with Apache Iceberg catalogs. This natively solves transactional writes, concurrent writer contention, and provides schema evolution and time-travel backups.
- **SOTA AI Capabilities (GraphRAG & Dynamic Memory)**: Implement Leiden community detection for hierarchical graph summarization (Microsoft GraphRAG pattern) and dynamic LRU/LFU memory decay based on agent usage, closing the theoretical gap with 2026 academic research.
- **Multi-Cloud IaC (OpenTofu)**: Implement a highly modular OpenTofu repository to provision GCP (GKE/Cloud SQL), AWS (EKS/RDS), and Azure (AKS/Azure PostgreSQL) environments.
- **HA/DR Governance**: Redesign the OPAL/Cedar deployment to run in HA mode (3+ replicas) to eliminate the authorization SPOF. Add local policy caching with TTLs.
- **Agent Coordination Resilience**: Add PostgreSQL thread persistence to Radkit A2A to ensure in-memory conversations survive pod restarts.
- **Zero-Trust Security**: Implement Workload Identity Federation (GCP Workload Identity / AWS IRSA) to replace static IAM JSON keys, combined with KMS for Customer Managed Encryption Keys (CMEK) at rest.

## Impact
- Affected specs: `storage`, `deployment`, `governance`, `agent-coordination`, `knowledge`
- Affected code: `storage/src/graph_duckdb.rs`, `deploy/helm/aeterna-opal/`, `agent-a2a/`
- Infrastructure: Introduces `infrastructure/` directory with OpenTofu modules.