## ADDED Requirements

### Requirement: Native CLI Distribution
The deployment and release path SHALL publish a supported native `aeterna` CLI binary distribution for macOS and Linux users.

#### Scenario: Release includes native CLI assets
- **WHEN** a supported CLI release is published
- **THEN** the release SHALL include the native binary assets or package artifacts documented for macOS and Linux
- **AND** the published installation instructions SHALL match the actual release outputs

#### Scenario: Supported package-manager entry point
- **WHEN** the project documents package-manager-based CLI installation
- **THEN** the documented package-manager path SHALL install the same supported `aeterna` binary version as the release assets
- **AND** the install path SHALL be verifiable in CI or release validation
