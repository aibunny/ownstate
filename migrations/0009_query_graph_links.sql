-- Immutable, evidence-backed association of canonical semantic versions to entities.
CREATE TABLE knowledge_entity_links (
    tenant_id UUID NOT NULL,
    project_id UUID NOT NULL,
    knowledge_version_id UUID NOT NULL REFERENCES knowledge_versions(id),
    entity_id UUID NOT NULL,
    entity_version_id UUID NOT NULL,
    evidence_event_ids UUID[] NOT NULL CHECK(cardinality(evidence_event_ids)>0),
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(knowledge_version_id,entity_id),
    FOREIGN KEY(tenant_id,project_id,entity_id) REFERENCES institutional_records(tenant_id,project_id,id),
    FOREIGN KEY(tenant_id,project_id,entity_version_id) REFERENCES institutional_versions(tenant_id,project_id,id)
);
CREATE INDEX knowledge_entity_scope ON knowledge_entity_links(tenant_id,project_id,entity_id,knowledge_version_id);
CREATE FUNCTION ownstate_validate_knowledge_entity_link() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    PERFORM 1 FROM institutional_records WHERE id=NEW.entity_id FOR SHARE;
    PERFORM 1 FROM institutional_versions WHERE id=NEW.entity_version_id FOR SHARE;
    IF NOT EXISTS (SELECT 1 FROM knowledge_versions kv
        JOIN knowledge_items ki ON ki.id=kv.knowledge_item_id
        JOIN institutional_records r ON r.id=NEW.entity_id
        JOIN institutional_version_data ev ON ev.id=NEW.entity_version_id AND ev.record_id=r.id
        WHERE kv.id=NEW.knowledge_version_id AND ki.tenant_id=NEW.tenant_id AND ki.project_id=NEW.project_id
        AND r.tenant_id=ki.tenant_id AND r.project_id=ki.project_id AND r.record_type='ENTITY'
        AND kv.status='ACTIVE' AND ev.status='ACTIVE'
        AND ev.valid_from<=clock_timestamp() AND (ev.valid_until IS NULL OR ev.valid_until>clock_timestamp())
        AND ownstate_classification_rank(ev.security_classification)<=ownstate_classification_rank(kv.security_classification)
        AND ev.evidence_event_ids=NEW.evidence_event_ids
        AND NOT EXISTS(SELECT 1 FROM institutional_versions h WHERE h.record_id=r.id AND h.status<>'CONFLICT'
            AND ownstate_classification_rank(h.security_classification)>ownstate_classification_rank(kv.security_classification))
        AND jsonb_typeof(kv.structured_content->'entity_ids')='array'
        AND kv.structured_content->'entity_ids' @> jsonb_build_array(NEW.entity_id::text)) THEN
        RAISE EXCEPTION 'knowledge link requires typed scoped canonical entity metadata and immutable provenance';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER knowledge_entity_link_provenance BEFORE INSERT ON knowledge_entity_links
    FOR EACH ROW EXECUTE FUNCTION ownstate_validate_knowledge_entity_link();
CREATE TRIGGER knowledge_entity_links_immutable BEFORE UPDATE OR DELETE ON knowledge_entity_links
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_evidence_mutation();

-- All existing FTS/vector/recent/final/history callers inherit current endpoint
-- safeguards. Historical contents and associations remain immutable on disk.
CREATE OR REPLACE VIEW knowledge_versions_with_safe_evidence WITH(security_invoker=true) AS
SELECT kv.* FROM knowledge_versions kv JOIN knowledge_items ki ON ki.id=kv.knowledge_item_id
WHERE (kv.structured_content->'entity_ids' IS NULL OR jsonb_typeof(kv.structured_content->'entity_ids')='array')
AND NOT EXISTS(SELECT 1 FROM knowledge_evidence ke JOIN interaction_events e ON e.id=ke.event_id
    WHERE ke.knowledge_version_id=kv.id AND (e.tenant_id<>ki.tenant_id OR e.project_id<>ki.project_id
        OR ownstate_classification_rank(e.security_classification)>ownstate_classification_rank(kv.security_classification)))
AND NOT EXISTS(SELECT 1 FROM knowledge_entity_links l WHERE l.knowledge_version_id=kv.id AND (
    l.tenant_id<>ki.tenant_id OR l.project_id<>ki.project_id
    OR NOT EXISTS(SELECT 1 FROM institutional_version_data ev WHERE ev.record_id=l.entity_id
        AND ev.tenant_id=ki.tenant_id AND ev.project_id=ki.project_id AND ev.record_type='ENTITY' AND ev.status='ACTIVE'
        AND ev.valid_from<=statement_timestamp() AND (ev.valid_until IS NULL OR ev.valid_until>statement_timestamp())
        AND ownstate_classification_rank(ev.security_classification)<=ownstate_classification_rank(kv.security_classification))
    OR EXISTS(SELECT 1 FROM institutional_versions h WHERE h.record_id=l.entity_id AND h.status<>'CONFLICT'
        AND ownstate_classification_rank(h.security_classification)>ownstate_classification_rank(kv.security_classification))))
AND NOT EXISTS(SELECT 1 FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(kv.structured_content->'entity_ids')='array'
        THEN kv.structured_content->'entity_ids' ELSE '[]'::jsonb END) ref
    WHERE NOT EXISTS(SELECT 1 FROM knowledge_entity_links l WHERE l.knowledge_version_id=kv.id AND l.entity_id::text=ref));

