# Implementation Tasks

## 1. Git Backend Setup
- [ ] 1.1 Create Git repository structure in storage
- [ ] 1.2 Initialize Git repository using git2 crate
- [ ] 1.3 Create directory structure: company/, orgs/, teams/, projects/
- [ ] 1.4 Create type subdirectories: adrs/, policies/, patterns/, specs/
- [ ] 1.5 Write unit tests for Git initialization

## 2. Knowledge Manager Core
- [ ] 2.1 Create knowledge_manager.rs in knowledge/ crate
- [ ] 2.2 Implement KnowledgeManager struct with Git backend
- [ ] 2.3 Implement new() constructor
- [ ] 2.4 Implement initialize() method
- [ ] 2.5 Implement shutdown() method
- [ ] 2.6 Implement health_check() method

## 3. Knowledge Operations - Query
- [ ] 3.1 Implement query() method with filtering
- [ ] 3.2 Implement text search on manifest
- [ ] 3.3 Implement filtering by type, layer, status, tags
- [ ] 3.4 Implement layer-aware scoping
- [ ] 3.5 Return QueryKnowledgeOutput with item summaries
- [ ] 3.6 Write unit tests for query operation

## 4. Knowledge Operations - Get
- [ ] 4.1 Implement get() method
- [ ] 4.2 Load full content from Git repository
- [ ] 4.3 Handle not found case (return null)
- [ ] 4.4 Support includeConstraints flag
- [ ] 4.5 Support includeHistory flag
- [ ] 4.6 Write unit tests for get operation

## 5. Knowledge Operations - Propose
- [ ] 5.1 Implement propose() method
- [ ] 5.2 Validate knowledge item structure
- [ ] 5.3 Set initial status to 'draft'
- [ ] 5.4 Generate unique ID
- [ ] 5.5 Create Git commit with type='create'
- [ ] 5.6 Write unit tests for propose operation

## 6. Knowledge Operations - Update Status
- [ ] 6.1 Implement update_status() method
- [ ] 6.2 Validate status transitions
- [ ] 6.3 Create Git commit with type='status'
- [ ] 6.4 Return updated item with commit hash
- [ ] 6.5 Write unit tests for status updates

## 7. Manifest System
- [ ] 7.1 Implement KnowledgeManifest struct
- [ ] 7.2 Implement ManifestEntry struct
- [ ] 7.3 Implement generate_manifest() function
- [ ] 7.4 Implement load_manifest() function
- [ ] 7.5 Implement manifest diffing for delta detection
- [ ] 7.6 Write unit tests for manifest operations

## 8. Git Commit Model
- [ ] 8.1 Implement KnowledgeCommit struct
- [ ] 8.2 Implement create_commit() method
- [ ] 8.3 Implement get_commit() method
- [ ] 8.4 Implement get_commits_since() method
- [ ] 8.5 Implement immutable commit tracking
- [ ] 8.6 Write unit tests for commit operations

## 9. Constraint Engine - DSL Parser
- [ ] 9.1 Implement Constraint struct
- [ ] 9.2 Implement parse_constraint() function
- [ ] 9.3 Parse operator (must_use, must_not_use, etc.)
- [ ] 9.4 Parse target (file, code, dependency, etc.)
- [ ] 9.5 Parse pattern (regex or glob)
- [ ] 9.6 Parse severity (info, warn, block)
- [ ] 9.7 Write unit tests for constraint parsing

## 10. Constraint Engine - Evaluation
- [ ] 10.1 Implement ConstraintContext struct
- [ ] 10.2 Implement ConstraintViolation struct
- [ ] 10.3 Implement ConstraintCheckResult struct
- [ ] 10.4 Implement evaluate_constraint() for each operator
- [ ] 10.5 Implement must_use operator (pattern present)
- [ ] 10.6 Implement must_not_use operator (pattern absent)
- [ ] 10.7 Implement must_match operator (content matches regex)
- [ ] 10.8 Implement must_not_match operator (content doesn't match)
- [ ] 10.9 Implement must_exist operator (file/path exists)
- [ ] 10.10 Implement must_not_exist operator (file/path doesn't exist)
- [ ] 10.11 Write unit tests for each operator

## 11. Constraint Engine - Check Operation
- [ ] 11.1 Implement check_constraints() method
- [ ] 11.2 Load applicable constraints from knowledge
- [ ] 11.3 Evaluate all constraints in context
- [ ] 11.4 Aggregate violations by severity
- [ ] 11.5 Return ConstraintCheckResult with summary
- [ ] 11.6 Write integration tests for check_constraints

## 12. Knowledge Type Configuration
- [ ] 12.1 Implement KnowledgeTypeConfig struct
- [ ] 12.2 Define config for ADR type
- [ ] 12.3 Define config for Policy type
- [ ] 12.4 Define config for Pattern type
- [ ] 12.5 Define config for Spec type
- [ ] 12.6 Implement validation based on type config

## 13. Layer Hierarchy
- [ ] 13.1 Implement layer precedence mapping
- [ ] 13.2 Implement layer-aware queries
- [ ] 13.3 Implement layer access control
- [ ] 13.4 Write unit tests for layer resolution

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
