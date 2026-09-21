-- Additive temporal institutional state. Existing evidence/history is untouched.
ALTER TABLE projects ADD CONSTRAINT projects_tenant_id_id UNIQUE (tenant_id, id);

CREATE TABLE institutional_candidates (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    proposal JSONB NOT NULL CHECK (jsonb_typeof(proposal)='object'
        AND proposal->>'type' IS NOT NULL AND proposal->>'type' IN ('ENTITY','RELATIONSHIP','CLAIM')),
    evidence_event_ids UUID[] NOT NULL CHECK (cardinality(evidence_event_ids) > 0),
    security_classification TEXT NOT NULL CHECK (ownstate_classification_rank(security_classification) < 2147483647),
    proposed_by TEXT NOT NULL CHECK (proposed_by IN ('HUMAN','AGENT','EXTRACTOR')),
    confidence REAL CHECK (confidence >= 0 AND confidence <= 1),
    status TEXT NOT NULL CHECK (status IN ('PENDING','PROMOTED','REJECTED')),
    observed_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    result_version_id UUID,
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects (tenant_id, id)
);

CREATE TABLE institutional_records (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    record_type TEXT NOT NULL CHECK (record_type IN ('ENTITY','RELATIONSHIP','CLAIM')),
    identity_key TEXT NOT NULL,
    subject_id UUID,
    target_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (tenant_id, project_id, id),
    UNIQUE (tenant_id, project_id, record_type, identity_key),
    FOREIGN KEY (tenant_id, project_id) REFERENCES projects (tenant_id, id),
    FOREIGN KEY (tenant_id, project_id, subject_id) REFERENCES institutional_records (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, target_id) REFERENCES institutional_records (tenant_id, project_id, id),
    CHECK ((record_type='ENTITY' AND subject_id IS NULL AND target_id IS NULL)
        OR (record_type='CLAIM' AND subject_id IS NOT NULL AND target_id IS NULL)
        OR (record_type='RELATIONSHIP' AND subject_id IS NOT NULL AND target_id IS NOT NULL))
);

CREATE TABLE institutional_versions (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    record_id UUID NOT NULL,
    version_number INT NOT NULL CHECK (version_number > 0),
    proposal JSONB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE','STALE','SUPERSEDED','CONFLICT','QUARANTINED','REVOKED')),
    trust_level TEXT NOT NULL CHECK (trust_level IN ('HUMAN_EXPLICIT','SOURCE_VERIFIED','REPO_VERIFIED','AGENT_DERIVED','EXTERNAL_VERIFIED','EXTERNAL_UNTRUSTED')),
    confidence REAL CHECK (confidence >= 0 AND confidence <= 1),
    security_classification TEXT NOT NULL CHECK (ownstate_classification_rank(security_classification) < 2147483647),
    evidence_event_ids UUID[] NOT NULL CHECK (cardinality(evidence_event_ids) > 0),
    candidate_id UUID NOT NULL,
    valid_from TIMESTAMPTZ NOT NULL,
    valid_until TIMESTAMPTZ CHECK (valid_until >= valid_from),
    observed_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    supersedes_version_id UUID REFERENCES institutional_versions (id),
    UNIQUE (record_id, version_number),
    UNIQUE (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, record_id) REFERENCES institutional_records (tenant_id, project_id, id),
    FOREIGN KEY (tenant_id, project_id, candidate_id) REFERENCES institutional_candidates (tenant_id, project_id, id)
);

