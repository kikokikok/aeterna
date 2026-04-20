# KMS & Tenant-Secret Envelope Encryption

> Maps the Helm `kms:` values block onto the `SecretBackend` / `KmsProvider` architecture. For the conceptual picture, start with [Secret Backend Architecture](../architecture/secret-backend.md).

Aeterna envelope-encrypts every tenant secret row (OpenAI keys, GitHub tokens, embedding credentials …). The `kms.provider` knob in `values.yaml` selects how the per-row data keys are wrapped.

| Mode | `kms.provider` | Use |
|---|---|---|
| **AWS KMS** | `"aws"` | Production. Required in any environment handling real tenant data. |
| **Local** | `"local"` | Development, CI, and DR drills only. The app emits `WARN: LocalKmsProvider in use …` on every boot. |

---

## Values reference

New top-level `kms:` block in `charts/aeterna/values.yaml`:

```yaml
kms:
  provider: "local"            # "aws" | "local"

  aws:
    keyArn: ""                 # arn:aws:kms:<region>:<account-id>:key/<uuid>
                               # arn:aws:kms:<region>:<account-id>:alias/<alias-name>
    auth: "irsa"               # "irsa" | "static"
    roleArn: ""                # IAM role ARN — annotates the SA when auth=irsa
    existingSecret: ""         # K8s Secret holding AWS AK/SK — only for auth=static
    accessKeyIdKey: "aws-access-key-id"
    secretAccessKeyKey: "aws-secret-access-key"

  local:
    generate: true             # chart creates a random 32B key Secret (default)
    existingSecret: ""         # externally-managed Secret (when generate=false)
    secretKey: "kms-local-key" # key inside that Secret
```

### Env variables produced

The chart projects `kms.*` into the aeterna Deployment as:

| Env var | Mode | Source |
|---|---|---|
| `AETERNA_KMS_PROVIDER` | always | `kms.provider` |
| `AETERNA_KMS_AWS_KEY_ARN` | aws | `kms.aws.keyArn` |
| `AWS_ACCESS_KEY_ID` | aws + static | `kms.aws.existingSecret` → `accessKeyIdKey` |
| `AWS_SECRET_ACCESS_KEY` | aws + static | `kms.aws.existingSecret` → `secretAccessKeyKey` |
| `AETERNA_LOCAL_KMS_KEY` | local | chart-generated or `kms.local.existingSecret` → `secretKey` |

### Fail-closed validation

`helm template` errors before rendering when:

- `kms.provider=aws` and `kms.aws.keyArn` is empty → `kms.provider=aws requires kms.aws.keyArn to be set`
- `kms.provider=aws`, `auth=static`, and `kms.aws.existingSecret` is empty → `kms.aws.existingSecret is required when kms.aws.auth=static`
- `kms.provider=local`, `generate=false`, and `kms.local.existingSecret` is empty → `kms.local.existingSecret is required when kms.local.generate=false`

---

## Recipe 1 — Production with IRSA (recommended)

```yaml
# values-prod.yaml
kms:
  provider: "aws"
  aws:
    keyArn: "arn:aws:kms:eu-west-1:123456789012:alias/aeterna-tenant-secrets"
    auth: "irsa"
    roleArn: "arn:aws:iam::123456789012:role/aeterna-kms"
```

1. Create the CMK and grant the IAM role `kms:Encrypt`, `kms:Decrypt`, `kms:GenerateDataKey`:

   ```
   aws kms create-key --description 'aeterna tenant-secret envelope DEKs'
   aws kms create-alias --alias-name alias/aeterna-tenant-secrets --target-key-id <key-id>
   ```
2. Create the IAM role with a trust policy for your EKS OIDC provider and the `kms:*` policy above. See [AWS IRSA docs](https://docs.aws.amazon.com/eks/latest/userguide/iam-roles-for-service-accounts.html).
3. Install / upgrade:

   ```
   helm upgrade --install aeterna charts/aeterna -f values-prod.yaml
   ```
4. Verify the ServiceAccount picked up the annotation:

   ```
   kubectl get sa aeterna -o jsonpath='{.metadata.annotations}'
   # {"eks.amazonaws.com/role-arn":"arn:aws:iam::123456789012:role/aeterna-kms"}
   ```
5. Verify the pod env:

   ```
   kubectl exec deploy/aeterna -- env | grep AETERNA_KMS
   # AETERNA_KMS_PROVIDER=aws
   # AETERNA_KMS_AWS_KEY_ARN=arn:aws:kms:eu-west-1:…
   ```

## Recipe 2 — Production with static AWS credentials

Only use when IRSA is unavailable (e.g. non-EKS clusters). Credentials live in a K8s Secret your operator manages out-of-band.

```yaml
kms:
  provider: "aws"
  aws:
    keyArn: "arn:aws:kms:eu-west-1:123456789012:key/…"
    auth: "static"
    existingSecret: "aeterna-aws-credentials"
    # optionally override key names inside the Secret:
    # accessKeyIdKey: "AWS_ACCESS_KEY_ID"
    # secretAccessKeyKey: "AWS_SECRET_ACCESS_KEY"
```

```
kubectl create secret generic aeterna-aws-credentials \
  --from-literal=aws-access-key-id=AKIA… \
  --from-literal=aws-secret-access-key=…
```

## Recipe 3 — Local / CI / DR drill

The default `provider: "local"` needs no configuration — the chart auto-generates a 32-byte random key into a `<release>-kms-local` Secret on first install.

The Secret is annotated `helm.sh/resource-policy: keep` and re-read via `lookup` on upgrade, so **the key survives chart upgrades and release rotations**. Rotating the Helm release will not corrupt existing ciphertext rows.

If you want to manage the key yourself (for a DR runbook that pre-seeds keys, for example):

```yaml
kms:
  provider: "local"
  local:
    generate: false
    existingSecret: "my-kms-local"
    secretKey: "key"
```

With:

```
head -c 32 /dev/urandom | base64 | \
  kubectl create secret generic my-kms-local --from-file=key=/dev/stdin
```

**⚠️ Never use `provider: local` in production.** The app emits a `WARN` on every KMS operation precisely so this shows up in log aggregators.

---

## Verification: `helm template` dry-runs

```
# local mode — renders the kms-local Secret + AETERNA_LOCAL_KMS_KEY env
helm template aeterna charts/aeterna \
  | grep -E 'AETERNA_(KMS|LOCAL)|kms-local'

# aws + IRSA — renders SA annotation + AETERNA_KMS_AWS_KEY_ARN env
helm template aeterna charts/aeterna \
  --set kms.provider=aws \
  --set kms.aws.keyArn=arn:aws:kms:eu-west-1:123456789012:alias/x \
  --set kms.aws.roleArn=arn:aws:iam::123456789012:role/y \
  | grep -E 'eks.amazonaws|AETERNA_KMS'

# fail-closed — errors without keyArn
helm template aeterna charts/aeterna --set kms.provider=aws
# Error: kms.provider=aws requires kms.aws.keyArn to be set
```

---

## Related

- [Secret Backend Architecture](../architecture/secret-backend.md) — trait shape, envelope encryption design
- [Secret Rotation runbook](../guides/secret-rotation.md) — rotating CMKs, local keys, tenant keys
- [External Secrets](./external-secrets.md) — for application-level Secrets (not the KMS DEK wrapping key)
- [SOPS Secrets](./sops-secrets.md) — for GitOps-managed Secret manifests
