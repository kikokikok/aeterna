# Tasks: Multi-Tenant Governance Architecture

## 1. Core Tenant Infrastructure

- [x] 1.1 Create `TenantId`, `UserId`, `TenantContext` types in `mk_core/`
- [x] 1.2 Add `HierarchyPath` type for Company > Org > Team > Project navigation
- [x] 1.3 Create `Role` enum (Developer, TechLead, Architect, Admin, Agent)
 - [x] 1.4 Add tenant context extraction middleware for API layer
 - [x] 1.5 Update `SyncStatePersister` trait to accept `TenantId` parameter
 - [x] 1.6 Update `SyncManager` to store per-tenant states (`HashMap<TenantId, SyncState>`)
 - [x] 1.7 Update all `MockPersister` implementations across test files
- [x] 1.8 Update memory tools to require `TenantContext` in API calls
- [x] 1.9 Update remaining repository traits (`KnowledgeRepository`, `MemoryProvider`) to enforce tenant context
- [x] 1.10 Align GovernanceEngine event publishing API with tool expectations


## 2. Permit.io + OPA/Cedar Integration

- [x] 2.1 Add `permit-io-rs` SDK and OPA client dependencies
- [x] 2.2 Create OPA/Cedar policy files for authorization model per design doc
- [x] 2.3 Implement `AuthorizationService` trait with Permit.io + OPA/Cedar backend
- [x] 2.4 Create relationship management APIs (add/remove user roles)
- [x] 2.5 Add authorization checks to memory and knowledge operations
- [x] 2.6 Implement agent-as-user delegation for LLM architects

## 3. Organizational Hierarchy

- [x] 3.1 Create database schema for Company, Organization, Team, Project entities
- [x] 3.2 Implement hierarchy CRUD operations
- [x] 3.3 Add policy inheritance logic (mandatory, optional, forbidden)
- [x] 3.4 Create hierarchy navigation queries (ancestors, descendants)
- [x] 3.5 Add migration scripts for hierarchy tables

## 4. Governance Event System

- [x] 4.1 Define `GovernanceEvent` enum with all event types
- [x] 4.2 Create Redis Streams publisher for real-time events
- [x] 4.3 Implement event persistence to PostgreSQL for audit log
- [x] 4.4 Add event subscription mechanism for dashboards
- [x] 4.5 Create event replay capability for missed events

## 5. Drift Detection Engine

- [x] 5.1 Implement vector-based contradiction detection
- [x] 5.2 Create missing policy detection logic
- [x] 5.3 Add stale reference detection (hash comparison)
- [x] 5.4 Implement drift score calculation formula
- [x] 5.5 Create drift result storage and retrieval

## 6. Batch Analysis Jobs

- [x] 6.1 Set up Tokio-based cron scheduler
- [x] 6.2 Implement hourly quick drift scan job
- [x] 6.3 Implement daily LLM semantic analysis job with caching
- [x] 6.4 Implement weekly governance report generation
- [x] 6.5 Add job status tracking and failure recovery

## 7. Governance Dashboard API

- [x] 7.1 Create `/api/v1/governance/drift/{project_id}` endpoint
- [x] 7.2 Create `/api/v1/governance/proposals` endpoint with filtering
- [x] 7.3 Create `/api/v1/governance/reports/{org_id}` endpoint
- [x] 7.4 Create proposal approval/rejection endpoints
- [x] 7.5 Add OpenAPI documentation for governance endpoints
- [x] 7.6 Create `/api/v1/governance/jobs` endpoint for status tracking

## 8. Deployment Mode Support

- [x] 8.1 Create deployment mode configuration (Local, Hybrid, Remote)
- [x] 8.2 Implement Hybrid mode sync protocol
- [x] 8.3 Add Remote mode thin client implementation
- [x] 8.4 Create mode-specific feature flags
- [x] 8.5 Add mode detection and auto-configuration

## 9. Testing

- [x] 9.1 Unit tests for tenant isolation
- [x] 9.2 Integration tests for Permit.io + OPA/Cedar authorization
- [x] 9.3 Tests for hierarchy inheritance
- [x] 9.4 Tests for drift detection accuracy
- [x] 9.5 End-to-end tests for governance workflow

## 10. Documentation

- [x] 10.1 Update API documentation with governance endpoints
- [x] 10.2 Create deployment guide for different modes
- [x] 10.3 Document Permit.io + OPA/Cedar policy model and role definitions
- [x] 10.4 Add troubleshooting guide for common governance issues

---

## 11. Production Gap Requirements

### 11.1 Tenant Data Isolation Security (MT-C1) - CRITICAL
- [ ] 11.1.1 Audit all SQL queries for parameterization
- [ ] 11.1.2 Implement query builder with mandatory tenant_id parameter
- [ ] 11.1.3 Create PostgreSQL RLS policies for all tenant tables
- [ ] 11.1.4 Enable RLS on memory_entries, knowledge_items, sync_states tables
- [ ] 11.1.5 Add RLS policy tests in integration test suite
- [ ] 11.1.6 Create penetration test suite for cross-tenant access
- [ ] 11.1.7 Document penetration test procedures and results format
- [ ] 11.1.8 Add automated cross-tenant access checks to CI

### 11.2 RBAC Policy Testing (MT-C2) - CRITICAL
- [ ] 11.2.1 Create RBAC test matrix covering all role-action-resource combinations
- [ ] 11.2.2 Implement positive authorization tests (allowed actions)
- [ ] 11.2.3 Implement negative authorization tests (denied actions)
- [ ] 11.2.4 Add privilege escalation prevention tests
- [ ] 11.2.5 Add role hierarchy enforcement tests
- [ ] 11.2.6 Create permission matrix generator script
- [ ] 11.2.7 Add matrix review step to deployment pipeline
- [ ] 11.2.8 Document RBAC testing procedures

