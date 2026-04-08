## 1. Domain model
- [x] 1.1 Add `Rejected` to runtime `KnowledgeStatus` if missing
- [x] 1.2 Add `KnowledgeVariantRole`
- [x] 1.3 Add `KnowledgeRelationType`
- [x] 1.4 Add `KnowledgeRelation`
- [x] 1.5 Add `PromotionMode`
- [x] 1.6 Add `PromotionRequestStatus`
- [x] 1.7 Add `PromotionDecision`
- [x] 1.8 Add `PromotionRequest`
- [x] 1.9 Add validation for legal target layers and upward-only promotion

## 2. Promotion lifecycle behavior
- [x] 2.1 Implement promotion preview logic
- [x] 2.2 Implement promotion request creation
- [x] 2.3 Implement promotion approval flow
- [x] 2.4 Implement promotion rejection flow
- [x] 2.5 Implement promotion retarget flow
- [x] 2.6 Implement promotion apply flow
- [x] 2.7 Ensure promotion never auto-deletes lower-layer knowledge by default
- [x] 2.8 Mark lower-layer knowledge `Superseded` only for full replacement
- [x] 2.9 Preserve residual lower-layer knowledge for partial promotion
- [x] 2.10 Persist semantic relations between source, promoted, and residual items

## 3. HTTP API
- [x] 3.1 Add `POST /api/v1/knowledge/{id}/promotions/preview`
- [x] 3.2 Add `POST /api/v1/knowledge/{id}/promotions`
- [x] 3.3 Add `GET /api/v1/knowledge/promotions`
- [x] 3.4 Add `GET /api/v1/knowledge/promotions/{promotionRequestId}`
- [x] 3.5 Add `POST /api/v1/knowledge/promotions/{promotionRequestId}/approve`
- [x] 3.6 Add `POST /api/v1/knowledge/promotions/{promotionRequestId}/reject`
- [x] 3.7 Add `POST /api/v1/knowledge/promotions/{promotionRequestId}/retarget`
- [x] 3.8 Add `POST /api/v1/knowledge/{id}/relations`
- [x] 3.9 Block unsafe direct status/relation mutations through generic update endpoints

## 4. CLI
- [x] 4.1 Add `aeterna knowledge promote`
- [x] 4.2 Add `aeterna knowledge promotion-preview`
- [x] 4.3 Add `aeterna knowledge pending`
- [x] 4.4 Add `aeterna knowledge approve`
- [x] 4.5 Add `aeterna knowledge reject`
- [x] 4.6 Add `aeterna knowledge retarget`
- [x] 4.7 Add `aeterna knowledge relate`
- [x] 4.8 Add interactive split UX for shared vs residual content
- [x] 4.9 Add JSON output for all promotion lifecycle commands

## 5. MCP / OpenCode
- [x] 5.1 Add `aeterna_knowledge_promotion_preview`
- [x] 5.2 Add `aeterna_knowledge_promote`
- [x] 5.3 Add `aeterna_knowledge_review_pending`
- [x] 5.4 Add `aeterna_knowledge_approve`
- [x] 5.5 Add `aeterna_knowledge_reject`
- [x] 5.6 Add `aeterna_knowledge_link`
- [x] 5.7 Extend governance status output with promotion request summaries

## 6. Resolver, search, and context assembly
- [x] 6.1 Define canonical-vs-residual precedence rules
- [x] 6.2 Return canonical knowledge plus local specialization/applicability/exception context
- [x] 6.3 Expose relation context in search/query results
- [x] 6.4 Update CCA/context assembly to inject canonical + local residual knowledge deterministically

## 7. Security, isolation, and policy
- [x] 7.1 Enforce tenant and scope boundaries for promotion
- [x] 7.2 Forbid implicit cross-tenant promotion
- [x] 7.3 Add confidentiality checks before promoting content to broader layers
- [x] 7.4 Validate reviewer authorization by target layer and action
- [x] 7.5 Enforce policy checks before promotion request submission and approval

## 8. Consistency, concurrency, and idempotency
- [x] 8.1 Add optimistic version checks for approve/reject/retarget/apply
- [x] 8.2 Make lifecycle endpoints idempotent
- [x] 8.3 Prevent duplicate promoted items and duplicate relations on retries
- [x] 8.4 Reject or refresh stale promotion requests when source/canonical items changed
- [x] 8.5 Define conflict handling for parallel promotions of the same source item

## 9. Audit, events, and notifications
- [x] 9.1 Emit `KnowledgePromotionRequested`
- [x] 9.2 Emit `KnowledgePromotionApproved`
- [x] 9.3 Emit `KnowledgePromotionRejected`
- [x] 9.4 Emit `KnowledgePromotionRetargeted`
- [x] 9.5 Emit `KnowledgePromotionApplied`
- [x] 9.6 Emit `KnowledgeRelationCreated`
- [x] 9.7 Extend audit logs with split/decision/reason metadata
- [x] 9.8 Add notification delivery for proposer, reviewers, and impacted dependents

## 10. Migration and compatibility
- [x] 10.1 Backfill default `KnowledgeVariantRole=Canonical` for existing accepted items
- [x] 10.2 Preserve legacy proposal/governance flows during rollout
- [x] 10.3 Keep additive API compatibility for older CLI/MCP clients
- [x] 10.4 Add migration path for historical superseded/deprecated items
- [x] 10.5 Document migration and compatibility behavior

## 11. Observability and reliability
- [x] 11.1 Add metrics for request counts, approval latency, rejection rate, retarget rate, and conflict rate
- [x] 11.2 Add tracing across preview → review → apply
- [x] 11.3 Add alerts for failed apply and notification delivery failures

## 12. Testing
- [x] 12.1 Add unit tests for promotion validation and relation integrity
- [x] 12.2 Add API tests for preview/create/approve/reject/retarget/apply
- [x] 12.3 Add CLI tests for promotion lifecycle commands
- [x] 12.4 Add MCP tool tests for promotion lifecycle tools
- [x] 12.5 Add resolver precedence tests
- [x] 12.6 Add tenant isolation tests
- [x] 12.7 Add confidentiality/redaction tests
- [x] 12.8 Add idempotency and retry tests
- [x] 12.9 Add concurrency/stale-review tests
- [x] 12.10 Add end-to-end tests for:
- [x] 12.10.1 full promotion replacing lower-layer knowledge
- [x] 12.10.2 partial promotion preserving specialization
- [x] 12.10.3 promotion rejected by reviewers
- [x] 12.10.4 promotion retargeted from org to team
