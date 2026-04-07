## Context

A fresh Aeterna Helm deployment is broken out of the box: the plugin auth bootstrap endpoint returns HTTP 500 because `AETERNA_PLUGIN_AUTH_TENANT` is not set and no PlatformAdmin user exists in PostgreSQL. The operator must manually exec into the database to seed an initial admin — a chicken-and-egg problem since the API itself requires authentication.

The existing bootstrap function (`cli/src/server/bootstrap.rs:43`) initializes PostgreSQL schema and wires up services, but performs no user or role seeding. The deployment template (`charts/aeterna/templates/aeterna/deployment.yaml`) has no mapping for plugin auth env vars — those are injected from a manually created K8s secret today.

Two parallel user/role schema paths exist:
- **Core Aeterna**: `user_roles` table (TEXT-based IDs, created via `storage/src/postgres.rs`)
- **IDP/OPAL path**: `users` table (UUID, with `idp_provider`/`idp_subject`) + `memberships` + `v_user_permissions` view (via `idp-sync/src/github.rs`)

For a user to be authorized by Cedar, they must appear in `v_user_permissions`, which JOINs `users` -> `memberships` -> `organizational_units`.

## Goals / Non-Goals

**Goals:**
- A fresh `helm install` with admin bootstrap values produces an operational deployment with zero manual DB intervention
- The PlatformAdmin seeding is idempotent — safe across restarts and upgrades
- Plugin auth tenant resolution works immediately via Helm-wired `AETERNA_PLUGIN_AUTH_TENANT`
- [REDACTED_TENANT]-specific values (email, GitHub provider) live exclusively in the private deployment repo

**Non-Goals:**
- CRD-based or web-interface admin provisioning (deferred)
- Sub-chart fixes (deferred per user request)
- Modifying the OPAL fetcher or Cedar policy format
- Supporting multiple PlatformAdmins at bootstrap time (single initial admin is sufficient)

## Decisions

### 1. Bootstrap seeding runs inside the Rust server startup, not as a Helm Job

**Decision**: Add seeding logic after `postgres.initialize_schema().await?` in `bootstrap()` (`cli/src/server/bootstrap.rs:48`).

**Rationale**: The seeding must happen before the HTTP server starts accepting traffic (chicken-and-egg). A Helm Job would race with the Deployment. Placing it in the existing bootstrap function guarantees ordering and leverages the already-established `PostgresBackend` connection.

**Alternatives considered**:
- Helm pre-install/pre-upgrade Job: Adds complexity (separate image, DB credentials duplication), races with Deployment startup
- Init container with psql: Requires PostgreSQL client image, credential plumbing, SQL maintenance separate from Rust code
- Existing `job-migration.yaml`: Could work but the migration job runs schema DDL, not seed data — mixing concerns

### 2. Seed into both `users` table and `user_roles` table

**Decision**: Insert into the `users` table (UUID id, email, idp_provider, idp_subject) from the IDP schema AND into the `user_roles` table (TEXT-based IDs) from the core schema. Also ensure the `default` company exists in `organizational_units` and a membership record exists so the user appears in `v_user_permissions`.

**Rationale**: The Cedar authorization path reads `v_user_permissions`, which JOINs `users` -> `memberships` -> `organizational_units`. If the admin doesn't appear there, Cedar denies all access even though `user_roles` has the grant. Both schema paths must be populated for the system to work end-to-end.

**Alternatives considered**:
- Seed only `user_roles`: Would break Cedar authorization path
- Seed only `users` + `memberships`: Would break core Aeterna role checks
- Trigger IDP sync to populate: Requires GitHub org access configured, adds latency, may not produce PlatformAdmin role

### 3. New `AdminBootstrapConfig` struct in the config crate

**Decision**: Add a new `AdminBootstrapConfig` with fields: `enabled`, `email`, `provider` (github/okta/local), `provider_subject`, loaded from `AETERNA_ADMIN_BOOTSTRAP_*` env vars.

**Rationale**: Follows the existing pattern where each feature has its own config struct (see `PluginAuthConfig`). Keeps bootstrap config separate from plugin auth config. The `enabled` flag allows disabling bootstrap seeding explicitly.

**Alternatives considered**:
- Reuse `PluginAuthConfig`: Conflates two different concerns (auth token issuance vs user seeding)
- Inline env reads in bootstrap.rs: Violates the project's config-loading pattern where all env parsing happens in `config/src/loader.rs`

### 4. Wire `AETERNA_PLUGIN_AUTH_TENANT` and admin bootstrap env vars via Helm values, not via external secret references

**Decision**: Add `pluginAuth.defaultTenantId` and `adminBootstrap.*` values in `values.yaml`. Map them to env vars in the deployment template. For secrets (like future provider credentials), use existing `existingSecret` pattern.

**Rationale**: The deployment template already wires dozens of env vars from values. Plugin auth env vars being outside Helm is an oversight, not a design choice. Making them Helm-native enables operators to configure everything declaratively.

**Alternatives considered**:
- Continue using manually created K8s secrets: Works but breaks the "operational from first install" goal
- ExternalSecret CRD: Adds dependency on external-secrets operator; overkill for a single env var

### 5. Do NOT create a new SQL migration — use runtime upsert

**Decision**: The bootstrap seeding uses `INSERT ... ON CONFLICT DO NOTHING` at runtime, not a new numbered migration file.

**Rationale**: The tables already exist (created by migration 009 and `postgres.rs`). The seed data is environment-specific (different admin email per deployment). SQL migrations are for schema, not environment-specific data. Runtime upsert is idempotent and configuration-driven.

**Alternatives considered**:
- New migration file with seed data: Hardcodes admin identity into the codebase — violates the "no leak" constraint
- Seed SQL in a ConfigMap: Extra template, extra coordination, harder to make idempotent

## Risks / Trade-offs

- **Race with IDP sync**: If GitHub org sync runs concurrently and creates the same user, the `ON CONFLICT DO NOTHING` ensures no crash, but the admin might temporarily lack the PlatformAdmin role until the bootstrap path completes. **Mitigation**: Bootstrap runs synchronously before the HTTP server starts; IDP sync starts after.

- **Stale bootstrap config**: If the admin email changes in Helm values, the old admin row persists. **Mitigation**: Use `ON CONFLICT (email) DO UPDATE` for the `users` table to update provider fields. The `user_roles` grant is additive and idempotent.

- **Two-schema consistency**: Keeping `users`/`memberships` and `user_roles` in sync is fragile. **Mitigation**: The bootstrap function seeds both atomically in a single transaction. Long-term, the two schema paths should converge.

- **No leak to public repo**: Admin email and provider details must never appear in the public Aeterna chart values. **Mitigation**: Chart `values.yaml` has empty defaults; real values live only in `[REDACTED_PRIVATE_DEPLOYMENT_REPO]`.

## Migration Plan

1. Merge config + bootstrap Rust changes, build new container image
2. Update Helm chart templates and values.yaml with admin bootstrap section
3. Update [REDACTED_TENANT] deployment values with `[REDACTED_EMAIL]` + GitHub provider config
4. `helm upgrade` on [REDACTED_ENV] — bootstrap seeds admin on next pod start
5. Verify: plugin auth bootstrap endpoint returns 200, E2E passes
6. **Rollback**: `helm rollback` to previous revision removes the env vars; the seeded DB rows are harmless
