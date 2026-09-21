-- Project names are identities within a tenant, so agents can address a
-- project by its workspace directory name (get-or-create without races).
CREATE UNIQUE INDEX projects_tenant_name ON projects (tenant_id, name);
