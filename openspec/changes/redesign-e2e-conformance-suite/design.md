# Design — redesign-e2e-conformance-suite

**Status:** scope draft, awaiting review
**Target branch:** `openspec/redesign-e2e-conformance-suite`

## Context

See `proposal.md`. In short: the existing Newman suite is not in CI, not self-bootstrapping, not self-cleaning, doesn't test the PR's code, and 70% duplicates cheaper layers. This document records the consequential design decisions for the rebuild.

## Decisions

### D1 — Suite identity: conformance, not feature-coverage

The new suite has **one** unique value proposition: assert that a freshly helm-installed `aeterna` on a real cluster, given one bootstrap secret, can be driven through a full tenant lifecycle by an unauthenticated client and ends with zero leaked state. Anything else (per-feature semantic correctness) belongs in the cheaper layers (`cargo test`, Playwright, §13 consistency suite).

**Implication:** when in doubt about whether a test belongs in the suite, the question is *"does this assertion need a real ingress / real TLS / real Dragonfly / real CNPG / real Cedar / real OPAL to be meaningful?"* If no, it does not belong here.

**Rejected alternative:** "make Newman the master e2e suite covering everything." Rejected because (a) it duplicates cargo+Playwright+§13 at 10× the runtime cost, and (b) Newman's per-request assertion model is weaker than Rust integration tests for anything that isn't pure HTTP.

### D2 — Bootstrap secret model: GitHub Actions secrets, two secrets + one var

The suite needs to mint a platform-admin JWT in-CI (so it can act as a PA without an OAuth flow) and call a real GitHub App (so connection grant/revoke exercises real OAuth wire).

**Decision:**
- `AETERNA_E2E_PA_SIGNING_KEY` (GH Actions secret) — the JWT signing key used by the helm-installed instance; the suite uses the same key to mint a short-lived (≤30 min) PA JWT in its bootstrap script.
- `AETERNA_E2E_GITHUB_APP_KEY` (GH Actions secret) — PEM private key of a test-only GitHub App in the `kikokikok-test` org.
- `AETERNA_E2E_GITHUB_APP_ID` (GH Actions var, not secret) — the App ID is not sensitive.

The helm install reads `AETERNA_E2E_PA_SIGNING_KEY` from a k8s secret (created by the workflow `before` step). The Newman bootstrap folder reads it from `process.env` and mints a JWT in a pre-request script. The signing key is **never** written to disk and **never** logged.

**Rejected alternatives:**
- *1Password via `op` CLI.* Better long-term hygiene, but adds a dependency and a service-account credential of its own. Park for v2.
- *AWS Secrets Manager.* Same logic — adds an SDK dep + IAM. v2 candidate.
- *Static fixtures committed to a `aeterna-test-secrets` private repo.* Rejected: deploy keys / sub-token rotation becomes ops debt with no upside vs. GH Actions secrets.

### D3 — GitHub side: real test-only App for CI, wiremock fallback for local

The connection grant/revoke flow is one of the few code paths that touches real GitHub OAuth. Using wiremock in CI would make the suite cheaper but would silently miss any GitHub-side wire-format drift.

**Decision:** CI uses a real GitHub App. Local dev defaults to a wiremock-backed fallback selected by `E2E_GITHUB_MODE=mock` (the default when `AETERNA_E2E_GITHUB_APP_KEY` is unset).

**Implication:** the wiremock fixture lives at `e2e/wiremock/github-app/` with one mapping file per endpoint we hit. CI verifies the wiremock fixture is in lock-step with the real GH responses by running both modes nightly on a `wiremock-drift-check` schedule and diffing the response bodies (excluding timestamps + nonces).

### D4 — Test infrastructure: kind cluster, PR's image loaded directly

Each CI run boots a fresh kind cluster, builds the PR's commit into a Docker image, loads it into the kind nodes via `kind load docker-image`, then `helm install`s the PR's chart pointing at that image. No registry push.

**Decision values:**
- kind version: pinned in the workflow (`v0.23.0` at time of writing). Bump explicitly.
- helm chart: `helm/aeterna` with `e2e/test-values.yaml` overrides (single-replica everything, low resource requests, hostPath-backed CNPG for fast boot, Dragonfly with `appendonly no`, no Okta sidecar).
- Cluster lifetime: ≤45 min (workflow timeout). Reaping is by GH Actions runner cleanup, not explicit `kind delete cluster` (runner is ephemeral).

**Rejected alternative:** GKE/EKS test cluster. Rejected: cost + IAM blast radius + 5–10× boot time.

