# Design: Add OpenBao deployment option to `aeterna-prereqs`

## Context

`charts/aeterna-prereqs/` is the existing pattern for "bring up backing services for dev/test/CI." It already conditionally deploys PostgreSQL (CloudNativePG), Dragonfly/Valkey (cache), and Qdrant (vector store) via Helm dependency conditions. The main `charts/aeterna/` chart is intentionally dependency-free; it consumes whatever the operator has running and points at it via `VAULT_ADDR`, `DATABASE_URL`, etc.

`mk_core::SecretReference::Vault { mount, path, field }` is already a first-class variant. The resolver in `cli/src/server/tenant_api.rs:4413` reads from a Vault-API-speaking endpoint at `VAULT_ADDR`. Today there is no chart-side support for actually deploying that endpoint; consumers are on their own.

## Decisions

### D1 — OpenBao over HashiCorp Vault

**Decision:** OpenBao (Linux Foundation, Apache-2.0).

- Wire-compatible with Vault API — `SecretReference::Vault` resolver works unchanged.
- Apache-2.0 license — no BSL ambiguity for downstream consumers redistributing aeterna.
- Active LF governance + IBM, Hashibox, GitLab as anchor contributors.
- Helm chart `openbao/openbao` is well-maintained.

**Rejected:** HashiCorp Vault. License concerns for redistribution; not an issue for our own use but is one for consumers vendoring this chart into commercial products.

**Rejected:** Bitnami `vault` chart. Tracks HashiCorp Vault, same license issue + Bitnami container repackaging concerns.

### D2 — Distribution shape: subchart of `aeterna-prereqs`, not main chart

**Decision:** Add OpenBao as a conditional dependency in `charts/aeterna-prereqs/Chart.yaml`. Default-off via `openbao.enabled: false`.

```yaml
# charts/aeterna-prereqs/Chart.yaml (new dependency block)
dependencies:
  # ... existing dragonfly / qdrant / valkey ...
  - name: openbao
    version: "0.10.*"          # pinned minor in D11
    repository: https://openbao.github.io/openbao-helm
    condition: openbao.enabled
    alias: openbao
```

The main `charts/aeterna/` chart stays dependency-free. It learns to address bao via `VAULT_ADDR` like any other Vault — no special-casing.

**Rejected:** Embedding OpenBao in the main `aeterna` chart. Violates the chart's "deploys only the application" contract; couples aeterna's release lifecycle to bao's, complicates upgrades, conflicts with consumers running their own Vault.

**Rejected:** A standalone `aeterna-vault` chart. Duplicates the prereqs pattern; no consumer benefit; more charts to publish + version.

### D3 — Default off; opt-in via single flag

**Decision:** `openbao.enabled: false` is the default. Enabling = `--set openbao.enabled=true` (plus the seal-mode choice from §D6).

Rationale: every existing `aeterna-prereqs` user gets identical behavior on upgrade. Migration story is "set the flag, install, optionally migrate secrets" — never forced.

### D4 — Storage backend: integrated raft

**Decision:** OpenBao integrated raft storage, single-node by default, scalable to 3- or 5-node HA via `openbao.server.ha.replicas`.

- Zero external dependencies (no Consul, no Postgres backend).
- Raft is what the OpenBao chart's HA mode uses; we follow upstream defaults.
- PVC-backed at `openbao.server.dataStorage.size: 10Gi` default (override-able).

**Rejected:** File backend. No HA path; no scale story.
**Rejected:** Postgres backend. Couples bao availability to aeterna's main DB; defeats the point of running bao for secrets isolation.

### D5 — Auth method: Kubernetes ServiceAccount

**Decision:** Kubernetes auth method (`auth/kubernetes/`) only. Aeterna's pod authenticates to bao via its projected SA token; bao validates against the cluster's TokenReview API.

- No long-lived bao tokens stored anywhere.
- No AppRole secret-id rotation problem.
- Works identically in kind, EKS, GKE, AKS, on-prem k8s.

The aeterna pod gets `VAULT_ADDR=http://aeterna-prereqs-openbao:8200` and `VAULT_K8S_AUTH_PATH=auth/kubernetes` env vars; the existing resolver does the SA-token exchange on first read and caches the resulting bao token until expiry.

**Rejected (for v1):** AppRole, JWT/OIDC. Tracked as future enhancements for non-k8s deployments. The resolver is auth-method-agnostic in its API surface, so adding them later is mechanical.

### D6 — Seal modes: three, with guardrails

**Decision:** Surface three named seal modes via `openbao.seal.mode`:

