# Managed Services Evaluation for Aeterna

## Overview

This guide evaluates three managed service alternatives to Aeterna's default self-hosted infrastructure. Each service maps directly to a core storage dependency and can be adopted independently via Aeterna's pluggable adapter architecture.

| Self-Hosted Default | Managed Alternative | Role in Aeterna |
|---------------------|---------------------|-----------------|
| Qdrant (cluster mode) | **Qdrant Cloud** | Vector database for memory embeddings |
| Redis Sentinel | **Upstash Redis** | Hot-tier cache, session state, pub/sub |
| PostgreSQL + Patroni | **Neon** | Relational store, pgvector, knowledge metadata |

## 1. Qdrant Cloud

**What it is**: Fully managed Qdrant clusters operated by Qdrant GmbH, available on AWS, GCP, and Azure.

### Pros

- **Zero-ops clustering**: Automatic replication, sharding, and failover — eliminates the need to operate a 3-node Qdrant cluster and monitor shard health.
- **Seamless compatibility**: Same gRPC/REST API as self-hosted Qdrant. Aeterna's `QdrantBackend` works without code changes; only the connection URL and API key change.
- **Horizontal scaling on demand**: Add capacity through the dashboard without downtime. No manual shard rebalancing.
- **Built-in snapshots and backups**: Automated, configurable retention — replaces the custom 6-hour snapshot scheduling described in the HA/DR design.
- **Multi-AZ by default**: Production clusters span availability zones automatically, matching the target architecture.

### Cons

- **Vendor lock-in**: Dependent on Qdrant GmbH's cloud platform. Mitigated by the fact that the API is identical to self-hosted, so switching back is a configuration change.
- **Latency overhead**: Adds network hop compared to in-cluster Qdrant. Typical added latency is 1–5 ms for same-region deployments.
- **Cost at scale**: More expensive per GB than self-hosted for large, stable workloads (see pricing below).
- **Limited region availability**: Fewer regions than major hyperscalers; verify your target region is supported.
- **Data residency**: Data lives on Qdrant's managed infrastructure. May require contractual review for regulated industries.

### Pricing Model

| Tier | Included | Price |
|------|----------|-------|
| Free | 1 GB RAM, 1 node | $0 |
| Standard | Multi-node, configurable RAM/disk | ~$0.045/GB-hour (~$33/GB-month) |
| Enterprise | Dedicated infra, SLA, support | Custom pricing |

Storage costs scale linearly with vector count and dimensionality. For Aeterna's default 1536-dimension embeddings, expect roughly 6 KB per vector (embedding + metadata overhead).

**Example**: 10 M vectors ≈ 60 GB → ~$2,000/month on Standard tier.

### Migration Path

1. Create a Qdrant Cloud cluster in your target region.
2. Set `QDRANT_URL` and `QDRANT_API_KEY` environment variables to the cloud endpoint.
3. Use Qdrant's snapshot export/import to migrate existing collections, or let Aeterna repopulate on first write.
4. Verify with `health_check()` — no code changes needed.

## 2. Upstash Redis

**What it is**: Serverless, pay-per-request Redis compatible service with global replication. Based on a custom storage engine that persists data to disk.

### Pros

- **True serverless pricing**: Pay per command (~$0.2 per 100K commands), no idle cost. Ideal for development, staging, and bursty production workloads.
- **Global replication**: Multi-region read replicas with single-digit-ms reads from edge. Useful for geographically distributed Aeterna deployments.
- **Persistent by default**: Data is persisted to disk, eliminating the need for separate RDB/AOF backup configuration.
- **TLS and authentication built in**: Encrypted connections out of the box — no manual TLS certificate management.
- **REST API fallback**: In addition to the standard Redis protocol, offers a REST API for environments where raw TCP connections are restricted (e.g., serverless functions).

### Cons

