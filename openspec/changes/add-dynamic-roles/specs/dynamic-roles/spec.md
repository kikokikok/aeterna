## ADDED Requirements

### Requirement: Dynamic Role Definitions
The system SHALL support role definitions via Cedar entity membership without requiring Rust compilation. System roles (`CompanyAdmin`, `OrgAdmin`, `TeamAdmin`, `ProjectAdmin`, `Architect`, `TechLead`, `Developer`, `Viewer`) SHALL be pre-defined, and custom roles SHALL be definable through configuration.

#### Scenario: Define a custom role without recompilation
- **WHEN** an operator adds a new role definition through configuration and corresponding Cedar entities
- **THEN** the system SHALL accept and evaluate that role without requiring a Rust rebuild or redeploy

### Requirement: RoleIdentifier Type Safety
The system SHALL use a `RoleIdentifier` newtype at all service boundaries that distinguishes between known system roles and custom-defined roles.

#### Scenario: Boundary conversion preserves role kind
- **WHEN** a service boundary receives either a built-in role or an unknown role string
- **THEN** it SHALL represent built-in roles as `RoleIdentifier::Known(Role)` and non-built-in roles as `RoleIdentifier::Custom(String)`

### Requirement: Cedar Entity Membership Authorization
The system SHALL evaluate role-based authorization using Cedar `principal in Role::"X"` entity membership patterns. OPAL entity data SHALL be the authority for role grants.

#### Scenario: Authorization decision resolves from entity membership
- **WHEN** Cedar evaluates an action requiring role membership
- **THEN** the authorization decision SHALL be based on principal membership in role entities supplied via OPAL-synced data

### Requirement: Role Assignment and Removal
The system SHALL implement real role assignment and removal operations. Changes MUST be reflected in Cedar entity store via OPAL.

#### Scenario: Assigned role becomes effective via Cedar entities
- **WHEN** a role assignment operation succeeds for a user
- **THEN** the role SHALL be propagated to OPAL entity data and reflected in subsequent Cedar authorization decisions

#### Scenario: Removed role is revoked via Cedar entities
- **WHEN** a role removal operation succeeds for a user
- **THEN** the role SHALL be removed from OPAL entity data and no longer grant authorization in Cedar

### Requirement: Unknown Role Handling
The system SHALL gracefully handle unknown or custom role strings in authorization middleware rather than silently dropping them.

#### Scenario: Middleware preserves custom role inputs
- **WHEN** middleware receives a role string that does not parse as a known built-in role
- **THEN** it SHALL preserve the role as a custom role identifier for downstream authorization evaluation

### Requirement: Authorization Pipeline Contract Testing
The system SHALL include contract tests verifying the full authorization pipeline from database role row to OPAL entity representation to Cedar authorization decision.

#### Scenario: End-to-end role grant contract test
- **WHEN** CI runs contract tests for authorization
- **THEN** tests SHALL verify the pipeline `DB row -> OPAL entity -> Cedar decision` for both allow and deny outcomes
