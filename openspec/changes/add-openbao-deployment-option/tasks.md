# Tasks: Add OpenBao deployment option to `aeterna-prereqs`

## Implementation status (updated 2026-04-27)

**Phase 1 (this commit) — DELIVERED:**
- ✅ Subchart wiring (Chart.yaml dep on openbao `~0.27.0`, Chart.lock updated)
- ✅ `openbao:` values block with 3-mode contract (`internal-dev-seal` | `internal-shamir` | `external`)
- ✅ Dev-seal guardrail helper (`prereqs.openbao.assertSealMode`) — fails render with actionable message on invalid mode or mode/subchart-values mismatch
- ✅ Bootstrap Job (Helm post-install/post-upgrade hook) — initializes OpenBao per mode, enables kv-v2 at `secret/`, persists `root_token` (+ `unseal_key` for shamir) into a namespace-local Secret
- ✅ Scoped RBAC (Role + RoleBinding to a single bootstrap Secret + pod read for readiness; no cluster-scoped permissions)
- ✅ NOTES.txt warning block when dev-seal mode is active
- ✅ `values-e2e.yaml` overlay for the e2e suite (PR #169)
- ✅ `helm lint` clean; `helm template` covers all 3 modes; bash syntax check on rendered bootstrap.sh

**Phase 2 (deferred to a follow-up change) — NOT in this PR:**
- ⏳ Kubernetes auth method enable/configure on OpenBao
- ⏳ `aeterna-pod` policy + role binding (SA → policy)
- ⏳ Marker secret `secret/aeterna/.bootstrap-marker` for chart version self-test
- ⏳ `helm test` smoke pod for bootstrap idempotency
- ⏳ `charts/aeterna` `vault:` block + deployment env injection (consumer-side wiring)

The Phase 2 scope is intentionally scoped out: it is "aeterna authenticates with bao",
which is logically separate from "bao is installable as a prereq". Phase 1 alone
unblocks PR #169 (which only needs OpenBao to exist with a KV mount and a known
root token in dev-seal mode).

## 0. Resolve design conflicts (blocking)

- [x] 0.1 Confirm OpenBao over HashiCorp Vault (per design.md §D1). Operator action: none required if confirmed; this is a license/governance choice, not a technical lock-in.
- [x] 0.2 Confirm subchart-in-prereqs (vs main-chart-dep, vs standalone chart) per §D2.
- [x] 0.3 Confirm default-off (per §D3) — backwards-compat for every existing consumer of `aeterna-prereqs`.
- [x] 0.4 Confirm k8s-SA-only auth for v1 (per §D5) — punts AppRole/JWT/OIDC to follow-ups. *(Phase 2)*
- [x] 0.5 Confirm three-mode seal contract with the dev-seal guardrail (per §D6) — production-safety lock-in.
- [x] 0.6 Confirm migration tooling is out of scope for v1 (per §D13).

## Phase B1 — Subchart wiring

### 1. `charts/aeterna-prereqs` dependency declaration

- [x] 1.1 Add OpenBao to `charts/aeterna-prereqs/Chart.yaml` `dependencies:` block. Pinned to `~0.27.0` (resolved to `0.27.2`, appVersion `v2.5.3`).
- [x] 1.2 `helm dependency update charts/aeterna-prereqs` — `Chart.lock` and `charts/openbao-0.27.2.tgz` produced.
- [ ] 1.3 Bump `charts/aeterna-prereqs/Chart.yaml` `version` (deferred — will bump together with merge of this change).

### 2. `charts/aeterna-prereqs/values.yaml` — bao section

- [x] 2.1 Added top-level `openbao:` block. Shape **revised vs original tasks.md**: simpler 3-mode contract (`mode: internal-dev-seal | internal-shamir | external`) instead of separate `seal.mode` + `allowDevSeal` flags. Bootstrap config: `enabled`, `image`, `secretName`, `kvPath`, `hookWeight`, `serviceAccount`, `resources`, `securityContext`. Subchart passthrough under `openbao.global/injector/csi/server`.
- [x] 2.2 Each block has inline `##` doc comments explaining the contract.
- [x] 2.3 Section header added with mode descriptions and links to upstream chart values.

### 3. Dev-seal guardrail

