# Secret Rotation Runbook

> Operator runbook for rotating the keys that protect tenant secrets. Uses the `SecretBackend` primitives documented in [Secret Backend Architecture](../architecture/secret-backend.md) and the Helm surface documented in [Helm KMS](../helm/kms.md).

Three kinds of keys exist; they rotate independently.

| Key | Where it lives | Owner | When to rotate |
|---|---|---|---|
| **CMK** (customer master key) | AWS KMS — ARN in `kms.aws.keyArn` | AWS account | Annually; on suspected compromise |
| **DEK** (data encryption key) | `tenant_secrets.wrapped_dek` — one per row | Aeterna | On re-put of a specific secret (automatic) |
| **Local KMS key** | `<release>-kms-local` K8s Secret | Helm chart (or operator) | Never rotate in place — re-seed only for new environments |

Most day-to-day rotation is the middle row and happens implicitly: re-putting a tenant secret performs a full envelope rotation for that row. The runbooks below cover the explicit cases.

---

## Runbook 1 — Rotate an AWS CMK (same key, new version)

Use when AWS KMS flags the key as due for rotation, or on annual policy.

**Mechanism.** AWS KMS key rotation creates a new *key version* but preserves the ARN. The ciphertext blobs we persisted remain valid: AWS KMS routes decryption from the embedded key hint in the blob, so existing `wrapped_dek` values continue to decrypt after rotation. New writes automatically use the new version.

**Prereq.** `kms.provider=aws`, `keyArn` set to an ARN pointing at the CMK being rotated.

1. Trigger rotation in AWS:

   ```
   aws kms enable-key-rotation --key-id <key-id-or-alias>
   # (Or: aws kms rotate-key-on-demand --key-id …  if automatic rotation already enabled.)
   ```
2. Verify:

   ```
   aws kms get-key-rotation-status --key-id <key-id>
   aws kms list-key-rotations --key-id <key-id>
   ```
3. Smoke-test from the pod. Any `secret_backend.get()` call against an existing secret still returns the correct plaintext:

   ```
   kubectl exec -it deploy/aeterna -- aeterna-cli tenants secret get \
     --tenant-id <uuid> --name openai-api-key
   ```
4. New writes use the rotated key material:

   ```
   kubectl exec -it deploy/aeterna -- aeterna-cli tenants secret put \
     --tenant-id <uuid> --name canary --value $(uuidgen)
   kubectl exec -it deploy/aeterna -- aeterna-cli tenants secret get \
     --tenant-id <uuid> --name canary
   ```

**No chart change.** `kms.aws.keyArn` is unchanged; no redeploy needed.

---

## Runbook 2 — Switch to a new AWS CMK (new ARN)

Use on a suspected key-material compromise, or when migrating to a new AWS account / region.

**Mechanism.** Existing rows stay decryptable as long as the old CMK remains *usable* (enabled and you have `kms:Decrypt` on it). Updating `kms.aws.keyArn` only changes the key used for **new** `put()` calls. To fully migrate off the old CMK, every row must be re-put (which rewraps its DEK with the new CMK).

1. Provision the new CMK + alias + IAM grants (see [Helm KMS Recipe 1](../helm/kms.md#recipe-1--production-with-irsa-recommended)).
2. **Keep** `kms:Decrypt` on the old CMK while migrating.
3. Flip `kms.aws.keyArn` to the new ARN and redeploy:

   ```
   helm upgrade aeterna charts/aeterna -f values-prod.yaml \
     --set kms.aws.keyArn=arn:aws:kms:eu-west-1:…:alias/aeterna-tenant-secrets-v2
   ```
4. Rewrap every existing row. The [`aeterna-cli tenants secret rewrap`] command iterates `SecretBackend::list` per tenant and performs a `put` + `get` roundtrip, bumping `generation`:

   ```
   kubectl exec -it deploy/aeterna -- \
     aeterna-cli tenants secret rewrap --all
   ```

   > **B1 scope note.** The `rewrap` subcommand is queued for the B5 bundle. Until it ships, use a short `psql` script that `SELECT id, tenant_id FROM tenant_secrets` and calls `secret put` with the current plaintext — or simply re-put every tenant secret via your provisioning pipeline.

5. Verify no row still references the old CMK:

   ```
   psql -c "SELECT DISTINCT kms_key_id FROM tenant_secrets;"
   # Expect the new ARN only.
   ```
6. Disable the old CMK once the row count matches.

---

## Runbook 3 — Rotate a single tenant secret

Use when a tenant's OpenAI key / GitHub PAT / etc. is rotated by its owner.

Re-`put` is a full envelope rotation (fresh DEK, fresh nonce, fresh ciphertext, `generation` bumped, same `secret_id`):

```
aeterna-cli tenants secret put \
  --tenant-id <tenant-uuid> \
  --name openai-api-key \
  --value "$(pbpaste)"
```

Consumers holding the `SecretReference::Postgres { secret_id }` do not need to update; the reference is stable across rotations. Their next `get()` picks up the new value.

---

## Runbook 4 — Re-seed a local KMS key (fresh environment only)

**Never do this on a cluster with existing tenant data.** The local key is the DEK-wrapping key for every `wrapped_dek` in `tenant_secrets`; rotating it in place orphans every row.

Supported scenarios:

- **Fresh dev / CI environment.** The chart auto-generates one; no action required.
- **DR drill starting from a wiped Postgres.** Same as above.
- **External key management for local mode.** Follow [Helm KMS Recipe 3](../helm/kms.md#recipe-3--local--ci--dr-drill) and point `kms.local.existingSecret` at a Secret you control.

If you must force a new local key:

1. Ensure no data in `tenant_secrets` depends on the current key (truncate, or accept loss).
2. Delete the chart-owned Secret:

   ```
   kubectl delete secret <release>-kms-local
   ```
3. Rerun `helm upgrade` — the chart regenerates a fresh key.

---

## Observability

During any rotation, watch for:

| Signal | Meaning |
|---|---|
| `LocalKmsProvider in use` WARN line | `provider=local` is active. In production this is a rollback / misconfiguration. |
| `kms error:` in `SecretBackendError` | CMK missing, IAM grants dropped, or SDK chain failed. First suspects: `kms.aws.roleArn`, `kms.aws.keyArn`. |
| `aead error:` on `get` | Row tampering, wrong CMK, or a half-migrated state — stop the rotation and investigate before continuing. |
| `tenant_secrets.generation` not advancing during Runbook 2 | The rewrap loop is not actually calling `put`; it is reading only. |

## Related

- [Secret Backend Architecture](../architecture/secret-backend.md)
- [Helm KMS configuration](../helm/kms.md)
- `openspec/changes/harden-tenant-provisioning/design.md` — decisions D2 (envelope), D4 (CMK rotation without orphan), D5 (rewrap tool)
