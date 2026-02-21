# Upgrade Guide

## Standard Upgrade Procedure

### Pre-Upgrade Checks
1. Check current release: `helm list -n aeterna`
2. Review changelog for breaking changes
3. Compare values: `helm get values aeterna -n aeterna > current-values.yaml`
4. Back up database before upgrade
5. Check cluster resources for new requirements

### Upgrade Steps
```bash
# Update repo
helm repo update

# Review changes (dry-run)
helm upgrade aeterna aeterna/aeterna -f values.yaml --dry-run --debug

# Perform upgrade
helm upgrade aeterna aeterna/aeterna -f values.yaml

# Verify
kubectl rollout status deployment/aeterna -n aeterna
helm test aeterna -n aeterna
```

### Post-Upgrade Verification
- Check all pods running
- Verify health endpoints
- Check metrics flowing
- Test memory operations

## Version Migration Notes

### 0.1.x → 0.2.x (placeholder)
Document the pattern for future migration notes:
- Breaking changes
- New required values
- Deprecated values
- Migration steps

### Values Schema Changes
How to diff values schemas between versions.

## Subchart Upgrades

### CloudNativePG (PostgreSQL)
- Check cnpg operator compatibility
- Review PostgreSQL version changes
- Backup before upgrade
- Monitor replication status

### Qdrant
- Check API compatibility
- Review storage format changes
- Snapshot before upgrade

### Dragonfly
- Check Redis protocol compatibility
- Review memory behavior changes

### OPAL Stack
- Check policy format compatibility
- Review Cedar schema changes

## Rollback Procedure

### Quick Rollback
```bash
# View history
helm history aeterna -n aeterna

# Rollback to previous
helm rollback aeterna -n aeterna

# Rollback to specific revision
helm rollback aeterna 3 -n aeterna
```

### When Rollback Fails
- Manual pod restart
- PVC recovery
- Database rollback considerations (migrations are NOT automatically reversed)

### Rollback Limitations
- Database migrations are forward-only
- PVC data may have been modified
- External service configurations won't rollback

## Blue-Green Upgrade Strategy
For zero-downtime upgrades:
1. Deploy new release to separate namespace
2. Run smoke tests
3. Switch ingress
4. Decommission old release

## Canary Upgrade Strategy
Using Argo Rollouts or Flagger (brief overview).

## Emergency Procedures
- How to quickly disable a broken component
- How to scale down to minimum
- How to switch from bundled to external services
