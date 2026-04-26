# Tasks — redesign-e2e-conformance-suite

> Legend:
> - `[ ]` = not started
> - `[~]` = partially done
> - `[!]` = design conflict — must be resolved before coding
> - `[x]` = done

---

## 0. Resolve design conflicts (blocking)

- [x] 0.1 ~~Confirm GitHub Actions secrets as the v1 source-of-truth~~ **RESOLVED** in conversation 2026-04-26: confirmed. Operator (Christian) will create the test-only GitHub App. _(unblocks every CI-side task in §3)_
- [x] 0.2 ~~Confirm kind + helm-install model~~ **RESOLVED** in conversation 2026-04-26: confirmed.
- [x] 0.3 ~~Confirm two-tier execution~~ **RESOLVED** in conversation 2026-04-26: implicitly confirmed by approval of the change as a whole.
- [x] 0.4 ~~Confirm configurability + downstream-redistribution scope~~ **RESOLVED** 2026-04-26: a Kyriba-internal repo will vendor + run the same suite against the Kyriba internal deployment. Drove §D13 + Phase E7. _(unblocks 19.*, 20.*, 21.*, AC6)_

---

## Phase E1 — Demolition (do this first; reduces noise)

### 1. Delete the stale suite

- [ ] 1.1 Delete `e2e/aeterna-e2e.postman_collection.json` (293 requests across 40 folders).
- [ ] 1.2 Delete `e2e/aeterna-e2e.postman_collection.json.bak` and `e2e/fix_collection.py`.
- [ ] 1.3 Delete `e2e/results/run-*.json` history except the most recent (kept as a single sample for sidecar tooling).
- [ ] 1.4 Move `e2e/run-e2e.sh` to `e2e/run-e2e.sh.legacy` to keep the comments/usage as reference until §2 lands. Delete in 1.5 once §2 is green.
- [ ] 1.5 Delete `e2e/run-e2e.sh.legacy` after §2 green-lights.

---

## Phase E2 — Sentinel residue endpoint (server-side; unblocks teardown)

### 2. `GET /admin/diagnostics/tenant-residue` (per §D11)

- [ ] 2.1 New module `cli/src/server/tenant_residue_api.rs` with handler + router; PA-only via `require_platform_admin_or_scope(_, _, "tenants:read")`.
- [ ] 2.2 New module `mk_core/src/tenant_residue.rs` with `INSPECTED_TABLES: &[&str]` registry of every tenant-scoped table.
- [ ] 2.3 Implement handler: parallel `SELECT count(*) FROM <t> WHERE tenant_id = $1` for every table in the registry; aggregate into `{ residual: bool, tables: HashMap<&str, u64> }`.
- [ ] 2.4 Registry unit test in `mk_core/src/tenant_residue.rs`: query `information_schema.columns` at test time, assert every table with a `tenant_id` column is listed in the registry. Fails loud on schema drift.
- [ ] 2.5 Handler integration test in `cli/src/server/tenant_residue_api.rs::tests`: provision a tenant via `provision_tenant`, assert `residual=true`; purge it, assert `residual=false`.
- [ ] 2.6 Wire into `router::router` next to the rest of the diagnostics surface.
- [ ] 2.7 OpenAPI (`cli/openapi.yaml` if it exists; else doc-only at `docs/api/diagnostics.md`).

---

## Phase E3 — CI workflow (cluster boot + image load + helm install)

### 3. `.github/workflows/e2e-conformance.yml`

