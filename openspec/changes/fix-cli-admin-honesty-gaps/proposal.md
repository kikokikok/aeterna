## Why

The CLI now fails explicitly instead of silently faking success for many unsupported paths, but major admin/operator command groups are still incomplete or misleading. Some live admin commands still emit fabricated analysis, and many org/user/team/governance flows remain preview-only or unsupported despite appearing in the shipped control-plane surface. Production readiness requires the CLI to either perform real server-backed work or fail honestly, consistently, and verifiably.

## What Changes

- Remove fabricated admin output from live command paths and replace it with real backend-backed execution or explicit unsupported errors.
- Complete or retain only the server-backed client/API wiring that can be supported honestly in this change, and return explicit unsupported errors for the remaining govern/org/user/team/admin control-plane workflows.
- Ensure locally executable context-selection flows perform real local state updates rather than printing what they would do.
- Add black-box and CLI end-to-end coverage for the operator/admin flows completed by this change.

## Capabilities

### New Capabilities
- `cli-admin-honesty`: honest CLI semantics for admin/operator workflows that must either execute real work or fail explicitly.

### Modified Capabilities
- `cli-control-plane`: ensure shipped admin/operator command paths use real backend-backed behavior only where routes actually exist, and fail explicitly elsewhere.
- `runtime-operations`: require live CLI admin paths to return real persisted results or explicit unsupported failures.

## Impact

- Affected code: `cli/src/commands/{admin,govern,org,user,team}.rs`, `cli/src/client.rs`, selected server admin/governance routes, CLI tests, and OpenSpec change artifacts describing HTTP/Newman coverage scope.
- Affected systems: operator workflows, CLI admin UX, control-plane API coverage, and release-readiness documentation.
