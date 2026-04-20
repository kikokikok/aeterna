## Why

Tenant provisioning today is split across many surfaces that do not share a single source of truth:

1. **The server** exposes a manifest-based endpoint (`POST /api/v1/admin/tenants/provision`) that accepts a `TenantManifest` and processes it in steps.
2. **The CLI** exposes a dozen fine-grained subcommands (`tenant create`, `tenant config upsert`, `tenant secret set`, `tenant repo-binding set`, …) that each mutate one slice of a tenant.
3. **The admin UI** has no guided "create tenant" flow that writes a manifest; tenant creation is today either a manual walk through the CLI subcommands or a direct API call.
4. **First-run bootstrap** relies on environment variables (`ADMIN_BOOTSTRAP_*`, `BOOTSTRAP_COMPANY_SLUG`, …) whose failure mode is silent: if bootstrap partially succeeds the server still accepts requests, but per-tenant runtime wiring (for example, memory-layer providers) is absent, so authenticated calls return HTTP 500 instead of a clean startup failure.

The resulting gaps have concrete consequences:

- A tenant can exist in the database with no per-tenant provider registration, which makes every `/api/v1/memory/*` request fail with `No provider for layer <X>` even though `/health` and `/ready` return 200.
- Operators cannot express "here is the full desired state of this tenant" in one artifact that is equally consumable by the CLI, the UI, CI automation, and disaster-recovery tooling.
- Secret values can be passed as raw strings on CLI flags, which leaks them into shell history, process listings, and audit logs.
- There is no guarantee that the three client surfaces (CLI, UI, direct API) produce the same outcome for the same input, because only the API path is reference-tested.

This change defines the unified, secure, consistent provisioning experience that closes those gaps.

## What Changes

- **Canonical input**: `TenantManifest` (`apiVersion: aeterna.io/v1`, `kind: TenantManifest`) becomes the single source of truth. The CLI, the UI, and CI automation all produce a manifest and submit it to `POST /api/v1/admin/tenants/provision`. Fine-grained CLI and API endpoints remain for convenience and are re-expressed internally as minimal manifests applied through the same handler.
- **New CLI verbs** on `aeterna tenant`: `apply`, `render`, `diff`, `validate`, `watch`. `apply` replaces "run ten subcommands in order" with a single idempotent operation. `render` and `diff` make round-trip (export → edit → re-apply) safe.
- **Secret handling**: CLI and server SHALL support reference-typed secrets (`kind: K8sSecretRef` / `FileRef` / `EnvRef` / `VaultRef`) in addition to inline values. Inline values remain supported for dev loopback but are gated behind an explicit `--allow-inline-secret` flag and are never accepted in non-development server modes.
- **Auth scopes for automation**: a scoped, short-lived, revocable token type for non-interactive callers (`tenant:provision`, `tenant:read`, `tenant:render`, etc.). Device-code flow remains the only interactive path.
- **Startup invariants**: the server SHALL fail readiness for any tenant whose required providers (memory layers declared in its manifest, LLM provider, embedding provider, repository binding if declared) are not wired at boot, instead of accepting traffic and returning HTTP 500 on use.
- **Audit parity**: every mutation SHALL record a `via` discriminator (`cli` | `ui` | `api`), a `client_version`, and a `manifest_hash` (SHA-256 of the applied manifest bytes), regardless of which surface initiated it.
- **Consistency acceptance suite**: a single set of manifest fixtures under `tests/tenant_provisioning/scenarios/` SHALL be exercised through three runners (CLI, direct API, UI via Playwright) that all assert the same rendered end-state via `aeterna tenant render`. Merges to `main` SHALL be blocked if any runner diverges.
- **First-run bootstrap refactor**: bootstrap SHALL be idempotent, produce a machine-readable status report, and SHALL fail the pod's readiness probe if it cannot complete.

## Capabilities

### Modified Capabilities

- `tenant-provisioning`: adds bootstrap semantics, CLI-apply semantics, secret reference types, manifest hashing, dry-run/diff/validate, and consistency guarantees.
- `cli-control-plane`: adds `tenant apply`, `tenant render`, `tenant diff`, `tenant validate`, `tenant watch`, and secure secret input modes. Tightens exit-code and output-format conventions.
- `server-runtime`: adds per-tenant startup invariants and readiness gating on provider wiring.
- `granular-authorization`: adds scoped, short-lived, revocable tokens for automation callers, with a minimal scope vocabulary for tenant provisioning.

## Impact

- **Affected code**: `cli/src/commands/tenant.rs` (new subcommands), `cli/src/server/tenant_api.rs` (provision step hardening, render/diff handlers, manifest hashing), `cli/src/server/bootstrap.rs` (readiness-gating bootstrap), `cli/src/server/plugin_auth.rs` (scoped tokens), new `tests/tenant_provisioning/` tree.
- **Affected APIs**: adds `GET /api/v1/admin/tenants/{slug}/manifest`, `POST /api/v1/admin/tenants/provision?dryRun=true`, `POST /api/v1/admin/tenants/diff`, `POST /api/v1/auth/tokens` (scoped). No removals.
- **Affected UX**: new admin-UI "Create tenant" wizard that composes a manifest and POSTs it; no more navigating through separate CRUD screens. CLI gains an apply-centric model; individual subcommands remain but their implementations converge on `apply`.
- **Migration**: existing tenants continue to work unchanged. Operators who want the full experience call `aeterna tenant render --slug <s> > tenants/<s>.yaml` once to capture current state, then use `apply` going forward.
