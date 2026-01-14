## 1. Project Setup
- [ ] 1.1 Create `charts/aeterna/` directory structure
- [ ] 1.2 Create `Chart.yaml` with metadata and dependencies
- [ ] 1.3 Create `values.yaml` with full schema
- [ ] 1.4 Create `values.schema.json` for IDE validation
- [ ] 1.5 Create `.helmignore` file

## 2. Core Templates
- [ ] 2.1 Create `_helpers.tpl` with naming/labeling helpers
- [ ] 2.2 Create `deployment.yaml` for Aeterna server
- [ ] 2.3 Create `service.yaml` for ClusterIP service
- [ ] 2.4 Create `configmap.yaml` for non-sensitive config
- [ ] 2.5 Create `secret.yaml` with password generation
- [ ] 2.6 Create `serviceaccount.yaml`

## 3. Health and Scaling
- [ ] 3.1 Add liveness probe to deployment
- [ ] 3.2 Add readiness probe to deployment
- [ ] 3.3 Add startup probe to deployment
- [ ] 3.4 Create `hpa.yaml` for horizontal pod autoscaling
- [ ] 3.5 Create `pdb.yaml` for pod disruption budget

## 4. Networking
- [ ] 4.1 Create `ingress.yaml` with TLS support
- [ ] 4.2 Create `networkpolicy.yaml` for isolation
- [ ] 4.3 Add Ingress class detection logic

## 5. Dependency Subcharts
- [ ] 5.1 Add Bitnami Redis as optional dependency
- [ ] 5.2 Add Bitnami PostgreSQL as optional dependency
- [ ] 5.3 Add Qdrant as optional dependency
- [ ] 5.4 Configure subchart value passthrough
- [ ] 5.5 Create conditional logic for external vs bundled

## 6. Observability
- [ ] 6.1 Create `servicemonitor.yaml` for Prometheus
- [ ] 6.2 Add metrics port to service
- [ ] 6.3 Document Grafana dashboard import

## 7. Security
- [ ] 7.1 Add securityContext to pods
- [ ] 7.2 Add RBAC roles if needed
- [ ] 7.3 Configure non-root user
- [ ] 7.4 Add read-only root filesystem option

## 8. Configuration Validation
- [ ] 8.1 Add validation templates for required fields
- [ ] 8.2 Add validation for mutual exclusivity
- [ ] 8.3 Create `NOTES.txt` with post-install instructions

## 9. Testing
- [ ] 9.1 Create `tests/` directory with helm test pods
- [ ] 9.2 Write connection test for each dependency
- [ ] 9.3 Test with only external dependencies
- [ ] 9.4 Test with only bundled dependencies
- [ ] 9.5 Test mixed configuration

## 10. Documentation
- [ ] 10.1 Create `charts/aeterna/README.md` with full reference
- [ ] 10.2 Create `values-local.yaml` example
- [ ] 10.3 Create `values-production.yaml` example
- [ ] 10.4 Add upgrade notes for version migrations

## 11. CI/CD
- [ ] 11.1 Add chart linting to CI
- [ ] 11.2 Add helm template validation
- [ ] 11.3 Add chart publishing workflow

---

## 12. Production Gap Requirements

### 12.1 Secure Secret Management (HC-C1) - CRITICAL
- [ ] 12.1.1 Add `secretProvider` value with options: `helm`, `sops`, `external-secrets`
- [ ] 12.1.2 Create SOPS values example file `values-sops.yaml`
- [ ] 12.1.3 Add External Secrets Operator template `externalsecret.yaml`
- [ ] 12.1.4 Implement secret checksum annotation for rolling restarts on secret change
- [ ] 12.1.5 Add `existingSecret` option for all components (redis, postgresql, qdrant)
- [ ] 12.1.6 Document SOPS setup with age/GPG keys
- [ ] 12.1.7 Document HashiCorp Vault integration
- [ ] 12.1.8 Document AWS Secrets Manager integration
- [ ] 12.1.9 Add secret rotation guide to documentation

### 12.2 Subchart Version Pinning (HC-C2) - CRITICAL
- [ ] 12.2.1 Pin Bitnami Redis to exact version (e.g., `18.6.1` not `^18.0.0`)
- [ ] 12.2.2 Pin Bitnami PostgreSQL to exact version
- [ ] 12.2.3 Pin Qdrant to exact version
- [ ] 12.2.4 Document pinned versions in CHANGELOG.md
- [ ] 12.2.5 Create subchart upgrade procedure documentation
- [ ] 12.2.6 Add subchart compatibility matrix to README