### D5 — Tenant naming: collision-safe across parallel runs

The slug must be unique across concurrent runs (matrix builds, re-runs after force-push). Cannot rely on a static `e2e-tenant` slug.

**Decision:** `e2e-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}`. Slug regex (`[a-z0-9-]+`) compatible. Bounded to ≤63 chars by GH Actions guarantees. The cluster is single-tenant-per-run anyway (one kind cluster per workflow run), but this safeguards against a future shared-cluster mode.

### D6 — Teardown contract: bash-trapped, asserts cleanliness, fails loud on leaks

Teardown is the single most important property for trust in the suite. If teardown is unreliable, every red CI run risks leaving a poisoned cluster that masks the next failure.

**Decision:**
1. Teardown runs in `bash trap` from the runner script — fires on success, on assertion failure, and on any signal. The trap is set *before* the bootstrap folder runs, so a bootstrap-phase crash still triggers teardown.
2. Teardown is a separate Newman folder (`Z. Teardown`) so it produces structured pass/fail signal in the report, not just stderr.
3. Final teardown step is a **leak assertion**: `GET /admin/tenants/{slug}` returns 404, `GET /admin/git-provider-connections?tenant=<slug>` returns empty, and a sentinel admin endpoint `GET /admin/diagnostics/tenant-residue?slug=<slug>` (new — see §D11) returns `{ residual: false }`.
4. If teardown itself fails, the workflow exits non-zero **even if all functional tests passed**. Loud > silent.
5. Cluster teardown happens after Newman teardown (kind cluster is the outermost destructible). If kind delete fails, that's a runner-level concern; we don't fight it.

### D7 — Coverage compression: 40 folders → 8

The current 40 folders span ≈293 requests. Audit (in `proposal.md`) shows 70% duplicate cheaper layers. The new suite's folder list is finite and motivated:

