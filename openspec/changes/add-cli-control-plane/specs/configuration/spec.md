## ADDED Requirements

### Requirement: CLI Configuration Management Surface
The system SHALL provide a supported CLI configuration management surface for control-plane usage.

#### Scenario: User inspects effective CLI configuration
- **WHEN** a user runs the supported CLI config display command
- **THEN** the CLI SHALL show the effective control-plane configuration after applying precedence rules
- **AND** the output SHALL identify the selected profile, target server URL, and source files or environment overrides in effect

#### Scenario: User updates a target profile
- **WHEN** a user updates a named target profile through the supported CLI configuration surface
- **THEN** the CLI SHALL persist the new settings to the canonical configuration location
- **AND** the CLI SHALL validate that the resulting profile remains usable

### Requirement: Canonical Aeterna Configuration Precedence
The system SHALL define and implement one canonical configuration precedence model for the current `AETERNA_*` runtime.

#### Scenario: User-level and project-level config coexist
- **WHEN** both user-level and project-level CLI configuration files exist
- **THEN** the system SHALL apply a documented precedence order between project config, user config, environment variables, and CLI overrides
- **AND** the same precedence model SHALL be used consistently by runtime commands and auth/config commands

#### Scenario: Canonical config file locations are documented
- **WHEN** the CLI reads or writes persistent control-plane configuration
- **THEN** it SHALL use the documented canonical file locations for user-level and project-level config
- **AND** documentation and error messages SHALL reference those same canonical paths consistently

## MODIFIED Requirements

### Requirement: Configuration Loading
Configuration loading for the current Aeterna runtime SHALL use the supported Aeterna configuration sources and precedence.

#### Scenario: Aeterna CLI/server configuration precedence
- **WHEN** the CLI or server resolves configuration for control-plane or runtime usage
- **THEN** CLI flags or explicit command overrides SHALL have the highest precedence
- **AND** `AETERNA_*` environment variables SHALL override persisted config files
- **AND** project-level Aeterna config SHALL override user-level config
- **AND** built-in defaults SHALL apply only when no higher-precedence source provides a value

#### Scenario: Unsupported legacy variable set is not treated as canonical
- **WHEN** configuration documentation describes environment-variable control of the current Aeterna runtime
- **THEN** the documented canonical variable namespace SHALL match the implemented `AETERNA_*` runtime variables
- **AND** unsupported legacy namespaces SHALL NOT be documented as the primary current contract
