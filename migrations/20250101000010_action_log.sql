CREATE TABLE action_log (
    id UUID PRIMARY KEY DEFAULT uuidv7(),
    action_id UUID NOT NULL REFERENCES actions(id) ON DELETE CASCADE,
    submission_id UUID NOT NULL REFERENCES submissions(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL,
    response JSONB,
    executed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_action_log_action ON action_log(action_id);
CREATE INDEX idx_action_log_submission ON action_log(submission_id);
