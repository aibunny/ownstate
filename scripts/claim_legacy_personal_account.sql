-- One-time ownership link for an existing Personal deployment.
-- Run only after the named signup account has verified its email address:
-- psql -v ON_ERROR_STOP=1 -v owner_email='address@example.com' \
--   -v legacy_tenant_id='00000000-0000-0000-0000-000000000001' \
--   "$DATABASE_URL" -f scripts/claim_legacy_personal_account.sql
--
-- The account's empty signup space is detached but its immutable policy rows
-- remain for audit. Existing projects, sessions, evidence, and knowledge keep
-- their IDs and tenant. No legacy credential is revoked here; transition it
-- only after the account's new OAuth or manual connection is working.

BEGIN;
SELECT set_config('ownstate.claim_email', :'owner_email', true);
SELECT set_config('ownstate.claim_tenant', :'legacy_tenant_id', true);

DO $claim$
DECLARE
    target_email text := lower(trim(current_setting('ownstate.claim_email')));
    legacy_tenant uuid := current_setting('ownstate.claim_tenant')::uuid;
    account uuid;
    old_tenant uuid;
    old_workspace uuid;
    legacy_workspace uuid;
    legacy_user uuid;
    legacy_principal uuid;
    changed int;
BEGIN
    SELECT id INTO STRICT account FROM signup_accounts
    WHERE email = target_email AND email_verified_at IS NOT NULL;

    SELECT tenant_id, workspace_id INTO STRICT old_tenant, old_workspace
    FROM account_tenant_memberships
    WHERE account_id = account AND kind = 'PERSONAL';

    IF old_tenant = legacy_tenant THEN
        RAISE NOTICE 'Account is already linked to the legacy Personal space';
        RETURN;
    END IF;

    IF EXISTS (SELECT 1 FROM projects WHERE tenant_id = old_tenant)
       OR EXISTS (SELECT 1 FROM conversation_threads WHERE tenant_id = old_tenant)
       OR EXISTS (SELECT 1 FROM repositories WHERE tenant_id = old_tenant)
       OR EXISTS (SELECT 1 FROM account_tenant_memberships
                  WHERE tenant_id = old_tenant AND account_id <> account)
       OR EXISTS (SELECT 1 FROM mcp_connections WHERE account_id = account)
       OR EXISTS (SELECT 1 FROM oauth_grants WHERE account_id = account) THEN
        RAISE EXCEPTION 'Signup space is no longer empty; manual review required';
    END IF;

    SELECT w.id, w.personal_user_id, p.id
    INTO STRICT legacy_workspace, legacy_user, legacy_principal
    FROM workspaces w
    JOIN principals p ON p.tenant_id = w.tenant_id
        AND p.kind = 'USER' AND p.subject_id = w.personal_user_id
        AND p.enabled
    WHERE w.tenant_id = legacy_tenant
      AND w.personal_user_id IS NOT NULL
      AND EXISTS (SELECT 1 FROM project_ownership o
                  WHERE o.tenant_id = w.tenant_id AND o.workspace_id = w.id)
      AND EXISTS (SELECT 1 FROM authorization_grants g
                  WHERE g.tenant_id = w.tenant_id AND g.workspace_id = w.id
                    AND g.principal_id = p.id AND g.revoked_at IS NULL
                    AND 'READ_PROJECT' = ANY(g.actions));

    IF EXISTS (SELECT 1 FROM account_tenant_memberships
               WHERE tenant_id = legacy_tenant AND workspace_id = legacy_workspace
                 AND account_id <> account) THEN
        RAISE EXCEPTION 'Legacy Personal space is already claimed';
    END IF;

    UPDATE account_tenant_memberships
    SET tenant_id = legacy_tenant,
        user_id = legacy_user,
        principal_id = legacy_principal,
        workspace_id = legacy_workspace
    WHERE account_id = account AND tenant_id = old_tenant
      AND workspace_id = old_workspace AND kind = 'PERSONAL';
    GET DIAGNOSTICS changed = ROW_COUNT;
    IF changed <> 1 THEN
        RAISE EXCEPTION 'Exactly one Personal membership must be moved';
    END IF;
END
$claim$;

COMMIT;
