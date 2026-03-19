# Okta-Backed Authentication Deployment Guide

This guide describes the supported interactive authentication path for Aeterna when deploying browser access behind Okta-backed federation.

## Supported Authentication Model

Aeterna supports **Okta as the identity authority** for interactive user access.

- Users authenticate with **Okta**
- Google or GitHub identities are supported **only if they are federated upstream into Okta**
- Aeterna does **not** implement separate native Google or GitHub login flows
- Interactive browser traffic is terminated by a dedicated **oauth2-proxy** auth boundary deployed with the Helm chart
- Service-to-service and automation clients continue to use **API-key authentication**

## Architecture

```text
Browser
  -> Ingress (TLS)
  -> oauth2-proxy (/oauth2/*, Okta OIDC redirect/callback/session)
  -> Aeterna app routes (trusted identity headers)
  -> OPAL + Cedar Agent (runtime policy sync/evaluation)
  -> Postgres (memberships, assignments, tenant/governance data)
```

## What Lives Where

Operators should be explicit about the split between authentication, permission storage, and runtime authorization:

- **Okta** authenticates the user and emits trusted identity claims
- **Postgres-backed Aeterna data stores** hold memberships, role assignments, and tenant/governance data
- **Cedar policy files** define authorization rules
- **OPAL + Cedar Agent** synchronize and evaluate those policies and authorization inputs at runtime

OPAL is **not** the permissions database.

## Helm Configuration Surface

The supported Helm values live under `okta:`.

Key fields:

- `okta.enabled`
- `okta.issuerUrl`
- `okta.clientId`
- `okta.clientSecret` or `okta.existingSecret`
- `okta.redirectUrl`
- `okta.cookieDomains`
- `okta.whitelistDomains`
- `okta.allowedGroups`

Example:

```yaml
okta:
  enabled: true
  issuerUrl: "https://example.okta.com/oauth2/default"
  clientId: "aeterna-production"
  existingSecret: "aeterna-okta-auth"
  redirectUrl: "https://aeterna.example.com/oauth2/callback"
  cookieDomains:
    - ".example.com"
  whitelistDomains:
    - ".example.com"
  allowedGroups:
    - "aeterna-users"
```

See also:
- `charts/aeterna/examples/values-production.yaml`
- `charts/aeterna/ci/okta-values.yaml`

## Required Okta Setup

Create an Okta OIDC application or authorization-server integration with:

- Authorization Code flow
- Redirect URI: `https://<your-host>/oauth2/callback`
- Issuer URL matching `okta.issuerUrl`
- Stable subject claim (`sub`)
- Email claim
- Groups claim enabled for the roles you want to map into Aeterna

Recommended:

- Treat `sub` as the stable user identity
- Use Okta groups for **coarse** authorization inputs
- Keep fine-grained application permissions in Aeterna/Cedar rather than exploding them into IdP groups

## Upstream Federation from Google or GitHub

If users authenticate with Google Workspace or GitHub, configure that federation **inside Okta**.

Supported model:

- Google/GitHub -> Okta federation -> Okta-issued session/claims -> Aeterna

Unsupported model:

- Google -> Aeterna directly
- GitHub -> Aeterna directly

## Secrets

Use `okta.existingSecret` in production.

Expected keys:

- `client-secret`
- `cookie-secret`

If `okta.existingSecret` is omitted, the chart can create a secret, but operators should prefer a managed secret flow for production.

## Trusted Identity Contract

`oauth2-proxy` authenticates the user and forwards normalized identity headers to Aeterna.

Current app-side trusted identity mapping expects headers for:

- tenant
- user identifier
- email
- groups
- trusted proxy marker

Requests that do not satisfy the configured trusted header contract fail closed.

## Tenant and Role Mapping

Interactive trusted identity handling in `agent-a2a` currently supports:

- tenant derivation from configured mapping rules
- Okta group-to-role mapping from configured rules
- fail-closed rejection when tenant mapping or role mapping is missing

This gives Aeterna immediate role/policy inputs for interactive identity.

## Service-to-Service Separation

Interactive user SSO and machine authentication are intentionally separate.

- Browser users: Okta -> oauth2-proxy -> Aeterna
- Machine clients / automation: API key

Do not assume all clients should use the Okta ingress path.

## Operational Notes

- `okta.enabled` requires `aeterna.ingress.enabled=true`
- `okta.enabled` also requires `opal.enabled=true` for the supported production authorization path
- The chart validates these conditions and fails render/lint when they are not satisfied
- The ingress strips trusted auth headers before forwarding to reduce spoofing risk

## Verification Checklist

Before rollout:

1. `helm lint charts/aeterna -f charts/aeterna/ci/okta-values.yaml`
2. `helm template aeterna charts/aeterna -f charts/aeterna/ci/okta-values.yaml`
3. Confirm ingress TLS/cert-manager configuration
4. Confirm Okta redirect URI matches the public hostname
5. Confirm groups claim is emitted by Okta
6. Confirm OPAL + Cedar Agent are enabled for production authorization

## Test and Coverage Compliance

The repository already contains both unit and integration-style Rust test suites across major crates.

Examples:

- `agent-a2a/tests/a2a_test.rs`
- `cli/tests/cli_e2e_test.rs`
- `storage/tests/postgres_test.rs`
- `storage/tests/tenant_isolation_test.rs`
- `knowledge/tests/api_test.rs`
- `tools/tests/policy_tools_test.rs`
- `adapters/tests/rbac_matrix_test.rs`

Minimum coverage compliance is defined and enforced in three places:

- `specs/testing-requirements/spec.md`
- `Cargo.toml` (`[workspace.metadata.tarpaulin].fail-under = 80`)
- `tarpaulin.toml` (`fail-under = 80`)

CI enforcement lives in:

- `.github/workflows/ci.yml`

The coverage job runs:

```bash
cargo tarpaulin --out Html --out Json --timeout 300 --workspace --fail-under 80
```

If measured coverage drops below 80%, the CI job fails.

For this Okta/auth change specifically, targeted verification currently includes:

- `cargo test -p agent-a2a --test a2a_test -- --nocapture`
- `cargo test -p agent-a2a auth:: -- --nocapture`
- `helm lint charts/aeterna -f charts/aeterna/ci/okta-values.yaml`
- `helm template aeterna charts/aeterna -f charts/aeterna/ci/okta-values.yaml`

## Troubleshooting

### `okta.enabled` validation failures

Check that all of the following are set:

- `okta.issuerUrl`
- `okta.clientId`
- `okta.redirectUrl`
- `okta.clientSecret` or `okta.existingSecret`
- `aeterna.ingress.enabled=true`
- `opal.enabled=true`

### User authenticates but Aeterna returns unauthorized

Check:

- trusted identity headers are forwarded from oauth2-proxy
- tenant mapping resolves a non-empty tenant
- at least one Okta group maps to an Aeterna role
- ingress is not bypassing the proxy path

### Where are permissions stored?

Not in OPAL.

- memberships / assignments / hierarchy -> Postgres-backed Aeterna stores
- rules -> Cedar policy files
- synchronization / runtime evaluation -> OPAL + Cedar Agent
