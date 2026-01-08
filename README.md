# Memory-Knowledge System

A universal framework for AI agent memory and knowledge management
with hierarchical storage and governed organizational knowledge.

## Quick Start

```bash
# Install dependencies
cargo install --locked

# Run tests
cargo test

# Build release
cargo build --release
```

## Project Structure

```
memory-knowledge-system/
├── Cargo.toml              # Workspace root (Edition 2024, latest deps)
├── core/                   # Shared types and traits
├── memory/                 # Memory system implementation
├── knowledge/              # Knowledge repository (Git-based)
├── sync/                   # Sync bridge (pointer architecture)
├── tools/                  # MCP tool interface (8 tools)
├── adapters/               # Provider + ecosystem adapters
├── storage/                # Storage layer (PostgreSQL, Qdrant, Redis)
├── config/                 # Configuration management
├── utils/                  # Utility functions (hashing, validation)
├── errors/                 # Error handling framework
└── docs/                   # Architecture and examples
```

## Development

### Requirements

- **Rust**: 1.70+ with Edition 2024
- **Tools**: Cargo, git
- **Services**: PostgreSQL 16+, Qdrant 1.12+, Redis 7+

### Best Practices

- **Rust Edition**: 2024 (never back to 2021)
- **Dependencies**: Latest compatible versions (resolver = "2")
- **Microsoft Pragmatic Rust Guidelines**: Comprehensive coding standards
  - Error handling: `anyhow` (apps), `thiserror` (libs)
  - Performance: Proper async/await with tokio
  - Documentation: M-CANONICAL-DOCS format
  - Safety: Avoid `unsafe` unless necessary
  - Traits: Send/Sync bounds where appropriate

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Check coverage
cargo tarpaulin --out Html
```

## License

Apache License 2.0 - See [LICENSE](LICENSE) file for details

## Documentation

- [OpenSpec Specification](openspec/specs/)
- [Implementation Plan](openspec/IMPLEMENTATION_PLAN.md)
- [Change Proposals](openspec/changes/)

## Contributing

1. Check existing [issues](../../issues) or [pull requests](../../pulls)
2. Follow [OpenSpec workflow](openspec/AGENTS.md)
3. Ensure all tests pass and coverage targets met