- [x] 3.1 In `charts/aeterna-prereqs/templates/_helpers.tpl`, added `prereqs.openbao.assertSealMode` helper. Fails render if `mode` is not in the allowed set, or if mode/subchart values are inconsistent (e.g. `mode=internal-shamir` with `server.dev.enabled=true`).
- [x] 3.2 N/A under revised contract (auto-unseal config validation is the operator's responsibility under `mode=external` — they bring the seal stanza). Documented in NOTES.
- [x] 3.3 Helper is called from `openbao-bootstrap-rbac.yaml` (the first hook to render), so `helm template/install` fails before any resource is created.
- [x] 3.4 `templates/NOTES.txt` emits a large multi-line warning block when `mode=internal-dev-seal` (including "DO NOT USE FOR PRODUCTION") and a smaller warning for `mode=internal-shamir`.

## Phase B2 — Bootstrap Job

### 4. Job manifest

- [x] 4.1 Created `charts/aeterna-prereqs/templates/openbao-bootstrap-job.yaml` with hooks `helm.sh/hook: post-install,post-upgrade`, configurable `hook-weight` (default `"5"`), `hook-delete-policy: before-hook-creation,hook-succeeded`.
- [x] 4.2 Job: `restartPolicy: OnFailure`, `backoffLimit: 3`, `ttlSecondsAfterFinished: 600`. SA `<release>-openbao-bootstrap` with namespace-scoped Role limited to (a) get/update/patch on the single bootstrap Secret by `resourceNames`, (b) create on Secrets in the namespace, (c) get/list/watch on Pods (for readiness). NO cluster-scoped permissions.
- [x] 4.3 Two-container pattern (because `openbao` image lacks `kubectl` and `bitnami/kubectl` lacks `bao`): initContainer copies `/bin/bao` from `openbao/openbao:2.5.3` into a shared emptyDir; main container `bitnami/kubectl:1.30` runs the script with `/shared` prepended to `PATH`. All security context: `runAsNonRoot`, `readOnlyRootFilesystem`, drops ALL caps, seccomp `RuntimeDefault`.
- [x] 4.4 Job env: `MODE`, `BAO_ADDR=http://<release>-openbao.<ns>.svc:8200`, `KV_PATH`, `SECRET_NAME`, `NAMESPACE`, plus `DEV_ROOT_TOKEN` only when `mode=internal-dev-seal`. The Job authenticates to OpenBao using the dev root token (dev-seal) or freshly-minted root token from `sys/init` (shamir/external).

### 5. Bootstrap script (revised scope — Phase 1 simpler than original §5)

- [x] 5.1 Inline ConfigMap `<release>-openbao-bootstrap-script` containing `bootstrap.sh`. Steps actually implemented (Phase 1):
  1. Wait for OpenBao `/v1/sys/health` to return 200/429 (60×2s timeout)
  2. Read `/v1/sys/init` to determine current state
  3. Per mode: dev-seal uses pre-set root token; shamir initializes 1/1 + unseals; external initializes with recovery 1/1 (assumes auto-unseal is configured by operator)
  4. Idempotently enable kv-v2 at `$KV_PATH/`
  5. `kubectl apply` the bootstrap Secret with `bao_addr`, `root_token`, `kv_path`, `mode` (+ `unseal_key` for shamir)
- [ ] 5.2–5.4 **Deferred to Phase 2** (k8s-auth enable, `aeterna-pod` policy, role binding, marker secret) — see top-of-file status block.
- [x] 5.b1 `set -euo pipefail`. No `set -x`. Token-bearing curl bodies are `>/dev/null`.
- [x] 5.b2 Each mutating step preceded by a state check (mounts list, `sys/init.initialized`, `sys/seal-status.sealed`).

### 5. Bootstrap script

- [ ] 5.1 Inline ConfigMap or scripts/ subdir: `bootstrap.sh`. Steps per design.md §D8:
  1. Wait for bao ready (`bao status` polling, 60s timeout)
  2. Enable KV v2 at `{{ .Values.openbao.bootstrap.kvMount }}/` (idempotent)
  3. Enable k8s auth at `auth/kubernetes/` (idempotent)
  4. Configure k8s auth with cluster CA + token-reviewer JWT
  5. Write `aeterna-pod` policy
  6. Create role `aeterna` binding policy to ServiceAccount `aeterna` in namespace `{{ .Release.Namespace }}` (or override via `openbao.bootstrap.aeternaServiceAccountNamespace`)
  7. Verification: issue a `bao login` via the aeterna SA token (TokenRequest API) and assert the resulting policy includes `aeterna-pod`
- [ ] 5.2 Every step uses `set -euo pipefail`. No `set -x`. All bao output that could contain token material is `>/dev/null`.
- [ ] 5.3 Each mutating step preceded by a state-check: e.g. `bao secrets list -format=json | jq -e '.["secret/"]' >/dev/null && echo "kv already enabled" || bao secrets enable -path=secret -version=2 kv`.
- [ ] 5.4 Final step writes a marker secret `secret/aeterna/.bootstrap-marker` with the chart version + timestamp; readable by aeterna-pod policy. Used by aeterna's startup self-test (Phase B3 §8.3).

### 6. Bootstrap-token Secret

- [ ] 6.1 The OpenBao chart auto-generates a root token in dev mode and writes it to `<release>-openbao-init` Secret in production. Bootstrap Job reads it via projected volume; never logged; never persisted in any aeterna-managed Secret.
- [ ] 6.2 In production seal modes, the bootstrap Job is configured with a service-account-token-projected-volume that calls bao's auto-unseal-init flow exposed by the upstream chart. Verify upstream chart's `server.bootstrap.enabled` semantics; align with our Job rather than duplicate.

### 7. Job idempotency + smoke test

- [ ] 7.1 `helm test charts/aeterna-prereqs` — add a test pod that re-runs `bootstrap.sh`'s state-check assertions and confirms all expected resources exist + roles bind correctly. Test pod uses the same image as the Job; takes <30s.
- [ ] 7.2 Validate `helm upgrade` runs the Job a second time and exits 0 with no state changes.

## Phase B3 — Aeterna integration

### 8. `charts/aeterna/values.yaml` — vault section

- [ ] 8.1 Add top-level `vault:` block: `enabled: false`, `address: ""` (defaults to prereqs-bao DNS when blank and `enabled=true`), `kubernetesAuthPath: "auth/kubernetes"`, `kubernetesAuthRole: "aeterna"`.
- [ ] 8.2 In `charts/aeterna/templates/deployment.yaml`, when `vault.enabled=true`, inject env vars `VAULT_ADDR`, `VAULT_K8S_AUTH_PATH`, `VAULT_K8S_AUTH_ROLE`. When `vault.address` is empty, default to `http://{{ .Release.Name }}-prereqs-openbao:8200` and emit a `helm.sh/notes` line saying we're assuming the prereqs chart was installed alongside.
- [ ] 8.3 Aeterna server startup self-test (existing health check or new): if `VAULT_ADDR` set, attempt to read `secret/aeterna/.bootstrap-marker` and log the marker's chart-version field at INFO. Failure = WARN, not fatal (vault might be intentional-but-not-yet-bootstrapped during upgrade).

### 9. ServiceAccount alignment

- [ ] 9.1 Verify `charts/aeterna/templates/serviceaccount.yaml` creates SA named `{{ include "aeterna.serviceAccountName" . }}` (likely already does); document the name in `vault.kubernetesAuthSubject`.
- [ ] 9.2 If the aeterna SA name doesn't match what the bootstrap Job binds to, surface an explicit values knob `vault.aeternaServiceAccount: ""` (defaults to chart's SA name).

