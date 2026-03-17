## Why

Docker image builds currently take ~2400 seconds (~40 minutes) because every source file change triggers a full recompilation of all 18 workspace crates and their dependencies. The Dockerfiles lack dependency caching layers, use no BuildKit cache mounts, and artificially throttle parallelism with `CARGO_BUILD_JOBS=2`. This makes the development feedback loop painfully slow and wastes CI/CD resources.

Additionally, `Dockerfile.agent-a2a` is a near-duplicate of the main `Dockerfile` with no CI/CD pipeline references, no docker-compose integration, and no meaningful differentiation. It should be removed in favor of a parameterized main Dockerfile.

## What Changes

- Introduce `cargo-chef` to separate dependency compilation from source compilation, enabling Docker layer caching of the dependency build step
- Add BuildKit cache mounts for Cargo registry, git checkouts, and sccache artifacts to persist compilation caches across builds
- Add `sccache` as a compilation cache wrapper so individual crate objects survive across builds even when the dependency graph changes
- Use `cargo-binstall` to install build tools as pre-built binaries instead of compiling them from source
- Remove `CARGO_BUILD_JOBS=2` to allow full CPU utilization during builds
- Add `PACKAGE` build arg to the main `Dockerfile` (defaults to `aeterna`) so it can build any workspace crate
- **Delete `Dockerfile.agent-a2a`** — replaced by `docker build --build-arg PACKAGE=agent-a2a .`
- Add `# syntax=docker/dockerfile:1` directive to enable BuildKit features
- Update Helm values and docs that referenced `Dockerfile.agent-a2a`

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `deployment`: Adding requirements for Docker build optimization including dependency caching strategy, BuildKit cache mounts, build parallelism configuration, and parameterized package selection

## Impact

- Affected files: `Dockerfile` (rewritten), `Dockerfile.agent-a2a` (deleted), `my-values.yaml`, `charts/aeterna/examples/values-local.yaml`, `docs/deployment/testing-guide.md`
- New build dependencies: `cargo-chef`, `sccache`, `cargo-binstall` (installed at build time only, not in runtime image)
- Build system: Requires Docker BuildKit (default in modern Docker/Rancher Desktop)
- Expected result: Subsequent builds (source-only changes) should drop from ~2400s to 2-5 minutes; cold builds should also be faster due to full CPU utilization