-- Stable external identifiers belong to identity, not its current version.
-- Revocation/staleness cannot free an identifier for a lower-classified alias.
CREATE TABLE institutional_identifiers (
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    namespace TEXT NOT NULL,
    external_id TEXT NOT NULL,
    entity_id UUID NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (tenant_id,project_id,namespace,external_id),
    FOREIGN KEY (tenant_id,project_id,entity_id) REFERENCES institutional_records(tenant_id,project_id,id)
);
CREATE TRIGGER institutional_identifiers_immutable BEFORE UPDATE OR DELETE ON institutional_identifiers
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_evidence_mutation();
CREATE FUNCTION ownstate_validate_institutional_identifier() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM institutional_records r JOIN institutional_versions v ON v.record_id=r.id
        WHERE r.id=NEW.entity_id AND r.tenant_id=NEW.tenant_id AND r.project_id=NEW.project_id
        AND r.record_type='ENTITY' AND v.status<>'CONFLICT'
        AND v.proposal->'external_ids'->>NEW.namespace=NEW.external_id) THEN
        RAISE EXCEPTION 'external identifier requires scoped canonical entity provenance';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER institutional_identifier_provenance BEFORE INSERT ON institutional_identifiers
    FOR EACH ROW EXECUTE FUNCTION ownstate_validate_institutional_identifier();
CREATE UNIQUE INDEX institutional_one_active ON institutional_versions (record_id) WHERE status='ACTIVE';
CREATE INDEX institutional_scope_time ON institutional_versions (tenant_id,project_id,valid_from,valid_until);
CREATE INDEX institutional_proposal ON institutional_versions USING gin (proposal);
CREATE INDEX institutional_edges ON institutional_records (tenant_id,project_id,subject_id,target_id);
ALTER TABLE institutional_candidates ADD CONSTRAINT institutional_candidate_result
    FOREIGN KEY (tenant_id,project_id,result_version_id) REFERENCES institutional_versions(tenant_id,project_id,id);

CREATE FUNCTION ownstate_validate_institutional_evidence() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF cardinality(NEW.evidence_event_ids) <> (SELECT count(DISTINCT e.id)
        FROM interaction_events e WHERE e.id=ANY(NEW.evidence_event_ids)
        AND e.tenant_id=NEW.tenant_id AND e.project_id=NEW.project_id
        AND ownstate_classification_rank(e.security_classification)<=ownstate_classification_rank(NEW.security_classification)) THEN
        RAISE EXCEPTION 'institutional provenance must resolve inside scope and classification';
    END IF;
    IF TG_TABLE_NAME='institutional_versions' THEN
        IF NOT EXISTS (SELECT 1 FROM institutional_candidates c
            JOIN institutional_records r ON r.id=NEW.record_id
            WHERE c.id=NEW.candidate_id AND c.status='PENDING'
            AND c.proposal=NEW.proposal AND c.evidence_event_ids=NEW.evidence_event_ids
            AND c.observed_at=NEW.observed_at AND c.confidence IS NOT DISTINCT FROM NEW.confidence
            AND r.record_type=NEW.proposal->>'type'
            AND (r.subject_id IS NULL OR EXISTS (SELECT 1 FROM institutional_versions s WHERE s.record_id=r.subject_id
                AND s.status='ACTIVE' AND s.valid_from<=clock_timestamp() AND (s.valid_until IS NULL OR s.valid_until>clock_timestamp())
                AND ownstate_classification_rank(s.security_classification)<=ownstate_classification_rank(NEW.security_classification)))
            AND (r.target_id IS NULL OR EXISTS (SELECT 1 FROM institutional_versions t WHERE t.record_id=r.target_id
                AND t.status='ACTIVE' AND t.valid_from<=clock_timestamp() AND (t.valid_until IS NULL OR t.valid_until>clock_timestamp())
                AND ownstate_classification_rank(t.security_classification)<=ownstate_classification_rank(NEW.security_classification)))
            AND (NEW.trust_level<>'HUMAN_EXPLICIT' OR c.proposed_by='HUMAN')
            AND (r.record_type='ENTITY'
                OR (r.record_type='CLAIM' AND r.subject_id=(NEW.proposal->>'subject_entity_id')::uuid)
                OR (r.record_type='RELATIONSHIP' AND r.subject_id=(NEW.proposal->>'source_entity_id')::uuid
                    AND r.target_id=(NEW.proposal->>'target_entity_id')::uuid))) THEN
            RAISE EXCEPTION 'canonical institutional content requires its pending candidate';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER institutional_candidate_evidence BEFORE INSERT ON institutional_candidates
    FOR EACH ROW EXECUTE FUNCTION ownstate_validate_institutional_evidence();
