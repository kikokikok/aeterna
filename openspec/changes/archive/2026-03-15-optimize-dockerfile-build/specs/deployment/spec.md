## ADDED Requirements

### Requirement: Docker Build Optimization
The Docker build system SHALL use a multi-stage dependency caching strategy to minimize rebuild times when only application source code changes.

#### Scenario: Source-only change rebuild
- **WHEN** a developer modifies Rust source files without changing Cargo.toml or Cargo.lock
- **THEN** the Docker build SHALL reuse the cached dependency compilation layer
- **AND** only recompile application source code
- **AND** complete the build in under 5 minutes on typical CI hardware

#### Scenario: Dependency change rebuild
- **WHEN** Cargo.toml or Cargo.lock are modified
- **THEN** the build SHALL recompile dependencies using sccache to cache individual crate object files
- **AND** reuse cached objects for unchanged transitive dependencies

#### Scenario: Cold build with full CPU utilization
- **WHEN** building with no prior cache
- **THEN** the build SHALL use all available CPU cores for compilation
- **AND** NOT artificially restrict build parallelism

#### Scenario: BuildKit cache persistence
- **WHEN** multiple sequential Docker builds are executed
- **THEN** the Cargo registry, git checkouts, sccache artifacts, and build target directory SHALL be persisted via BuildKit cache mounts
- **AND** cache mounts SHALL use locked sharing to prevent corruption during parallel builds

### Requirement: Parameterized Package Build
The Dockerfile SHALL support building any workspace crate via a build argument, eliminating the need for per-package Dockerfiles.

#### Scenario: Default package build
- **WHEN** building with no PACKAGE argument specified
- **THEN** the build SHALL compile the `aeterna` package

#### Scenario: Alternate package build
- **WHEN** building with `--build-arg PACKAGE=agent-a2a`
- **THEN** the build SHALL compile the `agent-a2a` package
- **AND** the runtime image SHALL contain only the specified binary

### Requirement: Runtime Image Integrity
The optimized build process SHALL NOT alter the runtime image characteristics.

#### Scenario: Runtime image unchanged
- **WHEN** the optimized Dockerfile produces a runtime image
- **THEN** the image SHALL use debian:bookworm-slim as its base
- **AND** run as a non-root user (aeterna, UID 1000)
- **AND** include only ca-certificates and libssl3 as runtime dependencies
- **AND** NOT include any build tools (cargo-chef, sccache, cargo-binstall, rustc)

## REMOVED Requirements

### Requirement: Consistent Build Pattern Across Dockerfiles
**Reason**: Dockerfile.agent-a2a is being deleted. The main Dockerfile now supports building any package via the PACKAGE build arg, making separate Dockerfiles unnecessary.
**Migration**: Use `docker build --build-arg PACKAGE=agent-a2a .` instead of `docker build -f Dockerfile.agent-a2a .`
