-- A Personal space can belong to one signup account only. Organization
-- workspaces intentionally allow many account memberships.
CREATE UNIQUE INDEX account_personal_workspace_single_owner
    ON account_tenant_memberships (tenant_id, workspace_id)
    WHERE kind = 'PERSONAL';
