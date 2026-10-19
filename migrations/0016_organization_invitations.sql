CREATE TABLE organization_invitations (
    token_digest text PRIMARY KEY,
    tenant_id uuid NOT NULL,
    organization_id uuid NOT NULL,
    workspace_id uuid NOT NULL,
    invited_email text NOT NULL,
    invited_by_account_id uuid NOT NULL REFERENCES signup_accounts(id),
    expires_at timestamptz NOT NULL,
    accepted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (tenant_id, organization_id) REFERENCES organizations(tenant_id, id),
    FOREIGN KEY (tenant_id, workspace_id) REFERENCES workspaces(tenant_id, id)
);
CREATE INDEX organization_invitations_recipient ON organization_invitations(invited_email, expires_at)
    WHERE accepted_at IS NULL;
