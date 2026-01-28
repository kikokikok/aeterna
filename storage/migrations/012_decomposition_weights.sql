-- Create tables for RLM decomposition policy weights
-- Migration: 012_decomposition_weights

-- Create decomposition_policy_weights table
CREATE TABLE IF NOT EXISTS decomposition_policy_weights (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    
    -- Policy state
    action_type TEXT NOT NULL,
    weight FLOAT NOT NULL DEFAULT 0.5,
    
    -- Metadata
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    
    -- Constraints
    CHECK (weight >= 0.0 AND weight <= 1.0),
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id),
    UNIQUE (tenant_id, action_type)
);

-- Create decomposition_policy_state table
CREATE TABLE IF NOT EXISTS decomposition_policy_state (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    
    -- Policy hyperparameters
    epsilon FLOAT NOT NULL DEFAULT 0.1,
    step_count INTEGER NOT NULL DEFAULT 0,
    
    -- Training configuration
    learning_rate FLOAT NOT NULL DEFAULT 0.001,
    gamma FLOAT NOT NULL DEFAULT 0.95,
    success_weight FLOAT NOT NULL DEFAULT 1.0,
    efficiency_weight FLOAT NOT NULL DEFAULT 0.3,
    
    -- Metadata
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    
    -- Constraints
    CHECK (epsilon >= 0.01 AND epsilon <= 1.0),
    CHECK (learning_rate > 0.0 AND learning_rate <= 1.0),
    CHECK (gamma >= 0.0 AND gamma <= 1.0),
    CHECK (success_weight >= 0.0),
    CHECK (efficiency_weight >= 0.0),
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id),
    UNIQUE (tenant_id)
);

-- Create decomposition_trajectories table for training data
CREATE TABLE IF NOT EXISTS decomposition_trajectories (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    
    -- Trajectory data
    query TEXT NOT NULL,
    started_at BIGINT NOT NULL,
    completed_at BIGINT,
    
    -- Outcome and reward
    outcome JSONB,
    reward FLOAT,
    tokens_used INTEGER NOT NULL DEFAULT 0,
    max_depth INTEGER NOT NULL DEFAULT 0,
    
    -- Actions (stored as JSON array)
    actions JSONB NOT NULL DEFAULT '[]',
    
    -- Metadata
    created_at BIGINT NOT NULL,
    
    -- Constraints
    CHECK (reward >= -1.0 AND reward <= 1.0),
    CHECK (tokens_used >= 0),
    CHECK (max_depth >= 0),
    FOREIGN KEY (tenant_id) REFERENCES organizational_units(id)
);

-- Create indexes for efficient querying
CREATE INDEX IF NOT EXISTS idx_decomposition_weights_tenant_action 
ON decomposition_policy_weights(tenant_id, action_type);

CREATE INDEX IF NOT EXISTS idx_decomposition_state_tenant 
ON decomposition_policy_state(tenant_id);

CREATE INDEX IF NOT EXISTS idx_decomposition_trajectories_tenant_created 
ON decomposition_trajectories(tenant_id, created_at);

CREATE INDEX IF NOT EXISTS idx_decomposition_trajectories_completed 
ON decomposition_trajectories(completed_at) WHERE completed_at IS NOT NULL;

-- Enable Row Level Security
ALTER TABLE decomposition_policy_weights ENABLE ROW LEVEL SECURITY;
ALTER TABLE decomposition_policy_state ENABLE ROW LEVEL SECURITY;
ALTER TABLE decomposition_trajectories ENABLE ROW LEVEL SECURITY;

-- Create RLS policies
CREATE POLICY "Users can view decomposition weights in their tenant" 
ON decomposition_policy_weights FOR SELECT 
USING (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

CREATE POLICY "Users can modify decomposition weights in their tenant" 
ON decomposition_policy_weights FOR ALL 
USING (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

CREATE POLICY "Users can view decomposition state in their tenant" 
ON decomposition_policy_state FOR SELECT 
USING (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

CREATE POLICY "Users can modify decomposition state in their tenant" 
ON decomposition_policy_state FOR ALL 
USING (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

CREATE POLICY "Users can view decomposition trajectories in their tenant" 
ON decomposition_trajectories FOR SELECT 
USING (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

CREATE POLICY "Users can insert decomposition trajectories in their tenant" 
ON decomposition_trajectories FOR INSERT 
WITH CHECK (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

CREATE POLICY "Users can update decomposition trajectories in their tenant" 
ON decomposition_trajectories FOR UPDATE 
USING (tenant_id IN (
    SELECT get_accessible_tenant_ids(auth.uid())
));

-- Add comments
COMMENT ON TABLE decomposition_policy_weights IS 'Stores RLM decomposition policy weights for different action types';
COMMENT ON TABLE decomposition_policy_state IS 'Stores RLM decomposition policy state and hyperparameters';
COMMENT ON TABLE decomposition_trajectories IS 'Stores RLM decomposition trajectories for training and analysis';

COMMENT ON COLUMN decomposition_policy_weights.weight IS 'Policy weight for action selection (0.0-1.0)';
COMMENT ON COLUMN decomposition_policy_state.epsilon IS 'Exploration rate for epsilon-greedy action selection';
COMMENT ON COLUMN decomposition_policy_state.step_count IS 'Number of training steps performed';
COMMENT ON COLUMN decomposition_trajectories.reward IS 'Computed reward for trajectory (-1.0 to 1.0)';
COMMENT ON COLUMN decomposition_trajectories.actions IS 'JSON array of timestamped actions in the trajectory';