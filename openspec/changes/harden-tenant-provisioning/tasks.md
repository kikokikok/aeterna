# Tasks — harden-tenant-provisioning

> **This file was rewritten on 2026-04-20 after a full audit of master**
> **(commit `fab6405`). See `AUDIT.md` for the evidence behind each status.**
>
> Legend:
> - `[ ]` = not started (MISSING in audit)
> - `[~]` = partially done (existing code needs extension; audit “PARTIAL”)
> - `[!]` = **design conflict must be resolved before coding** (audit “CONFLICTS”)
> - `[x]` = done

---

## 0. Resolve design conflicts (blocking all other groups)

- [!] 0.1 Decide `SecretResolver` shape: extend existing closure-based alias in `memory/src/provider_registry.rs` **or** introduce a kind-dispatched trait and migrate. Document in `design.md` under a new "Secret resolver model" section. _(blocks 3.*)_
- [!] 0.2 Decide provider wiring model: keep **lazy per-request resolution** (current architecture) with state transitions driven by first-use, or switch to **eager startup wiring** as originally proposed. Document in `design.md`. _(blocks 5.2, 5.3, 5.4, 6.2)_
- [!] 0.3 Define `TenantSecretReference` migration path: introduce `TenantSecretReferenceV2` sum type alongside the existing flat struct, with a deprecation window — or commit to a single breaking bump of `mk_core`. Document in `design.md`. _(blocks 1.2, 3.2-3.4)_
- [!] 0.4 Decide CLI `validate` placement: top-level `tenant validate` subsumes the existing nested `tenant repo-binding validate` and `tenant config validate`, or coexists. Document in `design.md`. _(blocks 7.4, 7.6)_

---

## Phase B1 — Manifest schema + hashing foundation

### 1. Manifest schema and hashing

- [ ] 1.1 Extend `TenantManifest` with `metadata.generation: u64` and `providers: ManifestProviders` (memoryLayers, llm, embedding). _(`cli/src/server/tenant_api.rs:3404`)_
- [ ] 1.2 Introduce `SecretReference` sum type per 0.3. Update `ManifestConfig.secret_references` accordingly.
- [ ] 1.3 Create `manifest_canonical` module: stable key order, inline secrets stripped.
- [ ] 1.4 Implement `manifest_hash(&TenantManifest) -> String` returning `sha256:<hex>`.
- [ ] 1.5 Add `last_applied_manifest_hash: Option<String>` and `generation: u64` to the tenant record; write SQL migration.
- [ ] 1.6 Update `provision_tenant` (`tenant_api.rs:3599`) to compute+persist hash, enforce monotonic generation, short-circuit no-op re-applies. Rewrite `provision_tenant_idempotent_reapply` (L5674) to assert the short-circuit path.
- [~] 1.7 Extend `validate_manifest` (called at L3609) to cover unknown `SecretReference.kind`, invalid generation, missing required provider declarations. The function exists; add these cases.

---

## Phase B2 — Observability + readiness

### 5. Per-tenant provider state and readiness gate _(design per 0.2)_

- [ ] 5.1 Define `TenantRuntimeState` enum `{ Loading, Available, LoadingFailed { reason } }`.
- [!] 5.2 Implement the wiring model chosen in 0.2. If lazy: record first-resolution outcome per tenant and surface it. If eager: startup loop over persisted tenants.
- [~] 5.3 Extend `/ready` (`cli/src/server/health.rs:40`, handler at L58) to include a tenant-state check in addition to current PG + vector checks. Return 503 until bootstrap completes AND all tenants resolved (per 0.2).
- [ ] 5.4 In tenant-scoped routes (`/api/v1/memory/*` and siblings), short-circuit to HTTP 503 `{error: "tenant_unavailable", tenantSlug, reason}` when target tenant is `LoadingFailed`.
- [~] 5.5 Extend existing status endpoint at `/admin/tenants/{tenant}/providers/status` (handler `tenant_api.rs:2818`). Current response is `{llm, embedding}` via `TenantProviderStatusResponse`. Add `state`, `reason`, `providersWired[]`, `providersFailed[]` fields (non-breaking: new optional fields).
- [ ] 5.6 Add metrics: `aeterna_tenant_state{slug, state}` gauge, `aeterna_tenant_wiring_duration_seconds` histogram.

