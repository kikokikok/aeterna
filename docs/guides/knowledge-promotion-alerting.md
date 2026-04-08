# Knowledge Promotion Lifecycle: Observability and Alerting

This guide documents the metrics, tracing, and alert rules introduced in
**Task 11** of the `add-knowledge-promotion-lifecycle` change.

---

## Metrics Reference

All metrics use the [metrics](https://docs.rs/metrics) crate and are emitted
from `knowledge/src/telemetry.rs` and `knowledge/src/manager.rs`.

### Counters

| Metric | Labels | Description |
|--------|--------|-------------|
| `knowledge_promotion_requests_total` | `source_layer`, `target_layer` | New promotion request submitted |
| `knowledge_promotion_approvals_total` | `target_layer` | Promotion request approved |
| `knowledge_promotion_rejections_total` | `target_layer`, `reason_category` | Promotion request rejected |
| `knowledge_promotion_retargets_total` | `old_layer`, `new_layer` | Promotion retargeted to a new layer |
| `knowledge_promotion_conflicts_total` | `conflict_type` | Parallel promotion conflict detected |
| `knowledge_promotion_apply_failed_total` | `reason` | Apply operation failed (alert-grade) |
| `knowledge_notification_delivery_failed_total` | `event_type` | Notification delivery failed (alert-grade) |

### Histograms

| Metric | Labels | Description |
|--------|--------|-------------|
| `knowledge_promotion_approval_latency_ms` | `target_layer` | End-to-end latency from request creation to approval |

---

## Distributed Tracing

Every promotion lifecycle operation includes the following span fields in
addition to standard `tracing` framework fields:

| Span field | Added to | Value |
|------------|----------|-------|
| `lifecycle_stage` | preview, approve, reject, retarget, apply | Stage name string |
| `request.source_layer` | create_promotion_request | Debug representation of source KnowledgeLayer |
| `request.target_layer` | create_promotion_request | Debug representation of target KnowledgeLayer |
| `request.promotion_mode` | create_promotion_request | Debug representation of PromotionMode (Full/Partial) |

This allows distributed tracing systems (e.g., Jaeger, Tempo) to trace a full
promotion lifecycle with a single trace ID from `preview` → `create` → `approve`
→ `apply`.

---

## Alert Rules

### 1. Apply failures

**Alert**: `KnowledgePromotionApplyFailed`

Fires when `knowledge_promotion_apply_failed_total` increases over a 5-minute
window. Any increment indicates a storage write error or unexpected state
transition during promotion apply. This should not happen in normal operation.

```yaml
# Prometheus alerting rule (example)
- alert: KnowledgePromotionApplyFailed
  expr: increase(knowledge_promotion_apply_failed_total[5m]) > 0
  for: 0m
  labels:
    severity: critical
  annotations:
    summary: "Knowledge promotion apply failed"
    description: "{{ $value }} promotion apply failure(s) in the last 5 minutes. Check Loki/stdout for the full error message."
```

### 2. Notification delivery failures

**Alert**: `KnowledgeNotificationDeliveryFailed`

Fires when `knowledge_notification_delivery_failed_total` increases. This is a
soft failure — the promotion itself succeeded — but reviewers or proposers may
not have received their notification.

```yaml
- alert: KnowledgeNotificationDeliveryFailed
  expr: increase(knowledge_notification_delivery_failed_total[15m]) > 0
  for: 0m
  labels:
    severity: warning
  annotations:
    summary: "Knowledge promotion notification delivery failed"
    description: "{{ $value }} notification failure(s) in the last 15 minutes for event type {{ $labels.event_type }}."
```

### 3. High conflict rate

**Alert**: `KnowledgePromotionHighConflictRate`

Fires when conflicting parallel promotions are detected at an unusual rate,
which may indicate a client bug or concurrent promotion storm.

```yaml
- alert: KnowledgePromotionHighConflictRate
  expr: rate(knowledge_promotion_conflicts_total[10m]) > 0.1
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "High knowledge promotion conflict rate"
    description: "Promotion conflict rate is {{ $value }} conflicts/sec over the last 10 minutes."
```

---

## Dashboards

### Key Panels (Grafana)

| Panel | Query |
|-------|-------|
| Promotion throughput | `rate(knowledge_promotion_requests_total[5m])` |
| Approval rate | `rate(knowledge_promotion_approvals_total[5m])` |
| Rejection rate | `rate(knowledge_promotion_rejections_total[5m])` |
| Mean approval latency | `histogram_quantile(0.95, rate(knowledge_promotion_approval_latency_ms_bucket[10m]))` |
| Conflict rate | `rate(knowledge_promotion_conflicts_total[5m])` |
| Apply failure rate | `rate(knowledge_promotion_apply_failed_total[5m])` |

---

## Runbook: Apply Failure Response

1. Check `reason` label on `knowledge_promotion_apply_failed_total` to identify the error category.
2. Search Loki/stdout logs for `promotion_apply_failed` or the `promotion_id` span field.
3. Common causes:
   - `RepositoryError`: storage backend unavailable — check Git or PostgreSQL connectivity.
   - `InvalidPromotionTransition`: promotion state machine bug — check client calling sequence.
   - `SourceNotFound`: source item was deleted between approval and apply — resubmit promotion.
4. Retrying `apply_promotion` with the same `promotion_id` is safe (idempotent).

## Runbook: Notification Failure Response

1. Check `event_type` label to identify which lifecycle event failed to notify.
2. Notification failures do NOT roll back the promotion — the operation succeeded.
3. Manually notify impacted parties if needed (query `GET /api/v1/knowledge/promotions/{id}`).
4. Fix the notification service configuration and restart the server to restore delivery.