- **Not fully Redis-compatible**: Most commands are supported, but some advanced features (certain Lua scripting edge cases, `MODULE LOAD`) may behave differently. Test Aeterna's pub/sub and Sentinel-dependent code paths.
- **No Sentinel/Cluster topology**: Upstash manages failover internally. If Aeterna code directly references Sentinel endpoints for leader discovery, that code path must be bypassed.
- **Throughput limits on lower tiers**: Free tier caps at 10K commands/day. Production workloads with high QPS should provision the Pro tier.
- **Tail latency**: P99 latency can be higher than self-hosted Redis in the same cluster network (~2–10 ms vs <1 ms).
- **Vendor-specific SDK recommended**: While standard Redis clients work, Upstash provides its own SDK for REST access. Not required but encouraged in their docs.

### Pricing Model

| Tier | Commands/day | Price |
|------|-------------|-------|
| Free | 10K | $0 |
| Pay-as-you-go | Unlimited | ~$0.2/100K commands |
| Pro (fixed) | Unlimited | From $10/month (higher throughput guarantees) |
| Enterprise | Unlimited | Custom |

Storage: ~$0.25/GB-month.

**Example**: 50 M commands/month + 5 GB storage ≈ $100 + $1.25 = ~$101/month.

### When to Use

- **Use Upstash when**: Running Aeterna in a serverless or edge environment, optimizing for low-traffic cost efficiency, or deploying to multiple regions with read replicas.
- **Stay with Redis Sentinel when**: Running high-throughput workloads (>100K ops/sec) where sub-millisecond latency is critical, or when full Lua scripting compatibility is required.

### Migration Path

1. Create an Upstash Redis database in the target region.
2. Replace `REDIS_URL` with the Upstash connection string (includes TLS by default).
3. If using Sentinel-based discovery in Aeterna, switch to direct connection mode.
4. Test pub/sub channels used for real-time collaboration — verify message ordering.

## 3. Neon

**What it is**: Serverless PostgreSQL with copy-on-write branching, autoscaling compute, and scale-to-zero. Built on a custom storage engine that separates compute from storage.

### Pros

- **Branching for staging/preview**: Create instant, zero-copy database branches for staging environments, PR previews, or migration testing. Each branch is a full PostgreSQL instance at near-zero additional storage cost (copy-on-write).
- **Scale-to-zero**: Compute scales down to zero after inactivity (configurable), eliminating cost for idle dev/staging databases.
- **pgvector support**: Full pgvector extension support including HNSW indexes. Aeterna's `PgvectorBackend` works without modification.
- **Automatic backups and PITR**: Point-in-time recovery without manual WAL archiving configuration. Replaces the S3-based WAL archiving in the HA/DR design.
- **Connection pooling built in**: PgBouncer-compatible pooling included, which simplifies Aeterna's connection management.
- **Autoscaling compute**: Compute units scale automatically based on load, from 0.25 to 10 CU.

### Cons

- **Cold start latency**: Scale-to-zero instances incur 0.5–3 second cold starts on first connection. Problematic for latency-sensitive production workloads unless "always-on" is enabled.
- **Storage-compute separation latency**: Neon's architecture adds ~1–5 ms read latency compared to local-disk PostgreSQL for random reads.
- **No synchronous replication control**: Patroni allows fine-grained synchronous replication tuning. Neon manages replication internally with eventual consistency for read replicas.
- **Limited extensions**: Most common extensions (pgvector, pg_trgm, hstore) are supported, but some system-level extensions may be unavailable.
- **Vendor lock-in**: Uses a proprietary storage engine. Exporting data is straightforward (standard pg_dump), but the branching and scale-to-zero features are not portable.

### Pricing Model

| Tier | Compute | Storage | Price |
|------|---------|---------|-------|
| Free | 0.25 CU, 10 branches | 512 MB | $0 |
| Launch | Up to 4 CU, autoscale | 10 GB included | From $19/month |
| Scale | Up to 10 CU, autoscale | 50 GB included | From $69/month |
| Enterprise | Custom CU | Custom | Custom |

Compute: ~$0.16/CU-hour. Storage: ~$0.033/GB-month (beyond included).

**Example**: 2 CU average × 730 hrs + 100 GB storage ≈ $234 + $1.65 = ~$236/month.

### pgvector Support Details

