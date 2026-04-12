# Contributing to Aeterna

Thank you for your interest in contributing to Aeterna. This guide covers everything you need to get started.

---

## Development Setup

### Prerequisites

- **Rust**: Stable toolchain (edition 2024). Install via [rustup](https://rustup.rs/).
- **Docker**: Required for PostgreSQL, Redis, and Qdrant.
- **Node.js**: Required for the admin UI (`npm`).
- **cargo-tarpaulin**: For coverage reports (`cargo install cargo-tarpaulin`).
- **cargo-mutants**: For mutation testing (`cargo install cargo-mutants`).

### Getting Started

```bash
# Clone and enter the repository
git clone <repo-url> && cd aeterna

# Start infrastructure services
docker compose up -d   # PostgreSQL 16+, Redis 7+, Qdrant 1.12+

# Build all workspace crates
cargo build --release

# Run all tests
cargo test --all

# Build the admin UI (optional)
cd admin-ui && npm install && npm run build && cd ..

# Start the server
cargo run -p aeterna-cli -- serve
```

### Verifying Your Setup

```bash
# Health check
curl http://localhost:8080/health

# Coverage report (must meet 80% minimum)
cargo tarpaulin --config tarpaulin.toml

# Lint
cargo fmt --check
cargo clippy --all -- -D warnings
```

---

## Code Conventions

### Rust Edition

The workspace enforces **edition 2024**. Never use `edition = "2021"`. This is set in the workspace `Cargo.toml` and inherited by all crates.

### Error Handling

- **Library crates** (`mk_core`, `memory`, `knowledge`, `storage`, etc.): Use `thiserror` for typed errors.
- **Application crate** (`cli`): Use `anyhow` for error chains with `.context("...")`.
- Return `Result<T, E>` from all fallible functions.
- Never use `unwrap()` in library code -- use `?` or explicit error handling.
- Never swallow errors with empty catch blocks.

### Formatting and Linting

- Run `cargo fmt` before every commit.
- Clippy is configured with `all` and `pedantic` warnings. Allowed exceptions are listed in the workspace `Cargo.toml`.
- Do not suppress new clippy warnings without discussion and justification in `Cargo.toml`.

### Documentation

- All public APIs must have `///` doc comments with usage examples.
- Avoid `unsafe` unless absolutely necessary; include a `// SAFETY:` comment.

### Dependencies

- All external dependencies are declared in `[workspace.dependencies]` in the root `Cargo.toml`.
- Crate-level `Cargo.toml` files reference workspace dependencies with `dep.workspace = true`.
- New crates must be added to the workspace `members` list.

---

## Testing Requirements

Testing standards are non-negotiable:

- **Minimum 80% test coverage** enforced in CI via `cargo-tarpaulin` (`fail-under = 80`).
- **TDD/BDD**: Write the failing test first, then implement (RED -> GREEN -> REFACTOR).
- **Property-based tests**: Use `proptest` for critical algorithms (promotion scoring, similarity, confidence).
- **Mutation testing**: 90%+ mutants killed for critical paths (`cargo mutants --jobs 4`).
- **External dependencies behind trait abstractions**: Mock with `wiremock` and `testcontainers`.
- **Deterministic fixtures**: Version all external API response fixtures.

### Running Tests

```bash
# All unit tests
cargo test --all

# Specific crate
cargo test -p aeterna-memory

# Integration tests (requires Docker services)
docker compose up -d
cargo test --all -- --include-ignored

# Coverage report
cargo tarpaulin --config tarpaulin.toml --out Html

# Mutation testing
cargo mutants --jobs 4
```

---

## OpenSpec Workflow

All non-trivial changes must go through the OpenSpec process. Skip this only for: bug fixes, typos, formatting, comment-only changes, or non-breaking dependency bumps.

### Creating a Change

1. **Check existing work**: Run `openspec list` and `openspec list --specs`.
2. **Choose a change ID**: Kebab-case, verb-led (e.g., `add-backup-encryption`, `fix-memory-dedup`).
3. **Scaffold artifacts** under `openspec/changes/<change-id>/`:
   - `proposal.md` -- Why, what changes, impact
   - `tasks.md` -- Sequential implementation checklist
   - `design.md` -- (Optional) Technical decisions, only when complexity warrants it
   - `specs/<capability>/spec.md` -- Delta specs with `## ADDED|MODIFIED|REMOVED Requirements`
4. **Validate**: `openspec validate <change-id> --strict`

### Implementing a Change

1. Read `proposal.md` and `design.md` (if exists).
2. Follow `tasks.md` sequentially.
3. Write failing tests first for each task.
4. Mark tasks `[x]` as completed.
5. Verify: `openspec validate <change-id> --strict`

### Archiving a Change

After deployment, archive the change:

```bash
openspec archive <change-id> --yes
```

This moves `changes/<name>/` to `changes/archive/YYYY-MM-DD-<name>/` and updates main specs.

---

## Git Workflow

### Branching

- Feature branches from `master`.
- Branch naming: `feat/<description>`, `fix/<description>`, `refactor/<description>`.
- PR reviews required before merge.

### Conventional Commits

All commits must follow the conventional commit format:

| Prefix | Use Case |
|---|---|
| `feat:` | New feature or capability |
| `fix:` | Bug fix |
| `refactor:` | Code restructuring without behavior change |
| `test:` | Adding or updating tests |
| `docs:` | Documentation changes |
| `chore:` | Build, CI, dependency updates |
| `perf:` | Performance improvement |

Examples:
```
feat(memory): add importance decay for cold-tier archival
fix(backup): handle empty NDJSON lines during import
refactor(storage): extract RLS setup into shared helper
test(governance): add property tests for approval workflow
docs(cli): update server bootstrap documentation
chore: bump sqlx to 0.9.0-alpha.2
```

### CI Checks

Every PR must pass:
- `cargo fmt --check`
- `cargo clippy --all -- -D warnings`
- `cargo test --all`
- `cargo tarpaulin` with coverage >= 80%

---

## Architecture Principles

### ReplicaSet Compatibility

Every feature must work with multiple replicas. This is not optional.

- **State**: Use Redis (ephemeral shared state) or PostgreSQL (durable shared state). Never in-memory stores for data that must be visible across replicas.
- **Background tasks**: Acquire a Redis distributed lock before execution. Use the pattern in `lifecycle.rs`.
- **Caches**: DashMap with TTL is fine for read-through caches. Document the staleness window.
- **File I/O**: Upload to S3 for anything that must be accessible from other replicas.
- **PR checklist item**: "Does this work with 3 replicas?"

---

## PR Checklist

Before opening a PR, verify:

- [ ] OpenSpec proposal exists and is validated (for non-trivial changes)
- [ ] All tasks in `tasks.md` are marked complete
- [ ] Tests written first (TDD), all passing
- [ ] Coverage >= 80% for touched code
- [ ] `cargo fmt` applied
- [ ] `cargo clippy --all -- -D warnings` passes
- [ ] Public APIs have doc comments with examples
- [ ] No `unwrap()` in library code
- [ ] Error types use `thiserror` (libs) or `anyhow` (apps)
- [ ] Edition is 2024 (never 2021)
- [ ] New crate added to workspace `members` if applicable
- [ ] Conventional commit message format used
- [ ] Works correctly with 3 replicas behind a load balancer (no in-process shared state)

---

## Adding a New Crate

1. Create the crate directory with `cargo init --lib <name>`.
2. Add it to `workspace.members` in the root `Cargo.toml`.
3. Add it to `[workspace.dependencies]` if other crates will depend on it.
4. Set `edition = "2024"` in the crate's `Cargo.toml` (or inherit from workspace).
5. Add `[lints] workspace = true` to inherit workspace lint configuration.
6. Write initial tests before implementation.

---

## Project Structure Reference

See `ARCHITECTURE.md` for the full system architecture, crate dependency graph, and data flow diagrams.

See `AGENTS.md` for MCP tool documentation and agent integration patterns.

See `docs/DEVELOPER_GUIDE.md` for detailed development workflows.
