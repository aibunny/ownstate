-- Knowledge layer: untrusted candidates, canonical items, immutable versions
-- and provenance links.

CREATE TABLE candidate_knowledge (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL REFERENCES projects (id),
    session_id UUID REFERENCES sessions (id),
    kind TEXT NOT NULL CHECK (kind IN (
        'FACT', 'ARCHITECTURE', 'DECISION', 'RATIONALE', 'CONSTRAINT',
        'REQUIREMENT', 'PROCEDURE', 'PREFERENCE', 'FAILURE', 'OUTCOME',
        'GOAL', 'RISK', 'RELATIONSHIP', 'DEFINITION', 'OPEN_QUESTION'
    )),
    subject_key TEXT NOT NULL CHECK (char_length(subject_key) BETWEEN 1 AND 200),
    content TEXT NOT NULL CHECK (char_length(content) > 0),
    structured_content JSONB,
    confidence REAL CHECK (confidence >= 0 AND confidence <= 1),
    proposed_by TEXT NOT NULL
        CHECK (proposed_by IN ('HUMAN', 'AGENT', 'EXTRACTOR')),
    source TEXT,
    evidence_event_ids UUID[] NOT NULL DEFAULT '{}',
    security_classification TEXT NOT NULL DEFAULT 'INTERNAL'
        CHECK (security_classification IN
            ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED', 'SECRET')),
    status TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (status IN ('PENDING', 'PROMOTED', 'REJECTED')),
    rejection_reason TEXT,
    promoted_version_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_candidates_project_status ON candidate_knowledge (project_id, status);

CREATE TABLE knowledge_items (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL REFERENCES projects (id),
    ownership_domain TEXT NOT NULL
        CHECK (ownership_domain IN ('PERSONAL', 'ORGANIZATION')),
    kind TEXT NOT NULL CHECK (kind IN (
        'FACT', 'ARCHITECTURE', 'DECISION', 'RATIONALE', 'CONSTRAINT',
        'REQUIREMENT', 'PROCEDURE', 'PREFERENCE', 'FAILURE', 'OUTCOME',
        'GOAL', 'RISK', 'RELATIONSHIP', 'DEFINITION', 'OPEN_QUESTION'
    )),
    subject_key TEXT NOT NULL CHECK (char_length(subject_key) BETWEEN 1 AND 200),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by TEXT NOT NULL,
    -- One conceptual identity per (project, kind, subject): new information
    -- about the same subject becomes a new VERSION, not a new item.
    UNIQUE (project_id, kind, subject_key)
);

CREATE INDEX idx_items_project_kind ON knowledge_items (project_id, kind);

CREATE TABLE knowledge_versions (
    id UUID PRIMARY KEY,
    knowledge_item_id UUID NOT NULL REFERENCES knowledge_items (id),
    version_number INT NOT NULL CHECK (version_number >= 1),
    content TEXT NOT NULL CHECK (char_length(content) > 0),
    structured_content JSONB,
    status TEXT NOT NULL CHECK (status IN (
        'CANDIDATE', 'ACTIVE', 'STALE', 'SUPERSEDED',
        'CONFLICT', 'QUARANTINED', 'REVOKED'
    )),
    trust_level TEXT NOT NULL CHECK (trust_level IN (
        'HUMAN_EXPLICIT', 'SOURCE_VERIFIED', 'REPO_VERIFIED',
        'AGENT_DERIVED', 'EXTERNAL_VERIFIED', 'EXTERNAL_UNTRUSTED'
    )),
    confidence REAL CHECK (confidence >= 0 AND confidence <= 1),
    security_classification TEXT NOT NULL DEFAULT 'INTERNAL'
        CHECK (security_classification IN
            ('PUBLIC', 'INTERNAL', 'CONFIDENTIAL', 'RESTRICTED', 'SECRET')),
    valid_from TIMESTAMPTZ NOT NULL DEFAULT now(),
    valid_until TIMESTAMPTZ,
    supersedes_version_id UUID REFERENCES knowledge_versions (id),
    candidate_id UUID REFERENCES candidate_knowledge (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by TEXT NOT NULL,
    -- Full-text index feed; kept in sync by PostgreSQL itself.
    content_tsv TSVECTOR GENERATED ALWAYS AS (to_tsvector('english', content)) STORED,
    UNIQUE (knowledge_item_id, version_number)
);

-- At most one ACTIVE version per item, enforced structurally.
CREATE UNIQUE INDEX one_active_version_per_item
    ON knowledge_versions (knowledge_item_id) WHERE status = 'ACTIVE';

CREATE INDEX idx_versions_item ON knowledge_versions (knowledge_item_id);
CREATE INDEX idx_versions_tsv ON knowledge_versions USING gin (content_tsv);

-- A written version's content is history. Lifecycle fields (status,
-- valid_until) may change; everything else is frozen by the database.
CREATE FUNCTION ownstate_protect_knowledge_version() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'knowledge_versions are immutable (DELETE forbidden)';
    END IF;
    IF NEW.id IS DISTINCT FROM OLD.id
        OR NEW.knowledge_item_id IS DISTINCT FROM OLD.knowledge_item_id
        OR NEW.version_number IS DISTINCT FROM OLD.version_number
        OR NEW.content IS DISTINCT FROM OLD.content
        OR NEW.structured_content IS DISTINCT FROM OLD.structured_content
        OR NEW.trust_level IS DISTINCT FROM OLD.trust_level
        OR NEW.confidence IS DISTINCT FROM OLD.confidence
        OR NEW.security_classification IS DISTINCT FROM OLD.security_classification
        OR NEW.valid_from IS DISTINCT FROM OLD.valid_from
        OR NEW.supersedes_version_id IS DISTINCT FROM OLD.supersedes_version_id
        OR NEW.candidate_id IS DISTINCT FROM OLD.candidate_id
        OR NEW.created_at IS DISTINCT FROM OLD.created_at
        OR NEW.created_by IS DISTINCT FROM OLD.created_by
    THEN
        RAISE EXCEPTION
            'knowledge_versions content is immutable: only status and valid_until may change';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER knowledge_versions_immutable
    BEFORE UPDATE OR DELETE ON knowledge_versions
    FOR EACH ROW EXECUTE FUNCTION ownstate_protect_knowledge_version();

CREATE TABLE knowledge_evidence (
    id UUID PRIMARY KEY,
    knowledge_version_id UUID NOT NULL REFERENCES knowledge_versions (id),
    event_id UUID REFERENCES interaction_events (id),
    source_type TEXT NOT NULL DEFAULT 'INTERACTION_EVENT',
    content_hash TEXT,
    repository TEXT,
    commit_sha TEXT,
    file_path TEXT,
    line_start INT,
    line_end INT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Evidence must point at something.
    CHECK (event_id IS NOT NULL OR content_hash IS NOT NULL OR file_path IS NOT NULL)
);

CREATE INDEX idx_evidence_version ON knowledge_evidence (knowledge_version_id);
CREATE INDEX idx_evidence_event ON knowledge_evidence (event_id);

-- Provenance links are as immutable as the versions they justify.
CREATE FUNCTION ownstate_forbid_evidence_mutation() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'knowledge_evidence is append-only (% forbidden)', TG_OP;
END;
$$;

CREATE TRIGGER knowledge_evidence_append_only
    BEFORE UPDATE OR DELETE ON knowledge_evidence
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_evidence_mutation();

-- The promoted_version_id back-reference is added after knowledge_versions
-- exists (forward declaration was not possible above).
ALTER TABLE candidate_knowledge
    ADD CONSTRAINT fk_candidate_promoted_version
    FOREIGN KEY (promoted_version_id) REFERENCES knowledge_versions (id);
