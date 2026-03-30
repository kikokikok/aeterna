# Aeterna — Claude Code Memory

## Project Purpose

Universal Memory & Knowledge Framework for Enterprise AI Agent Systems. Provides hierarchical memory storage, governed organizational knowledge, and a pluggable adapter architecture for AI agents at scale (LangChain, AutoGen, CrewAI, OpenCode).

---

## Tech Stack

| Layer | Technology |
|---|---|
| **Language** | Rust (Edition 2024 — NEVER 2021) |
| **Async runtime** | Tokio (full features) |
| **Memory storage** | Redis 7+ (Working/Session), PostgreSQL 16+ + pgvector (Episodic/Procedural/User/Org), Qdrant 1.12+ (Semantic/Archival) |
| **Graph layer** | DuckDB 0.9+ |
| **Embeddings** | rust-genai 0.4+ (OpenAI, Anthropic, Gemini, Z.AI, xAI, Ollama, Groq, Cohere…) |
| **Authorization** | Cedar policies + Permit.io + OPAL |
| **MCP interface** | 11 unified tools (memory, knowledge, graph) |
| **A2A protocol** | Radkit SDK 0.0.4 |
| **Error handling** | `anyhow` (apps), `thiserror` (libs) |
| **Serialisation** | serde / serde_json / serde_yaml / toml |
| **HTTP** | reqwest 0.13 |
| **DB (relational)** | sqlx 0.9 (PostgreSQL + Tokio) |
| **Testing** | cargo-tarpaulin, proptest, cargo-mutants, testcontainers, wiremock |

---

## Workspace Layout

```
aeterna/
├── mk_core/        # Shared types, traits, domain primitives
├── memory/         # Memory system (7 layers, R1 optimization, DuckDB graph)
├── knowledge/      # Knowledge repository (Git-backed, constraint DSL)
├── sync/           # Memory ↔ Knowledge sync bridge
├── tools/          # MCP tool interface (11 tools)
├── adapters/       # Ecosystem adapters (OpenCode, LangChain, Radkit)
├── storage/        # Storage layer (Postgres, Qdrant, Redis, DuckDB)
├── config/         # Configuration management, hot-reload
├── errors/         # Error handling framework
├── utils/          # Common utilities
├── context/        # Context compression (CCA)
├── cli/            # CLI (aeterna setup wizard)
├── agent-a2a/      # Agent-to-Agent protocol (Radkit)
├── opal-fetcher/   # OPAL policy sync
├── observability/  # OpenTelemetry + Prometheus metrics
├── testing/        # Shared test fixtures and helpers
├── idp-sync/       # Identity provider sync (Okta)
├── cross-tests/    # Integration tests (require Docker)
├── openspec/       # Change proposals and versioned specs
└── specs/          # Legacy spec documents
```

---

## Key Architectural Concepts

### Memory Layers (8 levels, ordered fastest → slowest)

```
Working       µs  — Redis in-memory
Session       ms  — Redis with TTL
Episodic       h  — PostgreSQL + pgvector
Semantic       d  — Qdrant vector search
Procedural     w  — PostgreSQL facts
User          mo  — PostgreSQL + pgvector
Organization  mo  — PostgreSQL + pgvector
Archival      yr  — Qdrant long-term
```

### Knowledge Hierarchy (4 levels, high → low precedence)

```
Company → Organization → Team → Project
```

Policies flow **down**; projects can override with `Merge` or `Override` strategy.

### Multi-Tenant Context

Every operation scoped to: `tenant → org → team → project → user → agent`

### Authorization (Cedar + OPAL)

- **Cedar files** hold authorization rules (not OPAL)
- **OPAL + Cedar Agent** synchronize and evaluate at runtime
- **Postgres-backed Aeterna** stores memberships, assignments, hierarchy
- **Okta** is the identity authority; Google/GitHub only via Okta federation

---

## Essential Commands

```bash
# Build
cargo build --release

# Run all tests
cargo test --all

# Run tests for a specific crate
cargo test -p aeterna-memory

# Coverage report (requires cargo-tarpaulin)
cargo tarpaulin --out Html --all

# Integration tests (requires Docker services running)
docker-compose up -d
cargo test --all -- --include-ignored

# Format
cargo fmt

# Lint
cargo clippy --all -- -D warnings

# Mutation testing
cargo mutants --jobs 4
```

---

## Testing Requirements (NON-NEGOTIABLE)

