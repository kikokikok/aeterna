# Distributed Tracing with Aeterna

Aeterna exports traces via OpenTelemetry Protocol (OTLP) over gRPC. This guide covers integration with Jaeger and Grafana Tempo.

## Enabling Tracing

```yaml
observability:
  tracing:
    enabled: true
    endpoint: "http://jaeger-collector.observability.svc:4317"
    samplingRatio: 0.1  # 10% of traces in production
```

When `observability.tracing.enabled` is `true`, the following environment variables are injected into the Aeterna deployment:

| Variable | Value | Purpose |
|----------|-------|---------|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | `observability.tracing.endpoint` | OTLP collector address |
| `OTEL_SERVICE_NAME` | `aeterna` | Service identifier in traces |
| `OTEL_TRACES_SAMPLER` | `parentbased_traceidratio` | Respects parent span decisions |
| `OTEL_TRACES_SAMPLER_ARG` | `observability.tracing.samplingRatio` | Sampling ratio (0.0–1.0) |

## Jaeger Integration

### Deploy Jaeger with Helm

```bash
helm repo add jaegertracing https://jaegertracing.github.io/helm-charts
helm repo update

helm install jaeger jaegertracing/jaeger \
  --namespace observability --create-namespace \
  --set provisionDataStore.cassandra=false \
  --set allInOne.enabled=true \
  --set storage.type=memory \
  --set agent.enabled=false \
  --set collector.enabled=false \
  --set query.enabled=false
```

For production with Elasticsearch storage:

```bash
helm install jaeger jaegertracing/jaeger \
  --namespace observability --create-namespace \
  --set provisionDataStore.cassandra=false \
  --set storage.type=elasticsearch \
  --set storage.elasticsearch.host=elasticsearch.observability.svc \
  --set storage.elasticsearch.port=9200 \
  --set collector.replicaCount=2 \
  --set query.replicaCount=2
```

### Configure Aeterna for Jaeger

```yaml
observability:
  tracing:
    enabled: true
    # Jaeger all-in-one OTLP gRPC port
    endpoint: "http://jaeger.observability.svc:4317"
    samplingRatio: 0.1
```

### Verify Traces

```bash
# Port-forward to Jaeger UI
kubectl port-forward svc/jaeger-query -n observability 16686:16686

# Open http://localhost:16686 and select "aeterna" from the Service dropdown
```

## Grafana Tempo Integration

### Deploy Tempo with Helm

```bash
helm repo add grafana https://grafana.github.io/helm-charts
helm repo update

helm install tempo grafana/tempo \
  --namespace observability --create-namespace \
  --set tempo.storage.trace.backend=local \
  --set tempo.storage.trace.local.path=/var/tempo/traces
```

For production with S3 backend:

```bash
helm install tempo grafana/tempo-distributed \
  --namespace observability --create-namespace \
  --set storage.trace.backend=s3 \
  --set storage.trace.s3.bucket=aeterna-traces \
  --set storage.trace.s3.endpoint=s3.amazonaws.com \
  --set storage.trace.s3.region=us-east-1 \
  --set compactor.config.compaction.block_retention=336h \
  --set ingester.replicas=3 \
  --set distributor.replicas=2
```

### Configure Aeterna for Tempo

```yaml
observability:
  tracing:
    enabled: true
    # Tempo distributor OTLP gRPC endpoint
    endpoint: "http://tempo-distributor.observability.svc:4317"
    samplingRatio: 0.1
```

### Configure Grafana Data Source

Add Tempo as a data source in Grafana:

```yaml
apiVersion: 1
datasources:
  - name: Tempo
    type: tempo
    access: proxy
    url: http://tempo-query-frontend.observability.svc:3100
    jsonData:
      httpMethod: GET
      tracesToMetrics:
        datasourceUid: prometheus
        tags:
          - key: service.name
            value: service
      serviceMap:
        datasourceUid: prometheus
      nodeGraph:
        enabled: true
      lokiSearch:
        datasourceUid: loki
```

## Sampling Strategies

| Environment | `samplingRatio` | Rationale |
|-------------|----------------|-----------|
| Development | `1.0` | Capture all traces for debugging |
| Staging | `0.5` | Capture half for representative coverage |
| Production | `0.05`–`0.1` | Balance visibility with overhead |
| Incident | `1.0` | Temporarily increase during incidents |

To increase sampling during an incident without redeploying:

```bash
helm upgrade aeterna ./charts/aeterna \
  --reuse-values \
  --set observability.tracing.samplingRatio=1.0
```

## Trace Context Propagation

Aeterna propagates W3C Trace Context headers (`traceparent`, `tracestate`) on all outbound HTTP calls. Downstream services that support OpenTelemetry will automatically join the same trace.

Services that receive trace context:
- PostgreSQL (via instrumented connection pool)
- Qdrant (via HTTP API calls)
- Redis/Dragonfly (via instrumented client)
- OPAL Server (via HTTP API)
- Central Server (in hybrid/remote modes)

## Network Policies

If `networkPolicy.enabled` is `true`, ensure the OTLP collector endpoint is reachable from the Aeterna namespace. Add an egress rule:

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: allow-otlp-egress
  namespace: aeterna
spec:
  podSelector:
    matchLabels:
      app.kubernetes.io/name: aeterna
  policyTypes:
    - Egress
  egress:
    - to:
        - namespaceSelector:
            matchLabels:
              kubernetes.io/metadata.name: observability
      ports:
        - protocol: TCP
          port: 4317
```

## Troubleshooting

**No traces appearing:**
1. Verify tracing is enabled: `helm get values aeterna | grep tracing`
2. Check the OTLP endpoint is reachable: `kubectl exec <aeterna-pod> -- curl -sf <endpoint>/v1/traces`
3. Review Aeterna logs for OTLP export errors: `kubectl logs <aeterna-pod> | grep otel`
4. Confirm network policies allow egress to the collector namespace

**High trace volume / storage costs:**
1. Reduce `samplingRatio` (e.g., from `0.1` to `0.01`)
2. Configure retention in your collector backend
3. Use tail-based sampling in an OpenTelemetry Collector pipeline for error-biased sampling
