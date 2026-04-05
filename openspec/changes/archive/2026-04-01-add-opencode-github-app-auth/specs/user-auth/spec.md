## MODIFIED Requirements

### Requirement: Service Authentication Separation
The system SHALL preserve separate supported authentication paths for browser-based user access, interactive OpenCode plugin access, and service-to-service or automation traffic.

#### Scenario: Machine client continues using service credentials
- **WHEN** a non-browser automation client accesses Aeterna using the supported service authentication method
- **THEN** the request SHALL continue to authenticate without requiring an Okta browser login
- **AND** interactive user SSO requirements SHALL NOT break existing service-to-service authentication flows

#### Scenario: OpenCode plugin uses dedicated interactive client authentication
- **WHEN** an end user authenticates to Aeterna from the OpenCode plugin
- **THEN** the plugin SHALL use the supported plugin authentication flow rather than the browser-oriented Okta ingress flow
- **AND** the resulting authenticated requests SHALL resolve the end user's identity for downstream Aeterna services

#### Scenario: Browser authentication remains Okta-backed
- **WHEN** a user accesses protected Aeterna browser endpoints
- **THEN** browser authentication SHALL continue to use the supported Okta-backed interactive path
- **AND** plugin-specific authentication changes SHALL NOT replace the browser login authority
