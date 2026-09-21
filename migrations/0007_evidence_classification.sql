-- Additive defense for event-derived classification and provenance. Existing
-- history is left untouched; unsafe legacy versions are excluded on read.

CREATE FUNCTION ownstate_classification_rank(classification TEXT) RETURNS INT
LANGUAGE SQL IMMUTABLE STRICT PARALLEL SAFE AS $$
    SELECT CASE classification
        WHEN 'PUBLIC' THEN 0
        WHEN 'INTERNAL' THEN 1
        WHEN 'CONFIDENTIAL' THEN 2
        WHEN 'RESTRICTED' THEN 3
        WHEN 'SECRET' THEN 4
        ELSE 2147483647
    END
$$;

-- This view handles all statuses, including version history. Each calling
-- query must still filter its tenant/project/classification/lifecycle scope.
-- security_invoker preserves future RLS on the underlying tables.
CREATE VIEW knowledge_versions_with_safe_evidence
WITH (security_invoker = true) AS
SELECT kv.*
FROM knowledge_versions kv
JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
WHERE NOT EXISTS (
    SELECT 1
    FROM knowledge_evidence ke
    JOIN interaction_events e ON e.id = ke.event_id
    WHERE ke.knowledge_version_id = kv.id
      AND (
          e.tenant_id <> ki.tenant_id
          OR e.project_id <> ki.project_id
          OR ownstate_classification_rank(e.security_classification)
             > ownstate_classification_rank(kv.security_classification)
      )
);

CREATE FUNCTION ownstate_validate_event_evidence() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.event_id IS NOT NULL AND NOT EXISTS (
        SELECT 1
        FROM knowledge_versions kv
        JOIN knowledge_items ki ON ki.id = kv.knowledge_item_id
        JOIN interaction_events e ON e.id = NEW.event_id
        WHERE kv.id = NEW.knowledge_version_id
          AND e.tenant_id = ki.tenant_id
          AND e.project_id = ki.project_id
          AND ownstate_classification_rank(e.security_classification)
              <= ownstate_classification_rank(kv.security_classification)
    ) THEN
        RAISE EXCEPTION 'event evidence violates knowledge scope or classification';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER knowledge_evidence_scope_and_classification
    BEFORE INSERT ON knowledge_evidence
    FOR EACH ROW EXECUTE FUNCTION ownstate_validate_event_evidence();
