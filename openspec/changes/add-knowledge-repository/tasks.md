# Implementation Tasks

## 1. Git Backend Setup
- [x] 1.1 Create Git repository structure in storage
- [x] 1.2 Initialize Git repository using git2 crate
- [x] 1.3 Create directory structure: company/, orgs/, teams/, projects/
- [x] 1.4 Create type subdirectories: adrs/, policies/, patterns/, specs/
- [x] 1.5 Write unit tests for Git initialization

## 10. Constraint Engine - Evaluation
- [x] 10.1 Implement ConstraintContext struct
- [x] 10.2 Implement ConstraintViolation struct
- [x] 10.3 Implement ConstraintCheckResult struct

## 13. Layer Hierarchy
- [x] 13.1 Implement layer precedence mapping
- [x] 13.2 Implement layer-aware queries
- [x] 13.3 Implement layer access control
- [x] 13.4 Write unit tests for layer resolution


## 14. Multi-Tenant Federation
- [ ] 14.1 Implement FederationConfig struct
- [ ] 14.2 Implement UpstreamConfig struct
- [ ] 14.3 Implement fetch_upstream_manifest() method
- [ ] 14.4 Implement sync_upstream() method
- [ ] 14.5 Implement conflict detection between upstream and local
- [ ] 14.6 Implement federation commit with type='federation'
- [ ] 14.7 Write integration tests for federation

## 15. Error Handling
- [ ] 15.1 Implement KnowledgeError enum
- [ ] 15.2 Define all error codes from spec
- [ ] 15.3 Implement Git error translation
- [ ] 15.4 Implement constraint error translation
- [ ] 15.5 Add retry logic with exponential backoff
- [ ] 15.6 Write unit tests for error handling

## 16. Observability
- [ ] 16.1 Integrate OpenTelemetry for Git operations
- [ ] 16.2 Add Prometheus metrics for knowledge operations
- [ ] 16.3 Emit metrics: knowledge.operations.total, knowledge.operations.errors, knowledge.operations.latency
- [ ] 16.4 Emit metrics: knowledge.constraint.checks, knowledge.constraint.violations
- [ ] 16.5 Add structured logging with tracing spans
- [ ] 16.6 Configure metric histograms

## 17. Integration Tests
- [ ] 17.1 Create full workflow test suite
- [ ] 17.2 Test propose → accept workflow
- [ ] 17.3 Test constraint creation and evaluation
- [ ] 17.4 Test multi-tenant federation
- [ ] 17.5 Test Git history and rollback
- [ ] 17.6 Test manifest regeneration and diffing
- [ ] 17.7 Ensure 85%+ test coverage

## 18. Documentation
- [ ] 18.1 Document KnowledgeManager public API
- [ ] 18.2 Document constraint DSL syntax
- [ ] 18.3 Document Git commit model
- [ ] 18.4 Add inline examples for all operations
- [ ] 18.5 Write architecture documentation
- [ ] 18.6 Update crate README
