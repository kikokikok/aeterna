## Context

The OpenCode plugin is already substantial and usable, but the implementation, specs, and docs have drifted apart. We identified four classes of fit-for-purpose issues:

1. **Stale contracts**: the OpenCode integration spec and docs still describe outdated installation steps, tool counts, and older runtime assumptions.
2. **Lifecycle correctness**: plugin startup/session handling can create duplicate backend session state, which weakens the integrity of captured context.
3. **Capture correctness**: the tool execution capture path does not reliably preserve executed arguments, and repeated-pattern significance detection is undermined by incomplete execution-history recording.
4. **Runtime clarity**: auth/device-flow expectations and local/shared-layer memory behavior need tighter, user-visible semantics.

This change is intentionally focused on the OpenCode plugin fit-and-finish path rather than introducing new backup/export capabilities or broad server-runtime redesign.

## Goals / Non-Goals

**Goals:**
- Align OpenSpec requirements with the actual supported OpenCode plugin behavior.
- Fix session lifecycle semantics so one OpenCode session maps to one coherent backend session context.
- Fix tool capture/significance behavior so recorded executions match what actually happened.
- Clarify the supported plugin auth experience and local/shared memory semantics.
- Provide practical user-facing OpenCode guidance for daily usage.

**Non-Goals:**
- Introducing a new backup/export/import capability for memory or knowledge.
- Replacing the entire auth model with a fundamentally new provider integration architecture.
- Redesigning the server-side governance or memory storage model.
- Reworking unrelated CLI control-plane behavior outside of documented integration touchpoints.

## Decisions

### 1. Treat spec and documentation drift as first-class implementation work

**Decision:** Update both the OpenSpec deltas and the website/docs surfaces as part of the same change.

**Why:** The plugin is already user-facing. If specs and docs continue to describe deprecated flows (`npm install -D`, outdated tool lists, stale auth assumptions), users and maintainers will make wrong decisions even if runtime code improves.

**Alternatives considered:**
- **Docs-only cleanup**: rejected because existing spec contracts would remain wrong.
- **Code-only cleanup**: rejected because the main user pain is partly caused by stale guidance.

### 2. Normalize plugin startup around a single active backend session

**Decision:** Ensure startup and `session.start` handling converge on one active backend session instead of allowing duplicated initialization.

**Why:** Session identity is the anchor for captured tool executions, memory attribution, and lifecycle cleanup. Duplicate session creation makes capture less trustworthy and complicates auth refresh semantics.

**Alternatives considered:**
- **Keep double-start and deduplicate later server-side**: rejected because the plugin itself should preserve session coherence.
- **Remove all startup session creation and wait only for hook events**: possible, but higher risk if other startup paths depend on early session state. Prefer explicit single-owner logic.

### 3. Preserve executed tool-call fidelity in the capture path

**Decision:** Make the `tool.execute.after` capture path preserve the actual executed args and feed a real per-session execution history into significance detection.

**Why:** A memory/capture system that records empty or lossy execution data undermines the value of hindsight, promotion, and operational debugging.

**Alternatives considered:**
- **Capture only outputs, not args**: rejected because the specs require arguments and outputs together.
- **Make significance purely heuristic from output length/tool name**: rejected because repeated-pattern detection is explicitly part of the intended behavior.

### 4. Keep the existing auth model but tighten the supported UX contract

**Decision:** Retain the existing GitHub OAuth App device-flow model, but align the spec/docs around a supported user-visible OpenCode interaction path and stable refresh/reuse semantics.

**Why:** The auth flow already works and is shared with the CLI. The immediate problem is contract clarity and lifecycle coherence, not a total auth rewrite.

**Alternatives considered:**
- **Immediate migration to a different OpenCode-native auth hook architecture**: rejected for this change because it is a broader product/design effort.
- **Leave auth docs vague**: rejected because users need to understand how sign-in appears and what reuse/refresh guarantees exist.

### 5. Preserve the local-first architecture and make provenance explicit

**Decision:** Keep personal layers local-first and shared layers remote/cached, while ensuring stale-cache and source provenance are explicit in returned results and user guidance.

**Why:** The current architecture is strong. The main fit issue is not the model itself but the clarity and consistency of how results are explained and represented.

**Alternatives considered:**
- **Flatten local and shared behavior into one opaque search path**: rejected because the distinction is valuable for offline resilience and user trust.
- **Disable stale cache fallback**: rejected because resilience is more important than strict freshness in degraded mode, as long as provenance is visible.

## Risks / Trade-offs

- **[Risk] Spec updates could lock in current implementation quirks** → **Mitigation:** only codify behaviors that are intentionally supported and user-relevant; keep deeper redesigns out of scope.
- **[Risk] Session lifecycle changes could break implicit startup assumptions** → **Mitigation:** keep the design focused on single-session ownership, add targeted tests for startup + `session.start` sequencing.
- **[Risk] Capture-path fixes may alter heuristics or produce more captured data** → **Mitigation:** validate output shape and significance behavior with focused unit tests.
- **[Risk] Auth UX wording may over-promise a native OpenCode experience** → **Mitigation:** document the actual supported user-visible behavior without claiming a broader auth integration than exists.
- **[Risk] Docs and website updates may diverge again later** → **Mitigation:** treat the website integration guide and playbook as explicit deliverables in the task list and validate key references after changes.

## Migration Plan

1. Update the OpenSpec delta specs to reflect the intended supported contract.
2. Fix plugin runtime correctness issues in the smallest possible code paths:
   - session start ownership
   - tool capture args preservation
   - significance history recording
3. Update website/docs to match the new contract.
4. Validate with targeted plugin tests plus docs/reference consistency checks.
5. Rollback strategy: revert plugin runtime changes independently from docs/spec updates if lifecycle or capture regressions appear.

## Open Questions

- Should the plugin continue to present device-flow sign-in through the current user-visible path, or should a later change migrate fully onto a more native OpenCode auth-hook experience?
- Should additional memory tools (for example delete/optimize) be exposed now that the tool surface is being revisited, or should that be handled in a follow-up capability?
- Should the stale `adapters/opencode/README.md` content be rewritten in this change or handled separately as a broader docs cleanup?
