# Implementation Tasks

## 1. Git Backend Setup
- [x] 1.1 Create Git repository structure in storage
- [x] 1.2 Initialize Git repository using git2 crate
- [x] 1.3 Create directory structure: company/, orgs/, teams/, projects/
- [x] 1.4 Create type subdirectories: adrs/, policies/, patterns/, specs/
- [x] 1.5 Write unit tests for Git initialization

## 2. Knowledge Manager
- [x] 2.1 Implement `GitRepository` in `knowledge/src/repository.rs`
- [x] 2.2 Support 4-layer hierarchy in paths
- [x] 2.3 Implement store/get/list/delete with Git commits
- [x] 2.4 Implement `get_affected_items` for diffing commits

## 3. Constraint Engine - Evaluation
- [x] 3.1 Implement rule evaluation logic in `governance.rs`
- [x] 3.2 Support must_use, must_not_use operators
- [x] 3.3 Support must_match, must_not_match (regex) operators
- [x] 3.4 Support must_exist, must_not_exist operators
- [x] 3.5 Implement full `validate` hierarchy (Company -> Org -> Team -> Project)

## 4. Multi-Tenant Federation
- [x] 4.1 Implement FederationConfig struct
- [x] 4.2 Implement UpstreamConfig struct
- [x] 4.3 Implement fetch_upstream_manifest() method
- [x] 4.4 Implement sync_upstream() method
- [x] 4.5 Implement conflict detection between upstream and local

## 5. Observability
- [x] 5.1 Add Prometheus metrics for knowledge operations
- [x] 5.2 Emit metrics: knowledge_operations_total, knowledge_violations_total
- [x] 5.3 Add tracing spans for Git operations

## 6. Integration
- [x] 6.1 Register knowledge tools in `tools/src/server.rs`
- [x] 6.2 Write integration tests for full knowledge lifecycle
