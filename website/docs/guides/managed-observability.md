# Managed Observability Platform Integration

## Overview

Aeterna supports two managed observability platforms: **Grafana Cloud** (default) and **Datadog** (enterprise option). Both integrate with the existing OpenTelemetry pipeline.

## Decision

**Default**: Grafana Cloud — 80% of Datadog features at ~20% of the cost.  
**Enterprise option**: Datadog — best-in-class UX, advanced APM, higher cost.

## Grafana Cloud Setup

### 1. Create Grafana Cloud Account

Sign up at https://grafana.com/products/cloud/ (free tier: 50GB logs, 10k series).

### 2. Obtain Credentials

From Grafana Cloud portal → API Keys:
- **Prometheus remote_write** endpoint + API key
- **Loki** endpoint + API key  
- **Tempo** (traces) endpoint + API key

### 3. Deploy OpenTelemetry Collector

```yaml
# deploy/k8s/observability/otel-collector.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: otel-collector-config
  namespace: aeterna
data:
  otel-collector-config.yaml: |
    receivers:
      otlp:
        protocols:
          grpc:
            endpoint: 0.0.0.0:4317
          http:
            endpoint: 0.0.0.0:4318
      prometheus:
        config:
          scrape_configs:
            - job_name: aeterna-memory
              static_configs:
                - targets: [aeterna-memory:9091]
            - job_name: aeterna-knowledge
              static_configs:
                - targets: [aeterna-knowledge:9092]
            - job_name: aeterna-governance
              static_configs:
                - targets: [aeterna-governance:9093]
            - job_name: aeterna-sync
              static_configs:
                - targets: [aeterna-sync:9094]

    processors:
      batch:
        timeout: 10s
        send_batch_size: 1000
      memory_limiter:
        limit_mib: 512

    exporters:
      prometheusremotewrite:
        endpoint: https://prometheus-prod-XX.grafana.net/api/prom/push
        headers:
          Authorization: "Basic ${GRAFANA_PROMETHEUS_TOKEN}"
      loki:
        endpoint: https://logs-prod-XXX.grafana.net/loki/api/v1/push
        headers:
          Authorization: "Basic ${GRAFANA_LOKI_TOKEN}"
      otlp/tempo:
        endpoint: https://tempo-prod-XX.grafana.net:443
        headers:
          Authorization: "Basic ${GRAFANA_TEMPO_TOKEN}"
        tls:
          insecure: false

    service:
      pipelines:
        metrics:
          receivers: [otlp, prometheus]
          processors: [memory_limiter, batch]
          exporters: [prometheusremotewrite]
        logs:
          receivers: [otlp]
          processors: [memory_limiter, batch]
          exporters: [loki]
        traces:
          receivers: [otlp]
          processors: [memory_limiter, batch]
          exporters: [otlp/tempo]
```

### 4. Import Dashboards

Upload all 5 Grafana dashboard JSON files from `deploy/grafana/dashboards/` to Grafana Cloud:

1. `system-health.json` — System health overview
2. `memory-operations.json` — Memory read/write operations
3. `knowledge-queries.json` — Knowledge query performance
4. `governance-compliance.json` — Policy evaluations and violations
5. `cost-analysis.json` — Per-tenant cost and budget tracking

**Import method**: Grafana UI → Dashboards → Import → Upload JSON

### 5. Configure SLO Dashboards

In Grafana Cloud → SLO tab, create the core SLOs:

| SLO | Target | Window |
|-----|--------|--------|
| Memory API P99 < 200ms | 99.9% | 30d |
| Knowledge API P99 < 500ms | 99.5% | 30d |
| Service availability | 99.9% | 30d |
| Error rate < 1% | 99% | 7d |

#### SLO Dashboard Panels

Create a dedicated SLO dashboard (import `deploy/grafana/dashboards/slo-overview.json` or build manually) with these panels:

**Error Budget Burn Rate**

Track how quickly each SLO consumes its error budget. A burn rate > 1.0 means the budget is depleting faster than the window allows.

