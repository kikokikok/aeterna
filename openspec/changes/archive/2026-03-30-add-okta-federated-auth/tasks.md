## 1. Supported Okta authentication deployment

- [x] 1.1 Add Helm values and secrets for the supported Okta authentication boundary, including issuer URL, client credentials, callback URL, session secret, and enablement flags
- [x] 1.2 Update the Helm ingress and deployment templates so protected interactive routes are served through the supported authentication boundary with trusted TLS/callback configuration
- [x] 1.3 Add chart validation, example values, and render tests for Okta-enabled deployment mode and fail-closed misconfiguration cases

## 2. Trusted identity propagation into Aeterna

- [x] 2.1 Extend application auth middleware to accept the normalized trusted identity contract for interactive requests while preserving existing service-to-service authentication paths
- [x] 2.2 Enforce fail-closed handling when trusted identity fields are missing, malformed, or arrive from an untrusted request path
- [x] 2.3 Add tests for authenticated interactive requests, unauthenticated interactive requests, spoofed identity attempts, and unchanged API-key machine-client behavior

## 3. Tenant and authorization mapping

- [x] 3.1 Implement configurable mapping from trusted Okta claims into tenant context fields used by downstream services
- [x] 3.2 Implement configurable mapping from trusted Okta group claims into Aeterna roles or policy attributes consumed by authorization checks
- [x] 3.3 Add tests for successful tenant resolution, missing tenant mapping, successful group-to-role mapping, and fail-closed authorization when required mappings are absent

## 4. Operator guidance and rollout verification

- [x] 4.1 Document the supported Okta setup, required claims/groups configuration, callback URLs, secrets, upstream Google/GitHub federation expectation, and the requirement to deploy OPAL/Cedar for the supported production authorization path
- [x] 4.2 Update README, INSTALL, and deployment/operator docs to describe the supported interactive auth path, the separation from service-to-service credentials, and where permissions, memberships, and policy rules are stored versus synchronized at runtime
- [x] 4.3 Run targeted verification for Helm rendering, application tests, and configuration failure paths, then update the checklist to reflect completed work
