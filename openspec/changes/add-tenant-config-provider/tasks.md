## 1. Canonical tenant config contract

- [x] 1.1 Add typed tenant config and tenant secret-reference models with explicit ownership/segregation metadata.
- [x] 1.2 Add a `TenantConfigProvider` trait for read/write/list/validate operations over tenant config and tenant secret entries.
- [x] 1.3 Add provider-level validation rules that reject cross-tenant secret references and raw secret material in config payloads.

## 2. Kubernetes provider implementation

- [x] 2.1 Implement a Kubernetes-backed tenant config provider that stores tenant config in `aeterna-tenant-<tenant-id>` ConfigMaps.
- [x] 2.2 Implement paired tenant Secret storage in `aeterna-tenant-<tenant-id>-secret` Secrets and expose only logical secret references through the API.
- [x] 2.3 Add tests for CRUD, segregation, validation, and redaction behavior of the Kubernetes provider.

## 3. Control-plane API and CLI

- [x] 3.1 Add server-backed tenant config endpoints for inspect, upsert, validate, and tenant-secret mutation workflows.
- [x] 3.2 Add supported CLI flows for GlobalAdmin and TenantAdmin to manage tenant config and tenant secret references honestly.
- [x] 3.3 Ensure role boundaries prevent TenantAdmin from mutating platform-owned or cross-tenant config surfaces.
- [x] 3.4 Add shared Git provider connection metadata, tenant visibility rules, and tenant-side assignment flows for supported GitHub connectivity.

## 4. Deployment integration and verification

- [x] 4.1 Extend Helm/deployment assets to render or consume the tenant ConfigMap/Secret convention.
- [x] 4.2 Integrate a private deployment repo and environment overlays with the canonical tenant config artifacts.
- [x] 4.3 Add end-to-end coverage for tenant config provisioning, secret administration, deployment materialization, and tenant bootstrap flows.