```promql
# 1-hour burn rate for memory API latency SLO
1 - (
  sum(rate(aeterna_memory_api_duration_seconds_bucket{le="0.2"}[1h]))
  /
  sum(rate(aeterna_memory_api_duration_seconds_count[1h]))
) / (1 - 0.999)
```

**Multi-Window Burn Rate Alerts**

Configure alerts using the multi-window, multi-burn-rate method recommended by Google SRE:

| Severity | Long Window | Short Window | Burn Rate | Budget Consumed |
|----------|-------------|--------------|-----------|-----------------|
| Critical (page) | 1h | 5m | 14.4× | 2% in 1h |
| Warning (ticket) | 6h | 30m | 6× | 5% in 6h |
| Info (log) | 3d | 6h | 1× | 10% in 3d |

**Per-Service SLO Panel**

| Panel | PromQL | Visualization |
|-------|--------|---------------|
| Memory API latency SLO | `histogram_quantile(0.99, sum(rate(aeterna_memory_api_duration_seconds_bucket[5m])) by (le))` | Gauge (green < 200ms) |
| Knowledge API latency SLO | `histogram_quantile(0.99, sum(rate(aeterna_knowledge_api_duration_seconds_bucket[5m])) by (le))` | Gauge (green < 500ms) |
| Availability (success rate) | `sum(rate(aeterna_http_requests_total{status!~"5.."}[5m])) / sum(rate(aeterna_http_requests_total[5m]))` | Stat (target: 99.9%) |
| Error budget remaining | `1 - (error_ratio / (1 - slo_target))` | Time series (30d) |

**Tenant-Level SLO Tracking**

For multi-tenant deployments, add a variable `$tenant_id` and filter all SLO queries:

```promql
# Per-tenant availability
sum(rate(aeterna_http_requests_total{status!~"5..", tenant_id="$tenant_id"}[5m]))
/
sum(rate(aeterna_http_requests_total{tenant_id="$tenant_id"}[5m]))
```

This enables per-tenant SLA reporting and identifies tenants experiencing degraded service before aggregate SLOs breach.

### 6. Set Up Alerting

In Grafana Cloud → Alerting → Contact Points, configure:
- Slack webhook for warnings
- PagerDuty for critical alerts

Alert rules to create (import from `deploy/grafana/alerts/`):
- `memory_error_rate_high` — Error rate > 5% for 5 minutes
- `budget_exceeded` — Tenant budget > 100%
- `service_down` — `up == 0` for 2 minutes
- `p99_latency_high` — P99 > 2s for 10 minutes

## Datadog Setup (Enterprise)

### 1. Install Datadog Agent

```bash
helm repo add datadog https://helm.datadoghq.com
helm install datadog-agent datadog/datadog \
  --set datadog.apiKey=<DD_API_KEY> \
  --set datadog.apm.portEnabled=true \
  --set datadog.logs.enabled=true \
  --set clusterAgent.enabled=true
```

### 2. Configure OpenTelemetry Export to Datadog

Add to the OTel Collector config under `exporters`:

```yaml
datadog:
  api:
    key: ${DD_API_KEY}
    site: datadoghq.com
```

### 3. Enable Datadog APM

Aeterna services emit OTLP traces. Configure:
```yaml
# In aeterna config
tracing:
  exporter: datadog
  agent_host: datadog-agent.default.svc:4317
```

## Migrating Existing Metrics

All Aeterna metrics follow `aeterna_*` naming convention and are already emitted via the `observability` crate. No migration steps required — metrics flow through OTel Collector automatically once the collector is deployed.

## Cost Estimates

| Platform | Scale | Monthly Cost |
|----------|-------|-------------|
| Grafana Cloud Free | < 10k series | $0 |
| Grafana Cloud Pro | < 100k series | ~$50-200 |
| Grafana Cloud Advanced | > 100k series | ~$500+ |
| Datadog | Per host | $23-34/host/mo |
