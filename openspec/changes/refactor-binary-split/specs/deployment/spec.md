## MODIFIED Requirements

### Requirement: CLI Release Artifacts
The CI pipeline SHALL produce release artifacts for both `aeterna` (full binary) and `aeterna-cli` (lean binary) across all supported targets. The `aeterna-migrate` binary SHALL be included in the Docker image but not distributed as a standalone release artifact.

#### Scenario: CLI release builds both binaries
- **WHEN** a version tag is pushed
- **THEN** the CLI release workflow builds `aeterna-cli` for x86_64-linux, aarch64-linux, x86_64-macos, and aarch64-macos
- **AND** uploads them as GitHub release assets alongside the existing `aeterna` archives

#### Scenario: Docker image includes migrate binary
- **WHEN** the Docker image is built
- **THEN** it contains both `aeterna` and `aeterna-migrate` binaries
- **AND** the Helm migration Job uses `aeterna-migrate up`
