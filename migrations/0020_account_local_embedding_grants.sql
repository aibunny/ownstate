-- Earlier signup and organization grants allowed context reads but omitted
-- the two actions required to generate local query embeddings. Grants are
-- immutable, so add one narrow supplemental grant per existing membership.
INSERT INTO authorization_grants (
    id, tenant_id, principal_id, workspace_id, actions,
    classification_ceiling, allowed_destinations
)
SELECT
    ownstate_stable_uuid('account-local-embedding:' || m.tenant_id::text || ':' || m.principal_id::text || ':' || m.workspace_id::text),
    m.tenant_id,
    m.principal_id,
    m.workspace_id,
    ARRAY['GENERATE_EMBEDDING', 'MODEL_EGRESS']::text[],
    'CONFIDENTIAL',
    ARRAY['LOCAL:fastembed', 'LOCAL:deterministic']::text[]
FROM account_tenant_memberships m
WHERE NOT EXISTS (
    SELECT 1 FROM authorization_grants g
    WHERE g.tenant_id = m.tenant_id
      AND g.principal_id = m.principal_id
      AND g.workspace_id = m.workspace_id
      AND g.project_id IS NULL
      AND g.revoked_at IS NULL
      AND 'GENERATE_EMBEDDING' = ANY(g.actions)
      AND 'MODEL_EGRESS' = ANY(g.actions)
)
ON CONFLICT (id) DO NOTHING;
