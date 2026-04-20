# AUDIT: harden-tenant-provisioning vs. current master

**Audit date:** 2026-04-20
**Audited against commit:** `fab6405` (release: 0.8.0-rc.3)
**Method:** Static read of source tree. No runtime verification.

> This audit is generic: no deployment names, customer names, environment
> identifiers, or hostnames are referenced anywhere below.

## Legend

- ✅ **DONE** — already implemented on master; task should be removed or marked complete
- 🟡 **PARTIAL** — type/endpoint exists but behavior/contract does not match the spec delta
- ❌ **MISSING** — genuine gap; task is a real delta
- ⚠️ **CONFLICTS** — task as written contradicts an existing architectural choice; needs redesign before implementation

---

## Group 1 — Manifest schema and hashing

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 1.1 | `metadata.generation` + `ManifestProviders` on `TenantManifest` | ❌ MISSING | `ManifestProviders` has zero hits anywhere. No `metadata` block on `TenantManifest` (`cli/src/server/tenant_api.rs:3404-3489`). |
| 1.2 | `SecretReference` sum type (K8s/File/Env/Vault) | ❌ MISSING | `TenantSecretReference` in `mk_core/src/types.rs:263` is a flat struct with no `kind` discriminator. **Breaking change in `mk_core` — needs migration plan.** |
| 1.3 | Canonical YAML serialization (`manifest_canonical` module) | ❌ MISSING | No such module. |
| 1.4 | `manifest_hash()` returning `sha256:<hex>` | ❌ MISSING | Zero hits for `manifest_hash`. |
| 1.5 | `last_applied_manifest_hash` + `generation` columns + migration | ❌ MISSING | Zero hits. |
| 1.6 | Hash persistence, monotonic generation, no-op short-circuit | ❌ MISSING | Test `provision_tenant_idempotent_reapply` (L5674) exists but tests idempotence via `ensure_tenant_with_source`, not via hash short-circuit. Rewrite needed. |
| 1.7 | Schema validation (`validate_manifest`) | 🟡 PARTIAL | `validate_manifest()` exists and is called at `tenant_api.rs:3609`. But it does not validate `SecretReference.kind` (doesn't exist yet) or `generation` monotonicity. |

## Group 2 — Dry-run, diff, render endpoints

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 2.1 | `?dryRun=true` on provision | ❌ MISSING | `dryRun` query param not handled in `provision_tenant` (L3599). |
| 2.2 | `GET .../manifest` render endpoint | ❌ MISSING | Not in router. |
| 2.3 | `?redact=true` mode | ❌ MISSING | No render endpoint exists yet. |
| 2.4 | `POST .../diff` endpoint | ❌ MISSING | Not in router. |
| 2.5 | Per-step `dry_run` marker in audit log | ❌ MISSING | Depends on 2.1. |

## Group 3 — Secret reference resolution

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 3.1 | `SecretResolver` trait | ⚠️ CONFLICTS | `SecretResolver` already exists in `memory/src/provider_registry.rs:92` — but as a **type alias for an async closure**, not a trait. Spec should decide: extend the existing alias, or introduce a trait alongside it. The two approaches are not interchangeable. |
| 3.2 | `K8sSecretRefResolver` impl | ❌ MISSING | No implementations exist. Current wiring uses a single `KubernetesTenantConfigProvider` as the concrete backend (`bootstrap.rs:171`) — not a per-kind resolver model. |
| 3.3 | `FileRefResolver` with mode ≤ 0600 check | ❌ MISSING | N/A without 1.2. |
| 3.4 | `EnvRefResolver` / `VaultRefResolver` | ❌ MISSING | N/A without 1.2. |
| 3.5 | Wire resolver into per-request secrets provider | ⚠️ CONFLICTS | Resolution is **already per-request** via the closure-based `SecretResolver` in the provider registry. Task wording implies this is missing; it is not. Task should be rewritten to describe the migration from the single-backend closure to a kind-dispatched resolver. |
| 3.6 | Never log / serialize / cache plaintext | 🟡 PARTIAL | Needs explicit audit. No obvious leaks in `provider_registry.rs`, but no systematic test guarantees this. |

## Group 4 — Inline-secret gating

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 4.1 | `allow_inline_secret` server config flag | ❌ MISSING | Zero hits. |
| 4.2 | `?allowInline=true` query param on provision | ❌ MISSING | Not handled. |
| 4.3 | Reject inline `secretValue` unless gated | ❌ MISSING | `ManifestSecret.secret_value: String` is currently accepted without gating. |

## Group 5 — Per-tenant provider wiring and readiness gate

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 5.1 | `TenantRuntimeState` enum | ❌ MISSING | Zero hits. |
| 5.2 | Startup iteration: resolve manifest providers, wire per-tenant | ⚠️ CONFLICTS | Current architecture is **lazy per-request resolution** via `TenantProviderRegistry` + `ConfigResolver`/`SecretResolver` closures (`bootstrap.rs:174-203`). The registry is wired into `MemoryManager` via `with_provider_registry`. Spec as written proposes eager startup-time wiring — this is an architectural change, not a gap fix. **Decide: keep lazy with health-probe-on-first-use, or switch to eager.** |
| 5.3 | `/ready` gates on tenant resolution | 🟡 PARTIAL | `/ready` exists (`cli/src/server/health.rs:40`) but only checks PG + vector backend. No tenant-state gate. |
| 5.4 | HTTP 503 `tenant_unavailable` short-circuit | ❌ MISSING | Zero hits for `tenant_unavailable`. |
| 5.5 | `GET .../status` endpoint | 🟡 PARTIAL | Endpoint exists at `/admin/tenants/{tenant}/providers/status` (router L276, handler L2818). Response types `ProviderStatusInfo` / `TenantProviderStatusResponse` exist (L2304, L2316). **But** the response shape is `{llm, embedding}` — does not include the spec's `state`, `reason`, `providersWired[]`, `providersFailed[]`. Extension needed, not greenfield. |
| 5.6 | Metrics `aeterna_tenant_state` + `aeterna_tenant_wiring_duration_seconds` | ❌ MISSING | Not found. |

## Group 6 — First-run bootstrap hardening

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 6.1 | Idempotent bootstrap + `/admin/bootstrap/status` | ❌ MISSING | No `bootstrap/status` route. No `BootstrapStatus` type. |
| 6.2 | `/ready` gated on bootstrap completion | ❌ MISSING | See 5.3. |
| 6.3 | Structured failure logging + retry on pod restart | 🟡 UNKNOWN | Needs code read of `bootstrap.rs:1-160` — not audited. |
| 6.4 | `BootstrapCompleted` governance event | ❌ MISSING | Zero hits. |

## Group 7 — CLI: apply, render, diff, validate, watch

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 7.1 | `TenantCommand::Apply` | ❌ MISSING | Not in enum (`cli/src/commands/tenant.rs:15`). |
| 7.2 | `TenantCommand::Render` | ❌ MISSING | Not in enum. |
| 7.3 | `TenantCommand::Diff` | ❌ MISSING | Not in enum. |
| 7.4 | `TenantCommand::Validate` (top-level) | ⚠️ PARTIAL/CONFLICTS | Two **nested** `Validate` subcommands exist: `tenant repo-binding validate` (L82) and `tenant config validate` (L94). Spec proposes a top-level `tenant validate`. Decide how these coexist. |
| 7.5 | `TenantCommand::Watch` | ❌ MISSING | Not in enum. |
| 7.6 | Re-implement fine-grained subcommands on top of `apply` | ❌ MISSING | All existing subcommands (`Create`, `Update`, `DomainMap`, `RepoBinding`, `Config`, `Secret`, `Connection`) are direct calls to legacy endpoints. |
| 7.7 | Inject `X-Aeterna-Client-Kind: cli` header | ❌ MISSING | Zero hits for `X-Aeterna-Client-Kind` anywhere. |

## Group 8 — CLI: secure secret input

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 8.1–8.5 | `--ref`, `--from-file`, `--from-stdin`, `--from-env`, mode/echo guards | ❌ MISSING | Current `tenant secret set` accepts plain `--value`. |

## Group 9 — CLI: output and exit codes

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 9.1 | `output::Renderer` (`table/json/yaml/name/jsonpath`) | ❌ MISSING | No such module. |
| 9.2 | TTY-aware default format | ❌ MISSING | |
| 9.3 | Standardized `ExitCode` enum (0/1/2/3/4/5) | ❌ MISSING | Ad-hoc `anyhow::Result<()>` everywhere; CLI exits via `main` propagation. |
| 9.4 | HTTP→exit-code mapping | ❌ MISSING | |

## Group 10 — Scoped tokens

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 10.1 | `token_type` + `scopes` JWT claims | 🟡 PARTIAL | `token_type: "Bearer"` is emitted in OAuth responses (`cli/src/client.rs`, `plugin_auth.rs`) but there is no JWT claim named `token_type` or `scopes`, and no `scoped` token kind. |
| 10.2 | `POST /api/v1/auth/tokens` endpoint | ❌ MISSING | Route does not exist. |
| 10.3 | `DELETE /auth/tokens/{id}` + 60s revocation cache | ❌ MISSING | |
| 10.4 | `GET /auth/tokens` list | ❌ MISSING | |
| 10.5 | Scope-check middleware | ❌ MISSING | Current middleware only does role checks. |
| 10.6 | CLI `--token` rejected; env → keychain → file chain | ❌ MISSING | |

## Group 11 — Audit parity

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 11.1 | Extend audit schema with `via`, `client_version`, `manifest_hash`, `generation`, `dry_run` | ❌ MISSING | No `client_version` / `manifest_hash` audit fields. |
| 11.2 | Extract `X-Aeterna-Client-Kind` in router | ❌ MISSING | Header not parsed anywhere. |
| 11.3 | Normalize unknown `client_kind` values | ❌ MISSING | |
| 11.4 | Every provision-path mutation records new fields | ❌ MISSING | Blocked by 11.1. |

## Group 12 — Admin UI wizard

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 12.1–12.9 | Create-tenant wizard, secret picker, preview, download | ❌ MISSING | No `*wizard*` / `*provision*` files under `admin-ui/src`. `TenantListPage.tsx` / `TenantDetailPage.tsx` exist but no manifest-based flow. |
|  | `GET /admin/secret-sources` | ❌ MISSING | Route does not exist. |

## Group 13 — Consistency acceptance suite

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 13.1–13.7 | Scenario fixtures + three runners + CI matrix | ❌ MISSING | No `tests/tenant_provisioning/` directory. Existing tests in `cli/tests/` are single-runner. |

## Group 14 — Documentation

| # | Task | Status | Evidence |
|---|------|--------|----------|
| 14.1 | `docs/tenant-provisioning.md` | ❌ MISSING | Not in `docs/`. |
| 14.2 | CLI reference updates | ❌ MISSING | |
| 14.3 | Security appendix | ❌ MISSING | |
| 14.4 | Migration guide | ❌ MISSING | |

---

## Summary

| Status | Count |
|--------|------:|
| ✅ DONE | 0 |
| 🟡 PARTIAL | 6 |
| ❌ MISSING | 58 |
| ⚠️ CONFLICTS (needs redesign) | 4 |
| **Total tasks** | **68** |

### The four conflicts that must be resolved before coding

1. **3.1 / 3.5** — `SecretResolver` already exists as a closure type alias. Decide: extend alias, or introduce a parallel trait and migrate. These are not compatible without a clear transition plan.
2. **5.2** — Existing architecture is **lazy per-request provider resolution** via `TenantProviderRegistry`. Spec assumes eager startup wiring. Pick one and rewrite the task accordingly.
3. **1.2** — Moving `TenantSecretReference` from flat struct to sum type is a breaking change in `mk_core`. Needs an explicit migration strategy (new type alongside old, deprecation window, or a hard cut).
4. **7.4** — Top-level `tenant validate` conflicts with existing nested `tenant repo-binding validate` and `tenant config validate`. Decide the command hierarchy before implementing.

### Recommended phasing (after conflicts are resolved)

**Phase B1 — Foundation (1 PR, ~2 days)**
- Resolve the four conflicts above (design note appended to `design.md`)
- Implement 1.1 (manifest metadata + providers block), 1.3, 1.4, 1.5, 1.6 (hash pipeline end-to-end)
- Migration for `last_applied_manifest_hash` + `generation`

**Phase B2 — Observability (1 PR, ~1 day)**
- 5.3 (/ready gate), 5.4 (tenant_unavailable short-circuit), 5.5 (extend existing status endpoint), 5.6 (metrics)
- 6.1 (bootstrap status endpoint), 6.4 (BootstrapCompleted event)

**Phase B3 — Dry-run + render + diff (1 PR, ~1 day)**
- 2.1, 2.2, 2.3, 2.4, 2.5

**Phase B4 — Secrets typing (1 PR, ~2 days, depends on conflict #1 and #3)**
- 1.2, 3.1–3.6, 4.1–4.3

**Phase B5 — CLI refactor (1 PR, ~2 days, depends on 7.4 conflict resolution)**
- 7.1–7.7, 8.1–8.5, 9.1–9.4

**Phase B6 — Scoped tokens (1 PR, ~1.5 days)**
- 10.1–10.6

**Phase B7 — Audit + UI + tests + docs (3 PRs, parallel)**
- 11.*, 12.*, 13.*, 14.*
