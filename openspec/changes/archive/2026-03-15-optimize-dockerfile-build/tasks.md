## 1. Main Dockerfile Optimization

- [x] 1.1 Add `# syntax=docker/dockerfile:1` directive and create `base` stage with nightly toolchain, cargo-binstall, and cargo-chef installation
- [x] 1.2 Add `PACKAGE` build arg (default: `aeterna`) for parameterized workspace crate builds
- [x] 1.3 Create `planner` stage that copies source and runs `cargo chef prepare --recipe-path recipe.json`
- [x] 1.4 Create `builder` stage with cargo-chef cook (dependencies-only) using BuildKit cache mounts for registry and git (sharing=locked)
- [x] 1.5 Add source copy and `cargo build --release --package ${PACKAGE}` with cache mounts
- [x] 1.6 Set `CARGO_INCREMENTAL=0` (no sccache — incompatible with native build scripts like openssl-sys)
- [x] 1.7 Preserve runtime stage (labels, non-root user, healthcheck, ports 8080/9090, entrypoint)

## 2. Remove Dockerfile.agent-a2a

- [x] 2.1 Delete `Dockerfile.agent-a2a`
- [x] 2.2 Update `docs/deployment/testing-guide.md` to use `docker build --build-arg PACKAGE=agent-a2a` instead of `-f Dockerfile.agent-a2a`

## 3. Validation

- [x] 3.1 Verify Dockerfile parses correctly with `docker build --check`
- [x] 3.2 Verify .dockerignore does not exclude files needed by cargo-chef planner stage (Cargo.toml, Cargo.lock, src/)
- [x] 3.3 Test that `docker build` succeeds for the main aeterna package (image: aeterna:test)
- [x] 3.4 Test that `docker build --build-arg PACKAGE=agent-a2a` succeeds (image: aeterna-a2a:test)
