-- Tighten tenant-linked policy relationships and make credential resolution
-- usable by a non-owner runtime role without granting it unscoped table reads.

ALTER TABLE authorization_grants
    ADD CONSTRAINT authorization_grants_tenant_id_id_key UNIQUE (tenant_id, id);

ALTER TABLE agent_executions
    DROP CONSTRAINT agent_executions_parent_execution_id_fkey,
    ADD CONSTRAINT agent_executions_parent_tenant_fkey
        FOREIGN KEY (tenant_id, parent_execution_id)
        REFERENCES agent_executions (tenant_id, id);

ALTER TABLE agent_grants
    DROP CONSTRAINT agent_grants_grant_id_fkey,
    DROP CONSTRAINT agent_grants_parent_grant_id_fkey,
    ADD CONSTRAINT agent_grants_grant_tenant_fkey
        FOREIGN KEY (tenant_id, grant_id)
        REFERENCES authorization_grants (tenant_id, id),
    ADD CONSTRAINT agent_grants_parent_grant_tenant_fkey
        FOREIGN KEY (tenant_id, parent_grant_id)
        REFERENCES authorization_grants (tenant_id, id);

CREATE FUNCTION ownstate_project_ownership_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' OR NEW IS DISTINCT FROM OLD THEN
        RAISE EXCEPTION 'project ownership is immutable';
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER project_ownership_immutable
BEFORE UPDATE OR DELETE ON project_ownership
FOR EACH ROW EXECUTE FUNCTION ownstate_project_ownership_immutable();

CREATE OR REPLACE FUNCTION ownstate_policy_immutable() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'policy history is append-only';
    END IF;
    -- Allow exact no-op updates (ON CONFLICT DO UPDATE SET id=EXCLUDED.id)
    IF NEW IS NOT DISTINCT FROM OLD THEN
        RETURN NEW;
    END IF;
    IF TG_TABLE_NAME = 'authorization_grants' THEN
        IF (to_jsonb(NEW) - 'revoked_at') IS DISTINCT FROM (to_jsonb(OLD) - 'revoked_at')
           OR OLD.revoked_at IS NOT NULL
           OR NEW.revoked_at IS NULL
           OR NEW.revoked_at < NEW.created_at THEN
            RAISE EXCEPTION 'grant content and revocation are immutable';
        END IF;
    END IF;
    IF TG_TABLE_NAME = 'principal_credentials' THEN
        IF (to_jsonb(NEW) - 'revoked_at') IS DISTINCT FROM (to_jsonb(OLD) - 'revoked_at')
           OR OLD.revoked_at IS NOT NULL
           OR NEW.revoked_at IS NULL
           OR NEW.revoked_at < NEW.created_at THEN
            RAISE EXCEPTION 'credential content and revocation are immutable';
        END IF;
    END IF;
    IF TG_TABLE_NAME = 'policy_decisions' THEN
        RAISE EXCEPTION 'policy decisions are append-only';
    END IF;
    RETURN NEW;
END;
$$;

CREATE FUNCTION ownstate_resolve_credential(p_digest text, p_now timestamptz)
RETURNS TABLE(principal_id uuid, tenant_id uuid, principal_kind text)
LANGUAGE sql
STABLE
SECURITY DEFINER
SET search_path = pg_catalog, public
AS $$
    SELECT p.id, p.tenant_id, p.kind
    FROM public.principal_credentials AS c
    JOIN public.principals AS p
      ON p.id = c.principal_id AND p.tenant_id = c.tenant_id
    WHERE c.credential_digest = p_digest
      AND p.enabled
      AND c.revoked_at IS NULL
      AND c.valid_from <= p_now
      AND (c.valid_until IS NULL OR c.valid_until > p_now)
$$;
