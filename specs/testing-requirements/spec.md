# Testing Requirements Specification

## Purpose

Define testing requirements and quality standards for OpenSpec Knowledge Provider implementations. Testing is not optional - it is a requirement for compliance.

### Goals

1. **Testing is Mandatory**: No implementation can claim OpenSpec compliance without meeting testing requirements
2. **Coverage is Safety**: Minimum 80% coverage for all components (85%+ for core logic)
3. **Test-Driven Development**: TDD/BDD must be followed (test before code)
4. **Quality Gates**: CI/CD must fail if coverage or quality thresholds not met
5. **Mockability**: All external dependencies must be behind traits for easy testing
6. **Test Fixtures**: All external service responses must have test fixtures
7. **Property-Based Testing**: Critical algorithms must be tested with property-based frameworks
8. **Mutation Testing**: 90%+ mutants must be killed for critical code

---

## Requirements

### Requirement: Test Coverage Standards
The system SHALL achieve minimum coverage thresholds across all components.

#### Scenario: Unit test coverage
**WHEN** business logic components are implemented
**THEN** unit test coverage SHALL be >= 85%
**AND** SHALL be measured with tarpaulin or equivalent tool
**AND** SHALL be enforced in CI/CD pipeline

#### Scenario: Integration test coverage
**WHEN** API endpoints are implemented
**THEN** integration test coverage SHALL be >= 80%
**AND** SHALL test with both mock and real dependencies
**AND** SHALL be enforced in CI/CD pipeline

### Requirement: Testability Architecture
All external dependencies SHALL be behind trait abstractions to enable easy mocking and testing.

#### Scenario: Dependency injection
- **WHEN** external services are integrated (database, vector store, LLM providers)
- **THEN** dependencies SHALL be behind trait abstractions
- **AND** test implementations SHALL be provided
- **AND** constructor dependency injection SHALL be supported
- **AND** mocking SHALL be possible without modifying production code

### Requirement: Property-Based Testing
All critical algorithms SHALL be tested with property-based testing frameworks.

#### Scenario: Property testing implementation
- **WHEN** critical algorithms are implemented (promotion scoring, similarity metrics)
- **THEN** property-based tests SHALL be written using proptest framework
- **AND** SHALL include minimum 10,000 test cases per property
- **AND** SHALL verify invariants (bounds, constraints, edge cases)
- **AND** SHALL achieve 90%+ coverage for property-based tests

---

## 📋 Abstract

This document defines **testing requirements** and **quality standards** that all OpenSpec-compliant knowledge providers must implement. Testing is not optional - it is a **requirement for compliance**.

### Core Principles

1. **Testing is Mandatory**: No implementation can claim OpenSpec compliance without meeting testing requirements
2. **Coverage is Safety**: Minimum 80% coverage for all components (85%+ for core logic)
3. **Test-Driven Development**: TDD/BDD must be followed (test before code)
4. **Quality Gates**: CI/CD must fail if coverage or quality thresholds not met
5. **Mockability**: All external dependencies must be behind traits for easy testing
6. **Test Fixtures**: All external service responses must have test fixtures
7. **Property-Based Testing**: Critical algorithms must be tested with property-based frameworks
8. **Mutation Testing**: 90%+ mutants must be killed for critical code

---

## 🏗️ Testing Architecture

### Test Pyramid

