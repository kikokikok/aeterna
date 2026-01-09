## 1. Governance Implementation
- [x] 1.1 Implement `GovernanceService` in `memory/src/governance.rs`
- [x] 1.2 Implement PII redaction logic (email, API keys)
- [x] 1.3 Implement sensitivity check logic
- [x] 1.4 Integrate `GovernanceService` into `PromotionService`
- [x] 1.5 Write unit tests for governance logic

## 2. Telemetry Enhancement
- [x] 2.1 Update `MemoryTelemetry` in `memory/src/telemetry.rs` with new metrics
- [x] 2.2 Add promotion-specific metrics
- [x] 2.3 Integrate telemetry calls into `MemoryManager` search/add/delete
- [x] 2.4 Integrate telemetry calls into `PromotionService`

## 3. Integration & Cleanup
- [x] 3.1 Update `MemoryManager` to initialize Governance and Telemetry
- [x] 3.2 Fix unused import warning in `memory/src/providers/qdrant.rs`
- [x] 3.3 Ensure MCP tools call `close_session` on termination
- [x] 3.4 Run all tests and verify metrics output
