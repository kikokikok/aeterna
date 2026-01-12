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
- [ ] 8.2 Implement Hybrid mode sync protocol
- [ ] 8.3 Add Remote mode thin client implementation
- [ ] 8.4 Create mode-specific feature flags
- [ ] 8.5 Add mode detection and auto-configuration

## 9. Testing

 - [x] 9.1 Unit tests for tenant isolation

- [ ] 9.2 Integration tests for Permit.io + OPA/Cedar authorization
- [ ] 9.3 Tests for hierarchy inheritance
- [ ] 9.4 Tests for drift detection accuracy
- [ ] 9.5 End-to-end tests for governance workflow

## 10. Documentation

- [ ] 10.1 Update API documentation with governance endpoints
- [ ] 10.2 Create deployment guide for different modes
- [ ] 10.3 Document Permit.io + OPA/Cedar policy model and role definitions
- [ ] 10.4 Add troubleshooting guide for common governance issues