```
                ┌─────────────────────────────────────────────────────┐
                │     E2E Tests (BDD Scenarios)        │
                │  • 10-20 scenarios                      │
                │  • Slow (10s - 5min)                     │
                │  • High-level user workflows                   │
                │  • Test as documentation                        │
                └─────────────────────────────────────────────────────┘
                                    │
                                    ▼
                    ┌─────────────────────────────────────────────────────┐
                    │     Integration Tests                        │
                    │  • 100-500 tests                         │
                    │  • Medium speed (100ms - 5s)              │
                    │  • API endpoints, providers                 │
                    │  • Real dependencies (PostgreSQL, Qdrant)      │
                    └─────────────────────────────────────────────────────┘
                                    │
                                    ▼
                    ┌─────────────────────────────────────────────────────┐
                    │     Unit Tests (TDD Tests)                │
                    │  • 1000-5000 tests                        │
                    │  • Fast (< 100ms)                         │
                    │  • 85%+ coverage target                   │
                    │  • Test fixtures, mock implementations           │
                    │  • Property-based testing (proptest)             │
                    │  • Mutation testing (cargo-mutants)          │
                    │  • Dependency injection for testability          │
                    └─────────────────────────────────────────────────────┘
                                    │
                                    ▼
                    ┌─────────────────────────────────────────────────────┐
                    │     Property-Based Tests                    │
                    │  • 50-100 properties                      │
                    │  • Critical algorithms                     │
                    │  • 90%+ coverage target                   │
                    │  • Invariant verification                   │
                    └─────────────────────────────────────────────────────┘
```

---

## 📊 Coverage Requirements

### Coverage Targets by Component Type

| Component Type | Unit Test Target | Integration Target | Overall Target | Tool |
|----------------|-------------------|---------------------|----------------|-------|
| **Business Logic** | 90%+ | 85%+ | 85%+ | tarpaulin |
| **Data Models** | 95%+ | N/A | 95%+ | tarpaulin |
| **API Endpoints** | 85%+ | 85%+ | 85%+ | tarpaulin |
| **Database Layer** | 85%+ | 80%+ | 80%+ | tarpaulin |
| **Vector DB Layer** | 85%+ | 80%+ | 80%+ | tarpaulin |
| **Provider Integration** | 85%+ | 80%+ | 80%+ | tarpaulin |
| **Query Engine** | 90%+ | 85%+ | 85%+ | tarpaulin |
| **Embedding Service** | 90%+ | 85%+ | 85%+ | tarpaulin |

### Minimum Coverage Thresholds

| Coverage Type | Minimum Threshold | Enforcement |
|-------------|------------------|-------------|
| **Unit Tests** | 85% | Must be >= 85% |
| **Integration Tests** | 80% | Must be >= 80% |
| **E2E Tests** | 75% | Must be >= 75% |
| **Property-Based Tests** | 90% | Must be >= 90% |
| **Mutation Tests** | 90% | Must be >= 90% mutants killed |
| **Overall Project** | 80% | Must be >= 80% |

### Coverage Exclusions

```
# Files and patterns excluded from coverage calculations
*/tests/*                    # Test files themselves
*/test_fixtures/*            # Test fixture files
*/tests/*_fixtures.rs       # Test fixture modules
*/examples/*                # Example code
*/bench/*.rs                 # Benchmark files
*/tests/*.mock.rs            # Mock implementations
```

---

## 🎨 Testability Requirements

### Dependency Injection for Testability

**Requirement**: All external dependencies must be behind trait abstractions to enable easy mocking and testing.

**Example**:
```rust
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, Error>;
    async fn embed_batch(&self, texts: Vec<&str>) -> Result<Vec<Vec<f32>>, Error>;
}

// Production implementation
pub struct GenaiEmbeddingProvider {
    client: genai::Client,
}

#[async_trait]
impl EmbeddingProvider for GenaiEmbeddingProvider {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, Error> {
        self.client.exec_embed("text-embedding-3-small", vec![text], None).await
    }
}

// Test mock implementation
#[cfg(test)]
pub struct MockEmbeddingProvider {
    responses: Arc<Mutex<Vec<Result<Vec<f32>, Error>>>>,
}

#[cfg(test)]
#[async_trait]
impl EmbeddingProvider for MockEmbeddingProvider {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
        self.responses.lock().await.pop().unwrap()
    }
}
```

**Compliance Checklist**:
- [ ] All database layers implement swappable traits
- [ ] All provider integrations implement swappable traits
- [ ] All external services have trait abstractions
- [ ] Test implementations are provided for all traits
- [ ] Constructor dependency injection for all components

