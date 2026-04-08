## Context

The current system supports knowledge CRUD, proposal, and generic governance review, but promotion is not a first-class lifecycle. As a result, reusable knowledge cannot be elevated to broader layers while preserving local residual meaning in a safe, explicit, and queryable way.

## Goals / Non-Goals

- Goals:
  - Support promotion as a first-class lifecycle
  - Preserve local specificity after partial promotion
  - Make semantic lineage explicit
  - Provide coherent REST, MCP, and CLI surfaces
  - Keep publication state separate from semantic role
  - Enforce tenant isolation, authorization, and confidentiality
  - Ensure deterministic precedence in resolver/search/context assembly
  - Support auditability, retries, idempotency, and concurrency safety
- Non-Goals:
  - A full web reviewer UI in this change
  - Fully replacing all generic governance APIs immediately
  - Fully automatic AI-only promotion without human confirmation

## Decisions

- Decision: Keep `KnowledgeStatus` limited to publication/governance state: `Draft`, `Proposed`, `Accepted`, `Deprecated`, `Superseded`, `Rejected`
- Decision: Add `KnowledgeVariantRole`: `Canonical`, `Specialization`, `Applicability`, `Exception`, `Clarification`
- Decision: Model promotion as a separate `PromotionRequest` lifecycle: `Draft`, `PendingReview`, `Approved`, `Rejected`, `Applied`, `Cancelled`
- Decision: Promotion never auto-deletes lower-layer knowledge by default
- Decision: Partial promotion preserves residual local knowledge and links it to the promoted canonical item
- Decision: Use explicit semantic relations: `PromotedFrom`, `PromotedTo`, `Specializes`, `ApplicableFrom`, `ExceptionTo`, `Clarifies`, `Supersedes`, `SupersededBy`, `DerivedFrom`
- Decision: Generic CRUD endpoints remain, but lifecycle endpoints own promotion lifecycle mutations

## Risks / Trade-offs

- Added model complexity
- Need to align knowledge-specific lifecycle with existing generic governance mechanisms
- Resolver/search behavior becomes more sophisticated and therefore easier to get subtly wrong

## Migration Plan

1. Add new types and endpoints additively
2. Default existing accepted items to `Canonical`
3. Preserve old proposal/govern flows
4. Add CLI/MCP support
5. Incrementally unify generic governance review with knowledge-native promotion review

## Open Questions

- Whether promotion preview should be heuristic-only or support user-provided structured splits from the first iteration
- Whether retarget decisions should preserve the same request ID or fork a new request while linking lineage
