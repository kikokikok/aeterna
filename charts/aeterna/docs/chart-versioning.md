# Chart Versioning Strategy

## Overview

The Aeterna Helm chart uses [Semantic Versioning 2.0.0](https://semver.org/) (SemVer) for both the chart version and the application version.

## Version Fields

| Field | Location | Purpose |
|-------|----------|---------|
| `version` | `Chart.yaml` | Helm chart version — tracks chart-level changes (templates, values, dependencies) |
| `appVersion` | `Chart.yaml` | Application version — tracks the Aeterna server binary version |

These versions are **independent**. A chart template fix bumps `version` without changing `appVersion`.

## SemVer Rules

```
MAJOR.MINOR.PATCH
```

| Component | When to bump | Examples |
|-----------|-------------|----------|
| **MAJOR** | Breaking changes to `values.yaml` schema, removed parameters, renamed keys, incompatible subchart upgrades | Renaming `cache.dragonfly` → `cache.redis`, removing a supported vector backend |
| **MINOR** | New features, new templates, new optional values, new subchart dependencies, new deployment modes | Adding VPA support, adding Valkey subchart, new `values-large.yaml` |
| **PATCH** | Bug fixes, documentation updates, dependency patch bumps, label corrections | Fixing a template rendering bug, updating CNPG from 0.23.1 to 0.23.2 |

## Pre-release Versions

During active development, use pre-release suffixes:

```
0.1.0-alpha.1   # Early development, API unstable
0.1.0-beta.1    # Feature-complete, testing
0.1.0-rc.1      # Release candidate
0.1.0           # Stable release
```

## Release Process

1. **Develop**: Work on feature branch, chart version stays at current + `-dev` suffix
2. **PR Review**: Validate with `helm lint`, `helm template`, schema validation
3. **Tag**: Create a git tag matching the chart version: `helm-chart-v0.2.0`
4. **Publish**: CI pipeline packages and publishes to:
   - GitHub Pages Helm repository: `https://kikokikok.github.io/aeterna`
   - OCI registry: `oci://ghcr.io/kikokikok/aeterna/helm/aeterna`
5. **Release Notes**: Auto-generated changelog from conventional commits

## Subchart Version Pinning

All subchart dependencies use wildcard patch versions to allow automatic security fixes while preventing breaking changes:

```yaml
dependencies:
  - name: cloudnative-pg
    version: "0.23.*"    # Any 0.23.x patch
  - name: qdrant
    version: "0.10.*"    # Any 0.10.x patch
```

When a subchart releases a new **minor** version:
1. Test the upgrade in a staging environment
2. Update `Chart.yaml` with the new minor range
3. Document migration steps in release notes
4. Bump the Aeterna chart **minor** version

## Compatibility Matrix

| Chart Version | K8s Version | Helm Version | App Version |
|---------------|-------------|--------------|-------------|
| 0.1.x | 1.27+ | 3.12+ | 0.1.x |

## Rollback Procedure

```bash
# View release history
helm history aeterna -n aeterna

# Rollback to previous revision
helm rollback aeterna <REVISION> -n aeterna

# Verify rollback
helm status aeterna -n aeterna
kubectl get pods -n aeterna
```

Rollback automatically reverts both chart templates and application version. Database migrations are **not** automatically rolled back — see [restore procedure](backup-restore.md) for data recovery.
