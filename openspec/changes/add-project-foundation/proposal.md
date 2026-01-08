# Change: Initialize Project Foundation

## Why
The Memory-Knowledge System specification requires a solid foundation before implementing specific capabilities. We need to set up Rust project structure, workspace configuration, and core data structures that all other components will depend on.

## What Changes

### Rust Best Practices Compliance
- **Rust Edition 2024**: Commit to `2024` edition in workspace (never back to 2021)
- **Latest Crates**: Always use latest compatible versions with `cargo update -p crate@latest`
- **Microsoft Pragmatic Rust Guidelines**: Follow comprehensive coding standards
  - Proper error handling with `anyhow`/`thiserror`
  - Performance optimization with proper async/await
  - Documentation with M-CANONICAL-DOCS format
  - Safety guidelines (avoid `unsafe` unless necessary)
  - Cross-cutting concerns (traits, Send/Sync bounds)

### Infrastructure
- Initialize Rust workspace with multiple crates
- Set up Cargo workspace configuration
- Configure build profiles (dev, test, release)
- Set up development dependencies

### Core Data Structures
- Implement all shared types from specs:
  - `MemoryEntry`, `MemoryLayer`, `LayerIdentifiers`
  - `KnowledgeItem`, `KnowledgeType`, `KnowledgeLayer`
  - `Constraint`, `ConstraintOperator`, `ConstraintTarget`
  - `KnowledgePointer`, `SyncState`
- Implement common traits and error types
- Create utility functions (hashing, validation)

### Development Tools
- Set up pre-commit hooks (format, clippy, tests)
- Configure CI/CD pipeline skeleton
- Create development Docker Compose setup
- Enable rust-analyzer and VSCode settings

## Impact

### Affected Specs
- `core-concepts` - Implement all foundational types
- `configuration` - Create configuration structure

### Affected Code
- New workspace structure with 11 crates
- New `core` crate with shared types and traits
- New `errors` crate with error handling
- New `utils` crate with utilities
- New `config` crate with configuration
- Development tooling configuration

### Dependencies
**All dependencies will use latest compatible versions:**
- Serde 1.0+ (serialization)
- ThisError 1.0+ (error handling)
- Tokio 1.35+ (async runtime)
- Sha2 0.10+ (hashing)
- UUID 1.6+ (unique identifiers)
- Tracing 0.1+ (structured logging)
- Validator 0.16+ (input validation)

## Breaking Changes
None - this is greenfield work
