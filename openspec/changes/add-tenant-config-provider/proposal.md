## Why

Tenant administration currently stores repository credential handles in isolated control-plane fields, while deployment secrets are managed separately through Helm values and environment-specific Kubernetes procedures. We need one tenant-scoped configuration model now so GlobalAdmin and TenantAdmin workflows can manage tenant settings consistently, keep secret values out of persisted control-plane records, and deploy the same tenant contract into Kubernetes without ad hoc per-environment glue.

## What Changes

- Add a tenant configuration provider capability that defines a canonical tenant configuration document plus secret references, separated from raw secret material.
- Introduce a provider abstraction for tenant configuration management so implementations can store tenant config and tenant secrets through different backends.
- Add a Kubernetes-backed provider implementation that stores tenant config in a ConfigMap named from the stable tenant ID and stores secret values in a paired Secret.
- Add platform-owned Git provider connection objects so reusable GitHub App connectivity can be shared with one or more tenants without duplicating certificate material in tenant-owned config.
- Add supported control-plane API and CLI workflows for GlobalAdmin and TenantAdmin to inspect, update, validate, and segregate tenant configuration and tenant secret references.
- Extend deployment behavior so tenant provisioning can materialize the Kubernetes ConfigMap/Secret convention through a private deployment repository.

## Capabilities

### New Capabilities
- `tenant-config-provider`: Canonical tenant configuration document, provider abstraction, Kubernetes ConfigMap/Secret implementation, and tenant-scoped secret-reference management.

### Modified Capabilities
- `deployment`: Deployment requirements change to support tenant-scoped Kubernetes config/secret materialization using the canonical provider contract.
- `multi-tenant-governance`: Tenant isolation requirements change to cover segregated tenant configuration ownership and secret-reference boundaries for GlobalAdmin and TenantAdmin workflows.

## Impact

- Affected code: tenant admin CLI/server surfaces, storage/provider abstractions, Helm chart templates/values, tenant provisioning flows, and deployment automation for a private environment repository.
- Affected APIs: tenant configuration CRUD/validation flows, tenant secret-reference administration, and deployment artifact generation for tenant-scoped ConfigMap/Secret resources.
- Affected systems: Kubernetes deployment topology, tenant bootstrap, secret handling, admin authorization boundaries, and E2E verification.