| Mode | Use case | Production-safe? | Guardrail |
|---|---|---|---|
| `dev` | e2e + local dev only | **No** | Chart refuses install unless `openbao.allowDevSeal=true` is *also* set; templates emit a `WARNING` notes block; readiness probe annotated `aeterna.io/dev-seal=true` |
| `manual` | Shamir unseal (3-of-5 keys) | Yes (with care) | Pod blocks ready until manually unsealed via `bao operator unseal`; documented, discouraged |
| `auto-unseal` | Production | **Yes** | Requires one of: `awsKms` / `gcpKms` / `azureKeyVault` / `k8sSealWrap` config blocks; chart fails template if `mode=auto-unseal` and no provider block is set |

Auto-unseal provider blocks are documented in `values.yaml` with full examples per cloud.

The dev-seal guardrail is critical: it prevents an operator from accidentally running an in-memory bao in production and discovering on the next pod restart that all secrets are gone.

### D7 — TLS

**Decision:** Default to `http://` inside the cluster (kube-proxy + ClusterIP). Operators can flip on `openbao.server.tls.enabled=true` to terminate TLS at the bao service via cert-manager (`openbao.server.tls.certManagerIssuer`).

Rationale: in-cluster traffic between aeterna and bao is one hop on the pod network; defaulting to TLS-everywhere doubles the failure surface during install for zero security gain when the cluster has network policies. Consumers who require TLS-everywhere flip the flag.

**No public ingress for bao**, ever. Documented as a hard rule. Bao is ClusterIP-only; access from outside the cluster goes through `kubectl port-forward` or a separate consumer-managed VPN.

### D8 — Bootstrap Job

**Decision:** A single Job, deployed as a Helm `post-install,post-upgrade` hook with `before-hook-creation,hook-succeeded` delete policy.

The Job runs a small bash script that:

1. Polls bao readiness (`bao status`) until ready or 60s timeout.
2. If KV v2 not enabled at `secret/aeterna/`, enable it (`bao secrets enable -path=secret -version=2 kv` — idempotent: ignore "path is already in use").
3. If k8s auth not enabled at `auth/kubernetes/`, enable it.
4. Configure k8s auth: `bao write auth/kubernetes/config kubernetes_host=https://kubernetes.default.svc kubernetes_ca_cert=@/var/run/secrets/kubernetes.io/serviceaccount/ca.crt token_reviewer_jwt=@/var/run/secrets/kubernetes.io/serviceaccount/token`.
5. Write policy `aeterna-pod`:
   ```hcl
   path "secret/data/aeterna/*" { capabilities = ["read"] }
   path "secret/metadata/aeterna/*" { capabilities = ["list", "read"] }
   ```
6. Create role `aeterna` binding the policy to the aeterna ServiceAccount in the aeterna namespace.

**Idempotency:** every step checks current state before mutating. Re-running the Job (e.g. after `helm upgrade`) is a no-op when state matches.

**Secrets-leak posture:**
- Job runs with no env vars containing token material (it uses the k8s SA token mounted at the standard path).
- Script uses `set -o pipefail` and `set +x` throughout — no command tracing.
- Job logs are scrubbed: bao token output is `>/dev/null` redirected; only step names log to stdout.
- Job's RBAC is the minimum: `serviceaccounts/token` create on the `aeterna` SA in the aeterna namespace, nothing more.

### D9 — How aeterna addresses bao

**Decision:** Two new env vars consumed by the existing Vault resolver, populated in `charts/aeterna/templates/deployment.yaml` only when the operator sets `vault.enabled=true` in the main chart's values:

- `VAULT_ADDR`: defaults to `http://aeterna-prereqs-openbao:8200` (assumes the prereqs chart was installed in the same release name + namespace; documented).
- `VAULT_K8S_AUTH_PATH`: defaults to `auth/kubernetes`; `auth/kubernetes/aeterna` if the consumer namespaced the auth path.

Operators using their own external Vault override `vault.enabled=true` + `vault.address=https://vault.internal.example.com:8200`. The aeterna chart doesn't care which target it points at, as long as Vault API speaks back.

### D10 — How the e2e suite (PR #169) uses it

**Decision:** A values overlay `charts/aeterna-prereqs/values-e2e.yaml` enables bao in `dev` seal mode (with `allowDevSeal=true`). The e2e runner script's `kind-bootstrap` cluster mode does:

```bash
helm install aeterna-prereqs charts/aeterna-prereqs \
  -f charts/aeterna-prereqs/values-e2e.yaml \
  --set openbao.enabled=true \
  --set openbao.allowDevSeal=true \
  --set openbao.seal.mode=dev
```