### Test Fixtures

**Requirement**: All external service responses must have deterministic test fixtures.

**Example**:
```rust
// tests/fixtures/git_api.rs
use serde_json::json;

pub fn github_commit_response() -> serde_json::Value {
    json!({
        "sha": "abc123",
        "commit": {
            "message": "Test commit message",
            "author": {
                "name": "Test User",
                "email": "test@example.com"
            }
        }
    })
}

pub fn github_tree_response() -> serde_json::Value {
    json!({
        "tree": [
            {
                "path": "README.md",
                "mode": "100644",
                "type": "blob",
                "sha": "def456"
            }
        ]
    })
}

// Usage in tests
#[cfg(test)]
mod tests {
    use super::fixtures::*;

    #[tokio::test]
    async fn test_git_provider_parse_commit() {
        let mock_server = MockServer::new()
            .with_response(github_commit_response());

        let provider = GitProvider::new(mock_server.url());
        let commit = provider.get_commit("abc123").await.unwrap();

        assert_eq!(commit.message, "Test commit message");
        assert_eq!(commit.author.name, "Test User");
    }
}
```

**Fixture Requirements**:
- [ ] All API responses have test fixtures
- [ ] Fixtures are deterministic (no random values)
- [ ] Fixtures cover success and error cases
- [ ] Fixtures are versioned with API versions
- [ ] Fixtures are documented with examples

---

## 🧪 Property-Based Testing Requirements

### Critical Algorithms

**Requirement**: All critical algorithms MUST be tested with property-based testing frameworks.

**Critical Algorithms List**:
1. Promotion Score Calculation
2. Cross-Layer Query Result Ordering
3. Embedding Similarity Metrics
4. Confidence Aggregation
5. Conflict Resolution
6. Version Merging
7. Access Policy Evaluation
8. Search Result Ranking

**Example**:
```rust
use proptest::prelude::*;

#[cfg(test)]
mod tests {
    use super::*;

    // Property 1: Promotion score is always in [0.0, 1.0]
    proptest! {
        #[test]
        fn prop_promotion_score_bounds(
            access_count in 0usize..10000,
            time_elapsed_hours in 0u64..8760u64,  // 1 year
            base_score in 0.0f32..1.0f32,
            importance in 0.0f32..1.0f32,
        ) {
            let entry = MemoryEntry {
                access_count,
                time_elapsed_hours,
                importance_score: base_score,
                importance,
                ..Default::default()
            };

            let score = entry.calculate_promotion_score();

            assert!(score >= 0.0);
            assert!(score <= 1.0);
        }
    }
}
```

**Property Testing Requirements**:
- [ ] All critical algorithms have property-based tests
- [ ] Minimum 10,000 test cases per property
- [ ] Coverage of invariants (bounds, constraints)
- [ ] Coverage of edge cases (empty, max values, null)
- [ ] Deterministic test execution

---

## 🧬 Mutation Testing Requirements

### Mutation Testing Configuration

**Requirement**: Critical code paths MUST achieve 90%+ mutation score (90%+ mutants killed).

**Critical Code Paths**:
1. Memory Models (all layers)
2. Promotion Score Calculation
3. Query Engine (semantic, keyword, hybrid)
4. Embedding Generation
5. Provider Integration (Git, NotebookLM, etc.)
6. Query result aggregation
7. Access policy evaluation
8. Metadata operations

**Mutation Testing Requirements**:
- [ ] All critical code paths have mutation tests
- [ ] Mutation score >= 90% (90%+ mutants killed)
- [ ] Mutants configured: "all" (not subset)
- [ ] CI/CD runs mutation tests on every PR
- [ ] Mutation coverage report uploaded with PR

---

## 🔌 Integration Testing Requirements

### API Endpoint Tests

**Requirement**: All OpenSpec endpoints must have integration tests with real and mock dependencies.

