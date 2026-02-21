# Security Best Practices

## Pod Security Standards

Aeterna enforces the **restricted** Pod Security Standard profile on all pods:

- `runAsNonRoot: true` (UID 1000)
- `readOnlyRootFilesystem: true`
- `allowPrivilegeEscalation: false`
- `capabilities.drop: [ALL]`
- `seccompProfile.type: RuntimeDefault`

These settings are applied to both the main Aeterna deployment and all auxiliary jobs (migration, backup, backup-verify).

To enforce at the namespace level:

```bash
kubectl label namespace aeterna \
  pod-security.kubernetes.io/enforce=restricted \
  pod-security.kubernetes.io/audit=restricted \
  pod-security.kubernetes.io/warn=restricted
```

## Network Policies

Enable network policies to restrict traffic:

```yaml
networkPolicy:
  enabled: true
```

When enabled, Aeterna creates `NetworkPolicy` resources for:

| Component | Ingress | Egress |
|-----------|---------|--------|
| Aeterna server | Ingress controller, Prometheus | PostgreSQL, Qdrant, Redis, OPAL, DNS |
| OPAL Server | Fetcher, Cedar Agent, Aeterna | PostgreSQL, Redis, DNS, external HTTPS |
| Cedar Agent | Aeterna | OPAL Server, DNS, external HTTPS |
| OPAL Fetcher | OPAL Server | OPAL Server, PostgreSQL, DNS, external HTTPS |

All other traffic is denied by default.

## Image Pull Secrets

For private registries, configure image pull secrets:

```yaml
aeterna:
  imagePullSecrets:
    - name: my-registry-secret

global:
  imagePullSecrets:
    - name: my-registry-secret
```

Create the secret:

```bash
kubectl create secret docker-registry my-registry-secret \
  --docker-server=ghcr.io \
  --docker-username=USERNAME \
  --docker-password=TOKEN
```

## Secret Management

Never store credentials in `values.yaml`. Use one of:

1. **Kubernetes Secrets** (default): Create secrets manually, reference via `existingSecret`
2. **SOPS**: Encrypt values files at rest. See [sops-secrets.md](./sops-secrets.md)
3. **External Secrets Operator**: Sync from Vault/AWS/Azure. See [external-secrets.md](./external-secrets.md)

```yaml
secrets:
  provider: helm          # helm (default), sops, external-secrets
```

## RBAC

Aeterna creates a `ServiceAccount`, `Role`, and `RoleBinding` scoped to its namespace. The service account token is auto-mounted for in-cluster API access required by the migration job.

To use a pre-existing service account:

```yaml
aeterna:
  serviceAccount:
    create: false
    name: "my-existing-sa"
```

## TLS

Enable TLS on the ingress:

```yaml
aeterna:
  ingress:
    enabled: true
    className: nginx
    annotations:
      cert-manager.io/cluster-issuer: letsencrypt-prod
    tls:
      - secretName: aeterna-tls
        hosts:
          - aeterna.example.com
```

For internal service-to-service TLS, configure your service mesh (Istio, Linkerd) to inject mTLS sidecars.

## Supply Chain Security

Aeterna container images are:

- Built with multi-arch support (amd64 + arm64)
- Scanned with Trivy for CVEs
- Signed with Cosign for provenance verification

Verify image signatures:

```bash
cosign verify ghcr.io/kikokikok/aeterna:latest \
  --certificate-identity-regexp=".*" \
  --certificate-oidc-issuer-regexp=".*"
```
