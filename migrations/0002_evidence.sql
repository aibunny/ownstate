-- Evidence layer: projects, sessions and append-only interaction events.
--
-- Every project-scoped row carries tenant_id so PostgreSQL Row Level Security
-- can be layered on without reshaping the schema.

CREATE TABLE projects (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    ownership_domain TEXT NOT NULL
        CHECK (ownership_domain IN ('PERSONAL', 'ORGANIZATION')),
    name TEXT NOT NULL CHECK (char_length(name) BETWEEN 1 AND 200),
    description TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_projects_tenant ON projects (tenant_id);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL REFERENCES projects (id),
    source TEXT NOT NULL CHECK (char_length(source) BETWEEN 1 AND 100),
    source_session_id TEXT,
    agent TEXT,
    provider TEXT,
    model TEXT,
    status TEXT NOT NULL DEFAULT 'ACTIVE'
        CHECK (status IN ('ACTIVE', 'COMPLETED', 'ABANDONED')),
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    ended_at TIMESTAMPTZ,
    repository TEXT,
    initial_commit TEXT,
    final_commit TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_sessions_project ON sessions (project_id);

CREATE TABLE interaction_events (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL REFERENCES projects (id),
    session_id UUID NOT NULL REFERENCES sessions (id),
    source TEXT NOT NULL,
    source_event_id TEXT,
    event_type TEXT NOT NULL CHECK (event_type IN (
        'USER_MESSAGE', 'ASSISTANT_MESSAGE',
        'TOOL_CALL', 'TOOL_RESULT',
        'COMMAND', 'COMMAND_RESULT',
        'FILE_READ', 'FILE_WRITE',
        'SEARCH', 'SEARCH_RESULT',
        'DOCUMENT_READ', 'ARTIFACT_CREATED',
        'GIT_DIFF', 'TEST_RESULT',
        'AGENT_RESULT', 'SYSTEM_EVENT'
    )),
    actor_type TEXT NOT NULL
        CHECK (actor_type IN ('USER', 'ASSISTANT', 'TOOL', 'SYSTEM')),
    actor_id TEXT,
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    model_provider TEXT,
    model_name TEXT,
    content TEXT,
    content_hash TEXT,
    tool_name TEXT,
    tool_call_id TEXT,
    repository TEXT,
    branch TEXT,
    commit_sha TEXT,
    file_path TEXT,
    security_classification TEXT NOT NULL DEFAULT 'INTERNAL'
        CHECK (security_classification IN
            ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED', 'SECRET')),
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (session_id, sequence)
);

CREATE INDEX idx_events_project ON interaction_events (project_id);

-- Raw evidence is append-only. Extraction logic reinterpreting history must
-- never rewrite it; the database enforces the invariant, not convention.
CREATE FUNCTION ownstate_forbid_event_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'interaction_events are append-only (% forbidden)', TG_OP;
END;
$$;

CREATE TRIGGER interaction_events_append_only
    BEFORE UPDATE OR DELETE ON interaction_events
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_event_mutation();
