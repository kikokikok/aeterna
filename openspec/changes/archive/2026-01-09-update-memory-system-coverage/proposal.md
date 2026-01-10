# Change: Update Memory System Coverage

## Why
We need to improve the test coverage of the memory crate to reach the 80% target. Currently, it stands at ~43%. This involves adding comprehensive tests for `MemoryManager`, `PromotionService`, and `GovernanceService`, specifically covering edge cases, error conditions, and the newly implemented promotion logic.

## What Changes
- **ADDED** unit tests for `MemoryManager` methods: `add_to_layer`, `delete_from_layer`, `get_from_layer`, `list_all_from_layer`, `promote_memory`, `promote_important_memories`, `close_session`, and `close_agent`.
- **ADDED** unit tests for `PromotionService`: `promote_layer_memories`, `evaluate_and_promote`, and `calculate_importance_score`.
- **ADDED** unit tests for `GovernanceService`: `redact_pii`, `is_sensitive`, and `can_promote`.
- **ADDED** unit tests for `mk_core` types and traits where applicable.

## Impact
- Affected specs: `memory-system`
- Affected code: `memory/src/manager.rs`, `memory/src/governance.rs`, `memory/src/promotion/mod.rs`
