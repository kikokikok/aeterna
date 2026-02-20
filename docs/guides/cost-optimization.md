# Cost Optimization Guide

## Overview

Aeterna provides mechanisms to reduce operational costs by optimizing embedding generation and storage usage. This guide describes semantic caching, tiered storage, and budget management.

## Semantic Caching Tuning

Semantic caching prevents duplicate embedding generation for identical or highly similar inputs. This can result in 60-80% cost savings for embedding-heavy workloads.

### 1. Configure the Similarity Threshold

The threshold determines how similar a new request must be to a cached entry to count as a "hit".

- **0.99+**: Very strict, highest quality, lower hit rate.
- **0.98**: Default, optimal balance of cost and performance.
- **0.95**: Aggressive, higher hit rate, potential for slight context drift.

**Example Configuration Snippet**:
```toml
[memory.cache]
provider = "redis"
similarity_threshold = 0.98
ttl_seconds = 86400  # 24 hours
max_entries = 1000000
```

### 2. Time-To-Live (TTL) Settings

Tune TTL based on your workload's temporal locality. For rapidly changing contexts, use shorter TTLs (e.g., 4-8 hours). For stable organizational knowledge, use longer TTLs (e.g., 7-30 days).

## Tiered Storage Configuration

Aeterna manages data across three tiers to balance performance and cost.

1. **Hot Tier (Redis)**:
   - **Data**: Most recent memories (< 7 days).
   - **Purpose**: Low-latency search and frequent access.
   - **Cost**: High (RAM-based).

2. **Warm Tier (PostgreSQL + pgvector)**:
   - **Data**: Memories from 7 to 90 days.
   - **Purpose**: Reliable long-term storage and occasional retrieval.
   - **Cost**: Moderate (SSD-based).

3. **Cold Tier (S3 + Parquet)**:
   - **Data**: Archived memories (> 90 days).
   - **Purpose**: Disaster recovery and historical analysis.
   - **Cost**: Low (Object storage).

### Tier Thresholds

Configure the migration thresholds in your deployment values or service configuration:

```yaml
storage:
  tiering:
    enabled: true
    hot_to_warm_days: 7
    warm_to_cold_days: 90
    auto_migration_job_schedule: "0 2 * * *"  # Daily at 2 AM
```

## Budget Management

Aeterna allows per-tenant budget limits for embedding and storage costs.

### 1. Set Tenant Limits

Define quotas for each tenant to prevent runaway costs from a single customer.

**Example Policy**:
```json
{
  "tenant_id": "acme-corp",
  "quotas": {
    "monthly_embedding_usd": 500,
    "storage_gb": 50,
    "max_qps": 100
  }
}
```

### 2. Alert Configuration

Configure alerts in the **Cost Analysis** dashboard to notify operators when a tenant reaches 80% and 100% of their budget.

- **80% Alert**: Send warning to Slack channel.
- **100% Alert**: Send critical notification to PagerDuty and trigger automatic rate-limiting for the tenant.

## Optimization Best Practices

- **Batch Embeddings**: Always use the batch embedding tool for multiple memories to reduce API overhead.
- **De-duplication**: Enable the `memory_optimize` tool to run autonomous background de-duplication jobs.
- **Compression**: Use the **Context Architect** agent to compress long memory trajectories before storing them in the warm or cold tiers.
- **Review Costs Regularly**: Use the **Cost Analysis** dashboard to identify high-cost tenants and optimize their cache hit rates.

## Expected Cost Reductions

| Optimization | Target Savings | Metrics to Watch |
|--------------|----------------|------------------|
| Semantic Caching | 60-80% | `aeterna_cache_hit_ratio` |
| Tiered Storage | 40% | `aeterna_storage_cost_per_gb` |
| Query Optimization | 15-25% | `aeterna_query_execution_time` |
| CCA Compression | 20-30% | `aeterna_memory_compressed_size` |
