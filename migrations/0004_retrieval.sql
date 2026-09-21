-- Retrieval index, context audit and the PostgreSQL-backed job queue.

-- Embeddings are an index over canonical knowledge, keyed by the model that
-- produced them so re-embedding never rewrites knowledge. The vector column
-- is sized for the initial local model (384 dims); changing models with a
-- different dimensionality is an explicit migration.
CREATE TABLE embeddings (
    id UUID PRIMARY KEY,
    entity_type TEXT NOT NULL CHECK (entity_type IN ('KNOWLEDGE_VERSION')),
    entity_id UUID NOT NULL,
    model TEXT NOT NULL,
    model_version TEXT NOT NULL,
    dimensions INT NOT NULL CHECK (dimensions > 0),
    embedding VECTOR(384) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (entity_type, entity_id, model)
);

-- Default HNSW parameters on purpose; tune only with evidence.
CREATE INDEX idx_embeddings_vec ON embeddings
    USING hnsw (embedding vector_cosine_ops);

-- Audit of exactly which knowledge versions were compiled for an AI.
CREATE TABLE context_packets (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL REFERENCES projects (id),
    session_id UUID REFERENCES sessions (id),
    provider TEXT,
    model TEXT,
    agent TEXT,
    task TEXT NOT NULL,
    max_items INT NOT NULL CHECK (max_items > 0),
    token_budget BIGINT CHECK (token_budget IS NULL OR token_budget > 0),
    max_classification TEXT NOT NULL
        CHECK (max_classification IN
            ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED', 'SECRET')),
    knowledge_version_ids UUID[] NOT NULL,
    estimated_tokens BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_packets_project ON context_packets (project_id, created_at);

CREATE FUNCTION ownstate_forbid_packet_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'context_packets are append-only (% forbidden)', TG_OP;
END;
$$;

CREATE TRIGGER context_packets_append_only
    BEFORE UPDATE OR DELETE ON context_packets
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_packet_mutation();

-- Durable job queue; workers claim with FOR UPDATE SKIP LOCKED.
CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('GENERATE_EMBEDDING')),
    payload JSONB NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'RUNNING', 'SUCCEEDED', 'FAILED')),
    attempts INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 5 CHECK (max_attempts >= 1),
    run_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_jobs_claimable ON jobs (run_at) WHERE status = 'PENDING';
