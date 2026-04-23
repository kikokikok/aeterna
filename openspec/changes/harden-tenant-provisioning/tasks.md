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
- [x] 0.2 ~~Decide provider wiring model~~ **RESOLVED** in `design.md` §D5: **Eager** (boot loop + Dragonfly pub/sub `tenant:changed` fan-out + lazy fallback on registry miss). Failure policy per-tenant by default, strict mode opt-in. Acceptance: freshly provisioned tenant is usable cluster-wide with no pod restart; zero user-visible 500s on race windows.
- [!] 0.3 Define `TenantSecretReference` migration path: introduce `TenantSecretReferenceV2` sum type alongside the existing flat struct, with a deprecation window — or commit to a single breaking bump of `mk_core`. Document in `design.md`. _(blocks 1.2, 3.2-3.4)_
- [!] 0.4 Decide CLI `validate` placement: top-level `tenant validate` subsumes the existing nested `tenant repo-binding validate` and `tenant config validate`, or coexists. Document in `design.md`. _(blocks 7.4, 7.6)_

---

## Phase B1 — Manifest schema + hashing foundation

### 1. Manifest schema and hashing

- [x] 1.1 Extend `TenantManifest` with `metadata.generation: u64` and `providers: ManifestProviders` (memoryLayers, llm, embedding). _(Shipped as part of the #94/#104 family: `ManifestMetadata { generation, labels, annotations }` and `ManifestProviders { llm, embedding, memory_layers }` at `cli/src/server/tenant_api.rs:3443-3525`; `ManifestProvider { kind, model, secret_ref, config }` with logical `secret_ref` into `config.secret_references`.)_
- [x] 1.2 Introduce `SecretReference` sum type per 0.3. Update `ManifestConfig.secret_references` accordingly. _(PR #127 — hard cut per design.md D1: full variant set shipped (`Inline`, `Postgres`, `Env`, `File`, `K8s`, `Vault`). Per-variant `validate_manifest` well-formedness checks; serde `tag="kind"` rejects unknown kinds at parse time (locked by dedicated test). `SecretBackend::{get,delete}` route non-Postgres variants to pre-existing `UnsupportedReference(kind)` until dispatching backends land. `SecretBytes`'s always-redact `Serialize` means `Inline` values cannot leak on reserialize; `expose_inline_plaintext()` is the only accessor, used solely on the write path. 10 mk_core tests + 9 validate_manifest tests added.)_
- [x] 1.3 Create `manifest_canonical` module: stable key order, inline secrets stripped. _(Shipped as `cli/src/server/manifest_hash.rs` by the #94/#104 family — `canonicalize()` emits lexicographic object keys via a recursive pass, `strip_plaintext` elides `secretValue`/`secret_value` fields before hashing. Not a separate `manifest_canonical` module — canonicalization lives with the hash since the two always travel together.)_
- [x] 1.4 Implement `manifest_hash(&TenantManifest) -> String` returning `sha256:<hex>`. _(`hash_manifest_value(&Value) -> Result<String, serde_json::Error>` at `cli/src/server/manifest_hash.rs:58`. 71-char output format `sha256:<64 hex>`. Takes `Value` (not `&TenantManifest` typed) on purpose so callers pass the raw wire shape and the hash stays stable across any typed-struct drift. 10 unit tests locking: prefix+length, byte-for-byte determinism, key-order stability, array-order preservation, plaintext stripping (both casings), generation sensitivity, empty-vs-missing distinction, known vector.)_
- [x] 1.5 Add `last_applied_manifest_hash: Option<String>` and `generation: u64` to the tenant record; write SQL migration. _(PR #105 — migration `027_tenant_manifest_state.sql`; `TenantStore::{get,set}_manifest_state` + `set_manifest_state_tx` with BYTEA↔hex helpers; 10 integration tests inc. idempotency, roundtrip, nonexistent-slug, tx rollback.)_
- [x] 1.6 Update `provision_tenant` (`tenant_api.rs:3599`) to compute+persist hash, enforce monotonic generation, short-circuit no-op re-applies. Rewrite `provision_tenant_idempotent_reapply` (L5674) to assert the short-circuit path. _(PR #121 — pre-mutation hash read+short-circuit returning 200 on unchanged manifest; post-commit `set_manifest_state_tx` under the same transaction as the tenant upsert; generation auto-increments; audit logs now carry `manifest_hash` and `generation`.)_
- [x] 1.7 Extend `validate_manifest` (called at L3609) to cover unknown `SecretReference.kind`, invalid generation, missing required provider declarations. _(Already shipped alongside 1.1: `validate_manifest_rejects_generation_zero` (tenant_api.rs:6132), `validate_manifest_rejects_unresolved_provider_secret_ref` (6207), `validate_manifest_rejects_provider_empty_kind` (6227) all green. "Unknown `SecretReference.kind`" case is structurally impossible today because serde's `#[serde(tag="kind")]` on the `SecretReference` enum rejects unknown tags at deserialization — the validator never sees a `SecretReference` it does not understand. Will need an explicit `UnknownKind` variant + rejection test once additional variants ship under task 1.2 / 0.3.)_

---

## Phase B2 — Observability + readiness

### 5. Per-tenant provider state and readiness gate _(design per 0.2)_

- [x] 5.1 Define `TenantRuntimeState` enum `{ Loading, Available, LoadingFailed { reason } }`. _(PR: `feat/b2-tenant-runtime-state` — adds `cli/src/server/tenant_runtime_state.rs` with the enum, an async-safe `TenantRuntimeRegistry`, monotonic `rev` counter that survives Loading/Failed transitions, `retry_count` on consecutive failures, and 11 unit tests.)_
- [x] 5.2 Implement the wiring model chosen in 0.2 (eager + pub/sub + lazy fallback). _(5.2a PR #118: eager boot loop `tenant_eager_wire::wire_all_known_tenants`, permissive by default, `AETERNA_EAGER_WIRE_STRICT=1` opt-in. 5.2b PR #110: Dragonfly pub/sub `tenant:changed` fan-out with instance-id self-echo filter and reconnect-with-backoff. 5.2c PR #119: `tenant_lazy_wire::ensure_wired` request-path fallback with per-slug coalescing and `AETERNA_LAZY_RETRY_COOLDOWN_SECS`-gated retry cache. 5.2-followup PR #122: `ResolverError {ConfigProviderFailed, BuildFailed}` + `try_get_*_service` fallible variants so `LoadingFailed{reason}` carries the real cause instead of a fake `Available`.)_
- [x] 5.3 Extend `/ready` (`cli/src/server/health.rs`) to include a tenant-state check in addition to current PG + vector checks. Return 503 when `failed > 0 && strict_mode`; never block on `Loading` to avoid pub/sub-induced LB flapping. _(PR #112 — `TenantCheck::from_snapshot`, sorted `failedSlugs`, reasons never leaked to the wire, `strictMode` toggled per-request so operators can flip `AETERNA_EAGER_WIRE_STRICT` without a pod restart. 7 unit tests including wire-contract camelCase guard.)_
- [x] 5.4 In tenant-scoped routes (`/api/v1/memory/*` and siblings), short-circuit to HTTP 503 `tenant_unavailable` when target tenant is `LoadingFailed`; 503 with `Retry-After: 1` when `Loading`. _(Shipped as part of the tenant-wiring stack — single chokepoint in `mod.rs::authenticated_tenant_context` calling `RequestContext::require_available_tenant` which runs `ensure_wired` then maps state→response: `Available`→pass, `Loading`→503 `loading` Retry-After:1, `LoadingFailed(reason="tenant not found")`→404, other `LoadingFailed`→503 `loadingFailed` Retry-After:30 aligned with lazy-wire cooldown. Reasons never leak: only `state` and `retryAfterSeconds` go on the wire.)_
- [x] 5.5 Admin-only wiring-state inspection endpoints. _(Implemented as the new `/admin/tenants/wiring` + `/admin/tenants/{slug}/wiring` family in `tenant_wiring_api.rs` rather than extending `/providers/status` — the latter describes **configured** providers (static), the former reflects **registry runtime state** (dynamic, per-pod). Both GETs are side-effect-free (explicitly do NOT call `ensure_wired` so "pod hasn't seen this tenant" stays distinguishable from "pod failed to wire"). PR #120. Reasons ARE exposed here — PlatformAdmin-gated — which is the authenticated surface referenced by 5.3/5.4 comments.)_
- [x] 5.6 Emit Prometheus metrics for wiring state. _(Shipped in `tenant_metrics.rs`: `tenant_state{slug,state}` gauge 0/1 per state following the `kube_pod_status_phase` idiom; `tenant_wiring_duration_seconds{result}` histogram recorded on transitions out of `Loading` (slug intentionally omitted — fleet-level concern); `tenant_state_transitions_total{from,to}` counter. Emission happens under the registry write lock so `prev→next` is atomic for the recorder. `forget()` resets all three gauge series to 0. b2-5.6-followup also landed: `metrics_util::debugging::DebuggingRecorder` + `with_local_recorder` parallel-safe capture helpers power emission assertions in unit tests.)_

### 6. First-run bootstrap hardening

- [x] 6.1 Add `BootstrapTracker` (cli/src/server/bootstrap_tracker.rs) + `GET /api/v1/admin/bootstrap/status` (PA-gated, cli/src/server/bootstrap_api.rs). `bootstrap()` instruments 6 phases — `env_and_config`, `database`, `knowledge_git`, `memory_and_providers`, `sync_and_protocols`, `redis_and_auth_stores`, `assemble_state` — and finalizes with `mark_ready()` before returning `AppState`. 14 unit tests cover wire shape, redaction, idempotency, and failure semantics. Idempotency of the underlying seed helpers (e.g. `seed_platform_admin`) is pre-existing (`ON CONFLICT DO NOTHING`) — no refactor required for the tracker to report truthfully.
- [x] 6.2 Structurally satisfied by current architecture: `bootstrap()` runs synchronously in `serve::run` **before** the HTTP listener binds, so no request can observe a mid-bootstrap state. A failing bootstrap exits the process → kubelet restart → 503 on the Service (no listener at all). `BootstrapTracker::is_completed()` is provided as a future-proofing seam for async-post-bind bootstrap refactors (see module docs in `bootstrap_tracker.rs`).
- [x] 6.3 Structurally satisfied (same reasoning as 6.2). Upstream errors from seed_* / postgres.initialize_schema propagate through `?` and crash the pod; the kubelet retry IS the restart path the task describes. Follow-up 6.4 will surface them as governance events.
- [ ] 6.4 Emit `BootstrapCompleted` governance event on success (follow-up; tracker snapshot is the exact payload).

---

## Phase B3 — Dry-run, diff, render

### 2. Dry-run, diff, render endpoints

- [x] 2.1 Add `?dryRun=true` branch to `provision_tenant` that runs every read-only check in plan-only mode and returns a structured `ProvisionPlan`. _(Shipped in PR #126. Branch lives **after** the generation gate and **before** the first write, so dry-run preserves validate_manifest + canonical hash + short-circuit + generation-conflict as real errors but emits no writes, no `TenantCreated`/`TenantConfigChanged` events, and a dedicated `tenant_provision_dry_run` audit action. Plan contains status classifier (`create`/`update`/`unchanged`), incoming/current hash pair, current/next generation, and per-section presence flags. Structural diff per section is §2.4's remit and is blocked on full-fidelity renderer.)_
- [x] 2.2 Implement `GET /api/v1/admin/tenants/{slug}/manifest` returning the rendered current-state manifest (secret values excluded; references only). _(Shipped in `cli/src/server/manifest_render.rs` + `manifest_api.rs`. **Rendered:** `tenant` (incl. `tenant.domainMappings` as of B3 follow-up — sorted ASC via new `TenantStore::list_domain_mappings`, omitted when empty to round-trip with unmapped-tenant input), `metadata` (generation + hash), `config` (fields + secret_references), `repository`. **Not rendered (tracked in `notRendered: [...]`):** `hierarchy`, `roles`, `providers`. **Intentionally excluded from `notRendered`:** top-level `secrets:` is wire-input-only (carries plaintext `SecretBytes` the server never retains — same class as `SecretReference::Inline`); the durable form of those secrets is visible via `config.secretReferences`. PA-gated via `authenticated_platform_context`.)_
- [x] 2.3 Implement `?redact=true` mode (reference names replaced with opaque placeholders for `tenant:read`-only callers). _(Same PR as 2.2. `redact=true` replaces every `secret_references` map key and `logical_name` with a deterministic `secret-N` placeholder (indexed by sorted original-key order, so two pods produce identical output), replaces each backend-specific reference with `{"kind":"redacted"}`, and routes the repository binding through the existing `TenantRepositoryBinding::redacted()` helper. Endpoint stays PA-gated today — a `tenant:read`-scoped lower-trust tier will arrive with the scoped-tokens work in §10.)_
- [ ] 2.4 Implement `POST /api/v1/admin/tenants/diff` returning structured delta between submitted manifest and current state. _(Sequenced after full-fidelity renderer: a diff is only trustworthy when every manifest section round-trips. §2.2-A landed `providers` (llm + embedding) forward-apply + reverse-render with secret_ref operator-name recovery. PR #128 landed `tenant.domainMappings` reverse-render and clarified top-level `secrets` as wire-only (not a gap). Remaining forward-path gaps: `hierarchy`, `roles`, `providers.memoryLayers` — details in `FINDINGS-2-2.md`. Diff ships after those close.)_
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

- [ ] 7.1 Add `TenantCommand::Apply(TenantApplyArgs)` with `-f/--file`, `--dry-run`, `--prune`, `--watch`, `--wait`. _(Partial: `aeterna tenant validate --file <path|->` shipped as the first CLI consumer of the dry-run endpoint — posts the manifest to `/admin/tenants/provision?dryRun=true`, renders the `ProvisionPlan` (status / hash pair / generation / section flags) on success, prints `validationErrors` and exits non-zero on HTTP 422. `tenant apply` proper — the non-dry-run variant with `--prune`/`--watch`/`--wait` — is the remaining scope of this task.)_
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