- [ ] 3.1 Workflow scaffold with two jobs (`fast`, `full`), shared `setup` step. Triggers: `pull_request` with paths filter (`cli/**`, `helm/**`, `mk_core/**`, `memory/**`, `admin-ui/**`, `e2e/**`); `push: master`; `schedule: '0 2 * * *'`; `workflow_dispatch`.
- [ ] 3.2 `setup` step: install kind (pinned `v0.23.0`), `kubectl` (latest stable), helm (pinned), newman (`npm i -g newman@6 newman-reporter-htmlextra`).
- [ ] 3.3 `setup` step (cont.): `kind create cluster --config e2e/kind-cluster.yaml --wait 5m`. Cluster config: 1 control-plane + 1 worker, port 8443→ingress.
- [ ] 3.4 `setup` step (cont.): build PR's image with `docker build -t aeterna:e2e .`, then `kind load docker-image aeterna:e2e`. (No registry push.)
- [ ] 3.5 `setup` step (cont.): create k8s secret with `AETERNA_E2E_PA_SIGNING_KEY`; `helm install aeterna ./helm/aeterna -f e2e/test-values.yaml --set image.tag=e2e --wait --timeout 5m`.
- [ ] 3.6 `setup` step (cont.): poll `/health` via `curl --resolve` (TLS-bypass for kind's self-signed cert) until 200 or 60 s timeout.
- [ ] 3.7 `fast` job: `bash e2e/run-e2e.sh --profile fast --bail false`. Timeout 15 min. Skipped via path filter for non-relevant PRs.
- [ ] 3.8 `full` job: `bash e2e/run-e2e.sh --profile full --bail false`. Timeout 30 min. Runs on master push, on `e2e:full` PR label, and on schedule. Includes the chaos test (3.9).
- [ ] 3.9 Chaos test (full profile only): mid-provision, `kubectl delete pod -l app=aeterna --grace-period=1`; assert tenant rolls forward to ready or rolls back to absent (never half-applied).
- [ ] 3.10 Failure artifacts: on red, upload `kubectl get all -A`, helm history, last 200 lines of aeterna pod logs, and Newman HTML report.
- [ ] 3.11 Branch protection rule update: `e2e-conformance / fast` becomes a required check for PRs touching the path-filter set.

### 4. Helm test-values + kind cluster config

- [ ] 4.1 `e2e/test-values.yaml`: replicas=1 everywhere, resource requests at minimum, CNPG hostPath-backed for fast boot, Dragonfly `appendonly no`, no Okta sidecar, ingress NodePort 8443.
- [ ] 4.2 `e2e/kind-cluster.yaml`: 1 cp + 1 worker, port mapping `8443:8443`, kubelet `eviction-hard` low so test pods don't get OOM-killed.

---

## Phase E4 — Newman collection (the actual tests)

### 5. Bootstrap folder `0. Bootstrap`

- [ ] 5.1 Pre-request script: read `AETERNA_E2E_PA_SIGNING_KEY` from `pm.environment.get`; mint a PA JWT (HS256, 30 min TTL, `sub: e2e-bootstrap`, `roles: [PlatformAdmin]`) using the `jsrsasign` library bundled in Postman.
- [ ] 5.2 Request `0.1 PA Mints Service Token`: `POST /api/v1/auth/tokens` with `scopes: [tenants:provision, tenants:diff, tenants:render, tenants:read, tenants:watch, connections:manage]`, `expiresIn: 1800`. Store `serviceToken` in collection vars.
- [ ] 5.3 Request `0.2 Create Fake-GitHub Connection`: `POST /api/v1/admin/git-provider-connections` with the test GH App credentials (or wiremock URL when `E2E_GITHUB_MODE=mock`). Store `connectionId`.
- [ ] 5.4 Request `0.3 Compute Tenant Slug`: pre-script computes `pm.collectionVariables.set("tenantSlug", "e2e-" + pm.environment.get("GITHUB_RUN_ID") + "-" + pm.environment.get("GITHUB_RUN_ATTEMPT") || "local")`. No HTTP call.

### 6. Folder `1. Topology Conformance` (4 requests)

- [ ] 6.1 `1.1 TLS Cert Chain`: `GET /health` with strict TLS validation against the cluster's self-signed CA (loaded from a workflow artifact). Asserts cert SAN matches ingress host.
- [ ] 6.2 `1.2 HTTP/2 Negotiation`: assert `pm.response.headers.get("alt-svc")` advertises h2 OR connection used h2 (Newman exposes the protocol via `pm.response.code` is insufficient; use the `--insecure` flag's protocol log).
- [ ] 6.3 `1.3 Health/Version/Metrics Shape`: schema-validate the JSON of `/health`, `/version`, `/metrics` (Prometheus exposition format).
- [ ] 6.4 `1.4 Gzip Negotiation`: `Accept-Encoding: gzip` returns `Content-Encoding: gzip`; absent header returns plain.

### 7. Folder `2. Manifest API Lifecycle` (8 requests)

- [ ] 7.1 `2.1 Render Empty State`: `GET /admin/tenants/{{tenantSlug}}/manifest` → 404.
- [ ] 7.2 `2.2 Apply Initial Manifest`: `POST /admin/tenants/provision` with a fixture manifest (`e2e/fixtures/manifests/baseline.json`). Asserts 200, captures `manifestHash` and `generation`.
- [ ] 7.3 `2.3 Render After Apply`: `GET .../manifest` → 200; `manifestHash` matches 2.2 response.
- [ ] 7.4 `2.4 Diff With No Changes`: `POST .../diff` with the same manifest → empty diff.
- [ ] 7.5 `2.5 Edit + Diff`: mutate `providers.llm.model`, `POST .../diff` → diff shows the one change, no others.
- [ ] 7.6 `2.6 Apply Edit`: `POST .../provision` with edited manifest → 200, `generation` incremented by 1.
- [ ] 7.7 `2.7 Generation Monotonicity Reject`: `POST .../provision` with `generation: 1` (stale) → 409 conflict.
- [ ] 7.8 `2.8 Hash Determinism`: re-render the manifest, recompute hash client-side, assert byte-equal to server's `manifestHash`.

### 8. Folder `3. SSE Event Stream` (full profile only — 3 requests)

- [ ] 8.1 `3.1 Connection Establishes`: `GET .../events` returns 200 + `Content-Type: text/event-stream`. Newman keeps connection open via the `eventsource` polyfill (pre-request script).
- [ ] 8.2 `3.2 Provisioning Step Ordering`: trigger an apply concurrently in another folder (or via Postman `pm.sendRequest`); assert events arrive in the documented order: `provisioning_step{step:"validate"}` → `…secrets` → `…providers` → `…ready`.
- [ ] 8.3 `3.3 Lagged Frame Recovery`: simulate slow consumer (deliberate `setTimeout` in the listener); assert a `lagged` frame surfaces and the connection stays open.

### 9. Folder `4. Service Token Lifecycle` (5 requests)

- [ ] 9.1 `4.1 Token Used Successfully`: `GET /admin/tenants` with `Authorization: Bearer {{serviceToken}}` → 200.
- [ ] 9.2 `4.2 Token Revoked`: `DELETE /api/v1/auth/tokens/{{tokenId}}` → 204.
- [ ] 9.3 `4.3 Revoked Token Rejected (cache hot)`: same `GET` immediately after → 401.
- [ ] 9.4 `4.4 Cache TTL Lower Bound`: assert revocation is rejected within ≤2 s of revoke (the §10.3 cross-instance contract is "≤60 s"; we test for ≤2 s as a tighter SLO and let CI catch regressions).
- [ ] 9.5 `4.5 Token Reissue After Revoke`: PA mints a fresh token; assertions resume.

### 10. Folder `5. Scope Enforcement Matrix` (full profile only — 6 requests)

- [ ] 10.1–10.6: one negative case per scope (`tenants:read`, `tenants:render`, `tenants:diff`, `tenants:provision`, `tenants:watch`, `connections:manage`). Mint a token *missing* the scope, hit a route requiring it, assert 403 `insufficient_scope`. Loop-driven via Newman's `pm.sendRequest` to keep the collection compact.

### 11. Folder `6. Connection Grant/Revoke` (4 requests)

- [ ] 11.1 `6.1 Grant`: `POST /admin/git-provider-connections/{{connectionId}}/tenants/{{tenantSlug}}` → 200.
- [ ] 11.2 `6.2 Idempotent Re-grant`: same call → 200, no duplicate audit row (assert via `GET /admin/audit?action=connection_granted&tenant=<slug>` count is exactly 1).
- [ ] 11.3 `6.3 Revoke`: `DELETE …` → 204.
- [ ] 11.4 `6.4 Revoke Nonexistent`: `DELETE …/tenants/does-not-exist` → 404.

### 12. Folder `7. Backup Round-Trip Smoke` (full profile only — 3 requests)

- [ ] 12.1 `7.1 Export`: `POST /admin/backup/export` with `targetSlug={{tenantSlug}}` → 202 + jobId.
- [ ] 12.2 `7.2 Poll To Completion`: poll `/admin/backup/jobs/{{jobId}}` until `status: succeeded` or 60 s timeout.
- [ ] 12.3 `7.3 Re-import Smoke`: `POST /admin/backup/import` with the export blob, `mode: dry-run`. Asserts the manifest hash matches the original (round-trip integrity). No real re-import — exhaustive backup correctness lives in cargo.

### 13. Folder `Z. Teardown` (5 requests)

- [ ] 13.1 `Z.1 Revoke Service Token`: `DELETE /api/v1/auth/tokens/{{tokenId}}` (idempotent — 204 or 404 both pass).
- [ ] 13.2 `Z.2 Delete Connection`: `DELETE /admin/git-provider-connections/{{connectionId}}` (idempotent).
- [ ] 13.3 `Z.3 Purge Tenant`: `POST /admin/tenants/{{tenantSlug}}/purge` → 200.
- [ ] 13.4 `Z.4 Verify Tenant Gone`: `GET /admin/tenants/{{tenantSlug}}` → 404.
- [ ] 13.5 `Z.5 Verify No Residue`: `GET /admin/diagnostics/tenant-residue?slug={{tenantSlug}}` → `residual: false` (depends on §2 endpoint).

---

## Phase E5 — Runner script + local-dev

### 14. `e2e/run-e2e.sh` rewrite

- [ ] 14.1 New flags: `--profile {fast|full}`, `--local`, `--bail {true|false}`, `--keep-cluster`.
- [ ] 14.2 `bash trap 'run_teardown' EXIT INT TERM` set before bootstrap.
- [ ] 14.3 `--local` path: kind boot + helm install + run with `E2E_GITHUB_MODE=mock`.
- [ ] 14.4 Non-local path: refuse to run unless `AETERNA_E2E_PA_SIGNING_KEY` is set; assume cluster already up (CI brings it up in the workflow `setup` step).
- [ ] 14.5 Failure artifact dump: `kubectl get all -A`, helm history, recent events, aeterna pod logs.

### 15. Wiremock fixture for local GitHub

- [ ] 15.1 `e2e/wiremock/github-app/` directory with mappings for the OAuth + App-token endpoints we hit.
- [ ] 15.2 `e2e/wiremock/docker-compose.yml` to run wiremock alongside kind on port 9999.
- [ ] 15.3 Drift-check workflow `.github/workflows/wiremock-drift-check.yml` (nightly): runs the suite in both `mock` and `real` modes against the same fixture cluster, diffs response bodies (excluding timestamps + nonces), fails on diff.

### 16. Make target

- [ ] 16.1 `make e2e-local`: invokes `bash e2e/run-e2e.sh --local --profile fast`.
- [ ] 16.2 `make e2e-local-full`: same with `--profile full`.

---

## Phase E7 — Portability & downstream redistribution (per §D13)

> Most of E7 lands **alongside** E3/E4/E5 rather than after them: the
> configurability shape is what E3/E4/E5 are built against, not a
> retrofit. Listed as its own phase for review-clarity, not for ordering.

### 19. Configuration surface — env vars + `e2e.config.yaml`

- [ ] 19.1 New file `e2e/config.lib.sh`: bash library that resolves every `AETERNA_E2E_*` var per §D13's defaults table. Order of precedence: env > `e2e.config.yaml` > documented defaults. Sourced by `run-e2e.sh` first thing.
- [ ] 19.2 New file `e2e/config.schema.json`: JSON Schema for `e2e.config.yaml`. Used by 19.3 and to validate consumer config files.
- [ ] 19.3 `run-e2e.sh --validate-config`: load config, print resolved values (with secrets redacted), exit 0 if valid + 1 if missing required vars. Documented as the diagnostic for downstream consumers.
- [ ] 19.4 Strict-mode flag `--strict-env`: rejects unknown `AETERNA_E2E_*` env vars (catches typos like `AETERNA_E2E_BASE_RUL`).
- [ ] 19.5 Env-var sanity test: every `AETERNA_E2E_*` referenced in the Newman collection must appear in the §D13 table. Lint script in `e2e/scripts/lint-config-vars.sh`, runs in CI.

### 20. Secrets-backend adapters

- [ ] 20.1 `e2e/secrets/env.sh`: trivial passthrough (default backend).
- [ ] 20.2 `e2e/secrets/op.sh`: 1Password CLI adapter. Maps logical name → `op://vault/item/field`; the mapping itself comes from `e2e/secrets/op.map` (consumer-customizable, not committed by `kikokikok/aeterna`).
- [ ] 20.3 `e2e/secrets/aws-sm.sh`: AWS Secrets Manager adapter. Logical name → `--secret-id` mapping in `e2e/secrets/aws-sm.map`.
- [ ] 20.4 `e2e/secrets/vault.sh`: HashiCorp Vault adapter. Logical name → `kv get` path in `e2e/secrets/vault.map`.
- [ ] 20.5 Adapter contract test: each adapter must accept `resolve <logical-name>` and emit value to stdout, errors to stderr, exit non-zero on miss. Test fixture exercises the contract for all four backends with mocked CLI binaries.
- [ ] 20.6 Secrets-leak guard test: run the suite end-to-end, grep all artifacts (newman report, pod logs, kubectl output) for the literal value of `AETERNA_E2E_PA_SIGNING_KEY`; fail if found. CI-enforced.

### 21. Cluster-mode dials

- [ ] 21.1 `run-e2e.sh` dispatches on `AETERNA_E2E_CLUSTER_MODE`:
  - `kind-bootstrap` → existing E3 path
  - `existing-kubeconfig` → skip kind/helm; verify `kubectl get ns aeterna` succeeds; set `AETERNA_E2E_BASE_URL` from ingress lookup if not already set
  - `external-https` → skip kubectl entirely; trust `AETERNA_E2E_BASE_URL`; failure-artifact path skips cluster-state dump
- [ ] 21.2 `existing-kubeconfig` mode adds a precondition check: tenant slug from `AETERNA_E2E_TENANT_SLUG_PREFIX${SUFFIX}` must not already exist (refuse to clobber).
- [ ] 21.3 `external-https` mode adds a self-test: `GET /version` must return a JSON shape with `apiVersion >= <min-supported>`; else fail with a clear "this aeterna deployment is too old for this conformance suite version" error.
- [ ] 21.4 Document each mode's failure-artifact path in §17.1.

### 22. CI templates + reusable workflow

- [ ] 22.1 `e2e/templates/ci/github-actions.yml`: a *generic* template (≠ the canonical `.github/workflows/e2e-conformance.yml`); ≈30 lines; populates env vars + invokes `run-e2e.sh`. Documented as the copy-paste path for consumers using vanilla GH Actions.
- [ ] 22.2 `.github/workflows/e2e-conformance-reusable.yml`: reusable workflow exposing the suite via `workflow_call`. Inputs: `cluster-mode`, `profile`, `base-url`, `secrets-backend`, `github-mode`. Secrets: `pa-signing-key`, `gh-app-key`. Consumers `uses: kikokikok/aeterna/.github/workflows/e2e-conformance-reusable.yml@<sha>`.
- [ ] 22.3 `e2e/templates/ci/gitlab-ci.yml`: GitLab CI template; mirrors GH Actions shape; uses GitLab's `variables:` and `secrets:` syntax.
- [ ] 22.4 `e2e/templates/ci/Makefile.snippet`: portable Make target for any-CI consumers.
- [ ] 22.5 README inside `e2e/templates/ci/` explaining when to use which.

### 23. Vendor smoke job (proves the consumer path stays green)

- [ ] 23.1 New CI job `e2e-conformance-vendor-smoke` (in `kikokikok/aeterna`'s CI, runs only on changes to `e2e/**` or `helm/**`): copies `e2e/` into a tmp directory simulating a downstream repo, populates the four required env vars from the canonical GH Actions secrets, runs `run-e2e.sh --profile fast` against the same kind cluster the regular `fast` job uses, asserts green.
- [ ] 23.2 Vendor smoke job verifies: no path outside `e2e/` is read (FS access auditing via `strace -f -e trace=open,openat | grep -v '^e2e/'` filtered against an allowlist).
- [ ] 23.3 If 23.2 fires, fail the build with the offending paths listed.

### 24. Versioning + downstream-pinning hygiene

- [ ] 24.1 `e2e/VERSION`: a single line `<aeterna-version>+e2e.<n>` (e.g. `1.0.0+e2e.1`). Bumped by hand on any breaking change to the suite contract.
- [ ] 24.2 `e2e/COMPATIBILITY.md`: tracks which `aeterna` versions a given `e2e` version supports. Useful for vendoring consumers stuck on an older aeterna release.
- [ ] 24.3 Suite refuses to run if `GET /version` returns a value not in `e2e/COMPATIBILITY.md`'s supported list (skippable via `AETERNA_E2E_SKIP_COMPAT_CHECK=true` for emergencies).

---

## Phase E6 — Documentation + handoff

### 17. Docs

- [ ] 17.1 `docs/guides/e2e-conformance.md` (new): bootstrap flow, secrets model, local-dev path with wiremock, how to debug a red CI run from artifacts, how to add a folder. **Plus** a "Running this suite from a downstream repo" section covering the four ways (vendor / reusable workflow / templates / Make snippet), each cluster mode, each secrets backend.
- [ ] 17.2 `docs/api/diagnostics.md` (new): `/admin/diagnostics/tenant-residue` endpoint contract.
- [ ] 17.3 Update `docs/DEVELOPER_GUIDE.md` to point at §17.1 from the testing section.
- [ ] 17.4 Mirror new docs into `website/docs/` and update `website/sidebars.ts`.
- [ ] 17.5 `e2e/README.md` (new): the consumer-facing entry point. Top of file: "If you want to run this suite against your aeterna deployment, you are in the right place. Read this file." Then quickstart → mode dials → secrets backends → troubleshooting.
- [ ] 17.6 `e2e/CONFIG.md` (new): the §D13 variables table, kept in sync with `e2e/config.lib.sh` defaults. CI lint enforces sync (one variable in code = one row in the doc).

### 18. Cleanup of prior doc plan

- [ ] 18.1 Land the stale-routes fix from the prior audit (`docs/security/tenant-provisioning-security.md` lines 64-91) — separately tracked, but listed here so it doesn't get lost.

---

## Acceptance criteria

- [ ] AC1 A clean checkout of master + `make e2e-local` (with `AETERNA_E2E_PA_SIGNING_KEY` set or the wiremock fixture) completes in ≤15 min and leaves zero residue.
- [ ] AC2 `e2e-conformance / fast` is a required CI check on every PR touching the path-filter set.
- [ ] AC3 The deleted-and-rewritten suite has zero false-positive flakes across 20 consecutive nightly runs (acceptance gate before declaring v1 stable).
- [ ] AC4 Total Newman folder count = 9 (`0, 1, 2, 3, 4, 5, 6, 7, Z`); total request count ≤ 60.
- [ ] AC5 Teardown leak-assertion (§D11 endpoint) returns `residual: false` after every successful run.
- [ ] AC6 **Downstream-vendor path is green:** an empty repo with only `e2e/` vendored + a 5-line CI snippet from `e2e/templates/ci/github-actions.yml` + the four required env vars (`AETERNA_E2E_BASE_URL`, `AETERNA_E2E_PA_SIGNING_KEY`, `AETERNA_E2E_GITHUB_APP_ID`, `AETERNA_E2E_GITHUB_APP_KEY`) produces a green run against a conforming aeterna deployment. Enforced by the §23 vendor-smoke CI job.
- [ ] AC7 **Configurability is honest:** every `AETERNA_E2E_*` var referenced anywhere in `e2e/` is documented in `e2e/CONFIG.md`; every var documented in `e2e/CONFIG.md` is referenced somewhere. Lint check in §19.5 enforces both directions.
- [ ] AC8 **No secret leaks:** §20.6 grep-the-artifacts test passes on every CI run.
