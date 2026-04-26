## Why

The end-to-end Newman suite at `e2e/aeterna-e2e.postman_collection.json` is, in 2026-04, **vestigial theatre**:

1. **It is not in CI.** Zero `.github/workflows/*` reference `newman` or `run-e2e.sh`. Nothing on a PR runs it; nothing on master runs it. The latest results in `e2e/results/` are from 2026-04-15 — eleven days before the §10 / §12 / §13 work landed and never re-validated against it.

2. **It is not self-bootstrapping.** Folders 13–40 require a human to run the GitHub OAuth device-code flow, paste a token into env, then re-run. `run-e2e.sh:11-30` documents this as a manual prerequisite. No CI runner can clear that gate without operator hand-holding.

3. **It is not self-cleaning.** Folder #37 "Tenant Purge" has a single request — a 404 negative on a fake slug. The tenant created by #29.1 is never deleted. Every run leaks state into the target cluster.

4. **It does not test the PR's code.** It tests whatever version of `aeterna` is running on the cluster `baseUrl` points at — usually a stale dev deploy. A PR that breaks `/admin/tenants/provision` would not be caught.

5. **70% of its 293 requests duplicate coverage** that `cargo test --all` (≈6,000 tests) and the §13 consistency suite already exercise in-process. The duplication adds runtime, not signal.

Meanwhile, the **deployment-topology layer** — does `aeterna` running behind a real nginx ingress with TLS, talking to a real Dragonfly + a real CNPG postgres + a real Cedar Agent + a real OPAL Server, behave correctly end-to-end? — is the **only** layer that nothing else covers, and it is exactly the layer the current suite *could* be useful for but isn't.

The result is a 600 KB stale artefact that no one runs, and a real gap (deployment conformance) that nothing fills.

## What Changes

This change **deletes the existing suite as a feature-coverage layer** and **rebuilds it as a conformance suite** with a single, narrow value proposition: *assert that a fresh `helm install aeterna` on a kind cluster, given a single bootstrap secret, can be driven through the full lifecycle of a real tenant by an unauthenticated client and ends with zero leaked state.*

- **Self-bootstrapping**: a new folder `0. Bootstrap` mints its own platform-admin JWT (from a signing key supplied via env), then mints a scoped service token, then creates a fake-github connection. No human OAuth dance. No pre-shared cookie.
- **Self-cleaning**: a new folder `Z. Teardown` revokes the service token, deletes the connection, purges the tenant, and asserts via API that no audit/secret/connection rows remain. Runs unconditionally via a `bash trap` so failed runs still clean up.
- **Tests the PR's code**: a new GitHub Actions workflow `.github/workflows/e2e-conformance.yml` builds the PR's commit into a Docker image, loads it into a fresh kind cluster, helm-installs the PR's chart, waits for `/health`, then runs the suite. PRs that break the deployment topology fail merge.
- **Compressed coverage**: 40 folders → ~8 folders of unique-value tests. Anything that duplicates cargo/Playwright/§13 coverage is deleted, not migrated.
- **Two-tier execution**: a `fast` profile (≤15 min, runs on every PR) covers core lifecycle; a `full` profile (≤30 min, runs on master push or PR label `e2e:full`) adds SSE-timing, scope-rejection matrix, backup round-trip, and one chaos check (pod restart mid-provision).
- **Honest secrets model**: two GitHub Actions secrets (`AETERNA_E2E_PA_SIGNING_KEY`, `AETERNA_E2E_GITHUB_APP_KEY`) plus one var (`AETERNA_E2E_GITHUB_APP_ID`). No `.env` files committed. Local-dev path uses a wiremock-backed GitHub fallback so contributors can run the suite without secret access.

The CLI runner script `run-e2e.sh` and the Postman collection format are kept — the bottleneck was never the tool. The collection is rewritten from scratch (no migration of the old folders).

## Capabilities

### Modified Capabilities

- `tenant-provisioning`: gains a deployment-conformance assertion. Documented contract: a freshly helm-installed cluster + one bootstrap secret = a fully exercised + torn-down tenant lifecycle in ≤15 min.

### New Capabilities

- `e2e-conformance-suite`: the reframed Newman suite as a CI-gated, self-bootstrapping, self-cleaning conformance layer. Owns: ingress/TLS verification, manifest API wire format over real HTTP/2, SSE event ordering over a real connection, scope enforcement, service-token lifecycle (mint → use → revoke → 401-within-60s contract from §10.3), connection grant/revoke audit parity, backup round-trip smoke, teardown completeness assertions.

## Impact

- **Net deletion**: ≈600 KB of stale Postman JSON + the `.bak` file + `fix_collection.py` go away. New collection target ≈80 KB.
- **CI runtime added**: ≈12–15 min on every PR (kind boot ~3 min, helm install ~2 min, Newman run ~6 min, teardown ~2 min). Acceptable: PRs that don't touch `cli/`, `helm/`, `mk_core/`, `memory/`, `admin-ui/`, or `e2e/` skip the workflow via path filter.
- **One-time secret rotation**: a test-only GitHub App must be created in a `kikokikok-test` org and its private key uploaded to GitHub Actions secrets. Cost: ≈30 min of operator time, zero recurring.
- **Documentation**: `docs/guides/e2e-conformance.md` (new) explains the bootstrap flow, the secrets model, how to run locally with wiremock, and how to debug a failing CI run from artifacts.
- **Risk**: the workflow can become a flake source if the deployment-topology layer is genuinely flaky. Mitigation: explicit timeout budgets per phase, retry-once on the bootstrap phase only (never on assertions), cluster-state artifact upload on failure.
- **Out of scope (explicit non-goals)**: Okta SSO round-trip (deferred — needs a separate test tenant in Okta), browser-driven UI flows (Playwright already owns those), feature-coverage parity with the deleted folders (cargo/Playwright/§13 own that).