-- Reusable static SQL relations centralize authorization before every limit,
-- aggregation or retrieval leg. Parameters are data, never executable SQL.
CREATE FUNCTION ownstate_query_institutional(t UUID,p UUID,classes TEXT[],at_time TIMESTAMPTZ)
RETURNS SETOF institutional_version_data LANGUAGE sql STABLE AS $$
SELECT v.* FROM institutional_version_data v WHERE v.tenant_id=t AND v.project_id=p
    AND v.security_classification=ANY(classes) AND v.status IN('ACTIVE','SUPERSEDED')
    AND v.valid_from<=at_time AND (v.valid_until IS NULL OR v.valid_until>at_time)
    AND NOT EXISTS(SELECT 1 FROM institutional_versions h WHERE h.record_id=v.record_id AND h.status<>'CONFLICT'
        AND (ownstate_classification_rank(h.security_classification)>(SELECT max(ownstate_classification_rank(c)) FROM unnest(classes)c)
        OR h.status IN('REVOKED','QUARANTINED')))
    AND (v.subject_id IS NULL OR EXISTS(SELECT 1 FROM institutional_version_data ev WHERE ev.record_id=v.subject_id
        AND ev.tenant_id=t AND ev.project_id=p AND ev.record_type='ENTITY' AND ev.security_classification=ANY(classes)
        AND ev.status IN('ACTIVE','SUPERSEDED') AND ev.valid_from<=at_time AND (ev.valid_until IS NULL OR ev.valid_until>at_time)
        AND NOT EXISTS(SELECT 1 FROM institutional_versions h WHERE h.record_id=ev.record_id AND h.status<>'CONFLICT'
            AND (h.status IN('REVOKED','QUARANTINED') OR ownstate_classification_rank(h.security_classification)>
                (SELECT max(ownstate_classification_rank(c)) FROM unnest(classes)c)))))
    AND (v.target_id IS NULL OR EXISTS(SELECT 1 FROM institutional_version_data ev WHERE ev.record_id=v.target_id
        AND ev.tenant_id=t AND ev.project_id=p AND ev.record_type='ENTITY' AND ev.security_classification=ANY(classes)
        AND ev.status IN('ACTIVE','SUPERSEDED') AND ev.valid_from<=at_time AND (ev.valid_until IS NULL OR ev.valid_until>at_time)
        AND NOT EXISTS(SELECT 1 FROM institutional_versions h WHERE h.record_id=ev.record_id AND h.status<>'CONFLICT'
            AND (h.status IN('REVOKED','QUARANTINED') OR ownstate_classification_rank(h.security_classification)>
                (SELECT max(ownstate_classification_rank(c)) FROM unnest(classes)c)))))
$$;
CREATE FUNCTION ownstate_query_entities(t UUID,p UUID,classes TEXT[],at_time TIMESTAMPTZ,kind TEXT,ids UUID[],relation TEXT,targets UUID[],trusts TEXT[],sources TEXT[])
RETURNS SETOF institutional_version_data LANGUAGE sql STABLE AS $$
SELECT e.* FROM ownstate_query_institutional(t,p,classes,at_time)e WHERE e.record_type='ENTITY'
    AND (kind IS NULL OR e.proposal->>'kind'=kind) AND (cardinality(ids)=0 OR e.record_id=ANY(ids))
    AND (cardinality(trusts)=0 OR e.trust_level=ANY(trusts))
    AND (cardinality(sources)=0 OR EXISTS(SELECT 1 FROM interaction_events ev WHERE ev.id=ANY(e.evidence_event_ids) AND ev.source=ANY(sources)))
    AND (relation IS NULL OR EXISTS(SELECT 1 FROM ownstate_query_institutional(t,p,classes,at_time)r
        WHERE r.record_type='RELATIONSHIP' AND r.subject_id=e.record_id AND r.proposal->>'relationship_type'=relation
        AND (cardinality(targets)=0 OR r.target_id=ANY(targets))))
$$;
CREATE FUNCTION ownstate_query_knowledge(t UUID,p UUID,classes TEXT[],at_time TIMESTAMPTZ,kinds TEXT[],ids UUID[],trusts TEXT[],sources TEXT[],entity_kind_filter TEXT,relation TEXT,targets UUID[])
RETURNS SETOF knowledge_versions LANGUAGE sql STABLE AS $$
SELECT kv.* FROM knowledge_versions_with_safe_evidence kv JOIN knowledge_items ki ON ki.id=kv.knowledge_item_id
    WHERE ki.tenant_id=t AND ki.project_id=p AND kv.security_classification=ANY(classes)
    AND EXISTS(SELECT 1 FROM knowledge_evidence ke JOIN interaction_events ev ON ev.id=ke.event_id
        WHERE ke.knowledge_version_id=kv.id AND ev.tenant_id=t AND ev.project_id=p
        AND ownstate_classification_rank(ev.security_classification)<=ownstate_classification_rank(kv.security_classification))
    AND kv.status IN('ACTIVE','SUPERSEDED') AND kv.valid_from<=at_time AND (kv.valid_until IS NULL OR kv.valid_until>at_time)
    AND (cardinality(kinds)=0 OR ki.kind=ANY(kinds)) AND (cardinality(trusts)=0 OR kv.trust_level=ANY(trusts))
    AND (cardinality(sources)=0 OR EXISTS(SELECT 1 FROM knowledge_evidence ke WHERE ke.knowledge_version_id=kv.id AND ke.source_type=ANY(sources)))
    AND ((ids IS NULL AND entity_kind_filter IS NULL AND relation IS NULL) OR EXISTS(SELECT 1 FROM knowledge_entity_links l
        JOIN ownstate_query_entities(t,p,classes,at_time,entity_kind_filter,COALESCE(ids,'{}'::uuid[]),relation,targets,'{}'::text[],'{}'::text[])e ON e.record_id=l.entity_id
        WHERE l.knowledge_version_id=kv.id AND l.tenant_id=t AND l.project_id=p AND (ids IS NULL OR l.entity_id=ANY(ids))))
$$;