**Required Tests**:
1. Discovery endpoint (`GET /openspec/v1/knowledge`)
2. Query endpoint (`POST /openspec/v1/knowledge/query`)
3. Create endpoint (`POST /openspec/v1/knowledge/create`)
4. Update endpoint (`PUT /openspec/v1/knowledge/{id}`)
5. Delete endpoint (`DELETE /openspec/v1/knowledge/{id}`)
6. Batch operations endpoint (`POST /openspec/v1/knowledge/batch`)
7. Streaming endpoint (`GET /openspec/v1/knowledge/stream`)
8. Metadata operations (`GET /openspec/v1/knowledge/{id}/metadata`)

**Integration Test Requirements**:
- [ ] All 8 OpenSpec endpoints have integration tests
- [ ] Tests with mock dependencies (fast, deterministic)
- [ ] Tests with real dependencies (PostgreSQL, Qdrant, Redis)
- [ ] Error handling tests (4xx, 5xx errors)
- [ ] Authentication/authorization tests
- [ ] Rate limiting tests
- [ ] Streaming response tests
- [ ] Batch operation tests

### Database Integration Tests

**Requirement**: All database layers (PostgreSQL, Qdrant, Redis) must have integration tests.

**Database Integration Test Requirements**:
- [ ] All memory layers have database integration tests
- [ ] Tests use appropriate frameworks (SQLx, qdrant-client, redis-rs)
- [ ] Transactions are tested (commit/rollback)
- [ ] Concurrency is tested (race conditions)
- [ ] Performance tested (1000+ entries)
- [ ] Error handling tested (connection failures, timeouts)

### Provider Integration Tests

**Requirement**: All knowledge providers must have integration tests with mock and real implementations.

**Provider Integration Test Requirements**:
- [ ] All knowledge providers have integration tests
- [ ] Tests with mock API responses (deterministic)
- [ ] Tests with real provider implementations (Git repos, APIs)
- [ ] Error handling tested (network errors, timeouts)
- [ ] Performance tested (1000+ operations)
- [ ] Fixture-based tests for all external APIs

---

## 🎭 End-to-End Testing (BDD) Requirements

### BDD Scenarios

**Requirement**: All critical user workflows must be tested with BDD scenarios (Given-When-Then).

**Required Scenarios**:

| Scenario | Priority | Complexity | Test Time | Status |
|----------|----------|------------|------------|--------|
| Basic knowledge query | P0 | Low | 10s | Required |
| Cross-layer query | P0 | Medium | 30s | Required |
| Memory promotion (auto) | P0 | Medium | 30s | Required |
| Memory promotion (manual with approval) | P1 | High | 60s | Required |
| Query with governance approval | P1 | High | 60s | Required |
| Batch operations | P0 | High | 90s | Required |
| Knowledge source integration | P0 | High | 120s | Required |
| Streaming updates | P0 | High | 120s | Required |

**Example**:
```gherkin
Feature: Multi-Layered Memory Queries

#### Scenario: Query across working and session memory
**WHEN** I have stored data in working memory
**AND** I have stored data in session memory
**AND** I query for information with "test query"
**THEN** results should come from both working and session layers
**AND** results should be ordered by relevance score
**AND** total execution time should be less than 100ms

#### Scenario: Promote entry from working to session memory
**WHEN** I have a working memory entry accessed 100 times
**AND** entry has high importance score
**AND** I trigger promotion to session memory
**THEN** entry should be promoted to session memory
**AND** entry should be removed from working memory
**AND** promotion should be logged in audit log
```

**BDD Requirements**:
- [ ] All 10-20 critical scenarios implemented
- [ ] Scenarios written in Gherkin syntax
- [ ] Each scenario has clear Given-When-Then structure
- [ ] Scenarios cover happy paths and error cases
- [ ] Scenarios test business rules and invariants
- [ ] Scenarios are versioned with OpenSpec version

---

## 📏 CI/CD Testing Pipeline

