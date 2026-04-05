## 1. Implementation Checklist

### Phase 1: Database & Storage
- [x] 1.1 Create DB migration for `project_team_assignments` table with `assignment_type` (`owner`, `contributor`)
- [x] 1.2 Add `StorageBackend` trait methods for project CRUD operations
- [x] 1.3 Implement project storage queries in `storage/src/postgres.rs`
- [x] 1.4 Add `get_effective_roles_at_scope()` to `StorageBackend`
- [x] 1.5 Implement effective role computation using existing recursive CTE hierarchy queries
- [ ] 1.6 Reconcile project ID namespace in drift tables with canonical organizational-unit UUID foreign keys

### Phase 2: API Layer
- [x] 2.1 Create `cli/src/server/project_api.rs` following `org_api.rs` / `team_api.rs` router and middleware pattern
- [x] 2.2 Implement project CRUD endpoints (`POST /api/v1/projects`, `GET /api/v1/projects/{id}`, `PUT /api/v1/projects/{id}`, `DELETE /api/v1/projects/{id}`)
- [x] 2.3 Implement project member management endpoints for add/remove member role assignments
- [x] 2.4 Implement team-project assignment endpoints for owner/contributor assignments
- [x] 2.5 Mount project routes in `cli/src/server/router.rs`

### Phase 3: Authorization
- [x] 3.1 Add Cedar policies for project CRUD actions
- [x] 3.2 Enforce `CreateProject` action checks on project creation endpoint(s)
- [x] 3.3 Add project-scoped permit/forbid policies for project resources

### Phase 4: Governance Integration
- [x] 4.1 Fix `current_scope_ids()` in `cli/src/server/govern_api.rs` to resolve project context
- [x] 4.2 Update OPAL entity output in `opal-fetcher/src/entities.rs` to include project membership/assignment data
- [x] 4.3 Add project-level scope handling to governance queries and resolution paths

### Phase 5: Testing
- [x] 5.1 Add unit tests for project storage queries
- [x] 5.2 Add integration tests for project API endpoints
- [x] 5.3 Add Cedar policy tests covering project authorization decisions
- [x] 5.4 Add effective role computation tests across hierarchy ancestor scopes
