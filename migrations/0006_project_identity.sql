-- Project names are case-insensitive identities within a tenant: "Dlala" and
-- "dlala" reached from different clients must resolve to one project.
DROP INDEX projects_tenant_name;
CREATE UNIQUE INDEX projects_tenant_name ON projects (tenant_id, lower(name));