### 6. First-run bootstrap hardening

- [ ] 6.1 Refactor `bootstrap.rs` to run idempotently; add `GET /api/v1/admin/bootstrap/status` returning per-step status.
- [ ] 6.2 Gate `/ready` on bootstrap completion (in concert with 5.3).
- [ ] 6.3 Structured error on failure; `/ready` stays 503; retry on pod restart.
- [ ] 6.4 Emit `BootstrapCompleted` governance event on success.

---

## Phase B3 — Dry-run, diff, render

### 2. Dry-run, diff, render endpoints

- [ ] 2.1 Add `?dryRun=true` branch to `provision_tenant` (L3599) that runs every step in plan-only mode and returns the planned step list.
- [ ] 2.2 Implement `GET /api/v1/admin/tenants/{slug}/manifest` returning the rendered current-state manifest (secret values excluded; references only).
- [ ] 2.3 Implement `?redact=true` mode (reference names replaced with opaque placeholders for `tenant:read`-only callers).
- [ ] 2.4 Implement `POST /api/v1/admin/tenants/diff` returning structured delta between submitted manifest and current state.
- [ ] 2.5 Add per-step `dry_run` marker to audit log (depends on 11.1).

---

## Phase B4 — Secret resolution + inline gating _(depends on 0.1, 0.3)_

### 3. Secret reference resolution

- [!] 3.1 Define resolver shape per 0.1 (trait or extended alias).
- [ ] 3.2 `K8sSecretRefResolver` using the pod's in-cluster SA credentials.
- [ ] 3.3 `FileRefResolver` that checks mode `<= 0600`.
- [ ] 3.4 `EnvRefResolver` and `VaultRefResolver` (Vault feature-gated stub).
- [!] 3.5 Wire chosen model into the per-request secrets provider. **Note:** the registry already resolves per-request via closures (`provider_registry.rs:92`) — this task is about kind-dispatch, not introducing per-request resolution.
- [~] 3.6 Add systematic coverage that resolved plaintext is never logged, never serialized into responses, and never cached beyond request scope. Add redaction tests.

### 4. Inline-secret gating

- [ ] 4.1 Add `allow_inline_secret: bool` to server config, default `false`, off in release builds.
- [ ] 4.2 Accept `?allowInline=true` on provision only when the server flag is set.
- [ ] 4.3 Reject non-empty `secrets[].secretValue` unless both conditions hold; return actionable error pointing to `config.secretReferences`.

---

## Phase B5 — CLI refactor _(depends on 0.4)_

### 7. CLI: apply, render, diff, validate, watch

- [ ] 7.1 Add `TenantCommand::Apply(TenantApplyArgs)` with `-f/--file`, `--dry-run`, `--prune`, `--watch`, `--wait`.
- [ ] 7.2 Add `TenantCommand::Render(TenantRenderArgs)` with `--slug`, `--redact`, `-o`.
- [ ] 7.3 Add `TenantCommand::Diff(TenantDiffArgs)` with `--slug`, `-f`, `-o unified|json`.
- [!] 7.4 Add `TenantCommand::Validate` per the decision in 0.4. Reconcile with existing nested `tenant repo-binding validate` (L82) and `tenant config validate` (L94).
- [ ] 7.5 Add `TenantCommand::Watch(TenantWatchArgs)` streaming per-step status.
- [ ] 7.6 Re-implement `Create`, `Update`, `DomainMap`, `RepoBinding`, `Config`, `Secret`, `Connection` internally as minimal-manifest `apply` invocations, preserving their CLI surface.
- [ ] 7.7 Inject `X-Aeterna-Client-Kind: cli` and `User-Agent: aeterna-cli/<version>` on every HTTP request.

### 8. CLI: secure secret input

- [ ] 8.1 Add `--ref`, `--from-file`, `--from-stdin`, `--from-env` flags on `tenant secret set`.
- [ ] 8.2 Reject `--value` unless `--allow-inline-secret` is also set.
- [ ] 8.3 `--from-file` checks mode `<= 0600`.
- [ ] 8.4 `--from-stdin` disables terminal echo when stdin is a TTY.
- [ ] 8.5 `--from-env` reads the named variable and clears it from child processes.

### 9. CLI: output and exit codes

