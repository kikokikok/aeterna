# Change: Add Memory Promotion Capability

## Why
Currently, session and agent memories are volatile and isolated. We need a way to "promote" important or frequently accessed memories to more persistent layers (User or Project) to enable long-term learning and knowledge retention across sessions.

## What Changes
- **ADDED** Memory Promotion logic: Evaluate memory importance based on scores or frequency.
- **ADDED** Promotion Bridge: Logic to move/copy memories between layers.
- **MODIFIED** `MemoryManager`: Add `promote_memory` method.
- **ADDED** `promotionThreshold` and `promoteImportant` configuration options.

## Impact
- Affected specs: `memory-system`
- Affected code: `memory/src/manager.rs`, `memory/src/governance.rs`, `memory/src/bridge.rs`