### 12.3 Pod Disruption Budget Configuration (HC-H1) - HIGH
- [ ] 12.3.1 Create `pdb.yaml` template with `minAvailable: 1` default
- [ ] 12.3.2 Add `podDisruptionBudget.minAvailable` config option
- [ ] 12.3.3 Add `podDisruptionBudget.maxUnavailable` as alternative
- [ ] 12.3.4 Document safe node drain procedure
- [ ] 12.3.5 Create pre-drain checklist in docs
- [ ] 12.3.6 Add PDB for each component (Aeterna, Redis, PostgreSQL)

### 12.4 Network Policy Completeness (HC-H2) - HIGH
- [ ] 12.4.1 Create `networkpolicy-aeterna.yaml` with ingress rules
- [ ] 12.4.2 Create `networkpolicy-redis.yaml` restricting to Aeterna only
- [ ] 12.4.3 Create `networkpolicy-postgresql.yaml` restricting to Aeterna only
- [ ] 12.4.4 Create `networkpolicy-qdrant.yaml` restricting to Aeterna only
- [ ] 12.4.5 Add egress rules for DNS resolution (port 53)
- [ ] 12.4.6 Add conditional Ingress controller traffic allowance
- [ ] 12.4.7 Document network policy configuration options
- [ ] 12.4.8 Add network policy test scenarios

### 12.5 Backup and Restore (HC-H3) - HIGH
- [ ] 12.5.1 Create `cronjob-backup.yaml` for PostgreSQL pg_dump
- [ ] 12.5.2 Add S3 backup destination configuration
- [ ] 12.5.3 Add GCS backup destination configuration
- [ ] 12.5.4 Add Azure Blob backup destination configuration
- [ ] 12.5.5 Document CloudNativePG backup integration
- [ ] 12.5.6 Create restore procedure documentation
- [ ] 12.5.7 Add point-in-time recovery guide
- [ ] 12.5.8 Add backup verification job (restore test)

### 12.6 Resource Limit Guidance (HC-H4) - HIGH
- [ ] 12.6.1 Create `values-small.yaml` for small deployments (<1000 memories)
- [ ] 12.6.2 Create `values-medium.yaml` for medium deployments (<100k memories)
- [ ] 12.6.3 Create `values-large.yaml` for large deployments (>100k memories)
- [ ] 12.6.4 Add sizing guide documentation with memory/CPU recommendations
- [ ] 12.6.5 Create `vpa.yaml` template for VerticalPodAutoscaler
- [ ] 12.6.6 Add `vpa.enabled` config option
- [ ] 12.6.7 Document VPA setup and recommendations

### 12.7 Multi-Region Architecture Documentation (HC-H5) - HIGH
- [ ] 12.7.1 Create `docs/multi-region.md` architecture guide
- [ ] 12.7.2 Document active-passive multi-region pattern
- [ ] 12.7.3 Document active-active limitations with current design
- [ ] 12.7.4 Add federation roadmap notes
- [ ] 12.7.5 Document cross-region PostgreSQL replication options

### 12.8 Redis Alternative Compatibility (HC-H6) - HIGH
- [ ] 12.8.1 Add `redis.alternative` config option (redis, dragonfly, keydb)
- [ ] 12.8.2 Create Dragonfly subchart configuration
- [ ] 12.8.3 Create KeyDB subchart configuration
- [ ] 12.8.4 Add compatibility test for Dragonfly
- [ ] 12.8.5 Add compatibility test for KeyDB
- [ ] 12.8.6 Document Redis vs Dragonfly feature differences
- [ ] 12.8.7 Document Redis vs KeyDB feature differences

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 5 | Project Setup |
| 2 | 6 | Core Templates |
| 3 | 5 | Health and Scaling |
| 4 | 3 | Networking |
| 5 | 5 | Dependency Subcharts |
| 6 | 3 | Observability |
| 7 | 4 | Security |
| 8 | 3 | Configuration Validation |
| 9 | 5 | Testing |
| 10 | 4 | Documentation |
| 11 | 3 | CI/CD |
| 12 | 56 | Production Gap Requirements (HC-C1 to HC-H6) |
| **Total** | **102** | |

**Estimated effort**: 4-5 weeks with 80% test coverage target

---

## Production Gap Tracking

| Gap ID | Priority | Requirement | Tasks |
|--------|----------|-------------|-------|
| HC-C1 | Critical | Secure Secret Management | 12.1.1-12.1.9 |
| HC-C2 | Critical | Subchart Version Pinning | 12.2.1-12.2.6 |
| HC-H1 | High | Pod Disruption Budget Configuration | 12.3.1-12.3.6 |
| HC-H2 | High | Network Policy Completeness | 12.4.1-12.4.8 |
| HC-H3 | High | Backup and Restore | 12.5.1-12.5.8 |
| HC-H4 | High | Resource Limit Guidance | 12.6.1-12.6.7 |
| HC-H5 | High | Multi-Region Architecture Documentation | 12.7.1-12.7.5 |
| HC-H6 | High | Redis Alternative Compatibility | 12.8.1-12.8.7 |
