# Tasks: Enhanced Code Search Repository Management

## Phase 1: Foundation & Storage (3 days)
- [x] 1.1 Create `014_codesearch_repo_management.sql` migration
- [x] 1.2 Implement `repo_manager.rs` module in `storage` crate
- [x] 1.3 Define core Rust types (`Repository`, `SyncStrategy`, etc.)
- [x] 1.4 Implement `RepoStorage` with basic CRUD methods

## Phase 2: Repository Lifecycle & State Machine (1 week)
- [x] 2.1 Implement `RepoManager::request_repository` with auto-approval for local
- [x] 2.2 Implement state transition logic (REQUESTED -> APPROVED -> READY)
- [x] 2.3 Implement Git operations (Clone/Fetch) in `RepoManager`
- [x] 2.4 Handle error states and retry logic

## Phase 3: Incremental Indexing Logic (2 weeks)
- [x] 3.1 Implement File System Watcher (`watch` strategy)
- [x] 3.2 Implement Git Delta calculation (Diff between commits)
- [x] 3.3 Integration with Code Search Search indexer for incremental updates
- [x] 3.4 Multi-branch tracking support

## Phase 4: CLI & MCP Tools (1 week)
- [x] 4.1 Implement `aeterna codesearch repo` command structure
- [x] 4.2 Implement `codesearch_repo_request` MCP tool
- [x] 4.3 Add `aeterna codesearch index` command

## Phase 5: Governance & Policies (1 week)
- [x] 5.1 Integrate Cedar policy engine for request evaluation
- [x] 5.2 Set up OPAL client for real-time policy synchronization (Design provided)
- [x] 5.3 Implement `PolicyEvaluator` service that calls Cedar

## Phase 6: Secret & Identity Management (1 week)
- [x] 6.1 Define `codesearch_identities` schema and RLS
- [x] 6.2 Implement `SecretProvider` trait and AWS mock
- [x] 6.3 Implement GitHub identity management API
- [x] 6.4 Implement permission verification logic in `RepoManager`
- [x] 6.5 Integration with HashiCorp Vault for secret retrieval

## Phase 7: GitHub & GitLab Integration (1 week)
- [x] 7.1 GitHub Owner auto-detection (CODEOWNERS)
- [x] 7.2 Webhook listener for push/merge events
- [x] 7.3 PR Delta indexing via Git diff analysis

## Phase 8: Automation & Job Strategies (1 week)
- [x] 8.1 Implement background job scheduler for periodic sync (`job` strategy)
- [x] 8.2 Implement Webhook trigger handler (`hook` strategy)
- [x] 8.3 Implement manual re-indexing trigger

## Phase 9: Usage Tracking & Cleanup (3 days)
- [x] 9.1 Track search/trace usage per repository
- [x] 9.2 Implement auto-cleanup policy for inactive repositories
- [x] 9.3 Detailed audit logs for cleanup actions

## Phase 10: E2E Testing & Polish (1 week)
- [x] 10.1 Integration tests for complex workflows
- [x] 10.2 Performance benchmarking for incremental indexing
- [x] 10.3 Documentation and user guides

## Phase 11: Distributed Indexing & Scalability (1 week)
- [x] 11.1 Add shard_id and cold_storage_uri to repositories schema
- [x] 11.2 Implement ShardRouter with consistent hashing
- [x] 11.3 Implement ColdStorageManager for S3 backup/restore
- [x] 11.4 Add shard assignment to clone_repository
- [x] 11.5 Implement prepare_for_shutdown for graceful pod termination
- [x] 11.6 Add codesearch_indexer_shards table for pod registration
- [x] 11.7 Implement affinity middleware for Kubernetes ingress (Helper logic implemented)
- [x] 11.8 Add rebalancing job for scale events (Logic implemented in RepoManager)
