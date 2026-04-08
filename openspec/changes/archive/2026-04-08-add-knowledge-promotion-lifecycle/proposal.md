## Why

The current knowledge system is CRUD/proposal-centric. Promotion exists only as a governed write pattern, not as a first-class lifecycle that preserves semantic relationships between higher-layer canonical knowledge and lower-layer local specialization.

We need a promotion-aware lifecycle where reusable shared truth can be elevated to broader layers while local applicability, specialization, exceptions, and clarifications remain explicit, linked, and governed.

## What Changes

- Add a first-class `PromotionRequest` lifecycle for knowledge promotion
- Add semantic `KnowledgeRelation` links between knowledge items
- Add `KnowledgeVariantRole` to distinguish canonical vs specialization/applicability/exception/clarification items
- Keep `KnowledgeStatus` focused on publication state and explicitly support `Rejected`
- Add promotion preview, submit, approve, reject, retarget, and relation-management APIs
- Add MCP tools for promotion lifecycle operations
- Add CLI commands under `aeterna knowledge` for promotion lifecycle operations
- Add deterministic precedence and resolution behavior for canonical vs local residual knowledge
- Add audit, notification, idempotency, concurrency, tenant isolation, and confidentiality requirements across the promotion flow

## Capabilities

### New Capabilities
- `knowledge-promotion-lifecycle`: First-class lifecycle for promoting knowledge upward while preserving local residual semantics and explicit relations

### Modified Capabilities
- `knowledge-repository`: Add promotion lifecycle, semantic relations, precedence, confidentiality, and tenant-boundary behavior
- `governance`: Add structured review decisions, idempotent promotion approvals, stale-review handling, and layer-aware authorization
- `server-runtime`: Add first-class promotion lifecycle HTTP endpoints and promotion event emission
- `opencode-integration`: Add MCP promotion lifecycle tools while preserving backward compatibility

## Impact

Affected code includes `mk_core/src/types.rs`, `knowledge/src/*`, `storage/src/*`, `cli/src/server/knowledge_api.rs`, `cli/src/commands/knowledge.rs`, `tools/src/knowledge.rs`, `packages/opencode-plugin/src/tools/*`, and `packages/opencode-plugin/src/client.ts`. Affected systems include REST APIs, governance workflows, CLI UX, MCP/OpenCode integration, resolver/search/context assembly, and audit/notification pipelines.
