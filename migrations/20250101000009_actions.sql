CREATE TABLE actions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    endpoint_id UUID NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
    action_type VARCHAR(50) NOT NULL,
    config JSONB NOT NULL DEFAULT '{}',
    position INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_actions_endpoint ON actions(endpoint_id);
