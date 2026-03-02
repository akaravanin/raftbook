CREATE TABLE IF NOT EXISTS event_log (
    seq BIGSERIAL PRIMARY KEY,
    event JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_event_log_created_at ON event_log (created_at);

CREATE TABLE IF NOT EXISTS command_log (
    command_id TEXT PRIMARY KEY,
    event_seq BIGINT NOT NULL UNIQUE REFERENCES event_log(seq),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_command_log_created_at ON command_log (created_at);
