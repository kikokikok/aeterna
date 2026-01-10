# Change: Add Importance Scoring Algorithm

## Why
To enable automated memory promotion, we need a robust way to determine which memories are worth keeping. A simple threshold isn't enough; we need to account for how often a memory is used (frequency) and how recently it was last used (recency), in addition to any explicit importance score.

## What Changes
- **MODIFIED** `MemoryEntry` metadata to include `access_count` and `last_accessed_at`.
- **ADDED** `calculate_importance_score` in `PromotionService`.
- **MODIFIED** `MemoryManager::get_from_layer` to automatically track access metadata.
- **ADDED** logic to weighted combine explicit score (60%), frequency (30%), and recency (10%).

## Impact
- Affected specs: `memory-system`
- Affected code: `memory/src/manager.rs`, `memory/src/promotion/mod.rs`
