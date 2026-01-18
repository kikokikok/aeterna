-- CCA Hindsight Learning Schema
-- Adds tables for error signatures, resolutions, and hindsight notes

-- Error signatures table
-- Stores normalized error patterns for semantic matching
CREATE TABLE IF NOT EXISTS error_signatures (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    error_type TEXT NOT NULL,
    message_pattern TEXT NOT NULL,
    stack_patterns JSONB DEFAULT '[]',
    context_patterns JSONB DEFAULT '[]',
    embedding JSONB,
    occurrence_count INTEGER DEFAULT 1,
    first_seen_at BIGINT NOT NULL,
    last_seen_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id)
);

CREATE INDEX IF NOT EXISTS idx_error_signatures_tenant 
    ON error_signatures(tenant_id);

CREATE INDEX IF NOT EXISTS idx_error_signatures_type 
    ON error_signatures(tenant_id, error_type);

CREATE INDEX IF NOT EXISTS idx_error_signatures_last_seen 
    ON error_signatures(tenant_id, last_seen_at DESC);

-- Resolutions table
-- Stores successful fix patterns linked to error signatures
CREATE TABLE IF NOT EXISTS resolutions (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    error_signature_id TEXT NOT NULL,
    description TEXT NOT NULL,
    changes JSONB DEFAULT '[]',
    success_rate REAL DEFAULT 0.0,
    application_count INTEGER DEFAULT 0,
    last_success_at BIGINT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id),
    FOREIGN KEY (error_signature_id) REFERENCES error_signatures(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_resolutions_error_signature 
    ON resolutions(error_signature_id);

CREATE INDEX IF NOT EXISTS idx_resolutions_success_rate 
    ON resolutions(tenant_id, success_rate DESC);

CREATE INDEX IF NOT EXISTS idx_resolutions_application_count 
    ON resolutions(tenant_id, application_count DESC);

-- Hindsight notes table
-- Stores distilled learnings from error/resolution pairs
CREATE TABLE IF NOT EXISTS hindsight_notes (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    error_signature_id TEXT NOT NULL,
    content TEXT NOT NULL,
    tags JSONB DEFAULT '[]',
    resolution_ids JSONB DEFAULT '[]',
    quality_score REAL,
    promoted_to_layer TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id),
    FOREIGN KEY (error_signature_id) REFERENCES error_signatures(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_hindsight_notes_tenant 
    ON hindsight_notes(tenant_id);

CREATE INDEX IF NOT EXISTS idx_hindsight_notes_error_signature 
    ON hindsight_notes(error_signature_id);

CREATE INDEX IF NOT EXISTS idx_hindsight_notes_quality 
    ON hindsight_notes(tenant_id, quality_score DESC)
    WHERE quality_score IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_hindsight_notes_tags 
    ON hindsight_notes USING GIN (tags);

-- Enable RLS on new tables
ALTER TABLE error_signatures ENABLE ROW LEVEL SECURITY;
ALTER TABLE resolutions ENABLE ROW LEVEL SECURITY;
ALTER TABLE hindsight_notes ENABLE ROW LEVEL SECURITY;

-- RLS policies for error_signatures
CREATE POLICY error_signatures_tenant_isolation ON error_signatures
    USING (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY error_signatures_insert_policy ON error_signatures
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY error_signatures_update_policy ON error_signatures
    FOR UPDATE USING (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY error_signatures_delete_policy ON error_signatures
    FOR DELETE USING (tenant_id = current_setting('app.tenant_id', true));

-- RLS policies for resolutions
CREATE POLICY resolutions_tenant_isolation ON resolutions
    USING (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY resolutions_insert_policy ON resolutions
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY resolutions_update_policy ON resolutions
    FOR UPDATE USING (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY resolutions_delete_policy ON resolutions
    FOR DELETE USING (tenant_id = current_setting('app.tenant_id', true));

-- RLS policies for hindsight_notes
CREATE POLICY hindsight_notes_tenant_isolation ON hindsight_notes
    USING (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY hindsight_notes_insert_policy ON hindsight_notes
    FOR INSERT WITH CHECK (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY hindsight_notes_update_policy ON hindsight_notes
    FOR UPDATE USING (tenant_id = current_setting('app.tenant_id', true));

CREATE POLICY hindsight_notes_delete_policy ON hindsight_notes
    FOR DELETE USING (tenant_id = current_setting('app.tenant_id', true));
