# Change: Enforce Governance in Memory Promotion

## Why
Promotion of memories from temporary session layers to persistent project or company layers carries risk of leaking PII or sensitive information. We must strictly enforce redaction and sensitivity checks during the promotion process to maintain data governance.

## What Changes
- **MODIFIED** `PromotionService` to call `GovernanceService::redact_pii` before adding a memory to a target layer.
- **MODIFIED** `PromotionService` to call `GovernanceService::can_promote` which checks for `sensitive: true` or `private: true` flags.
- **MODIFIED** `GovernanceService` to use centralized `utils::redact_pii`.
- **ADDED** telemetry for promotion blocks and redactions.

## Impact
- Affected specs: `memory-system`
- Affected code: `memory/src/promotion/mod.rs`, `memory/src/governance.rs`, `utils/src/lib.rs`
