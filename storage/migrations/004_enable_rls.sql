-- Row Level Security policies for tenant isolation
-- MT-C1: Enable RLS on all tenant tables (sync_states, memory_entries, knowledge_items)

-- 1. sync_states table
ALTER TABLE sync_states ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS sync_states_tenant_isolation ON sync_states;
CREATE POLICY sync_states_tenant_isolation ON sync_states
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
