## ADDED Requirements

### Requirement: Supported Okta Authentication Deployment
The supported Kubernetes deployment SHALL include a documented authentication boundary for interactive product access that is configured with Okta issuer, client, callback, session, and TLS settings.

For the supported production authorization path, the deployment SHALL also document that Cedar policy files are the authorization-rule source of truth, Postgres-backed membership and role data are the authorization-data source of truth, and OPAL/Cedar Agent components are required to synchronize and evaluate that data at runtime.

#### Scenario: Operator configures Okta-backed deployment
- **WHEN** an operator enables Okta-backed user authentication in the supported Helm deployment path
- **THEN** the deployment SHALL expose configuration for Okta issuer URL, client credentials, callback/redirect URL, and session secret material
- **AND** the deployment SHALL document the required ingress and TLS configuration for the authentication flow
- **AND** the deployment SHALL explain that OPAL is deployed as the runtime policy/data synchronization plane rather than as the permissions storage system

#### Scenario: Operator understands where permissions are stored
- **WHEN** an operator follows the supported Okta-backed deployment documentation
- **THEN** the documentation SHALL state that application permissions and role assignments are stored in the platform data stores and policy files rather than in OPAL itself
- **AND** the documentation SHALL describe OPAL and the Cedar Agent as required runtime distribution and evaluation components for the supported authorization path

#### Scenario: Authentication boundary protects product ingress
- **WHEN** interactive product ingress is enabled for an Okta-backed deployment
- **THEN** protected product routes SHALL be reachable only through the supported authentication boundary
- **AND** direct unauthenticated access to protected routes SHALL be denied

### Requirement: Trusted Identity Header Boundary
The deployment SHALL enforce that identity headers or equivalent trusted identity fields are only accepted from the supported authentication boundary.

#### Scenario: Trusted identity arrives from supported auth boundary
- **WHEN** a request is forwarded from the supported authentication boundary to Aeterna
- **THEN** the deployment SHALL permit the normalized trusted identity fields needed by Aeterna
- **AND** those fields SHALL be treated as authoritative for interactive user identity

#### Scenario: Spoofed identity header is blocked
- **WHEN** a request attempts to supply trusted identity fields from outside the supported authentication boundary
- **THEN** the deployment SHALL prevent those fields from being trusted
- **AND** the request SHALL fail closed rather than inheriting forged identity
