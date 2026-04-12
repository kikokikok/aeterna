-- Add last_accessed_at column to memory_entries for importance decay tracking.
-- This column records when a memory was last accessed via search, enabling
-- time-based importance decay in the lifecycle manager.

ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS last_accessed_at BIGINT;

CREATE INDEX IF NOT EXISTS idx_memory_entries_last_accessed
    ON memory_entries(last_accessed_at, memory_layer)
    WHERE last_accessed_at IS NOT NULL AND deleted_at IS NULL;
