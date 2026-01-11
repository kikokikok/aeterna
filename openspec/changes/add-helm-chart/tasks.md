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