- **Minimum 80% test coverage** enforced in CI via tarpaulin (`fail-under = 80`)
- **TDD/BDD**: Write failing test first, then implementation (RED → GREEN → REFACTOR)
- **Property-based tests** (proptest) for all critical algorithms (promotion score, similarity, confidence)
- **Mutation testing** — 90%+ mutants killed for critical paths
- **All external deps behind trait abstractions** — mock everything with wiremock / testcontainers
- **Deterministic fixtures** for all external API responses (versioned)
- **BDD scenarios** (Gherkin-style) for all critical workflows

> **Violation**: Do NOT commit code without tests. Do NOT skip coverage checks.

---

## Code Conventions

### Rust Style

- Edition **2024** (workspace enforces this — never `edition = "2021"`)
- `cargo fmt` before every commit
- Public APIs must have `///` doc comments with examples
- Avoid `unsafe` unless absolutely necessary with a safety comment
- Use `Result<T, E>` — no panics in library code
- Never suppress type errors with `as any` patterns or `#[allow(unused)]` without justification
- `anyhow` for binary/app error chains; `thiserror` for library error types

### Clippy Config

Enabled: `all`, `pedantic` (as warnings).  
Allowed exceptions are listed in `Cargo.toml` — don't suppress new ones without discussion.

### Error Handling

- Return `Result` from all fallible functions
- Never swallow errors with empty catch blocks
- Propagate with `?`, wrap with `.context("…")` from anyhow

---

## OpenSpec Workflow (MANDATORY for features)

All non-trivial changes go through OpenSpec:

```
openspec list               # See active changes
openspec list --specs       # See existing capabilities
openspec show <change>      # View change details
openspec validate <change> --strict   # Validate before implementing
openspec archive <change-id> --yes    # Archive after deployment
```

**Stage 1 — Create proposal** (`openspec/changes/<change-id>/`):
- `proposal.md` — Why + What + Impact
- `tasks.md` — Implementation checklist
- `design.md` — (optional) Technical decisions
- `specs/<capability>/spec.md` — Delta specs (`## ADDED|MODIFIED|REMOVED Requirements`)

**Stage 2 — Implement** (follow `tasks.md` sequentially, mark `[x]` when done)

**Stage 3 — Archive** (after deployment, move to `changes/archive/YYYY-MM-DD-<id>/`)

> Skip proposal only for: bug fixes, typos, formatting, comment-only changes, non-breaking dep bumps.

### Active Changes (as of 2026-03)

| Change | Status | Remaining |
|---|---|---|
| `add-cloud-llm-providers` | in-progress | 1 task |
| `fix-production-readiness-gaps` | in-progress | 4 tasks |
| `add-okta-federated-auth` | complete (needs archive) | — |

---

## Performance Targets

| Tier | Latency | Throughput |
|---|---|---|
| Working memory | < 10ms | — |
| Session memory | < 50ms | — |
| Semantic search | < 200ms | — |
| API | — | > 100 QPS |
| Creates | — | > 50 CPS |
| CPU | — | < 70% |
| Memory | — | < 80% |
| DB connections | — | < 80% pool |

---

## MCP Tools (11 total)

**Memory**: `memory_add`, `memory_search`, `memory_delete`, `memory_feedback`, `memory_optimize`  
**Knowledge**: `knowledge_query`, `knowledge_check`, `knowledge_show`  
**Graph**: `graph_query`, `graph_neighbors`, `graph_path`

---

## Git Workflow

- Feature branches from `main`
- PR reviews required before merge
- **Conventional commits**: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`
- CI enforces: coverage ≥ 80%, clippy clean, fmt clean

---

## OpenCode Plugin

Published as `@aeterna-org/opencode-plugin` — MCP tools + hooks + automatic context injection.

---

## Key Specs to Read Before Working

| Spec | Location |
|---|---|
| Memory system | `openspec/specs/memory-system/` |
| Knowledge repository | `openspec/specs/knowledge-repository/` |
| OpenCode integration | `openspec/specs/opencode-integration/` |
| Multi-tenant governance | `openspec/specs/multi-tenant-governance/` |
| Storage layer | `openspec/specs/storage/` |
| Codesearch integration | `openspec/specs/codesearch-integration/` |

---

## Do NOT

- Use `edition = "2021"` — always `2024`
- Write code without a failing test first
- Suppress clippy warnings without justification in `Cargo.toml`
- Use `unwrap()` in library code — use `?` or explicit error handling
- Skip `cargo fmt` before committing
- Create new Cargo crates without adding them to the workspace `members` list
- Commit without running `cargo test --all` locally
- Start implementing a feature change without a validated OpenSpec proposal
