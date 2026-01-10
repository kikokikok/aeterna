# Change: Implement Knowledge Repository

## Why
The Knowledge Repository provides version-controlled, immutable storage for organizational decisions (ADRs, policies, patterns, specs). It's essential for governed agent behavior and is referenced by both Memory System (via sync) and Tool Interface.

## What Changes

### Knowledge Repository
- Implement `KnowledgeManager` with Git-based versioning
- Implement 4 knowledge types: adr, policy, pattern, spec
- Implement 4-layer multi-tenant hierarchy: company, org, team, project
- Implement immutable commit model with full history
- Implement manifest index for fast lookups

### Constraint Engine
- Implement constraint DSL parser
- Implement constraint evaluation engine
- Support 6 operators: must_use, must_not_use, must_match, must_not_match, must_exist, must_not_exist
- Support 5 targets: file, code, dependency, import, config
- Support 3 severity levels: info, warn, block

### Git Integration
- Use well-maintained crates: `git2` for Git operations
- Implement branching and merging
- Implement commit and rollback
- Implement remote sync (push/pull)

### Operations
- `query(input: QueryKnowledgeInput) -> QueryKnowledgeOutput`
- `get(input: GetKnowledgeInput) -> GetKnowledgeOutput`
- `check_constraints(input: CheckConstraintsInput) -> CheckConstraintsOutput`
- `propose(input: ProposeKnowledgeInput) -> ProposeKnowledgeOutput`
- `update_status(input: UpdateStatusInput) -> UpdateStatusOutput`

## Impact

### Affected Specs
- `knowledge-repository` - Complete implementation

### Affected Code
- New `knowledge` crate
- Update `storage` crate with Git implementation

### Dependencies
- `git2` - Git bindings
- `regex` - Constraint pattern matching
- `serde_json` - JSON serialization
- `toml` - Frontmatter parsing

## Breaking Changes
None - this is greenfield work building on foundation
