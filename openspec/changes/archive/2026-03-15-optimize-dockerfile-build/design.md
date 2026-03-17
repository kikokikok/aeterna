## Context

The main `Dockerfile` uses a naive build pattern: `COPY . .` followed by `cargo build`. This means every source change invalidates the Docker layer cache and forces a full recompilation of all 18 workspace crates plus all transitive dependencies. Combined with `CARGO_BUILD_JOBS=2` (which limits the build to 2 CPU cores regardless of available hardware), builds take ~2400 seconds.

A separate `Dockerfile.agent-a2a` duplicates this same pattern for the `agent-a2a` package. It has no CI/CD integration, no docker-compose service, and is only referenced by local dev Helm values. This duplication is unnecessary maintenance overhead.

The Cargo workspace has 18 crates with a substantial dependency tree. Most builds are source-only changes where dependencies haven't changed - exactly the scenario where dependency caching provides the largest gains.

## Goals / Non-Goals

**Goals:**
- Reduce incremental Docker builds (source-only changes) from ~2400s to under 5 minutes
- Reduce cold builds by removing the artificial 2-core parallelism limit
- Consolidate to a single parameterized Dockerfile that can build any workspace package
- Preserve the existing runtime image structure (non-root user, healthcheck, slim base)

**Non-Goals:**
- Changing the runtime image or deployment topology
- Adding cross-compilation or multi-arch builds (handled by `update-deployment-infrastructure` change)
- Modifying CI/CD pipeline configuration (only the Dockerfiles themselves)
- Changing Cargo workspace structure or crate boundaries

## Decisions

### 1. cargo-chef for dependency layer caching

**Decision:** Use `cargo-chef` with a 4-stage build pattern (base → planner → builder → runtime).

**Rationale:** cargo-chef extracts a "recipe" (dependency graph without source code), then "cooks" dependencies in a separate layer. Docker caches this layer until `Cargo.toml`/`Cargo.lock` change. This is the single biggest optimization - source-only changes skip dependency compilation entirely.

**Alternatives considered:**
- Manual `cargo build --release` with dummy `main.rs` trick: fragile in workspaces with 18 crates, requires manual maintenance of dummy source files for each crate
- Just adding BuildKit cache mounts without cargo-chef: helps across builds but doesn't give Docker layer caching within a single build pipeline

### 2. sccache as compilation cache

**Decision:** Use `sccache` with a BuildKit cache mount at `/sccache`.

**Rationale:** Complements cargo-chef. When the dependency graph changes (e.g., one crate version bumped), sccache caches individual compiled object files. Only the changed crate recompiles. Requires `CARGO_INCREMENTAL=0` since sccache and incremental compilation are mutually exclusive - sccache provides better cross-build caching in Docker contexts.

**Alternatives considered:**
- Incremental compilation without sccache: doesn't persist across Docker builds even with cache mounts (incremental artifacts are tied to specific compiler invocations)
- No additional caching beyond cargo-chef: works but leaves optimization on the table when dependencies change

### 3. cargo-binstall for tool installation

**Decision:** Install `cargo-chef` and `sccache` via `cargo-binstall` (pre-built binaries) rather than `cargo install` (compile from source).

**Rationale:** Compiling cargo-chef + sccache from source adds 5-10 minutes to cold builds. cargo-binstall downloads pre-compiled binaries in seconds.

### 4. Remove CARGO_BUILD_JOBS=2

**Decision:** Remove the `CARGO_BUILD_JOBS=2` environment variable entirely, defaulting to all available CPU cores.

**Rationale:** This was artificially throttling builds to 2 cores. The Cargo default (number of logical CPUs) is correct for both local Docker builds and CI/CD environments.

### 5. BuildKit cache mounts

**Decision:** Use `--mount=type=cache` for Cargo registry, git checkouts, sccache directory, and build target directory with `sharing=locked`.

**Rationale:** Persists caches across Docker builds even when layer cache misses. The `sharing=locked` flag prevents corruption during parallel builds.

### 6. Explicit binary copy from cache-mounted target

**Decision:** Copy the built binary to `/usr/local/bin/` within the `RUN` command before it exits, since cache mounts are not baked into image layers.

**Rationale:** When `/app/target` is a cache mount, its contents don't persist in the Docker layer. The binary must be explicitly copied to a non-mounted path during the `RUN` step.

### 7. BuildKit syntax directive

**Decision:** Add `# syntax=docker/dockerfile:1` at the top of the Dockerfile.

**Rationale:** Enables BuildKit frontend features (cache mounts). This is a no-op if BuildKit is already the default builder (modern Docker), but makes the requirement explicit and ensures compatibility.

### 8. Delete Dockerfile.agent-a2a, parameterize main Dockerfile

**Decision:** Remove `Dockerfile.agent-a2a` and add a `PACKAGE` build arg to the main `Dockerfile` (defaulting to `aeterna`).

**Rationale:** `Dockerfile.agent-a2a` has no CI/CD pipeline references, no docker-compose service, and is identical in structure to the main Dockerfile. The only difference is `--package agent-a2a`. A build arg eliminates duplication: `docker build --build-arg PACKAGE=agent-a2a .`

## Risks / Trade-offs

- **New build tool dependencies** → cargo-chef, sccache, cargo-binstall are well-maintained, widely-used Rust ecosystem tools. They are build-time only and do not affect the runtime image.
- **BuildKit requirement** → Modern Docker (20.10+) uses BuildKit by default. Rancher Desktop and Docker Desktop both default to BuildKit. If a CI environment uses the legacy builder, builds will fail on `--mount=type=cache`. Mitigation: document `DOCKER_BUILDKIT=1` requirement.
- **Cache mount disk usage** → Persistent caches grow over time. Mitigation: CI/CD can periodically clear Docker build caches (`docker builder prune`).
- **Cold build slightly slower** → First build installs cargo-chef/sccache (seconds via binstall) and has no cache to leverage. Net effect is negligible compared to the 2400s baseline since removing CARGO_BUILD_JOBS=2 alone provides significant speedup.
- **Nightly toolchain compatibility** → cargo-chef works with nightly Rust. No known issues. The planner stage just extracts dependency metadata.
