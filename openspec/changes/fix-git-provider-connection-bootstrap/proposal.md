## Why

Shared Git provider connections are meant to be declared once at the platform layer and then referenced from tenant manifests by stable identifier. Today the create API always generates a UUID and the Helm-rendered connection ConfigMap is never consumed, so declarative tenant provisioning cannot resolve `gitProviderConnectionId` values such as `shared-github-app`.

## What Changes

- Add optional explicit IDs to shared Git provider connection creation so platform-owned connections can use stable declarative identifiers.
- Reject invalid or duplicate explicit IDs while preserving UUID generation when the caller omits `id`.
- Bootstrap shared Git provider connections from the chart-rendered `connections.json` during server startup and reconcile tenant visibility from the declared allow-list.
- Mount the chart-rendered connection seed file into the server deployment and document the bootstrap contract.

## Capabilities

### New Capabilities

- `git-provider-connection-bootstrap`: Stable shared Git provider connection identifiers, duplicate-safe creation semantics, and chart-driven bootstrap seeding for platform-owned connections.

### Modified Capabilities

- `tenant-provisioning`: Tenant repository bindings can rely on platform-bootstrapped shared Git provider connection IDs during manifest apply.

## Impact

- Affected code: `cli/src/server/tenant_api.rs`, `cli/src/server/bootstrap.rs`, `storage/src/git_provider_connection_store.rs`, `mk_core/src/types.rs`, chart templates under `charts/aeterna/templates/aeterna/`, and runtime tests.
- Affected APIs: `POST /api/v1/admin/git-provider-connections` accepts optional `id` and returns validation errors for invalid or duplicate IDs.
- Affected systems: shared Git provider connection registry bootstrap, Helm deployment wiring, and tenant manifest repository-binding flows.
