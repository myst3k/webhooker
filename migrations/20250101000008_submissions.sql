CREATE TABLE submissions (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    endpoint_id UUID NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
    data JSONB NOT NULL DEFAULT '{}',
    extras JSONB NOT NULL DEFAULT '{}',
    raw JSONB NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_submissions_endpoint ON submissions(endpoint_id);
CREATE INDEX idx_submissions_created ON submissions(created_at DESC);
