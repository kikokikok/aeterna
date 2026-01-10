# Change: Add Memory Governance and Telemetry

## Why
To ensure the Memory System is enterprise-ready, we need robust data governance (PII filtering, sensitivity checks) and comprehensive telemetry (metrics, tracing) to monitor system health and performance.

## What Changes
- **Governance**: Add `GovernanceService` to redact PII and enforce sensitivity-based promotion blocks.
- **Telemetry**: Enhance `MemoryTelemetry` with specific metrics for search latency, promotion counts, and provider health.
- **Integration**: Update `MemoryManager` and `PromotionService` to leverage these new capabilities.

## Impact
- Affected specs: `memory-system`
- Affected code: `memory/src/manager.rs`, `memory/src/promotion/mod.rs`, `memory/src/governance.rs`, `memory/src/telemetry.rs`
