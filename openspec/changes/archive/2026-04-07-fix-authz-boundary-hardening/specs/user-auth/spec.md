## MODIFIED Requirements

### Requirement: Plugin and API Request Authentication
The system SHALL authenticate tenant-scoped API requests before granting access to memory, knowledge, governance, or administrative operations.

#### Scenario: Tenant-scoped API request without validated auth
- **WHEN** a request reaches a tenant-scoped API route without a validated plugin, session, API-key, or equivalent trusted authentication context
- **THEN** the system SHALL reject the request
- **AND** the route SHALL NOT accept caller-controlled tenant or user headers as a substitute for authentication in production-capable modes

#### Scenario: Authenticated request carries validated tenant context
- **WHEN** a request presents validated authentication credentials
- **THEN** the system SHALL derive tenant and user context from the validated identity or its verified mappings
- **AND** the derived context SHALL be available to downstream route handlers and authorization checks
