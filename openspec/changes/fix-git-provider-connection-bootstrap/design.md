## Context

Aeterna already models shared Git provider connections as platform-owned records referenced by tenant repository bindings. The deployment chart renders `gitProviderConnections` into a ConfigMap with stable IDs and allow-lists, but the runtime never reads that ConfigMap. Meanwhile the admin create API always overwrites caller intent with a generated UUID, so operators cannot seed a stable ID such as `shared-github-app` even manually.

The fix has to work for both one-off control-plane API usage and declarative chart-based bootstrapping. It also has to remain backward compatible for callers that do not care about explicit IDs.

## Goals / Non-Goals

**Goals:**

- Allow platform admins to create shared Git provider connections with an explicit stable ID.
- Reject invalid or duplicate IDs before they become ambiguous runtime state.
- Bootstrap chart-declared shared connections into the runtime registry on startup.
- Apply the chart allow-list on first creation while preserving runtime/UI-managed tenant visibility for existing shared connections.

**Non-Goals:**

- Add a full mutable update API for shared Git provider connections.
- Persist chart bootstrap metadata anywhere other than the existing registry.
- Change tenant manifest schema beyond relying on existing `gitProviderConnectionId` behavior.

## Decisions

### 1. Make explicit IDs optional on the existing create API

The least disruptive change is to extend `CreateGitProviderConnectionRequest` with `id: Option<String>`. When omitted, the server preserves current UUID generation. When provided, the server persists the caller-supplied ID.

**Alternatives considered**

- Separate "create-with-id" endpoint: rejected because it splits one concept across two APIs.
- Force explicit IDs for every create: rejected for backward compatibility and operator ergonomics.

### 2. Validate IDs with the same slug-safe shape used elsewhere

Explicit IDs should be lowercase letters, digits, and hyphens with no leading or trailing hyphen. This admits both human-readable IDs (`shared-github-app`) and generated UUIDs.

**Alternatives considered**

- Allow arbitrary strings: rejected because manifests, CLI flags, and chart values need predictable identifier shapes.
- Restrict to alpha-starting slugs: rejected because generated UUIDs commonly start with digits.

### 3. Enforce uniqueness in the store and treat bootstrap as create-once metadata seeding

`create_connection` should fail when an ID already exists. Startup bootstrap will not blindly call create. Instead it will load each declared connection, fetch any existing record, compare immutable metadata, and preserve the existing runtime allow-list for already-created connections. The chart allow-list is applied only when the connection is first created.

**Alternatives considered**

- Keep create as blind upsert: rejected because API callers would silently overwrite existing records.
- Add a full update API in this change: rejected as useful but not required to unblock declarative bootstrapping.

### 4. Bootstrap from a mounted JSON file during server startup

The chart already renders a canonical `connections.json`. The simplest reliable bootstrap path is to mount that file into the server pod and let bootstrap code seed the registry before request handling starts. This avoids requiring a second hook job with auth/bootstrap sequencing.

**Alternatives considered**

- Post-install Job calling the HTTP API: rejected for now because it needs auth, readiness sequencing, and more drift-handling logic.
- Ignore chart values and require manual API creation: rejected because it breaks declarative deployment.

## Risks / Trade-offs

- **Startup fails on config drift** → If a bootstrapped ID already exists with different immutable metadata, startup will fail fast instead of silently mutating it.
- **Allow-list source of truth is split by lifecycle** → The chart owns initial visibility only; later grants/revokes happen through the runtime API/UI and are intentionally preserved.
- **No metadata update path yet** → Operators must delete/recreate a connection or add a future update API for app/install/Pem ref changes.
- **Multi-replica startup races** → Bootstrap reads before create and tolerates already-existing matching records, which keeps restarts safe.

## Migration Plan

1. Deploy the API/store changes and startup bootstrap support.
2. Upgrade the chart so the deployment mounts `connections.json` and sets the bootstrap env var.
3. Restart pods; startup seeds any missing shared connections and applies the chart allow-list only to newly created connections.
4. Re-run tenant manifest apply using stable `gitProviderConnectionId` values.
5. Rollback by removing the env var/volume mount and reverting the server binary; already-created connection records remain valid.

## Open Questions

- Should a future follow-up add `PUT /admin/git-provider-connections/{id}` for metadata reconciliation instead of fail-fast drift detection?
- Should bootstrap emit a dedicated readiness/audit event when seed reconciliation changes allow-lists?
