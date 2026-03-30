ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS device_id TEXT;
ALTER TABLE memory_entries ADD COLUMN IF NOT EXISTS importance_score REAL DEFAULT 0.0;
CREATE INDEX IF NOT EXISTS idx_memory_entries_device_id ON memory_entries(device_id, tenant_id);
CREATE INDEX IF NOT EXISTS idx_memory_entries_updated_at ON memory_entries(updated_at, tenant_id);
