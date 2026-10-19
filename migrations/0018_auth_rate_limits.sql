-- Fixed-window limits for unauthenticated account and OAuth entry points.
CREATE TABLE auth_rate_limits (
    key_digest text PRIMARY KEY,
    window_start timestamptz NOT NULL,
    attempts integer NOT NULL CHECK (attempts >= 1),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX auth_rate_limits_updated ON auth_rate_limits(updated_at);