### 11.3 Drift Detection Tuning (MT-C3) - CRITICAL
- [ ] 11.3.1 Add `drift_threshold` config option per project (default: 0.2)
- [ ] 11.3.2 Create `drift_suppressions` table in PostgreSQL
- [ ] 11.3.3 Implement suppression rule API (create, list, delete)
- [ ] 11.3.4 Add confidence scoring to drift detection results
- [ ] 11.3.5 Implement confidence calculation based on embedding quality
- [ ] 11.3.6 Add low-confidence drift flagging for manual review
- [ ] 11.3.7 Update drift reports to show suppressed vs active drifts
- [ ] 11.3.8 Write drift tuning documentation

### 11.4 Event Streaming Reliability (MT-H1) - HIGH
- [ ] 11.4.1 Add PostgreSQL `governance_events` table for durability
- [ ] 11.4.2 Implement write-ahead persistence before Redis publish
- [ ] 11.4.3 Add idempotency key to all events (event_id + timestamp hash)
- [ ] 11.4.4 Implement consumer deduplication using idempotency keys
- [ ] 11.4.5 Create dead letter stream in Redis
- [ ] 11.4.6 Implement DLQ processing job with alerting
- [ ] 11.4.7 Add event delivery metrics (delivered, retried, dead-lettered)
- [ ] 11.4.8 Write event reliability tests

### 11.5 Batch Job Coordination (MT-H2) - HIGH
- [ ] 11.5.1 Implement Redis-based distributed lock for batch jobs
- [ ] 11.5.2 Add lock TTL configuration (default: 35 minutes)
- [ ] 11.5.3 Implement job deduplication check before execution
- [ ] 11.5.4 Add skip event logging with reason
- [ ] 11.5.5 Implement graceful job termination on timeout
- [ ] 11.5.6 Add partial result persistence for long-running jobs
- [ ] 11.5.7 Create job coordination metrics (runs, skips, timeouts)
- [ ] 11.5.8 Write job coordination tests

### 11.6 Tenant Context Safety (MT-H3) - HIGH
- [ ] 11.6.1 Create `RequireTenantContext` middleware
- [ ] 11.6.2 Apply middleware to all API routes
- [ ] 11.6.3 Implement fail-closed policy (reject on extraction failure)
- [ ] 11.6.4 Add TenantContext to all operation logs
- [ ] 11.6.5 Create audit log reconstruction tool
- [ ] 11.6.6 Add context propagation tests
- [ ] 11.6.7 Document tenant context requirements for all operations

### 11.7 Authorization Fallback (MT-H4) - HIGH
- [ ] 11.7.1 Implement local policy cache with configurable TTL
- [ ] 11.7.2 Add cache refresh logic on auth service recovery
- [ ] 11.7.3 Create OPA fallback authorization adapter
- [ ] 11.7.4 Create Cedar fallback authorization adapter
- [ ] 11.7.5 Implement policy sync between Permit.io and local
- [ ] 11.7.6 Add graceful degradation mode logging
- [ ] 11.7.7 Define read vs write operation behavior during degradation
- [ ] 11.7.8 Write authorization fallback tests

### 11.8 Dashboard API Security (MT-H5) - HIGH
- [ ] 11.8.1 Implement JWT validation middleware for dashboard endpoints
- [ ] 11.8.2 Add token expiration checking
- [ ] 11.8.3 Integrate with OPAL authentication
- [ ] 11.8.4 Implement API key rotation mechanism
- [ ] 11.8.5 Configure CORS with explicit allowed origins
- [ ] 11.8.6 Block wildcard origins in production
- [ ] 11.8.7 Add security headers (HSTS, CSP, etc.)
- [ ] 11.8.8 Write dashboard security tests

---

## Summary

| Section | Tasks | Description |
|---------|-------|-------------|
| 1 | 10 | Core Tenant Infrastructure |
| 2 | 6 | Permit.io + OPA/Cedar Integration |
| 3 | 5 | Organizational Hierarchy |
| 4 | 5 | Governance Event System |
| 5 | 5 | Drift Detection Engine |
| 6 | 5 | Batch Analysis Jobs |
| 7 | 6 | Governance Dashboard API |
| 8 | 5 | Deployment Mode Support |
| 9 | 5 | Testing |
| 10 | 4 | Documentation |
| 11 | 64 | Production Gap Requirements (MT-C1 to MT-H5) |
| **Total** | **120** | |

**Estimated effort**: 5-6 weeks with 80% test coverage target

---

## Production Gap Tracking

| Gap ID | Priority | Requirement | Tasks |
|--------|----------|-------------|-------|
| MT-C1 | Critical | Tenant Data Isolation Security | 11.1.1-11.1.8 |
| MT-C2 | Critical | RBAC Policy Testing | 11.2.1-11.2.8 |
| MT-C3 | Critical | Drift Detection Tuning | 11.3.1-11.3.8 |
| MT-H1 | High | Event Streaming Reliability | 11.4.1-11.4.8 |
| MT-H2 | High | Batch Job Coordination | 11.5.1-11.5.8 |
| MT-H3 | High | Tenant Context Safety | 11.6.1-11.6.7 |
| MT-H4 | High | Authorization Fallback | 11.7.1-11.7.8 |
| MT-H5 | High | Dashboard API Security | 11.8.1-11.8.8 |
