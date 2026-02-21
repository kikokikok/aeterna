# External Secrets Operator Integration

Use the [External Secrets Operator](https://external-secrets.io/) (ESO) to sync secrets from external providers into Kubernetes.

## Prerequisites

- External Secrets Operator installed in the cluster
- A configured `SecretStore` or `ClusterSecretStore`

```bash
helm repo add external-secrets https://charts.external-secrets.io
helm install external-secrets external-secrets/external-secrets -n external-secrets --create-namespace
```

## Enabling External Secrets

```yaml
secrets:
  provider: external-secrets
  externalSecrets:
    enabled: true
    refreshInterval: "1h"
    secretStoreRef:
      name: my-secret-store
      kind: ClusterSecretStore
    remoteRefs:
      postgresqlPassword: "path/to/postgres-password"
      redisPassword: "path/to/redis-password"
      opalMasterToken: "path/to/opal-token"
      llmApiKey: "path/to/llm-api-key"
```

This creates an `ExternalSecret` resource that syncs the referenced keys into a Kubernetes secret consumed by Aeterna.

## AWS Secrets Manager

### Create a ClusterSecretStore

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: aws-secrets-manager
spec:
  provider:
    aws:
      service: SecretsManager
      region: us-east-1
      auth:
        jwt:
          serviceAccountRef:
            name: external-secrets-sa
            namespace: external-secrets
```

### Store secrets in AWS

```bash
aws secretsmanager create-secret --name aeterna/postgres-password --secret-string "my-password"
aws secretsmanager create-secret --name aeterna/llm-api-key --secret-string "sk-..."
```

### Reference in values

```yaml
secrets:
  provider: external-secrets
  externalSecrets:
    enabled: true
    secretStoreRef:
      name: aws-secrets-manager
      kind: ClusterSecretStore
    remoteRefs:
      postgresqlPassword: "aeterna/postgres-password"
      llmApiKey: "aeterna/llm-api-key"
```

## HashiCorp Vault

### Create a ClusterSecretStore

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: vault-backend
spec:
  provider:
    vault:
      server: "https://vault.example.com"
      path: "secret"
      version: "v2"
      auth:
        kubernetes:
          mountPath: "kubernetes"
          role: "aeterna"
          serviceAccountRef:
            name: external-secrets-sa
            namespace: external-secrets
```

### Store secrets in Vault

```bash
vault kv put secret/aeterna postgres-password="my-password" llm-api-key="sk-..."
```

### Reference in values

```yaml
secrets:
  provider: external-secrets
  externalSecrets:
    enabled: true
    secretStoreRef:
      name: vault-backend
      kind: ClusterSecretStore
    remoteRefs:
      postgresqlPassword: "aeterna#postgres-password"
      llmApiKey: "aeterna#llm-api-key"
```

## Azure Key Vault

### Create a ClusterSecretStore

```yaml
apiVersion: external-secrets.io/v1beta1
kind: ClusterSecretStore
metadata:
  name: azure-keyvault
spec:
  provider:
    azurekv:
      tenantId: "your-tenant-id"
      vaultUrl: "https://my-vault.vault.azure.net"
      authType: ManagedIdentity
      identityId: "your-managed-identity-client-id"
```

### Store secrets in Azure Key Vault

```bash
az keyvault secret set --vault-name my-vault --name aeterna-postgres-password --value "my-password"
az keyvault secret set --vault-name my-vault --name aeterna-llm-api-key --value "sk-..."
```

### Reference in values

```yaml
secrets:
  provider: external-secrets
  externalSecrets:
    enabled: true
    secretStoreRef:
      name: azure-keyvault
      kind: ClusterSecretStore
    remoteRefs:
      postgresqlPassword: "aeterna-postgres-password"
      llmApiKey: "aeterna-llm-api-key"
```

## Verifying

Check that the ExternalSecret synced:

```bash
kubectl get externalsecret
kubectl get secret aeterna-external -o yaml
```

Troubleshoot sync failures:

```bash
kubectl describe externalsecret aeterna
```
