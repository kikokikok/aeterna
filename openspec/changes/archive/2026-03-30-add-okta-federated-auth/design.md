## Context

Aeterna currently supports service-to-service API keys, authorization engines, and identity-provider sync, but it does not provide an end-user login flow. The `agent-a2a` service only accepts a static Bearer API key and explicitly rejects JWT mode because the in-process JWT path is not implemented. The Helm chart exposes a plain ingress and generic annotation passthrough, but it does not model any supported user-authentication architecture, Okta issuer configuration, callback path, or trusted identity propagation.

Corporate usage requires browser and dashboard users to authenticate with their existing Okta-managed identity. Google or GitHub identities may still appear, but only through upstream federation into Okta. Aeterna must consume a normalized, trusted identity surface so permissions and policy evaluation can be configured from a consistent source of truth rather than per-provider logic.

This change crosses deployment, ingress/auth, tenant-context propagation, and authorization mapping. It also has security implications because identity headers must only be trusted when they originate from the supported auth layer.

## Goals / Non-Goals

**Goals:**
- Establish a supported end-user authentication architecture with Okta as the identity authority
- Allow Google or GitHub identities only when they are federated upstream into Okta
- Normalize authenticated identity into a trusted claim/header contract that Aeterna can consume consistently
- Map Okta subject, email, and group claims into Aeterna tenant context, roles, and policy inputs
- Define supported deployment, secret, ingress, and callback configuration for operating this flow in Kubernetes via the Helm chart
- Preserve existing service-to-service authentication paths for non-browser automation

**Non-Goals:**
- Implement separate native login integrations for Google or GitHub inside Aeterna
- Replace Permit, Cedar, or OPAL with a new authorization engine
- Define an end-user self-service administration UI in this change
- Solve SCIM provisioning, background IdP sync expansion, or full user lifecycle management beyond what is needed for trusted authorization inputs

## Decisions

### Decision: Use Okta as the sole identity authority exposed to Aeterna

Aeterna will trust one upstream identity authority: Okta. If the enterprise wants Google Workspace or GitHub identities, they must be federated into Okta first. This keeps Aeterna's trust model, issuer validation, and claim mapping stable.

**Why:**
- Corporate policy centers on Okta
- Okta can normalize upstream providers while still issuing a stable subject and groups claim
- Avoids implementing GitHub-specific OAuth behavior and provider-specific claim logic inside Aeterna

**Alternatives considered:**
- Direct multi-provider support in Aeterna: rejected because it introduces multiple issuer models, multiple claim-mapping paths, and GitHub-specific non-OIDC behavior
- Google-first or GitHub-first support: rejected because it conflicts with the stated corporate identity model

### Decision: Terminate user login at a trusted auth layer in front of Aeterna

The supported design will terminate browser login and session management at a dedicated auth layer deployed with the Helm chart, rather than embedding the full Okta login flow directly into `agent-a2a` first.

The auth layer is responsible for:
- OIDC redirect and callback handling
- validating the Okta issuer and client configuration
- maintaining browser session state
- forwarding only trusted user identity claims to Aeterna

Aeterna remains responsible for:
- validating that trusted identity headers came from the supported auth boundary
- mapping identity claims into tenant context
- applying authorization policies and role mapping

**Why:**
- The current application has no browser login/session model to extend safely
- The ingress/chart surface is the natural place to add enterprise SSO without coupling Okta-specific protocol handling to every service
- Keeps Aeterna focused on authorization and tenant context rather than provider-specific login mechanics

**Alternatives considered:**
- Native OIDC login inside `agent-a2a`: rejected for v1 because it adds callback/session/token lifecycle complexity to a service that currently only knows API keys
- Ingress-only auth without app-side identity mapping: rejected because the user specifically needs permissions and authorizations to derive from authenticated identity

### Decision: Define a normalized trusted identity contract between the auth layer and Aeterna

The auth boundary must forward a normalized identity surface to Aeterna containing, at minimum:
- stable subject (`sub`)
- email
- groups
- issuer
- provider marker (Okta)

The application-side auth middleware will consume this contract and construct tenant context plus authorization inputs. Requests that lack the required trusted identity contract must fail closed.

**Why:**
- The current `TenantContext` only supports `x-tenant-id`, `x-user-id`, and `x-agent-id`
- Authorization needs stable user identity and group membership, not just an opaque API key
- Normalized claims make downstream policy mapping deterministic

**Alternatives considered:**
- Trust raw provider tokens in all services: rejected because it spreads token-validation complexity and creates inconsistent claim handling
- Continue using only custom headers injected externally without a defined contract: rejected because it is too ambiguous for secure authorization mapping

