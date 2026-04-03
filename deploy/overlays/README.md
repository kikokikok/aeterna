# Deployment Overlays

This directory contains environment-specific overlay values for Aeterna Helm deployments.
Each subdirectory corresponds to an environment and provides values that layer on top of
the base chart defaults.

## Structure

```
deploy/overlays/
├── staging/
│   ├── values.yaml          # Staging-specific overrides (non-secret)
│   └── tenant-config.yaml   # Staging tenant config + Git provider connections
└── production/
    ├── values.yaml          # Production-specific overrides (non-secret)
    └── tenant-config.yaml   # Production tenant config + Git provider connections
```

## Deploying

Layer values files from least to most specific:

```bash
# Staging
helm upgrade --install aeterna ./charts/aeterna \
  -f charts/aeterna/values.yaml \
  -f deploy/overlays/staging/values.yaml \
  -f deploy/overlays/staging/tenant-config.yaml

# Production
helm upgrade --install aeterna ./charts/aeterna \
  -f charts/aeterna/values.yaml \
  -f deploy/overlays/production/values.yaml \
  -f deploy/overlays/production/tenant-config.yaml
```

## Tenant Config Provider Contract

### ConfigMap naming convention
- Tenant config: `aeterna-tenant-<tenant-id>`
- Tenant secrets: `aeterna-tenant-<tenant-id>-secret`

### Git Provider Connection Secrets

PEM key material for GitHub App connectivity MUST be created externally before deploying:

```bash
# Create the GitHub App PEM secret (one per environment)
kubectl create secret generic aeterna-github-app-pem \
  --from-file=pem-key=./private-key.pem \
  -n <namespace>

# Optional: webhook secret
kubectl create secret generic aeterna-github-app-webhook \
  --from-literal=webhook-secret=<webhook-secret> \
  -n <namespace>
```

These secrets are then referenced by `pemSecretRef` / `webhookSecretRef` in the overlay
values files using the `secret/<secret-name>/<key>` URI format.

### Granting tenant access to a connection

After deploying, use the Aeterna CLI to grant tenant visibility:

```bash
aeterna tenant connection grant <tenant-id> --connection <connection-id>
```

Or use the platform API directly:

```bash
curl -X POST https://aeterna.example.com/api/v1/admin/git-provider-connections/<connection-id>/tenants/<tenant-id> \
  -H "x-api-key: <admin-api-key>"
```

### Verification

Verify that the tenant config surfaces are materialized:

```bash
# Check ConfigMap exists
kubectl get configmap aeterna-tenant-<tenant-id> -o jsonpath='{.data.tenant-config\.json}'

# Check Secret exists (keys only — values are opaque)
kubectl get secret aeterna-tenant-<tenant-id>-secret -o jsonpath='{.data}' | jq 'keys'

# List tenant-visible connections via CLI
aeterna tenant connection list <tenant-id>
```
