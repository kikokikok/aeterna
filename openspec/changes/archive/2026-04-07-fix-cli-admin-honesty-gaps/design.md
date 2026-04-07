## Context

The CLI control plane is far more honest than before, but it is still not production-complete for major operator workflows. Some admin commands emit fabricated drift, validation, migration, or import results on live paths, while `govern`, `org`, `user`, and `team` commands are still mostly unsupported or preview-only. This creates a gap between what the CLI advertises and what operators can safely rely on.

## Goals / Non-Goals

**Goals:**
- Ensure shipped admin/operator CLI commands either do real backend-backed work or fail explicitly with actionable errors.
- Remove fabricated or placeholder result payloads from live command paths.
- Complete only the client/server wiring that can be backed by real routes in this change, and preserve explicit unsupported behavior elsewhere.
- Cover the completed CLI/admin journeys in Rust E2E tests and add Newman/Postman API scenarios only for HTTP workflows that are newly completed by this change.

**Non-Goals:**
- Implementing the full tenant control plane (tracked separately).
- Replacing the CLI as the main operator surface.

## Decisions

### No fabricated live admin output
Commands like drift analysis, validation, migrations, or imports may support preview/dry-run, but a live execution path must never print fabricated example results as if they were real.

### Unsupported is acceptable only when explicit
If a backend path truly does not exist yet, the CLI should return an explicit unsupported error with non-zero exit semantics rather than example output.

### Shared client methods may exceed immediately supported routes
The shared authenticated client abstraction may expose helper methods ahead of full route availability, but command handlers in this change must only call server-backed paths that actually exist and must preserve explicit unsupported behavior for the rest.

### Newman coverage only for genuinely completed HTTP workflows
If this change finishes no new HTTP admin/operator workflows beyond already-covered endpoints such as `/health`, the change artifacts should record that broader Newman/Postman additions are deferred rather than inventing speculative HTTP scenarios.

## Risks / Trade-offs

- **[Risk] Removing fabricated output makes the CLI feel less feature-complete in the short term** → Mitigation: prefer explicit unsupported errors over dishonest output, and wire high-value flows first.
- **[Risk] Client/API completion spans multiple server domains and some advertised routes still do not exist** → Mitigation: preserve explicit unsupported behavior for missing paths and narrow verification/documentation to the routes that are truly implemented.

## Migration Plan

1. Define honest admin CLI requirements in specs.
2. Remove fabricated live outputs from admin commands.
3. Add missing client and server endpoint wiring only for govern/org/user/team/admin flows that can be completed honestly in this change.
4. Add real local context update behavior for locally executable flows.
5. Add CLI and Newman coverage for the newly completed workflows, or explicitly defer Newman additions when no new HTTP workflow is actually completed.

## Open Questions

- Which admin/govern flows should move into a follow-up change once real server APIs exist?