### Decision: Add explicit authorization mapping from Okta claims/groups into tenant context and roles

Aeterna will define a configurable mapping from trusted Okta claims into:
- tenant identifier
- user identifier
- role set
- policy engine inputs

The mapping layer must support fail-closed behavior when:
- required claims are missing
- group-to-role mapping does not resolve
- tenant cannot be derived for the request context

**Why:**
- The current governance spec requires valid tenant context on every request
- The user specifically wants authentication to drive permission management and authorization configuration
- Okta groups are the practical enterprise control plane for role assignment

**Alternatives considered:**
- Hardcode role logic in handlers: rejected because it bypasses governance requirements and is not operator-manageable
- Depend only on synced users without authenticated claim mapping: rejected because it does not prove who the current actor is

### Decision: Keep permissions storage policy-centric with Postgres plus Cedar, and treat OPAL as the runtime sync plane

The supported authorization architecture for this change will keep the existing policy-centric model:
- Cedar policy files remain the source of truth for authorization rules
- Postgres remains the source of truth for tenant hierarchy, memberships, role assignments, and related authorization data
- OPAL and the Cedar Agent remain the runtime distribution and evaluation plane for policy and entity data

This means operators deploying Okta-backed authentication must also deploy the supported OPAL/Cedar components when they want the full production authorization path described by the existing platform architecture.

**Why:**
- The repository already has first-class Cedar policies, OPAL deployment surfaces, and Postgres-backed membership/role data
- Okta group-to-role mapping fits naturally into the current Postgres + Cedar model
- OPAL distributes policy and entity data efficiently, but it is not itself the permissions source of truth
- Introducing SpiceDB now would add a second authorization control plane before relationship-driven use cases have been proven

**Alternatives considered:**
- Replace Cedar/OPAL with SpiceDB now: rejected because the current repo shape and active deployment model are already centered on Cedar policy evaluation and Postgres-backed membership data
- Store permissions primarily in Okta groups: rejected because identity-provider groups should remain coarse-grained identity inputs rather than the full application authorization source of truth

### Decision: Preserve API-key authentication for service-to-service and automation paths

Existing non-browser automation paths will continue to support API-key authentication. Okta-backed user authentication augments browser and interactive product access; it does not replace service credentials in the first iteration.

**Why:**
- Current tooling and agent workflows already rely on service auth
- Separating human SSO from automation credentials reduces migration risk

**Alternatives considered:**
- Forcing all traffic through user SSO: rejected because background agents and machine clients need a non-browser path

## Risks / Trade-offs

- **[Trusted header spoofing]** → Only trust identity headers when requests arrive through the supported auth boundary; reject direct spoofable headers from untrusted sources
- **[Incorrect group claim configuration in Okta]** → Document required issuer, audience, and groups-claim setup; fail closed when expected claims are absent
- **[Tenant derivation ambiguity]** → Require an explicit mapping rule for tenant resolution and reject requests that cannot be mapped deterministically
- **[Operational complexity at ingress/auth layer]** → Mitigate by defining one supported deployment pattern instead of multiple parallel auth topologies
- **[Mismatch between user provisioning and live authentication]** → Keep authorization driven by live trusted identity claims, using background sync only as a supplemental source where needed
- **[Operator confusion about where permissions are stored]** → Document explicitly that Okta authenticates users, Postgres stores assignments, Cedar files store rules, and OPAL only synchronizes runtime authorization data

## Migration Plan

1. Add Helm-supported auth-layer configuration for Okta issuer, client credentials, callback URL, session secret, and trusted identity forwarding
2. Update ingress/deployment configuration so interactive product traffic passes through the supported auth layer
3. Extend application auth middleware to consume the normalized trusted identity contract and derive tenant context plus authorization inputs
4. Add configurable Okta group-to-role and claim-to-tenant mapping
5. Validate fail-closed behavior for missing claims, invalid mappings, and bypass attempts
6. Document rollout, required Okta authorization-server configuration, where permissions and policies are stored, why OPAL must be deployed, and rollback to API-key-only access if needed

Rollback strategy:
- disable the Okta-backed auth-layer configuration in Helm
- revert interactive access to the previous supported authentication mode
- keep service-to-service API-key auth operational throughout rollback

## Open Questions

- Which exact auth-layer component will be the supported default in the Helm chart for Okta login termination and session handling?
- Which Okta claims are mandatory for tenant resolution in multi-tenant deployments beyond `sub`, `email`, and `groups`?
- Should first-login user records be provisioned just-in-time, or must users already exist in an internal store before authorization is allowed?
- How should group-to-role mapping be configured for environments that need team- or tenant-specific overrides?
