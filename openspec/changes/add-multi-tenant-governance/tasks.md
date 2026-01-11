# Tasks: Multi-Tenant Governance Architecture

## 1. Core Tenant Infrastructure

- [ ] 1.1 Create `TenantId`, `UserId`, `TenantContext` types in `core/`
- [ ] 1.2 Add `HierarchyPath` type for Company > Org > Team > Project navigation
- [ ] 1.3 Create `Role` enum (Developer, TechLead, Architect, Admin, Agent)
- [ ] 1.4 Add tenant context extraction middleware for API layer
- [ ] 1.5 Update all repository traits to accept `TenantContext` parameter

## 2. OpenFGA Integration

- [ ] 2.1 Add `openfga-rs` dependency to workspace
- [ ] 2.2 Create OpenFGA authorization model (FGA DSL) per design doc
- [ ] 2.3 Implement `AuthorizationService` trait with OpenFGA backend
- [ ] 2.4 Create relationship management APIs (add/remove user roles)
- [ ] 2.5 Add authorization checks to memory and knowledge operations
- [ ] 2.6 Implement agent-as-user delegation for LLM architects

## 3. Organizational Hierarchy

- [ ] 3.1 Create database schema for Company, Organization, Team, Project entities
- [ ] 3.2 Implement hierarchy CRUD operations
- [ ] 3.3 Add policy inheritance logic (mandatory, optional, forbidden)
- [ ] 3.4 Create hierarchy navigation queries (ancestors, descendants)
- [ ] 3.5 Add migration scripts for hierarchy tables

## 4. Governance Event System

- [ ] 4.1 Define `GovernanceEvent` enum with all event types
- [ ] 4.2 Create Redis Streams publisher for real-time events
- [ ] 4.3 Implement event persistence to PostgreSQL for audit log
- [ ] 4.4 Add event subscription mechanism for dashboards
- [ ] 4.5 Create event replay capability for missed events

## 5. Drift Detection Engine

- [ ] 5.1 Implement vector-based contradiction detection
- [ ] 5.2 Create missing policy detection logic
- [ ] 5.3 Add stale reference detection (hash comparison)
- [ ] 5.4 Implement drift score calculation formula
- [ ] 5.5 Create drift result storage and retrieval

## 6. Batch Analysis Jobs

- [ ] 6.1 Set up Tokio-based cron scheduler
- [ ] 6.2 Implement hourly quick drift scan job
- [ ] 6.3 Implement daily LLM semantic analysis job with caching
- [ ] 6.4 Implement weekly governance report generation
- [ ] 6.5 Add job status tracking and failure recovery

## 7. Governance Dashboard API

- [ ] 7.1 Create `/api/v1/governance/drift/{project_id}` endpoint
- [ ] 7.2 Create `/api/v1/governance/proposals` endpoint with filtering
- [ ] 7.3 Create `/api/v1/governance/reports/{org_id}` endpoint
- [ ] 7.4 Create proposal approval/rejection endpoints
- [ ] 7.5 Add OpenAPI documentation for governance endpoints

## 8. Deployment Mode Support

- [ ] 8.1 Create deployment mode configuration (Local, Hybrid, Remote)
- [ ] 8.2 Implement Hybrid mode sync protocol
- [ ] 8.3 Add Remote mode thin client implementation
- [ ] 8.4 Create mode-specific feature flags
- [ ] 8.5 Add mode detection and auto-configuration

## 9. Testing

- [ ] 9.1 Unit tests for tenant isolation
- [ ] 9.2 Integration tests for OpenFGA authorization
- [ ] 9.3 Tests for hierarchy inheritance
- [ ] 9.4 Tests for drift detection accuracy
- [ ] 9.5 End-to-end tests for governance workflow

## 10. Documentation

- [ ] 10.1 Update API documentation with governance endpoints
- [ ] 10.2 Create deployment guide for different modes
- [ ] 10.3 Document OpenFGA model and role definitions
- [ ] 10.4 Add troubleshooting guide for common governance issues
