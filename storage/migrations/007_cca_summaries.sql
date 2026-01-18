-- CCA Summary Schema Extensions
-- Adds summary columns to memory_entries for hierarchical context compression

ALTER TABLE memory_entries 
    ADD COLUMN IF NOT EXISTS summaries JSONB DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS context_vector VECTOR(1536),
    ADD COLUMN IF NOT EXISTS summary_updated_at BIGINT;

CREATE INDEX IF NOT EXISTS idx_memory_entries_summary_updated 
    ON memory_entries(tenant_id, memory_layer, summary_updated_at)
    WHERE summary_updated_at IS NOT NULL;

ALTER TABLE knowledge_items
    ADD COLUMN IF NOT EXISTS summaries JSONB DEFAULT '{}';

CREATE TABLE IF NOT EXISTS layer_summary_cache (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    memory_layer TEXT NOT NULL,
    entry_id TEXT NOT NULL,
    depth TEXT NOT NULL,
    content TEXT NOT NULL,
    token_count INTEGER NOT NULL,
    source_hash TEXT NOT NULL,
    personalized BOOLEAN DEFAULT FALSE,
    personalization_context TEXT,
    generated_at BIGINT NOT NULL,
    expires_at BIGINT,
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id),
    FOREIGN KEY (entry_id) REFERENCES memory_entries(id) ON DELETE CASCADE,
    UNIQUE (tenant_id, entry_id, depth)
);

CREATE INDEX IF NOT EXISTS idx_layer_summary_cache_lookup 
    ON layer_summary_cache(tenant_id, memory_layer, entry_id);

CREATE INDEX IF NOT EXISTS idx_layer_summary_cache_expiry 
    ON layer_summary_cache(expires_at)
    WHERE expires_at IS NOT NULL;