### GitHub Actions Workflow

**Requirement**: CI/CD pipeline must run all test types and enforce coverage thresholds.

**Required Jobs**:
1. Unit tests with coverage (tarpaulin)
2. Integration tests with real dependencies (PostgreSQL, Qdrant, Redis)
3. E2E BDD tests
4. Coverage enforcement (fail if < 80%)
5. Mutation tests with cargo-mutants
6. Quality checks (clippy, fmt)

**CI/CD Requirements**:
- [ ] All 4 job types implemented (unit, integration, E2E, coverage)
- [ ] Coverage reports merged and uploaded
- [ ] Pipeline fails if coverage < 80%
- [ ] Quality checks (clippy, fmt) run
- [ ] Mutants tested on every PR
- [ ] Test database isolated from production
- [ ] All artifacts retained for debugging

### Docker Compose for Testing

```yaml
# docker-compose.test.yml
version: '3.8'

services:
  postgres-test:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: provider_test
      POSTGRES_USER: test_user
      POSTGRES_PASSWORD: test_password
    ports:
      - "5432:5432"
    volumes:
      - ./init/test-db.sql:/docker-entrypoint-initdb.d

  qdrant-test:
    image: qdrant/qdrant:v1.12
    ports:
      - "6333:6333"
    volumes:
      - qdrant_data:/qdrant/storage

  redis-test:
    image: redis:7-alpine
    ports:
      - "6379:6379"
```

---

## 📋 Quality Gates

### Pre-Commit Hooks

**Requirement**: All commits must pass pre-commit hooks for code quality.

**Required Hooks**:
1. `cargo fmt` - Code formatting
2. `cargo clippy` - Linting with warnings
3. `cargo test` - Run tests locally
4. `check-json` - Validate JSON files
5. `typos` - Check for typos

**Pre-Commit Requirements**:
- [ ] All 5 hooks installed
- [ ] All commits pass pre-commit checks
- [ ] CI/CD runs same checks
- [ ] Hooks are fast (< 5s total)
- [ ] No new warnings in clippy
- [ ] All JSON files valid

### Quality Metrics

| Metric | Target | Measurement | Enforcement |
|---------|--------|-------------|-------------|
| Code Formatting | 100% | `cargo fmt --check` | CI gate |
| Clippy Warnings | 0 warnings | `cargo clippy -- -D warnings` | CI gate |
| Test Coverage | 80%+ | `cargo tarpaulin` | CI gate |
| Mutation Score | 90%+ | `cargo-mutants` | CI gate |
| Test Flakiness | < 1% | Historical analysis | Monitoring |
| Documentation Coverage | 100% | Manual review | CI gate |

---

## 🔑 Non-Compliance Consequences

### CI/CD Failures

**Consequence**: CI/CD pipeline will fail if testing requirements are not met.

**Failure Scenarios**:
- Coverage < 80%: Pipeline fails, PR cannot be merged
- Pre-commit hooks fail: Commit blocked, must fix issues
- Clippy warnings: Pipeline fails, must fix before merge
- Missing tests: Pipeline fails, must add tests before merge

### PR Rejection Criteria

**Consequence**: PRs will be rejected if testing requirements are not met.

**Rejection Criteria**:
- No test coverage report
- Coverage < 80%
- Failing tests in CI/CD
- Missing test fixtures
- No mutation tests for critical paths
- No property-based tests for critical algorithms
- Missing BDD scenarios for critical workflows
- Failing pre-commit checks

---

## 📊 Success Metrics

### Coverage Metrics

| Metric | Target | Measurement | Status |
|---------|--------|-------------|--------|
| **Unit Test Coverage** | 85%+ | Line coverage | ✅ Required |
| **Integration Test Coverage** | 80%+ | Line coverage | ✅ Required |
| **E2E Test Coverage** | 75%+ | Scenario coverage | ✅ Required |
| **Property-Based Test Coverage** | 90%+ | Mutants killed | ✅ Required |
| **Mutation Test Coverage** | 90%+ | Mutants killed | ✅ Required |
| **Overall Project Coverage** | 80%+ | Combined coverage | ✅ Required |

