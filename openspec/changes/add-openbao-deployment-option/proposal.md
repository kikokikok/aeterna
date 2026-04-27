# Add OpenBao deployment option to `aeterna-prereqs`

## Why

`SecretReference::Vault { mount, path, field }` already exists in `mk_core::secret`, with a working resolver in `cli/src/server/tenant_api.rs:4413`. What's missing is **a way for a consumer to actually have a vault running** without going off and provisioning one themselves.

Two consumer scenarios drive this:

1. **Production / staging deployments.** Operators who want `Vault`-backed secrets (instead of the default Postgres-encrypted `tenant_secrets` table) currently have to bring their own HashiCorp Vault cluster. Significant ops burden + a license question post-2023 BSL relicense.
2. **The redesigned e2e conformance suite (PR #169).** Task 20.4 ships a `vault.sh` secrets-backend adapter so downstream consumers can resolve test secrets via Vault. Without a deployable target, that adapter has nothing to talk to in `kind-bootstrap` mode and the conformance contract for the `vault` backend can't be exercised in our own CI.

**OpenBao** is the natural fit:
- LF-hosted Apache-2.0 fork of HashiCorp Vault (post-BSL split, governed under the Linux Foundation)
- Wire-compatible with the Vault API — existing resolver code, CLI, and SDKs work unchanged
- First-class Helm chart (`openbao/openbao`) with k8s-native auth, raft storage, KMS auto-unseal
- Replaces "bring your own vault" with "flip a values flag"

## What Changes

- **New conditional dependency** in `charts/aeterna-prereqs/Chart.yaml` on the official `openbao/openbao` Helm chart, gated by `openbao.enabled` (default **off** — backwards-compatible with every existing prereqs deployment).
- **Bootstrap Job** (post-install / post-upgrade Helm hook) that idempotently configures OpenBao for aeterna's needs:
  - Enables KV v2 secret engine at `secret/aeterna/`
  - Enables Kubernetes ServiceAccount auth at `auth/kubernetes/`
  - Creates policy `aeterna-pod` granting `read` on `secret/data/aeterna/*`
  - Binds the policy to the `aeterna` ServiceAccount in the aeterna namespace
- **Three documented seal modes** with explicit guardrails:
  - `dev` — auto-unsealed in-memory; **for e2e + local dev only**; chart refuses to install with `dev` seal unless `openbao.allowDevSeal=true` is also set
  - `manual` — Shamir unseal; documented but discouraged
  - `auto-unseal` — production-grade via AWS KMS / GCP KMS / Azure Key Vault / k8s seal-wrap; values surface for each
- **No changes to the `aeterna` main chart and no changes to the resolver code** — aeterna talks to bao via `VAULT_ADDR` env, which already works because OpenBao speaks Vault's API. Default `VAULT_ADDR` value in `charts/aeterna` is updated to point at `http://aeterna-prereqs-openbao:8200` when (and only when) the consumer enables bao.
- **e2e suite integration** (cross-links #169 task 20.4): a values overlay `charts/aeterna-prereqs/values-e2e.yaml` enabling bao in `dev` seal mode for the `kind-bootstrap` cluster mode. The existing `vault.sh` adapter from #169 task 20.4 works unchanged against it.
- **Documentation:** runbook for production auto-unseal config (per cloud), migration sketch from `Postgres`-stored to `Vault`-stored secrets (operational guide; no migration tooling shipped — out of scope for v1), upgrade and DR notes.

## What does NOT Change

- `SecretReference` enum and its resolver — already shipped (harden-tenant-provisioning task 3.4 ✅).
- Default secrets backend remains `Postgres` (the encrypted `tenant_secrets` table). OpenBao is opt-in.
- Existing tenants. No data migration is forced by this change.
- The main `charts/aeterna` chart's "no dependencies" stance — bao stays in prereqs where postgres / dragonfly / qdrant live.

## Capabilities

- **New:** `openbao-deployment` (subchart wiring + bootstrap job + seal-mode contract)
- **Modified:** `aeterna-prereqs-chart` (new conditional dependency + values surface)
- **Modified (docs only):** `secrets-backend` capability (production runbook for the Vault variant)

## Impact

- **Code:** zero Rust changes. All Helm + Bash.
- **Security posture:** dev seal mode is loud-by-default; production seal config is required not optional; secrets-leak posture for the bootstrap Job is covered in design.md §D8.
- **Ops:** consumers gain a one-flag option. Existing consumers unaffected.
- **Cross-PR coordination:** unblocks PR #169's `vault` secrets-backend mode in `kind-bootstrap` cluster mode (closing what would otherwise be a documentation-only feature in our own CI).

## Out of scope

- Backup / DR snapshot CronJob (raft snapshots): tracked as a v2 follow-up.
- Migration tooling from `SecretReference::Postgres` → `SecretReference::Vault` (doc-only sketch; rotation tooling is its own change).
- HashiCorp Vault Enterprise features (namespaces, performance replication): OpenBao does not implement them; consumers needing those bring their own Vault Enterprise (the resolver still works).
- Publishing OpenBao as a transitive dep of `charts/aeterna` itself: explicitly rejected — keeps the main chart dependency-free.