## Phase B4 — e2e suite integration (cross-PR with #169)

### 10. Values overlay for kind-bootstrap mode

- [ ] 10.1 New file `charts/aeterna-prereqs/values-e2e.yaml`: enables bao in dev seal mode with `allowDevSeal=true`; pinned single-replica; minimal resources.
- [ ] 10.2 New file `charts/aeterna/values-e2e.yaml` (or extend existing): sets `vault.enabled=true`, leaves `vault.address` blank to use the default DNS.

### 11. Runner script integration (depends on #169 task 20.4 + 21.1)

- [ ] 11.1 Coordinate with PR #169: add `AETERNA_E2E_VAULT_ADDR` to the §D13 env-var table (default `http://aeterna-prereqs-openbao.aeterna-e2e.svc.cluster.local:8200` in kind mode).
- [ ] 11.2 Update `e2e/secrets/vault.sh` (introduced by #169 task 20.4) to read `AETERNA_E2E_VAULT_ADDR`; no other change needed since OpenBao is wire-compatible.
- [ ] 11.3 Extend `run-e2e.sh`'s `kind-bootstrap` path (#169 task 21.1) to install the prereqs chart with `--set openbao.enabled=true ...` when `AETERNA_E2E_SECRETS_BACKEND=vault`.
- [ ] 11.4 Newman folder addition: a single test in folder `0` (preflight) that asserts `bao status` returns ready when the test profile uses `vault` backend. Skipped otherwise.

## Phase B5 — Production runbooks (docs)

### 12. Auto-unseal per cloud

- [ ] 12.1 `docs/runbooks/openbao/auto-unseal-aws-kms.md`: IAM role config, KMS key policy, `awsKms` values block example, recovery procedure.
- [ ] 12.2 `docs/runbooks/openbao/auto-unseal-gcp-kms.md`: equivalent for GCP.
- [ ] 12.3 `docs/runbooks/openbao/auto-unseal-azure-keyvault.md`: equivalent for Azure.
- [ ] 12.4 `docs/runbooks/openbao/auto-unseal-k8s-sealwrap.md`: for clusters without cloud KMS access (single-cloud-region failover scenario).

### 13. Migration runbook (operator guide, no code)

- [ ] 13.1 `docs/runbooks/secrets-migration-postgres-to-vault.md`: step-by-step per design.md §D13. Includes inventory query, migration script template (operator-supplied; we provide the SQL + the bao CLI commands), verification steps, rollback.
- [ ] 13.2 Migration runbook explicitly states: no aeterna-shipped tooling; this is a v2 follow-up; current scope is ops-only.

### 14. Disaster recovery

- [ ] 14.1 `docs/runbooks/openbao/disaster-recovery.md`: snapshot mechanics (manual `bao operator raft snapshot save` for v1), restore procedure, RPO/RTO statements.
- [ ] 14.2 Doc explicitly notes: snapshot CronJob is v2 follow-up.

### 15. Upgrade notes

- [ ] 15.1 `docs/runbooks/openbao/upgrades.md`: minor-version upgrade procedure (Helm + chart minor bump), seal-aware upgrade ordering, single-node-vs-HA differences.
- [ ] 15.2 `OPENBAO_VERSION_COMPAT.md` at repo root: `aeterna-prereqs` chart version → OpenBao version compatibility matrix.

## Phase B6 — CI

### 16. Chart linting + render tests

- [ ] 16.1 Add OpenBao-related cases to existing `helm-lint.yml` workflow: render with `openbao.enabled=true` + each seal mode; assert dev-seal guardrail fires correctly; assert auto-unseal-without-provider fails.
- [ ] 16.2 `helm template` matrix: `[disabled, dev+allowDevSeal, manual, auto-unseal+awsKms-stub, auto-unseal+gcpKms-stub]`. All five must render successfully (or fail loudly, for the negative cases).

### 17. Functional kind-cluster test

- [ ] 17.1 New CI job `openbao-smoke` (or fold into existing prereqs-smoke if one exists): kind cluster, install prereqs with `openbao.enabled=true allowDevSeal=true seal.mode=dev`, install aeterna with `vault.enabled=true`, assert pod becomes ready, assert it can read `secret/aeterna/.bootstrap-marker`.
- [ ] 17.2 Job logs scanned for the literal strings `hvs.` and `root` token prefixes (per design.md §D8 secrets-leak posture); fails if either appears outside expected log lines.

### 18. Coordination with #169

- [ ] 18.1 Once both PRs are ready, land this PR first (introduces the values overlay #169 references).
- [ ] 18.2 Update #169 task 20.4 to remove the "TODO: needs deployable bao" caveat.

## Acceptance criteria

- [ ] AC1 `helm install prereqs charts/aeterna-prereqs --set openbao.enabled=true --set openbao.allowDevSeal=true --set openbao.seal.mode=dev` → all pods ready in under 90s on a 2-node kind cluster.
- [ ] AC2 The bootstrap-marker secret is readable via `bao kv get secret/aeterna/.bootstrap-marker` after install completes.
- [ ] AC3 An aeterna pod with `vault.enabled=true` resolves a `SecretReference::Vault { mount: "secret", path: "aeterna/test/key", field: "value" }` to the value previously written via `bao kv put secret/aeterna/test/key value=hunter2`.
- [ ] AC4 `helm install` with `seal.mode=dev` and **no** `allowDevSeal=true` **fails** at template render time with a clear error message naming the guardrail.
- [ ] AC5 `helm install` with `seal.mode=auto-unseal` and **no** provider block **fails** at template render time with a clear error message listing the four valid providers.
- [ ] AC6 `helm upgrade` (no values changes) re-runs the bootstrap Job and exits 0 with no observable state changes (idempotency).
- [ ] AC7 e2e conformance suite (PR #169) passes with `AETERNA_E2E_SECRETS_BACKEND=vault` against the deployed bao in `kind-bootstrap` cluster mode.
- [ ] AC8 No bao token (root or otherwise) appears in any pod log, Job log, or k8s Event across the install + upgrade + helm-test cycle.
- [ ] AC9 Default behavior (`openbao.enabled=false`) on an existing `aeterna-prereqs` upgrade is byte-identical to the previous chart version's render output (verified via `helm template` diff in CI).
