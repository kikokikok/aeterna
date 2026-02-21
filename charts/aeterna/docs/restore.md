# Backup Restore Procedure

## Prerequisites

- `kubectl` access to the cluster
- `pg_restore` (PostgreSQL 16 client tools)
- Access to the backup storage (S3/GCS/Azure)

## Locating Backups

Backups are stored under `aeterna/<YYYY>/<MM>/<DD>/` in your configured destination.

### S3

```bash
aws s3 ls s3://YOUR_BUCKET/aeterna/ --recursive | sort | tail -5
```

### GCS

```bash
gsutil ls -r gs://YOUR_BUCKET/aeterna/ | sort | tail -5
```

### Azure Blob Storage

```bash
az storage blob list --account-name YOUR_ACCOUNT --container-name YOUR_CONTAINER --prefix aeterna/ --output table
```

## Download the Backup

```bash
aws s3 cp s3://YOUR_BUCKET/aeterna/2026/02/21/backup-20260221-020000.sql /tmp/restore.sql
```

## Restore to Existing Cluster

### 1. Scale down Aeterna

```bash
kubectl scale deployment aeterna --replicas=0
```

### 2. Connect to PostgreSQL

For bundled CloudNativePG:

```bash
kubectl get secret aeterna-pg-app -o jsonpath='{.data.password}' | base64 -d
kubectl port-forward svc/aeterna-pg-rw 5432:5432 &
```

For external PostgreSQL, use your existing connection details.

### 3. Restore the database

```bash
pg_restore \
  --host=localhost \
  --port=5432 \
  --username=aeterna \
  --dbname=aeterna \
  --clean \
  --if-exists \
  --verbose \
  /tmp/restore.sql
```

The `--clean --if-exists` flags drop existing objects before recreating them, making the restore idempotent.

### 4. Scale Aeterna back up

```bash
kubectl scale deployment aeterna --replicas=2
```

### 5. Verify

```bash
kubectl port-forward svc/aeterna 8080:8080 &
curl http://localhost:8080/health/ready
```

## Restore to a New Cluster

1. Install Aeterna with the same `values.yaml` configuration
2. Wait for the migration job to complete (creates the schema)
3. Scale down Aeterna: `kubectl scale deployment aeterna --replicas=0`
4. Restore the backup (steps 2-3 above)
5. Scale back up

## Backup Verification

Enable the verification CronJob to automatically validate backup integrity:

```yaml
backup:
  enabled: true
  verify:
    enabled: true
    schedule: "0 6 * * *"
```

The verify job downloads the latest backup, runs `pg_restore --list` to check integrity, and exits non-zero on corruption.

Check verification results:

```bash
kubectl get jobs -l app.kubernetes.io/component=backup-verify
kubectl logs job/aeterna-backup-verify-<timestamp>
```

## Qdrant Snapshots

For Qdrant vector data, snapshots are managed separately:

```yaml
qdrant:
  snapshots:
    enabled: true
    schedule: "0 3 * * *"
    destination: "s3://YOUR_BUCKET/qdrant-snapshots"
```

Restore Qdrant snapshots via the Qdrant API:

```bash
curl -X POST "http://qdrant:6333/collections/aeterna/snapshots/upload" \
  -H "Content-Type: multipart/form-data" \
  -F "snapshot=@/path/to/snapshot.tar"
```