The existing `vault.sh` secrets-backend adapter from #169 task 20.4 works unchanged. New e2e env var:

- `AETERNA_E2E_VAULT_ADDR` — defaults to `http://aeterna-prereqs-openbao.aeterna-e2e.svc.cluster.local:8200` in `kind-bootstrap`. Overridable for `existing-kubeconfig` and `external-https` modes.

This closes #169's open AC for the `vault` backend in CI: we now exercise the `vault` adapter against a real OpenBao instance on every full-profile run, not just smoke-test it against a mock.

### D11 — Versioning and upgrades

**Decision:** Pin OpenBao Helm chart to `0.10.*` (or whatever's current at implementation time — concrete pin in tasks.md 1.1). Bump policy:

- Patch versions: auto-accepted via `0.10.*` constraint; CI matrix tests against `latest` weekly.
- Minor versions: explicit PR; updates `Chart.yaml`, regenerates `Chart.lock`, runs full e2e profile.
- Major versions of OpenBao itself (1.x → 2.x): full design review; never silent.

`OPENBAO_VERSION_COMPAT.md` (new file) tracks which `aeterna-prereqs` chart versions support which OpenBao versions, with upgrade notes.

### D12 — Backup / DR (out of scope for v1)

**Decision:** Out of scope for this change. Documented in §D11 of the v2 follow-up sketch:
- Raft snapshot CronJob (`bao operator raft snapshot save`) on a configurable schedule
- Snapshot upload to S3/GCS via init-container pattern
- Restore runbook
- Data residency considerations

For v1, operators using `auto-unseal` are pointed at OpenBao's upstream DR docs.

### D13 — Migration from `Postgres`-stored to `Vault`-stored secrets

**Decision:** Doc-only for v1. Operational steps:

1. Operator deploys bao via this change.
2. Operator runs a one-off script (provided as a runbook, not a binary): for each tenant secret, read plaintext via the Postgres resolver path, write to bao at `secret/aeterna/<tenant-id>/<key>`, update the `tenant_secrets` row to flip the `SecretReference` from `Postgres` to `Vault`.
3. Verification: re-render manifests, confirm resolution works, confirm Postgres rows can be soft-deleted.

A migration *binary* (`aeterna-cli secrets migrate --from postgres --to vault`) is its own change. Sketch in `docs/runbooks/secrets-migration.md` (new in this change), but no code.

## Risks & Mitigations

| Risk | Likelihood | Severity | Mitigation |
|---|---|---|---|
| Operator enables `dev` seal in production and loses all secrets on pod restart | medium | **catastrophic** | Two-flag guardrail (D6); chart-rendered NOTES emit a screaming warning; readiness annotation flagged; doc front-loads this risk |
| OpenBao governance instability (project is young) | low | medium | Pin minor (D11); CI matrix runs against `latest` to detect drift early; design accepts swap-back to HashiCorp Vault is a one-line `Chart.yaml` change since the API is identical |
| Bootstrap Job fails idempotency check on upgrade and corrupts policy | medium | high | Each step queries-then-writes; Job is `--dry-run` validated in CI; `helm test` includes a smoke that re-runs the Job to confirm no-op behavior |
| K8s auth misconfiguration (wrong SA, wrong namespace) silently denies aeterna pod | medium | medium | Bootstrap Job emits a final verification step that issues a `bao login -method=kubernetes role=aeterna` using the aeterna SA token via TokenRequest API and asserts success |
| OpenBao chart's HA mode (raft) loses quorum on cluster maintenance | low | high | HA mode is opt-in; default is single-node; HA-mode docs include explicit PDB + PVC retention guidance |
| Bootstrap Job logs leak bao root token | low | catastrophic | Root token is generated by bao itself and never logged; Job uses k8s auth only — no root token exists in our control plane after init; explicit grep test in CI scans Job logs for `hvs.` and `s.` token prefixes (D8) |
| Cross-chart coupling: aeterna upgraded before/after prereqs upgrade breaks bao addressability | low | medium | `VAULT_ADDR` is a Service DNS name, not a versioned reference; Service contract is unchanged across OpenBao chart minors; documented |
| Existing consumers enable `openbao.enabled=true` accidentally on `helm upgrade` | very low | low | Default is `false`; behavior change requires explicit values flip |

## Open questions

None as of this draft. (License and lineage answered in §D1; subchart placement in §D2; default-off behavior in §D3; auth-method choice in §D5; seal-mode safety in §D6; migration-tooling scope in §D13.)
