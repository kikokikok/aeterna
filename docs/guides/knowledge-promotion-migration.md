# Knowledge Promotion Lifecycle — Migration and Compatibility Guide

This document covers migration steps and backward-compatibility notes for the
`add-knowledge-promotion-lifecycle` change.

---

## Overview

The promotion lifecycle change is **entirely additive** from an API and data
perspective.  No existing endpoints, CLI commands, MCP tools, or data formats
are modified.  Older clients that do not use the new promotion endpoints
continue to work unchanged.

---

## New Capabilities

| Capability | Where |
|---|---|
| Promotion request CRUD | `POST /api/v1/knowledge/{id}/promotions` |
| Approve / reject / retarget | `POST /api/v1/knowledge/promotions/{id}/approve` etc. |
| Semantic relation creation | `POST /api/v1/knowledge/{id}/relations` |
| CLI promotion workflow | `aeterna knowledge promote`, `pending`, `approve`, `reject`, `retarget`, `relate` |
| MCP promotion tools | `aeterna_knowledge_promote`, `aeterna_knowledge_approve`, etc. |
| `KnowledgeVariantRole` metadata | New `variant_role` key in `KnowledgeEntry.metadata` |

---

## Task 10.1 — Backfill `variant_role` for Existing Accepted Items

Existing `KnowledgeEntry` records do not have a `variant_role` metadata key.
The resolution and context-assembly layers treat a missing `variant_role` as
`Canonical` by default, so older records are read correctly without migration.

However, for consistency and explicit auditability, operators can run the
backfill helper after deploying:

```rust
// One-time migration — safe to call repeatedly (idempotent)
let updated = knowledge_manager
    .backfill_variant_roles(tenant_ctx)
    .await?;
tracing::info!(count = updated, "variant_role backfill complete");
```

**Assignment rules:**

| Entry status | Assigned `variant_role` |
|---|---|
| `Accepted` | `Canonical` |
| `Superseded` | `Superseded` |
| `Deprecated` | `Superseded` |
| `Proposed`, `Rejected`, `Draft` | (skipped — no role yet) |

---

## Task 10.2 — Legacy Proposal / Governance Flows

The existing `KnowledgeStatus::Proposed → Accepted` flow (via
`update_status`) continues to work.  The new `PromotionRequest` lifecycle
is a parallel, opt-in pathway.  Teams can migrate at their own pace.

---

## Task 10.3 — Additive API Compatibility

All promotion endpoints are new paths.  Existing clients reading the
knowledge store via `GET /api/v1/knowledge/query` or
`GET /api/v1/knowledge/{id}` receive unchanged response bodies.

The only addition visible to existing clients is the optional `variant_role`
field inside `KnowledgeEntry.metadata`.  This is a free-form JSON map; older
clients simply ignore unknown keys.

---

## Task 10.4 — Historical Superseded / Deprecated Items

Items previously marked `KnowledgeStatus::Superseded` or
`KnowledgeStatus::Deprecated` without explicit semantic relations can be
retroactively tagged using the `backfill_variant_roles` helper
(see 10.1 above) and then linked using the relation API:

```bash
aeterna knowledge relate \
  --source <old-item-id> \
  --target <new-canonical-id> \
  --type SupersededBy
```

---

## Rollout Checklist

1. Deploy the updated binary (no schema migration needed — data is append-only).
2. Run `backfill_variant_roles` for each tenant that has pre-existing knowledge items.
3. Optionally wire a `NotificationService` into `GovernanceEngine` for promotion alerts.
4. Inform teams of the new `aeterna knowledge promote` workflow.
5. Legacy proposal paths remain active — teams can continue using them.

---

## Rollback

Because the change is purely additive:

- Reverting the binary leaves existing knowledge data intact.
- New `PromotionRequest` objects stored in the Git repo are ignored by
  the old binary (they live under `_meta/promotions/`).
- No data loss occurs on rollback.
