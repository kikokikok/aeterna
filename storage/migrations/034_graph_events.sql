-- Graph event log for event-sourced DuckDB coordination across pods.
-- Each pod tails this log and projects events into its local DuckDB instance.
-- seq is per-tenant monotonic, allocated via advisory lock in the append path.

CREATE TABLE IF NOT EXISTS graph_events (
    id          BIGSERIAL PRIMARY KEY,
    tenant_id   TEXT NOT NULL,
    seq         BIGINT NOT NULL,
    kind        TEXT NOT NULL,
    payload     JSONB NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_graph_events_tenant_seq
    ON graph_events(tenant_id, seq);

CREATE INDEX IF NOT EXISTS idx_graph_events_tenant_created
    ON graph_events(tenant_id, created_at);

ALTER TABLE graph_events ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS graph_events_tenant_isolation ON graph_events;
CREATE POLICY graph_events_tenant_isolation ON graph_events
    FOR ALL
    USING (tenant_id = current_setting('app.tenant_id', true)::text);