### Quality Metrics

| Metric | Target | Measurement | Enforcement |
|---------|--------|-------------|-------------|
| **Test Execution Time** | < 5s | CI runtime | ✅ Required |
| **Test Flakiness** | < 1% | Flaky test rate | ✅ Required |
| **Code Review Coverage** | 100% | All PRs reviewed | ✅ Required |
| **Pre-Commit Hook Compliance** | 100% | All code formatted | ✅ Required |
| **Coverage Upload Rate** | 100% | All PRs have reports | ✅ Required |

---

## 📚 Documentation Requirements

### Test Documentation

**Requirement**: All tests must have module-level documentation.

**Required Documentation**:
```rust
/// Tests for working memory layer
///
/// ## Test Coverage
/// - Memory entry creation and retrieval
/// - Memory size limits
/// - TTL (time-to-live) expiration
/// - Concurrent access
///
/// ## Test Strategy
/// - Property-based testing for invariants
/// - Edge cases (empty content, large content)
/// - Concurrency tests
mod tests {
    // ...
}
```

**Documentation Requirements**:
- [ ] Each test module has module documentation
- [ ] Coverage sections documented
- [ ] Test strategies documented
- [ ] Fixture usage documented
- [ ] BDD scenarios documented in Gherkin syntax

---

## 🎯 Implementation Timeline

### Phase 0: Testing Infrastructure (Weeks 1-2)

**Before any code is written**:

1. Set up GitHub Actions workflows
2. Configure cargo-tarpaulin for coverage
3. Create test database setup
4. Create Docker compose for dependencies
5. Write test utilities and helpers
6. Configure pre-commit hooks
7. Set up mutation testing with cargo-mutants
8. Create test fixtures and generators

### Phase 1: Foundation Testing (Weeks 1-4)

**For each feature**:

1. Write BDD scenarios (Given/When/Then)
2. Write failing unit test (TDD - RED)
3. Run test to confirm failure
4. Write minimal implementation
5. Run test to confirm passing (TDD - GREEN)
6. Refactor code
7. Write property-based tests
8. Run mutation tests
9. Verify coverage >= 80%
10. Update documentation

### Phase 2: Integration Testing (Weeks 5-8)

**For each API endpoint**:

1. Write integration test
2. Test with mock dependencies
3. Test with real dependencies (PostgreSQL, Qdrant, Redis)
4. Test error handling
5. Test authentication/authorization
6. Test streaming responses
7. Verify coverage >= 80%
8. Update API documentation

---

## 📋 Compliance Checklist

### Testing Compliance Requirements

| Requirement | Status | Notes |
|-------------|--------|-------|
| **Test-Driven Development** | ✅ Required | TDD/BDD from Day 1 |
| **Coverage Thresholds** | ✅ Required | 80%+ overall enforced |
| **Property-Based Testing** | ✅ Required | Critical algorithms |
| **Mutation Testing** | ✅ Required | 90%+ mutants killed |
| **Testability** | ✅ Required | Dependency injection + traits |
| **Test Fixtures** | ✅ Required | All external APIs have fixtures |
| **Integration Tests** | ✅ Required | All endpoints + providers |
| **E2E Tests** | ✅ Required | 10-20 BDD scenarios |
| **CI/CD Pipeline** | ✅ Required | 4 job types + quality gates |
| **Pre-Commit Hooks** | ✅ Required | 5 hooks configured |
| **Quality Gates** | ✅ Required | Coverage + linting enforced |

---

## 📜 License

This OpenSpec testing requirements specification is licensed under **MIT License**.

Copyright © 2026 Velluma

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of Software, and to permit persons to whom Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

---

*Testing Requirements Specification v1.0.0*
*Last Updated: 2026-01-07*
*OpenSpec-Compliant*
