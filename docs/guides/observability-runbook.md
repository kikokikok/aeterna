# Observability Runbook

## Overview

This runbook provides standardized procedures for responding to alerts, navigating observability dashboards, and investigating system behavior in Aeterna.

## Dashboard Navigation

Aeterna provides 5 core Grafana dashboards for system monitoring:

1. **System Health**: CPU, memory, network, and disk usage for all services.
2. **Memory Operations**: Throughput, latency, and error rates for memory reads/writes.
3. **Knowledge Queries**: Performance metrics for knowledge repository searches and sync operations.
4. **Governance & Compliance**: Policy evaluation counts, violation trends, and tenant isolation status.
5. **Cost Analysis**: Per-tenant embedding and storage costs against configured budgets.

## Alert Response Procedures

### 1. High Latency Spike (p95 > 200ms)

**Trigger**: `p99_latency_high` alert from Grafana.

**Investigation**:
1. Open the **Memory Operations** dashboard.
2. Filter by `tenant_id` if specific to one customer.
3. Use Tempo to search for traces with duration > 200ms.
4. Identify if the bottleneck is in **Qdrant** (vector search) or **PostgreSQL** (metadata retrieval).

**Remediation**:
- If vector search is slow: Check Qdrant collection optimization status.
- If metadata retrieval is slow: Verify database indexing and query execution plans.
- Scale services if the bottleneck is in the Aeterna application layer.

### 2. Elevated Error Rate (> 5%)

**Trigger**: `memory_error_rate_high` alert.

**Investigation**:
1. Navigate to the **Loki Logs** panel in Grafana.
2. Search for logs with `level: error` or `level: fatal`.
3. Filter logs by the `trace_id` from failing requests.
4. Check for common failure patterns (e.g., database connection timeouts, API rate limits).

**Remediation**:
- Restart failing service instances if the error is intermittent.
- Increase connection pool sizes for PostgreSQL or Redis.
- Scale up the affected service (HPA should handle this automatically).

### 3. Cost Budget Breach (> 100%)

**Trigger**: `budget_exceeded` alert.

**Investigation**:
1. Open the **Cost Analysis** dashboard.
2. Identify the `tenant_id` that exceeded its budget.
3. Review the **Semantic Cache Hit Rate** for that tenant.
4. Check if a high volume of new embeddings is being generated.

**Remediation**:
- Contact the tenant owner to discuss budget increases.
- Apply a temporary rate limit to the tenant's embedding requests.
- Tune the semantic cache threshold to increase hit rates.

### 4. Anomaly Detected (Statistical Baseline Departure)

**Trigger**: Anomaly detection alert on system resources or operation frequency.

**Investigation**:
1. Compare the current metric against the historical baseline (last 7 days).
2. Look for correlated changes in other metrics (e.g., traffic spike + memory increase).
3. Investigate if the anomaly coincides with a new deployment or configuration change.

**Remediation**:
- Roll back recent deployments if they correlate with the anomaly.
- Increase resources if the anomaly is due to legitimate traffic growth.
- Isolate the source of the anomaly (e.g., a specific tenant's aggressive scraping).

## Trace Investigation

Aeterna uses OpenTelemetry for distributed tracing. Traces are linked to logs and metrics via `trace_id`.

**Standard Investigation Path**:
1. Find an error in **Loki Logs**.
2. Click the `trace_id` link in the log entry to jump to the corresponding **Tempo Trace**.
3. Analyze the span hierarchy to identify the specific operation that failed or was slow.
4. Review span metadata for request parameters and error details.

## Metric Summary Reference

| Metric Name | Type | Description |
|-------------|------|-------------|
| `aeterna_memory_ops_total` | Counter | Total memory operations (add, search, delete) |
| `aeterna_memory_latency_seconds` | Histogram | Latency of memory operations |
| `aeterna_knowledge_sync_duration` | Histogram | Time taken to sync memory to knowledge |
| `aeterna_governance_eval_total` | Counter | Number of policy evaluations performed |
| `aeterna_cost_embeddings_usd` | Counter | Cumulative cost of embedding generation |
| `aeterna_cache_hit_ratio` | Gauge | Semantic cache hit ratio (0.0 - 1.0) |
