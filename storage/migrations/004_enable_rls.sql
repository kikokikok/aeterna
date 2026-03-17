-- Row Level Security policies for tenant isolation
-- MT-C1: Enable RLS on all tenant tables (sync_state, memory_entries, knowledge_items)

-- 1. sync_state table
ALTER TABLE sync_state ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS sync_state_tenant_isolation ON sync_state;
CREATE POLICY sync_state_tenant_isolation ON sync_state
    FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true)::text);

-- 2. memory_entries table
ALTER TABLE memory_entries ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS memory_entries_tenant_isolation ON memory_entries;
CREATE POLICY memory_entries_tenant_isolation ON memory_entries
    FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true)::text);

-- 3. knowledge_items table
ALTER TABLE knowledge_items ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS knowledge_items_tenant_isolation ON knowledge_items;
CREATE POLICY knowledge_items_tenant_isolation ON knowledge_items
    FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true)::text);
