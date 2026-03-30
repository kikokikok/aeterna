## Why

Aeterna currently supports service-to-service API keys, policy engines, and identity-provider sync, but it does not provide a real end-user login path for browser or dashboard access. Corporate deployment requires users to authenticate with their existing Okta-backed identity so the system can identify the actor, map trusted group claims to Aeterna roles, and enforce authorization consistently.

## What Changes

- Add Okta-based federated user authentication for product access, with Okta acting as the identity authority
- Support Google or GitHub identities only when they are federated upstream into Okta, rather than implementing separate provider-specific login flows in Aeterna
- Introduce a supported login architecture that terminates user authentication at a trusted auth layer and propagates normalized user identity claims to Aeterna services
- Add authorization mapping rules that convert trusted Okta identity and group claims into Aeterna tenant context, roles, and policy inputs
- Document the required deployment, secret, issuer, callback, and claim configuration needed to operate Okta-backed authentication in supported environments

## Capabilities

### New Capabilities
- `user-auth`: End-user authentication using Okta-backed federated identity, normalized identity claims, and trusted session propagation into Aeterna

### Modified Capabilities
- `deployment`: Add supported deployment requirements for Okta-backed authentication components, trusted ingress/auth integration, and secret/configuration handling
- `multi-tenant-governance`: Change tenant context and authorization requirements so authenticated user identity and Okta group claims can be mapped into roles and policy evaluation

## Impact

- Affected code:
  - `agent-a2a/`
  - `charts/aeterna/`
  - `deploy/`
  - `docs/`, `README.md`, `INSTALL.md`
- Affected systems:
  - ingress/authentication layer for browser access
  - tenant context extraction and authorization mapping
  - Helm values, secrets, and deployment guidance for Okta issuer/client configuration
- External dependencies:
  - Okta OIDC / authorization server configuration
  - optional auth proxy or identity broker components used to normalize trusted identity headers/claims