Neon supports pgvector 0.7+ with:
- `CREATE EXTENSION vector;` — available out of the box
- HNSW indexes (`CREATE INDEX ... USING hnsw`)
- IVFFlat indexes
- All distance operators (`<->`, `<#>`, `<=>`)

Aeterna's `PgvectorBackend` auto-creates the extension and HNSW index on first use — no changes needed.

### Migration Path

1. Create a Neon project and database.
2. Run `pg_dump` from existing Patroni primary, `pg_restore` to Neon.
3. Update `PGVECTOR_URL` / `DATABASE_URL` to the Neon connection string.
4. Enable "always-on" for the production branch to avoid cold-start latency.
5. Create branches for staging and CI environments.

## Recommendation Matrix

| Criterion | Qdrant Cloud | Upstash Redis | Neon |
|-----------|:---:|:---:|:---:|
| **Ops burden reduction** | High | High | High |
| **Latency vs self-hosted** | +1–5 ms | +2–10 ms | +1–5 ms (warm) |
| **Cost at low traffic** | Medium | Very Low | Low |
| **Cost at high traffic** | Higher than self-hosted | Higher than self-hosted | Comparable |
| **Scalability** | Excellent (managed sharding) | Excellent (serverless) | Excellent (autoscale CU) |
| **Data residency control** | Medium | Medium | Medium |
| **Migration effort** | Config change only | Config change + Sentinel bypass | pg_dump/restore + config |
| **Feature parity** | Full (same API) | ~95% Redis compat | Full pgvector support |
| **Lock-in risk** | Low (same API) | Low (Redis protocol) | Medium (proprietary storage) |

### Cost Comparison at Scale

| Workload Profile | Self-Hosted (3-node HA) | Managed |
|------------------|------------------------|---------|
| **Vector DB**: 10 M vectors, 1536-dim | ~$500/mo (3× r6g.xlarge) | ~$2,000/mo (Qdrant Cloud) |
| **Redis**: 50 M cmds/mo, 5 GB | ~$300/mo (3× r6g.large) | ~$100/mo (Upstash) |
| **PostgreSQL**: 100 GB, 2 CU avg | ~$400/mo (Patroni 3-node) | ~$236/mo (Neon) |
| **Total** | ~$1,200/mo + ops team | ~$2,336/mo, zero ops |

**Note**: Self-hosted costs exclude engineer time for operations, on-call, upgrades, and incident response. For small teams, managed services are typically more cost-effective when fully loaded ops costs are included.

## Decision Tree: When to Choose Managed

```
Start
├── Team has < 2 dedicated infra engineers?
│   ├── YES → Prefer managed services
│   └── NO ↓
├── Running in serverless / edge environment?
│   ├── YES → Upstash Redis + Neon (scale-to-zero)
│   └── NO ↓
├── Need instant staging/preview environments?
│   ├── YES → Neon (branching)
│   └── NO ↓
├── Workload is bursty with long idle periods?
│   ├── YES → Managed (pay-per-use)
│   └── NO ↓
├── Sustained high throughput (>100K ops/sec)?
│   ├── YES → Self-hosted (cost + latency)
│   └── NO ↓
├── Strict data residency / air-gapped requirement?
│   ├── YES → Self-hosted
│   └── NO ↓
└── Default → Managed for dev/staging, evaluate for production
```

## Hybrid Strategy (Recommended)

For most Aeterna deployments, a hybrid approach works best:

| Environment | Vector DB | Redis | PostgreSQL |
|-------------|-----------|-------|------------|
| Development | Qdrant Cloud (free tier) | Upstash (free tier) | Neon (free tier) |
| Staging | Qdrant Cloud (standard) | Upstash (pay-as-you-go) | Neon branch |
| Production (small) | Qdrant Cloud | Upstash Pro | Neon Scale |
| Production (large) | Self-hosted Qdrant cluster | Self-hosted Redis Sentinel | Self-hosted Patroni |

All transitions are configuration-only thanks to Aeterna's provider adapter architecture. See [Provider Adapters Guide](provider-adapters.md) for implementation details.
