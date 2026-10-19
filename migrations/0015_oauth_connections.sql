CREATE TABLE oauth_clients (
    client_id text PRIMARY KEY,
    client_name text NOT NULL CHECK (char_length(client_name) BETWEEN 1 AND 100),
    redirect_uris text[] NOT NULL CHECK (cardinality(redirect_uris) BETWEEN 1 AND 10),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE oauth_grants (
    id uuid PRIMARY KEY,
    account_id uuid NOT NULL REFERENCES signup_accounts(id),
    client_id text NOT NULL REFERENCES oauth_clients(client_id),
    tenant_id uuid NOT NULL,
    principal_id uuid NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    revoked_at timestamptz,
    FOREIGN KEY (tenant_id, principal_id) REFERENCES principals(tenant_id, id)
);
CREATE INDEX oauth_grants_account ON oauth_grants(account_id, revoked_at);

CREATE TABLE oauth_authorization_codes (
    code_digest text PRIMARY KEY,
    grant_id uuid NOT NULL REFERENCES oauth_grants(id),
    redirect_uri text NOT NULL,
    code_challenge text NOT NULL,
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE oauth_refresh_tokens (
    token_digest text PRIMARY KEY,
    grant_id uuid NOT NULL REFERENCES oauth_grants(id),
    expires_at timestamptz NOT NULL,
    consumed_at timestamptz,
    revoked_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX oauth_refresh_grant ON oauth_refresh_tokens(grant_id, revoked_at);

ALTER TABLE mcp_connections ADD COLUMN oauth_grant_id uuid REFERENCES oauth_grants(id);
CREATE INDEX mcp_connections_grant ON mcp_connections(oauth_grant_id) WHERE oauth_grant_id IS NOT NULL;
