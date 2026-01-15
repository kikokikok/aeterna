-- Migration 006: Event Streaming Reliability (MT-H1)
-- Adds durable event storage with write-ahead logging for governance events

-- Governance events table for write-ahead persistence
CREATE TABLE IF NOT EXISTS governance_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id VARCHAR(255) NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    tenant_id VARCHAR(255) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    payload JSONB NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    retry_count INTEGER NOT NULL DEFAULT 0,
    max_retries INTEGER NOT NULL DEFAULT 3,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    published_at TIMESTAMPTZ,
    acknowledged_at TIMESTAMPTZ,
    dead_lettered_at TIMESTAMPTZ
);

-- Indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_governance_events_tenant_id ON governance_events(tenant_id);
CREATE INDEX IF NOT EXISTS idx_governance_events_status ON governance_events(status);
CREATE INDEX IF NOT EXISTS idx_governance_events_created_at ON governance_events(created_at);
CREATE INDEX IF NOT EXISTS idx_governance_events_event_type ON governance_events(event_type);
CREATE INDEX IF NOT EXISTS idx_governance_events_idempotency ON governance_events(idempotency_key);

-- Composite index for pending event queries
CREATE INDEX IF NOT EXISTS idx_governance_events_pending 
    ON governance_events(tenant_id, status, created_at) 
    WHERE status = 'pending';

-- Dead letter events view for monitoring
CREATE OR REPLACE VIEW dead_letter_events AS
SELECT 
    id,
    event_id,
    tenant_id,
    event_type,
    payload,
    retry_count,
    last_error,
    created_at,
    dead_lettered_at
FROM governance_events
WHERE status = 'dead_lettered';

-- Event delivery metrics table
CREATE TABLE IF NOT EXISTS event_delivery_metrics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id VARCHAR(255) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    event_type VARCHAR(100) NOT NULL,
    period_start TIMESTAMPTZ NOT NULL,
    period_end TIMESTAMPTZ NOT NULL,
    total_events BIGINT NOT NULL DEFAULT 0,
    delivered_events BIGINT NOT NULL DEFAULT 0,
    retried_events BIGINT NOT NULL DEFAULT 0,
    dead_lettered_events BIGINT NOT NULL DEFAULT 0,
    avg_delivery_time_ms DOUBLE PRECISION,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for metrics queries
CREATE INDEX IF NOT EXISTS idx_event_metrics_tenant_period 
    ON event_delivery_metrics(tenant_id, period_start, period_end);

-- Consumer deduplication table for idempotent processing
CREATE TABLE IF NOT EXISTS event_consumer_state (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    consumer_group VARCHAR(255) NOT NULL,
    idempotency_key VARCHAR(255) NOT NULL,
    tenant_id VARCHAR(255) NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(consumer_group, idempotency_key)
);

-- Index for deduplication lookups
CREATE INDEX IF NOT EXISTS idx_consumer_state_lookup 
    ON event_consumer_state(consumer_group, idempotency_key);

-- Cleanup old consumer state (retention: 7 days)
CREATE INDEX IF NOT EXISTS idx_consumer_state_cleanup 
    ON event_consumer_state(processed_at);

-- Enable RLS on new tables
ALTER TABLE governance_events ENABLE ROW LEVEL SECURITY;
ALTER TABLE event_delivery_metrics ENABLE ROW LEVEL SECURITY;
ALTER TABLE event_consumer_state ENABLE ROW LEVEL SECURITY;

-- RLS policies for governance_events
CREATE POLICY governance_events_tenant_isolation ON governance_events
    FOR ALL
    USING (tenant_id = current_setting('app.current_tenant_id', true));

-- RLS policies for event_delivery_metrics
CREATE POLICY event_metrics_tenant_isolation ON event_delivery_metrics
    FOR ALL
    USING (tenant_id = current_setting('app.current_tenant_id', true));

-- RLS policies for event_consumer_state
CREATE POLICY consumer_state_tenant_isolation ON event_consumer_state
    FOR ALL
    USING (tenant_id = current_setting('app.current_tenant_id', true));

-- Function to calculate idempotency key from event
CREATE OR REPLACE FUNCTION calculate_idempotency_key(
    event_id VARCHAR,
    event_timestamp BIGINT,
    tenant_id VARCHAR
) RETURNS VARCHAR AS $$
BEGIN
    RETURN encode(sha256(
        (event_id || ':' || event_timestamp::TEXT || ':' || tenant_id)::BYTEA
    ), 'hex');
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function to move event to dead letter
CREATE OR REPLACE FUNCTION dead_letter_event(event_uuid UUID, error_message TEXT)
RETURNS VOID AS $$
BEGIN
    UPDATE governance_events
    SET 
        status = 'dead_lettered',
        last_error = error_message,
        dead_lettered_at = NOW()
    WHERE id = event_uuid;
END;
$$ LANGUAGE plpgsql;

-- Function to acknowledge event delivery
CREATE OR REPLACE FUNCTION acknowledge_event(event_uuid UUID)
RETURNS VOID AS $$
BEGIN
    UPDATE governance_events
    SET 
        status = 'acknowledged',
        acknowledged_at = NOW()
    WHERE id = event_uuid;
END;
$$ LANGUAGE plpgsql;

-- Function to retry failed event
CREATE OR REPLACE FUNCTION retry_event(event_uuid UUID, error_message TEXT)
RETURNS BOOLEAN AS $$
DECLARE
    current_retries INTEGER;
    max_allowed INTEGER;
BEGIN
    SELECT retry_count, max_retries INTO current_retries, max_allowed
    FROM governance_events
    WHERE id = event_uuid;
    
    IF current_retries >= max_allowed THEN
        PERFORM dead_letter_event(event_uuid, error_message);
        RETURN FALSE;
    ELSE
        UPDATE governance_events
        SET 
            retry_count = retry_count + 1,
            status = 'pending',
            last_error = error_message
        WHERE id = event_uuid;
        RETURN TRUE;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Comments for documentation
COMMENT ON TABLE governance_events IS 'Durable storage for governance events with write-ahead logging';
COMMENT ON TABLE event_delivery_metrics IS 'Aggregated metrics for event delivery monitoring';
COMMENT ON TABLE event_consumer_state IS 'Consumer deduplication state for idempotent processing';
COMMENT ON COLUMN governance_events.idempotency_key IS 'SHA256 hash of event_id:timestamp:tenant_id';
COMMENT ON COLUMN governance_events.status IS 'pending, published, acknowledged, dead_lettered';
