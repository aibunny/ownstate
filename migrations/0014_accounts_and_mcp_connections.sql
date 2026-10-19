-- Login identities live outside tenant-local users. An account may later join
-- multiple organization tenants; each membership gets its own local principal.
CREATE TABLE signup_accounts (
    id uuid PRIMARY KEY,
    email text NOT NULL CHECK (char_length(email) BETWEEN 3 AND 320),
    password_hash text NOT NULL,
    email_verified_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (email)
);

CREATE TABLE account_tenant_memberships (
    account_id uuid NOT NULL REFERENCES signup_accounts(id),
    tenant_id uuid NOT NULL,
    user_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    workspace_id uuid NOT NULL,
    kind text NOT NULL CHECK (kind IN ('PERSONAL', 'ORGANIZATION')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (account_id, tenant_id, workspace_id),
    FOREIGN KEY (tenant_id, user_id) REFERENCES users(tenant_id, id),
    FOREIGN KEY (tenant_id, principal_id) REFERENCES principals(tenant_id, id),
    FOREIGN KEY (tenant_id, workspace_id) REFERENCES workspaces(tenant_id, id)
);

CREATE TABLE email_verification_challenges (
    token_digest text PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES signup_accounts(id),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE web_login_sessions (
    token_digest text PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES signup_accounts(id),
    expires_at timestamptz NOT NULL,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

-- A credential is accepted at /mcp only when explicitly registered here.
-- Legacy single-user deployments retain their configured MCP principal.
CREATE TABLE mcp_connections (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    account_id uuid NOT NULL REFERENCES signup_accounts(id),
    credential_digest text NOT NULL UNIQUE,
    client_name text NOT NULL CHECK (char_length(client_name) BETWEEN 1 AND 100),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    revoked_at timestamptz,
    FOREIGN KEY (tenant_id, principal_id) REFERENCES principals(tenant_id, id),
    FOREIGN KEY (credential_digest) REFERENCES principal_credentials(credential_digest)
);
CREATE INDEX mcp_connections_owner ON mcp_connections(account_id, revoked_at);

-- No model-facing tools are allowed to provision or revoke identities.
