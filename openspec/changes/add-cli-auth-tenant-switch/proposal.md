## Why

The `aeterna` CLI already has `auth status`, `tenant use`, `context show/set/clear`, and a profiles system. What it lacks is a gh-CLI-style tenant switcher: one command that lists the user's accessible tenants, prompts a selection, and persists the choice both locally (for the repo-pinned context) and server-side (for the portable `users.default_tenant_id` preference). Today users either remember a slug and run `aeterna tenant use <slug>` per repo, or they fall back to the UI's browser-session selector — neither carries across machines or clients.

Additionally, when the server returns the new `400 select_tenant` error (introduced by `refactor-platform-admin-impersonation`), the CLI must recognize the enriched payload, render a clean picker, and retry the original request transparently. Without this, multi-tenant users hit a dead-end error at every command.

## What Changes

- Add `aeterna auth switch [slug]` command — interactive picker when invoked without a slug, non-interactive when a slug is provided. Updates both the local context file (`.aeterna/context.toml`) and the server-side `users.default_tenant_id` via `PUT /api/v1/user/me/default-tenant`. Flags: `--server-only` (skip local write), `--local-only` (skip server write).
- Add `aeterna auth default-tenant [slug]` — narrower command that manages ONLY the server-side default. `aeterna auth default-tenant` (no args) prints current; `aeterna auth default-tenant <slug>` sets; `aeterna auth default-tenant --clear` unsets.
- Extend `aeterna auth status` output to show: current profile, authenticated user, access-token validity window, resolved tenant and the resolution source (flag/env/header/local-context/server-default/auto-select), full list of accessible tenants, and the server-side default tenant slug if set. Add `--json` flag for machine-readable output.
- Add transparent `select_tenant` handling: when any CLI command receives `400 select_tenant`, the CLI SHALL use the `availableTenants` payload to prompt interactively, persist the selection per the `AETERNA_TENANT_PICKER_PERSISTENCE` env var (default: session-only), and retry the original request with the chosen tenant injected as `X-Tenant-ID`. A `--non-interactive` global flag disables the picker and exits with the error.
- Add `aeterna tenant use --server` flag so the existing `tenant use <slug>` command can optionally persist to the server-side default in addition to the local context.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli-device-flow-auth`: adds `aeterna auth switch` and `aeterna auth default-tenant` subcommands; extends `aeterna auth status` output shape; specifies CLI behavior when the server emits `400 select_tenant`; adds `--server` flag to the existing `tenant use` command.

## Impact

- **Affected code**: `cli/src/commands/auth.rs` (new subcommands), `cli/src/commands/tenant.rs` (new `--server` flag on `use`), new `cli/src/ui/tenant_picker.rs` (dialoguer-based picker), `cli/src/api/client.rs` (detect `select_tenant` error and delegate to picker with single-retry guard), `cli/src/profile.rs` (optional: cache last-resolved tenant per profile).
- **Affected APIs consumed**: `GET|PUT|DELETE /api/v1/user/me/default-tenant` and extended `/api/v1/auth/session` payload (both provided by `refactor-platform-admin-impersonation`).
- **Dependencies**: requires `refactor-platform-admin-impersonation` to be merged first — both for the new endpoints and for the `select_tenant` error shape.
- **UX**: `--non-interactive` global behavior must be respected across all existing commands that now defer to the picker; CI pipelines using the CLI already set `AETERNA_TENANT` and SHALL be unaffected.
