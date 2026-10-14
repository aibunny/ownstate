-- Scope handles are convenience references exposed to clients. Bind every new
-- handle to the authenticated principal that created it so a leaked handle
-- cannot be replayed by another principal, even inside the same workspace.
--
-- Existing pre-binding handles remain NULL and are intentionally unusable by
-- the new resolver. They are append-only historical records, not upgraded into
-- capabilities after the fact.

ALTER TABLE scope_handles
  ADD COLUMN principal_id uuid;

ALTER TABLE scope_handles
  ADD CONSTRAINT scope_handles_principal_fkey
  FOREIGN KEY (tenant_id, principal_id)
  REFERENCES principals(tenant_id, id);

CREATE INDEX scope_handles_principal_scope
  ON scope_handles(tenant_id, principal_id, project_id, repository_id)
  WHERE revoked_at IS NULL AND principal_id IS NOT NULL;

-- NOT VALID preserves any historical pre-binding rows while enforcing the
-- principal requirement for every new handle written after this migration.
ALTER TABLE scope_handles
  ADD CONSTRAINT scope_handles_principal_required
  CHECK (principal_id IS NOT NULL) NOT VALID;

CREATE FUNCTION ownstate_validate_scope_handle_links() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.repository_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM repositories r
    WHERE r.tenant_id = NEW.tenant_id
      AND r.id = NEW.repository_id
      AND r.workspace_id = NEW.workspace_id
  ) THEN
    RAISE EXCEPTION 'scope handle repository is outside workspace';
  END IF;

  IF NEW.project_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM project_ownership o
    WHERE o.tenant_id = NEW.tenant_id
      AND o.project_id = NEW.project_id
      AND o.workspace_id = NEW.workspace_id
      AND o.ownership_domain = NEW.ownership_domain
      AND (NEW.repository_id IS NULL
           OR o.repository_id IS NULL
           OR o.repository_id = NEW.repository_id)
  ) THEN
    RAISE EXCEPTION 'scope handle project is outside resolved ownership';
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER scope_handles_validate_links
  BEFORE INSERT ON scope_handles
  FOR EACH ROW EXECUTE FUNCTION ownstate_validate_scope_handle_links();
