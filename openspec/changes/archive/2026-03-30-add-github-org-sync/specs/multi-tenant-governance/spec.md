## MODIFIED Requirements

### Requirement: Hierarchical Organization Structure

The system SHALL support a four-level organizational hierarchy within each tenant:
1. Company (tenant root)
2. Organization (business unit)
3. Team (working group)
4. Project (codebase/repository)

The hierarchy MUST be bootstrappable from an external identity provider (GitHub, Okta, Azure AD) via the IdP sync framework, in addition to manual creation via the API.

#### Scenario: Hierarchy navigation
- **WHEN** a user queries for knowledge at the Team level
- **THEN** the system SHALL include inherited knowledge from Organization and Company levels
- **AND** the system SHALL mark each result with its originating hierarchy level

#### Scenario: Hierarchy creation
- **WHEN** an admin creates a new Team under an Organization
- **THEN** the Team SHALL inherit default policies from the parent Organization
- **AND** the Team SHALL be visible to all Organization members with appropriate permissions

#### Scenario: IdP-bootstrapped hierarchy
- **WHEN** an IdP sync provider (GitHub, Okta, or Azure AD) is configured
- **THEN** the system SHALL create the Company, Organization, and Team units automatically from the IdP's group/team structure
- **AND** manually-created units SHALL coexist with IdP-synced units without conflict
- **AND** IdP-synced units SHALL be tagged with their source provider for audit purposes
