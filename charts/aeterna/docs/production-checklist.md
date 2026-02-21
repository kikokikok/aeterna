# Production Deployment Checklist

This checklist provides a set of actionable items to ensure the Aeterna Helm chart is deployed with production-grade security, high availability, and observability.

## Infrastructure
- [ ] Kubernetes cluster running version 1.25+
- [ ] At least 3 worker nodes distributed across different availability zones
- [ ] Default storage class configured with `volumeBindingMode: WaitForFirstConsumer`
- [ ] Ingress controller installed and operational (e.g., ingress-nginx, AWS ALB)
- [ ] cert-manager installed for automated TLS certificate management
- [ ] Prometheus Operator installed to support `ServiceMonitor` resources

## Security
- [ ] TLS enabled on ingress with a valid certificate from a trusted CA
- [ ] Network policies enabled (`networkPolicy.enabled: true`) to restrict traffic
- [ ] All sensitive credentials use `existingSecret` instead of plaintext values
- [ ] Pod security standards enforced using the `restricted` profile
- [ ] Service accounts configured with minimal RBAC permissions
- [ ] Image pull secrets configured for private registries
- [ ] Container images scanned for vulnerabilities in the CI/CD pipeline
- [ ] Read-only root filesystem enabled for all containers (default in chart)
- [ ] Non-root user execution enforced for all containers (default in chart)

## High Availability
- [ ] Aeterna replicas set to 3 or more with pod anti-affinity rules
- [ ] Horizontal Pod Autoscaler (HPA) enabled with appropriate CPU/Memory thresholds
- [ ] Pod Disruption Budget (PDB) configured (e.g., `minAvailable: 2`)
- [ ] Topology spread constraints used to distribute pods across zones
- [ ] PostgreSQL instances set to 3 or more using CloudNativePG HA
- [ ] Qdrant replicas set to 3 or more for vector storage redundancy
- [ ] OPAL Server replicas set to 2 or more for policy distribution

## Data Protection
- [ ] PostgreSQL backups enabled with a defined cron schedule
- [ ] Backup destination configured to an external store (S3, GCS, or Azure Blob)
- [ ] Backup retention policy defined and applied
- [ ] Backup verification jobs enabled to ensure data restorability
- [ ] Qdrant snapshots enabled and stored externally
- [ ] Disaster recovery procedure documented and tested in a staging environment

## Observability
- [ ] ServiceMonitor enabled with labels matching your Prometheus instance
- [ ] Distributed tracing enabled with a valid OpenTelemetry endpoint
- [ ] Log aggregation configured (e.g., Grafana Loki, ELK stack)
- [ ] Alerting rules defined for key metrics (latency, error rate, saturation)
- [ ] Grafana dashboards imported for Aeterna and its dependencies
- [ ] Log level set to `info` or `warn` (avoid `debug` in production)

## Resources
- [ ] Resource requests and limits explicitly set for all containers
- [ ] Storage volumes sized based on the sizing guide and expected growth
- [ ] Node selectors or tolerations configured if using dedicated node pools

## Configuration
- [ ] `deploymentMode` set correctly based on the environment (local, hybrid, or remote)
- [ ] LLM provider configured using an `existingSecret` for API keys
- [ ] Vector backend configured for production scale (e.g., Qdrant HA)
- [ ] Redis/Dragonfly cache sized appropriately for the workload
- [ ] Sync settings tuned for low-latency memory-knowledge bridging

## Pre-Deployment
- [ ] `helm lint` passes without any errors or warnings
- [ ] `helm template` renders successfully with production values
- [ ] Values file validated against the `values.schema.json`
- [ ] Dry-run deployment (`helm install --dry-run`) completes successfully
- [ ] Rollback procedure documented and verified

## Post-Deployment
- [ ] All pods reach the `Running` state and pass readiness probes
- [ ] `helm test` execution passes (confirms internal connectivity)
- [ ] Health endpoints (`/health/ready`, `/health/live`) responding with 200 OK
- [ ] Metrics visible in Prometheus and dashboards populating in Grafana
- [ ] Ingress endpoint accessible over HTTPS with a valid certificate
- [ ] First memory write and read operation successful through the API

## Quick Validation

Run these commands after deployment to verify the health of the installation.

### 1. Check Helm Release Status
```bash
helm status aeterna -n aeterna
```

### 2. Verify Pod Readiness
```bash
kubectl get pods -n aeterna -l app.kubernetes.io/instance=aeterna
```

### 3. Check High Availability Components
```bash
# Verify HPA
kubectl get hpa -n aeterna -l app.kubernetes.io/instance=aeterna

# Verify PDB
kubectl get pdb -n aeterna -l app.kubernetes.io/instance=aeterna
```

### 4. Verify External Connectivity
```bash
# Replace <host> with your configured ingress host
curl -f -k https://<host>/health/ready
```

### 5. Run Integration Tests
```bash
helm test aeterna -n aeterna
```

### 6. Check Infrastructure Logs
```bash
# Check Aeterna logs for errors
kubectl logs -n aeterna -l app.kubernetes.io/name=aeterna --tail=100

# Check PostgreSQL cluster status (if using CloudNativePG)
kubectl get cluster -n aeterna aeterna-postgresql
```
