# Log Aggregation with Aeterna

Aeterna outputs structured JSON logs by default. This guide covers integration with Grafana Loki and the ELK stack.

## Log Configuration

```yaml
observability:
  logging:
    level: info    # debug, info, warn, error
    format: json   # json, pretty
```

These values control the following environment variables on the Aeterna deployment:

| Variable | Value | Purpose |
|----------|-------|---------|
| `RUST_LOG` | `observability.logging.level` | Rust log level filter |
| `LOG_FORMAT` | `observability.logging.format` | Output format |

### Log Levels

| Level | Use Case |
|-------|----------|
| `error` | Production — only errors and panics |
| `warn` | Production — includes deprecation warnings |
| `info` | Default — request lifecycle, startup, shutdown |
| `debug` | Development — internal state, query plans, cache hits |

### Per-Component Filtering

The `RUST_LOG` directive supports fine-grained control:

```yaml
observability:
  logging:
    level: "info,aeterna_memory=debug,aeterna_storage=warn,tower_http=debug"
```

This sets the global level to `info`, enables `debug` for memory operations, restricts storage logs to `warn`, and enables HTTP request tracing.

## Grafana Loki + Promtail

### Deploy Loki Stack

```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update

helm install loki grafana/loki-stack \
  --namespace observability --create-namespace \
  --set promtail.enabled=true \
  --set loki.persistence.enabled=true \
  --set loki.persistence.size=50Gi
```

For production with S3 backend:

```bash
helm install loki grafana/loki \
  --namespace observability --create-namespace \
  --set loki.storage.type=s3 \
  --set loki.storage.s3.bucketnames=aeterna-logs \
  --set loki.storage.s3.endpoint=s3.amazonaws.com \
  --set loki.storage.s3.region=us-east-1 \
  --set loki.storage.s3.insecure=false \
  --set loki.compactor.retention_enabled=true \
  --set loki.limits_config.retention_period=720h
```

### Deploy Promtail

Promtail runs as a DaemonSet and ships pod logs to Loki automatically. No Aeterna-specific configuration is required — Promtail discovers pods via Kubernetes labels.

Verify Promtail is collecting Aeterna logs:

```bash
kubectl logs -l app.kubernetes.io/name=promtail -n observability | grep aeterna
```

### Grafana Data Source

```yaml
apiVersion: 1
datasources:
  - name: Loki
    type: loki
    access: proxy
    url: http://loki.observability.svc:3100
    jsonData:
      derivedFields:
        - datasourceUid: tempo
          matcherRegex: '"trace_id":"(\w+)"'
          name: TraceID
          url: "$${__value.raw}"
```

### Useful LogQL Queries

```logql
# All Aeterna error logs
{app="aeterna"} |= "ERROR"

# Memory operations slower than 100ms
{app="aeterna"} | json | duration > 100ms | component = "memory"

# Failed authorization decisions
{app="aeterna"} | json | msg =~ "authorization.*denied"

# Request rate by endpoint
sum(rate({app="aeterna"} | json | __error__="" [5m])) by (path)
```

## ELK Stack (Elasticsearch + Logstash + Kibana)

### Deploy with Filebeat

Filebeat collects logs from Kubernetes pods and ships them to Elasticsearch.

```bash
helm repo add elastic https://helm.elastic.co
helm repo update

helm install elasticsearch elastic/elasticsearch \
  --namespace observability --create-namespace \
  --set replicas=3 \
  --set minimumMasterNodes=2 \
  --set volumeClaimTemplate.resources.requests.storage=100Gi

helm install kibana elastic/kibana \
  --namespace observability

helm install filebeat elastic/filebeat \
  --namespace observability \
  --set daemonset.filebeatConfig.filebeat\.yml="$(cat <<'EOF'
filebeat.autodiscover:
  providers:
    - type: kubernetes
      node: ${NODE_NAME}
      hints.enabled: true
      hints.default_config:
        type: container
        paths:
          - /var/log/containers/*${data.kubernetes.container.id}.log
processors:
  - decode_json_fields:
      fields: ["message"]
      process_array: false
      max_depth: 3
      target: ""
      overwrite_keys: true
  - add_kubernetes_metadata:
      host: ${NODE_NAME}
      matchers:
        - logs_path:
            logs_path: "/var/log/containers/"
output.elasticsearch:
  hosts: ["http://elasticsearch-master.observability.svc:9200"]
  index: "aeterna-logs-%{+yyyy.MM.dd}"
setup.ilm.enabled: true
setup.ilm.rollover_alias: "aeterna-logs"
setup.ilm.policy_name: "aeterna-logs-policy"
EOF
)"
```

### Index Lifecycle Management

Create an ILM policy for log retention:

```json
PUT _ilm/policy/aeterna-logs-policy
{
  "policy": {
    "phases": {
      "hot": {
        "min_age": "0ms",
        "actions": {
          "rollover": {
            "max_age": "1d",
            "max_primary_shard_size": "50gb"
          }
        }
      },
      "warm": {
        "min_age": "7d",
        "actions": {
          "shrink": { "number_of_shards": 1 },
          "forcemerge": { "max_num_segments": 1 }
        }
      },
      "delete": {
        "min_age": "30d",
        "actions": { "delete": {} }
      }
    }
  }
}
```

### Kibana Dashboard

Create a saved search for Aeterna logs:

1. Open Kibana → Discover
2. Create index pattern `aeterna-logs-*`
3. Filter by `kubernetes.labels.app_kubernetes_io/name: aeterna`
4. Pin useful columns: `level`, `msg`, `component`, `duration`, `trace_id`

## Log Correlation with Traces

Aeterna includes `trace_id` and `span_id` in structured log entries when tracing is enabled. Use these fields to jump between logs and traces:

```json
{
  "timestamp": "2026-01-15T10:30:00.123Z",
  "level": "INFO",
  "msg": "memory_search completed",
  "component": "memory",
  "duration_ms": 42,
  "trace_id": "abc123def456",
  "span_id": "789012345678",
  "query": "database selection",
  "results": 5
}
```

In Grafana, configure derived fields on the Loki data source to link `trace_id` values to Tempo. In Kibana, create a scripted field that links to the Jaeger UI.

## Troubleshooting

**Logs not appearing in Loki/Elasticsearch:**
1. Verify Promtail/Filebeat DaemonSet is running on the Aeterna node
2. Check log format: `kubectl logs <aeterna-pod> | head -1` should show JSON
3. Verify labels: `kubectl get pod <aeterna-pod> --show-labels`
4. Check Promtail/Filebeat logs for ingestion errors

**JSON parsing failures:**
1. Ensure `observability.logging.format` is set to `json` (not `pretty`)
2. Check for multi-line log entries that break JSON parsing
3. Verify Filebeat `decode_json_fields` processor is configured

**High log volume:**
1. Increase log level to `warn` or `error` in production
2. Use per-component filtering to silence noisy modules
3. Configure retention policies in your log backend
