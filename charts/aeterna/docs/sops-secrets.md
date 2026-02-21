# SOPS Secrets Management

Encrypt sensitive Helm values using [SOPS](https://github.com/getsops/sops) and the [helm-secrets](https://github.com/jkroepke/helm-secrets) plugin.

## Prerequisites

- [SOPS](https://github.com/getsops/sops/releases) v3.8+
- [helm-secrets](https://github.com/jkroepke/helm-secrets) plugin
- An encryption key (age or GPG)

```bash
helm plugin install https://github.com/jkroepke/helm-secrets
```

## Setup with age Keys

### Generate a key pair

```bash
age-keygen -o age-key.txt
# Public key: age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p
```

### Store the private key

```bash
mkdir -p ~/.config/sops/age
cp age-key.txt ~/.config/sops/age/keys.txt
```

### Create `.sops.yaml` in the project root

```yaml
creation_rules:
  - path_regex: charts/aeterna/.*\.enc\.yaml$
    age: "age1ql3z7hjy54pw3hyww5ayyfg7zqgvc7w3j2elw8zmrj2kg5sfn9aqmcac8p"
```

## Setup with GPG Keys

### Generate a GPG key

```bash
gpg --full-generate-key
# Note the fingerprint, e.g. 1234ABCD5678EFGH
```

### Create `.sops.yaml`

```yaml
creation_rules:
  - path_regex: charts/aeterna/.*\.enc\.yaml$
    pgp: "1234ABCD5678EFGH"
```

## Encrypting Values

Start from the example file:

```bash
cp charts/aeterna/examples/values-sops.yaml charts/aeterna/values-secrets.yaml
```

Edit `values-secrets.yaml` with real credentials, then encrypt:

```bash
sops --encrypt charts/aeterna/values-secrets.yaml > charts/aeterna/values-secrets.enc.yaml
rm charts/aeterna/values-secrets.yaml
```

## Deploying with Encrypted Values

```bash
helm secrets install aeterna ./charts/aeterna \
  -f charts/aeterna/values.yaml \
  -f charts/aeterna/values-secrets.enc.yaml
```

Upgrade:

```bash
helm secrets upgrade aeterna ./charts/aeterna \
  -f charts/aeterna/values.yaml \
  -f charts/aeterna/values-secrets.enc.yaml
```

## Editing Encrypted Files

```bash
sops charts/aeterna/values-secrets.enc.yaml
```

This opens the decrypted content in your `$EDITOR`, then re-encrypts on save.

## Key Rotation

```bash
sops --rotate --in-place charts/aeterna/values-secrets.enc.yaml
```

## CI/CD Integration

Store the age private key or GPG key as a CI secret, then export before running helm:

```bash
export SOPS_AGE_KEY_FILE=/path/to/age-key.txt
helm secrets upgrade aeterna ./charts/aeterna -f values-secrets.enc.yaml
```

For GitHub Actions:

```yaml
- name: Deploy
  env:
    SOPS_AGE_KEY: ${{ secrets.SOPS_AGE_KEY }}
  run: |
    echo "$SOPS_AGE_KEY" > /tmp/age-key.txt
    export SOPS_AGE_KEY_FILE=/tmp/age-key.txt
    helm secrets upgrade aeterna ./charts/aeterna -f values-secrets.enc.yaml
```
