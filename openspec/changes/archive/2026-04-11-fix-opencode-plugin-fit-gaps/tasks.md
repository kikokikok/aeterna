## 1. Spec and documentation alignment

- [x] 1.1 Update `openspec/specs/opencode-integration/spec.md` purpose text and align its requirements with the supported OpenCode plugin contract.
- [x] 1.2 Update `openspec/specs/opencode-plugin-auth/spec.md` purpose text and align plugin-auth requirements with the supported interactive device-flow UX.
- [x] 1.3 Update `openspec/specs/local-memory-store/spec.md` where needed so the OpenCode local-first behavior matches the intended runtime semantics.
- [x] 1.4 Update user-facing docs for OpenCode integration and plugin usage so installation, auth, memory/knowledge usage, and plugin workflow guidance are consistent.

## 2. Plugin lifecycle correctness

- [x] 2.1 Audit plugin startup and `session.start` handling in `packages/opencode-plugin/src/index.ts` and `src/hooks/session.ts`.
- [x] 2.2 Fix session initialization so a single OpenCode session creates exactly one active backend session context.
- [x] 2.3 Add or update tests covering startup, `session.start`, and authenticated session reuse/refresh behavior.

## 3. Capture fidelity and significance detection

- [x] 3.1 Update `packages/opencode-plugin/src/hooks/tool.ts` so captured tool executions preserve the actual executed args.
- [x] 3.2 Ensure execution-history recording is updated for completed tool executions before repeated-pattern significance is evaluated.
- [x] 3.3 Add or update tests for capture payload fidelity, significance detection, and repeated-pattern behavior.

## 4. Auth and local-first runtime clarity

- [x] 4.1 Tighten the OpenCode plugin auth UX implementation/docs so the device-flow sign-in path is explicit and user-visible.
- [x] 4.2 Verify local/shared memory routing, stale-cache fallback, and source provenance behavior in the plugin implementation.
- [x] 4.3 Add or update tests covering stale cache fallback, source metadata, and offline/shared-layer semantics.

## 5. Validation

- [x] 5.1 Run targeted plugin tests for auth, capture, session lifecycle, and local-store behavior.
- [x] 5.2 Run relevant typecheck/build validation for `packages/opencode-plugin` and related docs surfaces.
- [x] 5.3 Run `openspec validate fix-opencode-plugin-fit-gaps --strict` and resolve any issues.