CREATE TRIGGER institutional_version_evidence BEFORE INSERT ON institutional_versions
    FOR EACH ROW EXECUTE FUNCTION ownstate_validate_institutional_evidence();

CREATE FUNCTION ownstate_validate_institutional_endpoints() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.subject_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM institutional_records
        WHERE id=NEW.subject_id AND record_type='ENTITY') THEN
        RAISE EXCEPTION 'assertion subject must be an entity';
    END IF;
    IF NEW.target_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM institutional_records
        WHERE id=NEW.target_id AND record_type='ENTITY') THEN
        RAISE EXCEPTION 'relationship target must be an entity';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER institutional_record_endpoints BEFORE INSERT ON institutional_records
    FOR EACH ROW EXECUTE FUNCTION ownstate_validate_institutional_endpoints();

CREATE FUNCTION ownstate_protect_institutional_history() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN RAISE EXCEPTION 'institutional history is immutable'; END IF;
    IF TG_TABLE_NAME='institutional_candidates' THEN
        IF OLD.status<>'PENDING' AND NEW IS DISTINCT FROM OLD THEN
            RAISE EXCEPTION 'terminal institutional candidate is immutable';
        END IF;
        IF NEW.status='PROMOTED' AND NOT EXISTS (SELECT 1 FROM institutional_versions v
            WHERE v.id=NEW.result_version_id AND v.tenant_id=NEW.tenant_id AND v.project_id=NEW.project_id
            AND v.proposal=NEW.proposal) THEN
            RAISE EXCEPTION 'promoted candidate requires its scoped canonical result';
        END IF;
        IF NEW.status<>'PROMOTED' AND NEW.result_version_id IS NOT NULL THEN
            RAISE EXCEPTION 'only promoted candidates have canonical results';
        END IF;
        IF (to_jsonb(NEW)-'status'-'result_version_id') IS DISTINCT FROM (to_jsonb(OLD)-'status'-'result_version_id') THEN
            RAISE EXCEPTION 'institutional candidate proposal is immutable';
        END IF;
    ELSE
        IF (to_jsonb(NEW)-'status'-'valid_until') IS DISTINCT FROM (to_jsonb(OLD)-'status'-'valid_until') THEN
            RAISE EXCEPTION 'institutional content and provenance are immutable';
        END IF;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER institutional_candidate_immutable BEFORE UPDATE OR DELETE ON institutional_candidates
    FOR EACH ROW EXECUTE FUNCTION ownstate_protect_institutional_history();
CREATE TRIGGER institutional_version_immutable BEFORE UPDATE OR DELETE ON institutional_versions
    FOR EACH ROW EXECUTE FUNCTION ownstate_protect_institutional_history();
CREATE TRIGGER institutional_record_immutable BEFORE UPDATE OR DELETE ON institutional_records
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_evidence_mutation();

-- JSON view is a typed transport for storage, never a model-issued query.
CREATE VIEW institutional_version_data WITH (security_invoker=true) AS
SELECT v.*,r.record_type,r.subject_id,r.target_id,
    (to_jsonb(v)-'record_id') || jsonb_build_object(
        'entity_id',CASE WHEN r.record_type='ENTITY' THEN r.id END,
        'assertion_id',CASE WHEN r.record_type<>'ENTITY' THEN r.id END) AS data
FROM institutional_versions v JOIN institutional_records r ON r.id=v.record_id
WHERE NOT EXISTS (SELECT 1 FROM unnest(v.evidence_event_ids) eid
    LEFT JOIN interaction_events e ON e.id=eid
    WHERE e.id IS NULL OR e.tenant_id<>v.tenant_id OR e.project_id<>v.project_id
    OR ownstate_classification_rank(e.security_classification)>ownstate_classification_rank(v.security_classification));
