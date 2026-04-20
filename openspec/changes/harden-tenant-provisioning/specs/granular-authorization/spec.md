## ADDED Requirements

### Requirement: Scoped API Tokens
The system SHALL issue short-lived, scoped, revocable API tokens for non-interactive callers (CI, automation). Scoped tokens SHALL be JWTs signed by the same key as user access tokens but distinguished by a `token_type: "scoped"` claim and a `scopes: [...]` claim. Interactive users SHALL continue to use the existing OAuth-based access tokens.

#### Scenario: Create a scoped token
- **WHEN** a PlatformAdmin POSTs `/api/v1/auth/tokens` with `{scopes: ["tenant:provision"], expiresIn: "4h", description: "ci-deploy"}`
- **THEN** the server SHALL mint a JWT containing `token_type: "scoped"`, the requested `scopes`, `exp` set according to `expiresIn`, and a unique `tokenId`
- **AND** the server SHALL persist the token metadata (id, subject, scopes, expiresAt, description, not the token itself) to allow revocation and listing
- **AND** the response SHALL include the raw token string exactly once

#### Scenario: Token TTL is bounded
- **WHEN** a caller requests `expiresIn` greater than 24 hours
- **THEN** the server SHALL reject with HTTP 400 and message `"expiresIn exceeds maximum (24h)"`

#### Scenario: Token TTL default
- **WHEN** `expiresIn` is omitted
- **THEN** the server SHALL default to 4 hours

### Requirement: Scope Vocabulary for Tenant Provisioning
The system SHALL define the following scopes for tenant provisioning workflows:

- `tenant:read` — list and get tenant records, render redacted manifests.
- `tenant:render` — render non-redacted manifests (still excludes secret values, only reveals references).
- `tenant:provision` — submit manifests to `/api/v1/admin/tenants/provision`.
- `tenant:diff` — submit manifests to `/api/v1/admin/tenants/diff`.

Scope checks SHALL be enforced by the authorization layer in addition to any existing role checks, using an OR relationship: a request succeeds if the caller has either the required role OR the required scope.

#### Scenario: Scope permits a single operation
- **WHEN** a scoped token bearing `tenant:provision` submits a manifest to `/api/v1/admin/tenants/provision`
- **THEN** the request SHALL succeed even if the subject is not a PlatformAdmin
- **AND** the audit entry SHALL record the subject and the `tokenId` that authorized the call

#### Scenario: Scope does not grant adjacent operations
- **WHEN** a scoped token bearing only `tenant:read` attempts to call `/api/v1/admin/tenants/provision`
- **THEN** the request SHALL fail with HTTP 403
- **AND** the response SHALL indicate the missing scope

### Requirement: Token Revocation and Listing
The system SHALL provide endpoints to list and revoke scoped tokens. Revocation SHALL take effect within the token cache TTL (maximum 60 seconds) across all server instances.

#### Scenario: Revoke a token
- **WHEN** a PlatformAdmin DELETEs `/api/v1/auth/tokens/{tokenId}`
- **THEN** the server SHALL mark the token revoked in persistent storage
- **AND** subsequent validations SHALL reject the token within 60 seconds (cache TTL)
- **AND** the audit log SHALL record the revocation

#### Scenario: List tokens
- **WHEN** a PlatformAdmin GETs `/api/v1/auth/tokens`
- **THEN** the server SHALL return metadata for all non-expired tokens issued by the caller (and all tokens if the caller is a PlatformAdmin)
- **AND** the raw token strings SHALL NOT appear in the response

### Requirement: Token Flag on CLI Rejected
The CLI SHALL NOT accept a `--token` command-line flag, to prevent tokens leaking into shell history or process listings. Tokens SHALL be supplied via `AETERNA_API_TOKEN`, the OS keychain, or the mode-gated `~/.aeterna/credentials` file.

#### Scenario: --token flag rejected
- **WHEN** a user invokes any CLI command with `--token <value>`
- **THEN** the CLI SHALL exit with code 1
- **AND** the error message SHALL list the three accepted sources