- [ ] 9.1 Extract `output::Renderer` supporting `table`, `json`, `yaml`, `name`, `jsonpath=<expr>`.
- [ ] 9.2 Default to `table` on TTY, `json` otherwise, unless `-o` specified.
- [ ] 9.3 Replace ad-hoc exit codes with a standard table (0/1/2/3/4/5); add `ExitCode` enum + unit tests.
- [ ] 9.4 Map HTTP status codes to CLI exit codes consistently (401/403 → 2, 409 → 3, 5xx → 4/5, 4xx schema → 1).

---

## Phase B6 — Scoped tokens

### 10. Scoped tokens

- [~] 10.1 Add `token_type` and `scopes` **JWT claims**. Note: `token_type: "Bearer"` is currently emitted in OAuth *response bodies* (`cli/src/client.rs`, `plugin_auth.rs`) but is not a JWT claim. This is a new claim, not a rename.
- [ ] 10.2 Implement `POST /api/v1/auth/tokens` (PlatformAdmin only) with bounded `expiresIn` (max 24h, default 4h).
- [ ] 10.3 `DELETE /api/v1/auth/tokens/{tokenId}` with a 60s revocation cache TTL across instances.
- [ ] 10.4 `GET /api/v1/auth/tokens` listing non-expired tokens (without raw strings).
- [ ] 10.5 Middleware that checks `scopes` claim against the route's required scope; fall back to role check for user callers.
- [ ] 10.6 Reject `--token` flag in the CLI; read tokens from `AETERNA_API_TOKEN`, OS keychain, then `~/.aeterna/credentials` (mode-gated).

---

## Phase B7 — Audit + UI + tests + docs (parallel PRs)

### 11. Audit parity

- [ ] 11.1 Extend audit log schema with `via`, `client_version`, `manifest_hash`, `generation`, `dry_run`.
- [ ] 11.2 Extract `X-Aeterna-Client-Kind` header in router and propagate through request-scoped audit context.
- [ ] 11.3 Normalize unknown client-kind values to `api`, preserving original in `client_kind_raw`.
- [ ] 11.4 Ensure every provision-path mutation records the new fields.

### 12. Admin UI wizard

- [ ] 12.1 Add a "Create tenant" wizard (multi-step form) that composes a `TenantManifest` client-side.
- [ ] 12.2 Step 1: tenant identity (slug, name, domain mappings).
- [ ] 12.3 Step 2: secret reference picker backed by a new `GET /api/v1/admin/secret-sources` endpoint (names only, never values).
- [ ] 12.4 Step 3: initial hierarchy (company / org / team / members).
- [ ] 12.5 Step 4: role assignments.
- [ ] 12.6 Step 5: provider declarations (memory layers, LLM, embedding).
- [ ] 12.7 Preview screen: render composed manifest as YAML.
- [ ] 12.8 Submit screen: POST with `X-Aeterna-Client-Kind: ui` and per-step progress.
- [ ] 12.9 "Download manifest" action on tenant detail page calling the render endpoint.

### 13. Consistency acceptance suite

- [ ] 13.1 Create `tests/tenant_provisioning/scenarios/` with at least five fixtures (bootstrap, add-company, rotate-reference, no-op re-apply, prune).
- [ ] 13.2 Implement `runner_api.rs` (direct POST with test-minted scoped token).
- [ ] 13.3 Implement `runner_cli.rs` (spawns `aeterna tenant apply`).
- [ ] 13.4 Implement `runner_ui.rs` (Playwright against `/admin/*`).
- [ ] 13.5 Implement `assertions.rs` that renders the tenant and diffs against an expected baseline; allowlist timestamps and IDs.
- [ ] 13.6 CI job `consistency-matrix` running all scenarios through all three runners in parallel.
- [ ] 13.7 CI job running `aeterna tenant validate` against every fixture as a fast pre-check.

### 14. Documentation

- [ ] 14.1 `docs/tenant-provisioning.md` — lifecycle (bootstrap → render → edit → diff → apply), manifest reference, worked example using `FileRef` in dev and `K8sSecretRef` in cluster.
- [ ] 14.2 Update CLI reference docs for new subcommands.
- [ ] 14.3 Security appendix: secret input modes, token scopes, readiness contract.
- [ ] 14.4 Migration guide: moving from fine-grained subcommands to `render` + `apply`.