| New folder | What it's good for | Replaces in old suite |
|---|---|---|
| `0. Bootstrap` | PA JWT mint, service token mint, fake-github connection setup, env hydration | (new) |
| `1. Topology Conformance` | TLS cert chain, ingress routing, `/health` `/version` `/metrics` shape, HTTP/2 negotiation, gzip negotiation | shrinks old `1. Deployment Validation` from 8 requests to 4 |
| `2. Manifest API Lifecycle` | render → edit → diff → apply → render-again over real HTTP, hash determinism, generation-monotonicity rejection | (new — old #29 used legacy CRUD) |
| `3. SSE Event Stream` | `/events` ordering, lagged-frame recovery, keep-alive timing, connection re-establishment | (new) |
| `4. Service Token Lifecycle` | mint → use → revoke → 401-within-60s (the §10.3 cross-instance cache TTL contract) | (new) |
| `5. Scope Enforcement Matrix` | one negative case per scope: token without scope X gets 403 on every route requiring X | (new — covers §10.5 wiring) |
| `6. Connection Grant/Revoke` | `connections:manage` happy + idempotent re-grant + revoke of nonexistent = 404 | partially old #33 |
| `7. Backup Round-Trip Smoke` | export → import on the live tenant, no exhaustive permutations (cargo owns those) | shrinks old `38. Backup` from 9 requests to 3 |
| `Z. Teardown` | revoke token, delete connection, purge tenant, leak assertions | replaces useless old #37 |

**Explicitly deleted (reasons in tasks.md):** old folders `2-12, 15-28, 30-36, 39-40` — every assertion duplicated in cargo or Playwright or §13.

### D8 — Two-tier execution: `fast` on every PR, `full` on master + label

Running the full conformance suite on every PR commit is expensive. Most PRs don't touch deployment topology.

**Decision:**
- **`fast` profile** (target ≤15 min): folders `0, 1, 2, 4, 6, Z`. Runs on every PR that touches `cli/`, `helm/`, `mk_core/`, `memory/`, `admin-ui/`, or `e2e/`. Skipped via path filter otherwise.
- **`full` profile** (target ≤30 min): adds folders `3, 5, 7` plus one chaos test (kill the aeterna pod mid-provision and assert the orphan tenant either rolls forward to ready or rolls back to absent — never half-applied). Runs on master push and on PRs with the `e2e:full` label.
- A nightly schedule (`02:00 UTC`) runs `full` on master to catch flake.

**Rejected alternative:** "always run full." Rejected: ≈30 min of CI on every commit is a steep tax for the marginal signal beyond `fast`.

### D9 — Failure model: fail loud on infra, continue on assertions

Different failure classes warrant different responses.

**Decision:**
- **Infra failure** (kind boot, helm install, image load, `/health` never goes green): fail the workflow immediately, do **not** run Newman. No teardown needed (cluster is ephemeral). Surface as `infra-bootstrap-failure` distinct status.
- **Bootstrap folder failure** (folder `0`): one automatic retry with a fresh PA JWT. If retry fails, abort and run teardown. This is the only retry in the suite.
- **Assertion failure** (folders `1`–`7`): `newman --bail false` — keep running remaining folders to maximise per-run signal, then run teardown. Workflow exits non-zero. Failed assertions are uploaded as artifacts.
- **Teardown failure**: workflow exits non-zero even if functional tests passed. Cluster-state dump uploaded as artifact (`kubectl get all -A`, helm history, recent events).

### D10 — Where the suite lives + tooling choice

The collection stays in `e2e/`. The runner script grows but its shape (`check_prereqs → smoke_test → run_newman`) is preserved. Newman remains the runner. Postman/Newman is good enough at HTTP and supports pre-request scripts for the bootstrap-then-collection pattern natively.

**Rejected alternatives:**
- *k6.* Rejected: load-testing tool, weaker per-request assertions, would need a parallel test framework for the lifecycle flow.
- *Playwright.* Rejected: it already owns the UI layer; using it for HTTP-only conformance is a square peg.
- *Bash + curl + jq.* Rejected: passable for one-off scripts, painful for 60+ assertions.
- *Cargo integration tests against the deployed cluster.* Tempting (we already have ≈6,000 cargo tests), but conflates "what does my code do" with "what does the deployed system do" — and the test binary would need its own client config, retry semantics, and HTTP/2 plumbing that Postman gives free.

### D11 — Sentinel "tenant residue" endpoint

The teardown leak-assertion in §D6 needs a single API call that authoritatively answers *"is there any leftover state for this tenant?"* Querying every relevant table individually is brittle (next migration adds a table; teardown silently misses it).

**Decision:** add `GET /admin/diagnostics/tenant-residue?slug=<slug>` (PA-only) returning `{ residual: bool, tables: { tenant_secrets: 0, tenant_provider_configs: 0, tenant_audit: 0, … } }`. The list of tables is sourced from a single registry (`mk_core::tenant_residue::INSPECTED_TABLES`), updated whenever a new tenant-scoped table lands. The registry itself has a unit test that asserts every `tenant_id`-bearing table in the schema is listed (queries `information_schema.columns` at test time).

This endpoint is also useful for operator-side incident response, so it's not test-only scaffolding.

### D12 — Local-dev story

Contributors must be able to run the suite locally without GH App credentials.

**Decision:**
- `e2e/run-e2e.sh --local` boots a kind cluster the same way CI does, helm-installs the local working-tree's chart, runs the suite with `E2E_GITHUB_MODE=mock` against the wiremock fixture.
- Without `--local`, the script targets whatever cluster `KUBECONFIG` points to, refuses to run unless `AETERNA_E2E_PA_SIGNING_KEY` is set, and uses real GitHub.
- A make target `make e2e-local` is the documented entry point for new contributors.

### D13 — Configurability & downstream redistribution

The suite must be runnable not just from `kikokikok/aeterna`'s CI, but from any consumer's internal repo against their own aeterna deployment, with their own secrets backend and (potentially) their own GitHub Enterprise instance. Concrete consumer in mind: an internal Kyriba repo dedicated to the Kyriba production aeterna deployment that wants to run the same conformance suite as a recurring health gate.

**Decision:** every deployment-specific value is an environment variable with a documented default. The suite ships three orthogonal mode dials:

1. **`AETERNA_E2E_CLUSTER_MODE`** — what brings up the system under test:
   - `kind-bootstrap` (default for `--local` / `kikokikok/aeterna` CI) — boot kind, build image, helm-install. Phase E3 owns this path.
   - `existing-kubeconfig` — assume `KUBECONFIG` points at a working cluster with aeterna already running; skip kind/helm bring-up; useful for staging-conformance runs against a long-lived cluster.
   - `external-https` — no kubectl required at all; treat the target purely as an HTTPS endpoint; skip cluster-state failure-artifact dump (use `--keep-logs-only` instead). Useful for SaaS-style consumers verifying a managed aeterna instance.
2. **`AETERNA_E2E_SECRETS_BACKEND`** — where secret values come from:
   - `env` (default) — values already injected as env by the caller (the GH Actions case satisfies this with `secrets:` blocks).
   - `op` — 1Password CLI: `op read op://vault/item/field`
   - `aws-sm` — AWS Secrets Manager: `aws secretsmanager get-secret-value --secret-id`
   - `vault` — HashiCorp Vault: `vault kv get -field`
   - Each backend is implemented as `e2e/secrets/<backend>.sh` (a small adapter taking a logical secret name, returning the value to stdout). Runner script shells out: `value=$(bash "e2e/secrets/${BACKEND}.sh" resolve "AETERNA_E2E_PA_SIGNING_KEY")`. New backends are one file each.
3. **`AETERNA_E2E_GITHUB_MODE`** — extended from §D3:
   - `mock` (default when no GH App key) — wiremock fixture
   - `real` — `https://api.github.com` with a real test App (the `kikokikok/aeterna` CI case)
   - `ghe` — same as `real` but targets `AETERNA_E2E_GITHUB_API_URL` (e.g. `https://github.kyriba.com/api/v3`); for consumers running against GitHub Enterprise.

**Full configurable surface** (defaults shown; all overridable via env or `e2e.config.yaml`):

| Variable | Default | Purpose |
|---|---|---|
| `AETERNA_E2E_BASE_URL` | `https://aeterna.local:8443` | API target |
| `AETERNA_E2E_INGRESS_HOST` | `aeterna.local` | TLS SAN check (folder 1.1) |
| `AETERNA_E2E_TLS_CA_FILE` | _(empty = system CAs)_ | Custom CA bundle path for internal CA-signed certs |
| `AETERNA_E2E_TLS_INSECURE` | `false` | Allow `--insecure`; only for kind self-signed mode |
| `AETERNA_E2E_TENANT_SLUG_PREFIX` | `e2e-` | Slug namespacing per consumer (e.g. `kyriba-conformance-`) |
| `AETERNA_E2E_TENANT_SLUG_SUFFIX` | `${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}` or `local-${EPOCH}` | Collision avoidance |
| `AETERNA_E2E_PA_SIGNING_KEY` | _(required)_ | Resolved via secrets backend |
| `AETERNA_E2E_PA_JWT_ALG` | `HS256` | `RS256` for asymmetric setups (key file then in `AETERNA_E2E_PA_SIGNING_KEY_FILE`) |
| `AETERNA_E2E_PA_JWT_AUDIENCE` | `aeterna` | JWT `aud` |
| `AETERNA_E2E_PA_JWT_ISSUER` | `e2e-bootstrap` | JWT `iss` |
| `AETERNA_E2E_PA_JWT_TTL_SECONDS` | `1800` | PA JWT validity window |
| `AETERNA_E2E_GITHUB_MODE` | `mock` _(or `real` if `…GITHUB_APP_KEY` set)_ | See above |
| `AETERNA_E2E_GITHUB_API_URL` | `https://api.github.com` | GHE override |
| `AETERNA_E2E_GITHUB_APP_ID` | _(required for `real`/`ghe`)_ | Test App ID |
| `AETERNA_E2E_GITHUB_APP_KEY` | _(required for `real`/`ghe`)_ | PEM; resolved via secrets backend |
| `AETERNA_E2E_GITHUB_ORG` | `kikokikok-test` | Test org / installation target |
| `AETERNA_E2E_CLUSTER_MODE` | `kind-bootstrap` | See above |
| `AETERNA_E2E_SECRETS_BACKEND` | `env` | See above |
| `AETERNA_E2E_PROFILE` | `fast` | `fast` or `full` |
| `AETERNA_E2E_HTTP_TIMEOUT_MS` | `30000` | Per-request newman timeout |
| `AETERNA_E2E_KEEP_CLUSTER` | `false` | Skip `kind delete cluster` on exit (debug aid) |
| `AETERNA_E2E_REPORT_DIR` | `./e2e/results` | Where Newman HTML / JSON lands |

A single `e2e.config.yaml` file may set all of the above (env wins); useful for downstream consumers who want their config in source control rather than CI variables.

**Distribution shape:** `e2e/` is self-contained — no hard dependencies on paths outside itself **except** `helm/aeterna`, which it reads only when `CLUSTER_MODE=kind-bootstrap`. In `existing-kubeconfig` and `external-https` modes the suite needs nothing from the rest of the repo. A consumer can:

1. **Vendor:** `git subtree add --prefix=tools/aeterna-e2e https://github.com/kikokikok/aeterna openspec/redesign-e2e-conformance-suite --squash` (or simple copy). Their CI calls `bash tools/aeterna-e2e/run-e2e.sh --profile fast` with their env populated. Pinning is by commit SHA.
2. **Same-repo (the `kikokikok/aeterna` case):** `.github/workflows/e2e-conformance.yml` is the canonical caller. Phase E3.
3. **OCI image (v2 — out of scope here, but designed to support):** publish `ghcr.io/kikokikok/aeterna-e2e:<aeterna-version>` bundling newman + collection + run-e2e.sh. Consumers `docker run -e ... ghcr.io/.../aeterna-e2e:1.2.3`. Tracked as a follow-up; the design here doesn't preclude it.

**CI portability:** `e2e/templates/ci/` ships templates for the common CI systems:
- `github-actions.yml` — a generic version (different from `kikokikok/aeterna`'s own `e2e-conformance.yml`); reusable workflow shape so consumers `uses: kikokikok/aeterna/.github/workflows/e2e-conformance-reusable.yml@main` if they prefer not to vendor.
- `gitlab-ci.yml` — for GitLab consumers.
- `Makefile.snippet` — for any-CI use; just sets env + invokes `run-e2e.sh`.

Each template is a thin wrapper around env-var population + `run-e2e.sh` invocation. Anything CI-specific (artifact upload syntax, secret syntax) lives in the template; orchestration lives in `run-e2e.sh`.

**Reusable workflow vs vendoring:** consumers pick one. Vendoring trades update-friction for full control + air-gap support. The reusable workflow trades pin-version for simpler updates. Document both in §17.1; don't pick a winner.

**Rejected alternatives:**
- *"Make `kikokikok/aeterna` the only consumer; downstream just runs `helm test`."* Rejected: `helm test` covers `/health` + a few smoke-level assertions, not ingress/TLS/SSE/scope-matrix/teardown-leak. Downstream consumers operating internal deployments need the same signal we want.
- *"Hardcode all values; consumers fork."* Rejected: forks drift; conformance assertion drift defeats the suite's purpose.
- *"Use Helm `tests/` directory."* Rejected: Helm tests run inside the cluster, can't exercise external HTTPS / TLS chain / SSE properly, and have a poor reporting story.

**Acceptance:** AC6 (added in tasks.md) — a fresh checkout of an empty repo with only `e2e/` vendored + a 5-line CI snippet + the four required env vars must produce a green run against any conforming aeterna deployment. Verified by a smoke job in `kikokikok/aeterna`'s CI that simulates the consumer path against a private fixture deployment.

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Real GitHub rate-limits the test App | low | high (CI flakes) | App-token caching across folders; one App per branch (main + dev) so concurrency is bounded; nightly drift-check rotates separately |
| Kind boot is flaky on GH Actions runners | medium | high | Explicit timeout per boot phase; one retry on boot only; surface boot-phase logs as artifact |
| Teardown leaks a tenant despite the trap | low | high (next run sees stale state) | Cluster is ephemeral per run — kind cluster destruction is the ultimate cleanup, leak assertion is a belt-and-braces |
| Wiremock drifts from real GH | medium | medium | Nightly drift-check workflow runs both modes, fails on diff |
| §D11 residue endpoint becomes stale as new tables land | medium | medium | Registry unit test queries `information_schema` at test time and fails if any `tenant_id`-bearing table is missing from the registry |
| `fast` profile blows the 15-min budget | medium | medium | Per-folder timeout in Newman; profile shrinks before adding parallelism |
| Downstream consumer's env diverges from the documented variable set (D13) | medium | medium | Strict-mode flag on `run-e2e.sh` rejects unknown `AETERNA_E2E_*` vars; CI-side smoke job in §17.4 exercises the consumer-vendor path on every change to `e2e/` |
| Vendored copies in downstream repos go stale | high | low–medium | Document semver discipline on the `e2e/` directory; ship a `e2e/VERSION` file consumers can check; reusable-workflow alternative for consumers who don't want to vendor |
| Secrets backend shell-out leaks values into logs | low | high | Adapter scripts must redirect stderr only; `set -o pipefail` + `+x` discipline; explicit test in 14.5 that `AETERNA_E2E_PA_SIGNING_KEY` never appears in any artifact |

## Open questions

None as of this draft. (1Password vs GH Actions secrets answered in §D2 + extended in §D13; wiremock vs real GH in §D3; runtime tooling in §D10; portability + downstream redistribution in §D13.)
