# Change: Helm Chart for Kubernetes Deployment

## Why

The Aeterna Memory-Knowledge system requires a standardized, production-ready deployment mechanism for Kubernetes environments. Teams need flexibility to either deploy dependencies (Redis, PostgreSQL, Qdrant) as part of the chart or reference existing infrastructure.

## What Changes

- Add Helm chart with configurable dependency deployment
- Support for external Redis, PostgreSQL, and Qdrant instances
- Support for bundled dependencies using Bitnami subcharts
- Horizontal Pod Autoscaling (HPA) configuration
- Service accounts, RBAC, and network policies
- ConfigMaps and Secrets management
- Health checks (liveness, readiness, startup probes)
- Ingress configuration with TLS support
- Prometheus ServiceMonitor for observability

## Impact

- Affected specs: New `helm-deployment` capability
- Affected code: New `charts/aeterna/` directory
- Dependencies: Dragonfly (Redis-compatible), CloudNativePG, Qdrant Helm charts (all Apache 2.0 / open source)
