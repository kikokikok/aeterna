## ADDED Requirements

### Requirement: Authenticated CLI Control Plane
The system SHALL provide the `aeterna` binary as the supported authenticated control plane for Aeterna users and operators.

#### Scenario: Connected command uses real backend behavior
- **WHEN** a user runs a backend-facing CLI command against a configured Aeterna server
- **THEN** the CLI SHALL execute the real backend-backed operation for that command
- **AND** the CLI SHALL return actual result data or backend errors rather than placeholder responses

#### Scenario: Unsupported command fails explicitly
- **WHEN** a command path is not supported for the current deployment or integration mode
- **THEN** the CLI SHALL return an explicit unsupported error
- **AND** the CLI SHALL NOT return simulated success, fake audit data, or placeholder healthy status

### Requirement: CLI Authentication Lifecycle
The CLI SHALL support interactive user authentication, token refresh, logout, and auth status for remote Aeterna usage.

#### Scenario: User logs in interactively
- **WHEN** a user runs the supported CLI login command for a target server
- **THEN** the CLI SHALL complete the supported interactive authentication flow
- **AND** the CLI SHALL exchange the resulting upstream identity with the Aeterna server for CLI session credentials
- **AND** the CLI SHALL persist the resulting credentials securely for later commands

#### Scenario: Auth status shows active identity and target
- **WHEN** a user runs the supported CLI auth status command
- **THEN** the CLI SHALL show the active target profile, authenticated user identity, and token/session status
- **AND** the command SHALL indicate when credentials are missing, expired, or require refresh

#### Scenario: Logout removes active credentials
- **WHEN** a user runs the supported CLI logout command
- **THEN** the CLI SHALL revoke or discard locally stored credentials for the selected profile
- **AND** later backend-facing commands SHALL require re-authentication

### Requirement: Secure CLI Credential Storage
The CLI SHALL store only the credentials required for continued authenticated use and SHALL protect them from accidental disclosure.

#### Scenario: Secure storage available
- **WHEN** the local platform provides a supported secure credential store
- **THEN** the CLI SHALL persist CLI session credentials in that secure store
- **AND** the CLI SHALL NOT require the user to manage raw bearer tokens manually

#### Scenario: Secure storage unavailable
- **WHEN** the local platform does not provide the supported secure credential store
- **THEN** the CLI SHALL use the documented fallback persistence path
- **AND** the CLI SHALL warn the user that the fallback is less secure

#### Scenario: Token refresh preserves ongoing usage
- **WHEN** the CLI refreshes credentials for an authenticated profile
- **THEN** the CLI SHALL update the persisted credentials atomically
- **AND** backend-facing commands SHALL continue to use the refreshed credentials without requiring manual token editing

### Requirement: CLI Profile and Target Management
The CLI SHALL support named target profiles for different Aeterna environments.

#### Scenario: User selects a default target profile
- **WHEN** a user sets a profile as the default CLI target
- **THEN** later backend-facing commands SHALL use that profile unless an explicit override is provided
- **AND** the CLI SHALL surface which profile is active in auth and status output

#### Scenario: Command overrides target profile
- **WHEN** a user runs a CLI command with an explicit target/profile override
- **THEN** the command SHALL use the overridden profile for that invocation only
- **AND** the default profile SHALL remain unchanged

### Requirement: CLI Configuration Management
The CLI SHALL provide a supported configuration surface for viewing, editing, and validating control-plane configuration.

#### Scenario: User views effective CLI config
- **WHEN** a user runs the supported CLI config display command
- **THEN** the CLI SHALL show the effective configuration after applying precedence rules
- **AND** the output SHALL identify the active profile, server URL, and config sources

#### Scenario: User validates config before use
- **WHEN** a user runs the supported CLI config validation command
- **THEN** the CLI SHALL validate config structure, required fields, and target profile references
- **AND** the command SHALL report actionable errors for invalid configuration

### Requirement: End-to-End CLI User Journeys
The system SHALL document the supported CLI workflows from the user perspective with end-to-end scenarios.

#### Scenario: First-time developer onboarding
- **WHEN** a new developer installs the CLI, configures a target, logs in, and verifies connectivity
- **THEN** the documentation and tests SHALL cover the full sequence from install to first successful authenticated command

#### Scenario: Daily authenticated usage
- **WHEN** an authenticated user switches between environments and performs memory, knowledge, governance, and admin workflows
- **THEN** the documented end-to-end scenarios SHALL show how the CLI target, auth state, and command outputs behave throughout the workflow

#### Scenario: Operator/admin flow
- **WHEN** an operator installs the CLI on a supported platform, authenticates, and runs runtime/admin workflows
- **THEN** the end-to-end scenarios SHALL document the supported packaging, target selection, auth, and command execution behavior

### Requirement: Native CLI Distribution
The system SHALL publish supported native installation paths for macOS and Linux users of the `aeterna` binary.

#### Scenario: macOS installation
- **WHEN** a macOS user follows the documented install path
- **THEN** the user SHALL be able to install a supported `aeterna` binary from release artifacts or package-manager integration
- **AND** the installed binary SHALL report the published version correctly

#### Scenario: Linux installation
- **WHEN** a Linux user follows the documented install path
- **THEN** the user SHALL be able to install a supported `aeterna` binary from release artifacts or package-manager integration
- **AND** the documented installation path SHALL match the published release assets
