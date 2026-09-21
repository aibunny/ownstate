-- Additive ownership and default-deny principal policy. Existing canonical and
-- evidence rows, ids and contents are never rewritten.
CREATE FUNCTION ownstate_stable_uuid(label text) RETURNS uuid LANGUAGE sql IMMUTABLE AS $$
SELECT (substr(md5(label),1,8)||'-'||substr(md5(label),9,4)||'-'||substr(md5(label),13,4)||'-'||substr(md5(label),17,4)||'-'||substr(md5(label),21,12))::uuid
$$;
CREATE TABLE users(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,display_name text NOT NULL,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_id,id));
CREATE TABLE organizations(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,name text NOT NULL,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_id,id));
CREATE TABLE organization_memberships(tenant_id uuid NOT NULL,organization_id uuid NOT NULL,user_id uuid NOT NULL,role text NOT NULL CHECK(role IN('MEMBER','ADMIN','OWNER')),created_at timestamptz NOT NULL DEFAULT clock_timestamp(),PRIMARY KEY(organization_id,user_id),FOREIGN KEY(tenant_id,organization_id) REFERENCES organizations(tenant_id,id),FOREIGN KEY(tenant_id,user_id) REFERENCES users(tenant_id,id));
CREATE TABLE workspaces(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,name text NOT NULL,personal_user_id uuid,organization_id uuid,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_id,id),CHECK((personal_user_id IS NULL)<>(organization_id IS NULL)),FOREIGN KEY(tenant_id,personal_user_id) REFERENCES users(tenant_id,id),FOREIGN KEY(tenant_id,organization_id) REFERENCES organizations(tenant_id,id));
CREATE TABLE repositories(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,workspace_id uuid NOT NULL,canonical_origin text NOT NULL,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_id,workspace_id,canonical_origin),UNIQUE(tenant_id,id),FOREIGN KEY(tenant_id,workspace_id) REFERENCES workspaces(tenant_id,id));
CREATE TABLE project_ownership(tenant_id uuid NOT NULL,project_id uuid PRIMARY KEY,workspace_id uuid NOT NULL,repository_id uuid,ownership_domain text NOT NULL CHECK(ownership_domain IN('PERSONAL','ORGANIZATION')),recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),FOREIGN KEY(tenant_id,project_id) REFERENCES projects(tenant_id,id),FOREIGN KEY(tenant_id,workspace_id) REFERENCES workspaces(tenant_id,id),FOREIGN KEY(tenant_id,repository_id) REFERENCES repositories(tenant_id,id));
CREATE FUNCTION ownstate_validate_project_ownership() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM projects p JOIN workspaces w ON w.id=NEW.workspace_id AND w.tenant_id=p.tenant_id WHERE p.id=NEW.project_id AND p.tenant_id=NEW.tenant_id AND p.ownership_domain=NEW.ownership_domain AND ((NEW.ownership_domain='PERSONAL' AND w.personal_user_id IS NOT NULL) OR (NEW.ownership_domain='ORGANIZATION' AND w.organization_id IS NOT NULL))) THEN RAISE EXCEPTION 'project ownership domain must match workspace owner'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER project_ownership_valid BEFORE INSERT ON project_ownership FOR EACH ROW EXECUTE FUNCTION ownstate_validate_project_ownership();
CREATE INDEX project_ownership_scope ON project_ownership(tenant_id,workspace_id,project_id);
CREATE TABLE principals(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,kind text NOT NULL CHECK(kind IN('USER','SERVICE','AGENT')),subject_id uuid NOT NULL,enabled boolean NOT NULL DEFAULT true,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_id,id),UNIQUE(tenant_id,kind,subject_id));
CREATE TABLE principal_credentials(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,principal_id uuid NOT NULL,credential_digest text NOT NULL UNIQUE,valid_from timestamptz NOT NULL DEFAULT clock_timestamp(),valid_until timestamptz,revoked_at timestamptz,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),CHECK(valid_until IS NULL OR valid_until>valid_from),FOREIGN KEY(tenant_id,principal_id) REFERENCES principals(tenant_id,id));
CREATE TABLE authorization_grants(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,principal_id uuid NOT NULL,workspace_id uuid NOT NULL,project_id uuid,actions text[] NOT NULL CHECK(cardinality(actions)>0),delegable_actions text[] NOT NULL DEFAULT '{}',classification_ceiling text NOT NULL CHECK(ownstate_classification_rank(classification_ceiling)<2147483647),allowed_destinations text[] NOT NULL DEFAULT '{}',max_tokens bigint CHECK(max_tokens>0),max_items int CHECK(max_items>0),valid_from timestamptz NOT NULL DEFAULT clock_timestamp(),valid_until timestamptz,revoked_at timestamptz,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),CHECK(valid_until IS NULL OR valid_until>valid_from),FOREIGN KEY(tenant_id,principal_id) REFERENCES principals(tenant_id,id),FOREIGN KEY(tenant_id,workspace_id) REFERENCES workspaces(tenant_id,id),FOREIGN KEY(tenant_id,project_id) REFERENCES projects(tenant_id,id));
CREATE INDEX authorization_grants_lookup ON authorization_grants(tenant_id,principal_id,workspace_id,project_id);
CREATE TABLE agent_executions(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,principal_id uuid NOT NULL,parent_execution_id uuid REFERENCES agent_executions(id),task text NOT NULL CHECK(char_length(task) BETWEEN 1 AND 10000),harness text NOT NULL,model_destination text,expires_at timestamptz NOT NULL,status text NOT NULL CHECK(status IN('PENDING','ACTIVE','COMPLETED','FAILED','REVOKED')),created_at timestamptz NOT NULL DEFAULT clock_timestamp(),UNIQUE(tenant_id,id),FOREIGN KEY(tenant_id,principal_id) REFERENCES principals(tenant_id,id));
CREATE TABLE agent_grants(tenant_id uuid NOT NULL,execution_id uuid NOT NULL,grant_id uuid NOT NULL,parent_grant_id uuid NOT NULL,created_at timestamptz NOT NULL DEFAULT clock_timestamp(),PRIMARY KEY(execution_id,grant_id),FOREIGN KEY(tenant_id,execution_id) REFERENCES agent_executions(tenant_id,id),FOREIGN KEY(grant_id) REFERENCES authorization_grants(id),FOREIGN KEY(parent_grant_id) REFERENCES authorization_grants(id));
CREATE TABLE policy_decisions(id uuid PRIMARY KEY,tenant_id uuid NOT NULL,principal_id uuid,action text NOT NULL,workspace_id uuid,project_id uuid,execution_id uuid,allowed boolean NOT NULL,reason_code text NOT NULL,classification text NOT NULL,destination text,requested_tokens bigint,requested_items int,grant_id uuid,policy_revision int NOT NULL DEFAULT 1,recorded_at timestamptz NOT NULL DEFAULT clock_timestamp());

CREATE FUNCTION ownstate_validate_policy_grant() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE valid_actions text[]:=ARRAY['READ_PROJECT','READ_EVIDENCE','READ_KNOWLEDGE','COMPILE_CONTEXT','CREATE_PROJECT','CREATE_SESSION','APPEND_EVIDENCE','PROPOSE_KNOWLEDGE','ATTRIBUTE_HUMAN_PROPOSAL','PROMOTE_KNOWLEDGE','REJECT_CANDIDATE','GENERATE_EMBEDDING','MODEL_EGRESS','DELEGATE_AGENT'];
BEGIN
 IF EXISTS(SELECT 1 FROM unnest(NEW.actions||NEW.delegable_actions)a WHERE NOT a=ANY(valid_actions)) OR NOT NEW.delegable_actions<@NEW.actions THEN RAISE EXCEPTION 'invalid grant action vocabulary or delegation'; END IF;
 IF NEW.project_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM project_ownership o WHERE o.tenant_id=NEW.tenant_id AND o.workspace_id=NEW.workspace_id AND o.project_id=NEW.project_id) THEN RAISE EXCEPTION 'grant project must belong to workspace'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER authorization_grant_valid BEFORE INSERT ON authorization_grants FOR EACH ROW EXECUTE FUNCTION ownstate_validate_policy_grant();
CREATE FUNCTION ownstate_policy_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'policy history is append-only'; END IF;
 IF TG_TABLE_NAME='authorization_grants' AND (to_jsonb(NEW)-'revoked_at') IS DISTINCT FROM (to_jsonb(OLD)-'revoked_at') THEN RAISE EXCEPTION 'grant content is immutable'; END IF;
 IF TG_TABLE_NAME='principal_credentials' AND (to_jsonb(NEW)-'revoked_at') IS DISTINCT FROM (to_jsonb(OLD)-'revoked_at') THEN RAISE EXCEPTION 'credential content is immutable'; END IF;
 IF TG_TABLE_NAME='policy_decisions' THEN RAISE EXCEPTION 'policy decisions are append-only'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER grants_immutable BEFORE UPDATE OR DELETE ON authorization_grants FOR EACH ROW EXECUTE FUNCTION ownstate_policy_immutable();
CREATE FUNCTION ownstate_principal_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR (to_jsonb(NEW)-'enabled') IS DISTINCT FROM (to_jsonb(OLD)-'enabled') THEN RAISE EXCEPTION 'principal identity is immutable'; END IF; RETURN NEW;
END;$$;
CREATE TRIGGER principals_immutable BEFORE UPDATE OR DELETE ON principals FOR EACH ROW EXECUTE FUNCTION ownstate_principal_immutable();
CREATE TRIGGER credentials_immutable BEFORE UPDATE OR DELETE ON principal_credentials FOR EACH ROW EXECUTE FUNCTION ownstate_policy_immutable();
CREATE TRIGGER decisions_immutable BEFORE UPDATE OR DELETE ON policy_decisions FOR EACH ROW EXECUTE FUNCTION ownstate_policy_immutable();

-- Every legacy PERSONAL tenant gets one deterministic local owner/workspace and
-- an explicit bootstrap grant. ORGANIZATION projects get ownership records but
-- no inferred membership, principal, or grant.
INSERT INTO users(id,tenant_id,display_name) SELECT ownstate_stable_uuid('legacy-user:'||tenant_id),tenant_id,'Legacy personal owner' FROM projects WHERE ownership_domain='PERSONAL' GROUP BY tenant_id;
INSERT INTO workspaces(id,tenant_id,name,personal_user_id) SELECT ownstate_stable_uuid('legacy-personal-workspace:'||tenant_id),tenant_id,'Personal',ownstate_stable_uuid('legacy-user:'||tenant_id) FROM projects WHERE ownership_domain='PERSONAL' GROUP BY tenant_id;
INSERT INTO principals(id,tenant_id,kind,subject_id) SELECT ownstate_stable_uuid('legacy-personal-principal:'||tenant_id),tenant_id,'USER',ownstate_stable_uuid('legacy-user:'||tenant_id) FROM projects WHERE ownership_domain='PERSONAL' GROUP BY tenant_id;
INSERT INTO project_ownership(tenant_id,project_id,workspace_id,ownership_domain) SELECT tenant_id,id,ownstate_stable_uuid('legacy-personal-workspace:'||tenant_id),'PERSONAL' FROM projects WHERE ownership_domain='PERSONAL';
INSERT INTO authorization_grants(id,tenant_id,principal_id,workspace_id,actions,delegable_actions,classification_ceiling,allowed_destinations)
SELECT ownstate_stable_uuid('legacy-personal-grant:'||tenant_id),tenant_id,ownstate_stable_uuid('legacy-personal-principal:'||tenant_id),ownstate_stable_uuid('legacy-personal-workspace:'||tenant_id),ARRAY['READ_PROJECT','READ_EVIDENCE','READ_KNOWLEDGE','COMPILE_CONTEXT','CREATE_PROJECT','CREATE_SESSION','APPEND_EVIDENCE','PROPOSE_KNOWLEDGE','ATTRIBUTE_HUMAN_PROPOSAL','PROMOTE_KNOWLEDGE','REJECT_CANDIDATE','GENERATE_EMBEDDING','MODEL_EGRESS','DELEGATE_AGENT'],ARRAY['READ_PROJECT','READ_EVIDENCE','READ_KNOWLEDGE','COMPILE_CONTEXT','CREATE_SESSION','APPEND_EVIDENCE','PROPOSE_KNOWLEDGE'], 'SECRET',ARRAY['LOCAL:fastembed','LOCAL:deterministic'] FROM projects WHERE ownership_domain='PERSONAL' GROUP BY tenant_id;
INSERT INTO organizations(id,tenant_id,name) SELECT ownstate_stable_uuid('legacy-organization:'||tenant_id),tenant_id,'Legacy organization' FROM projects WHERE ownership_domain='ORGANIZATION' GROUP BY tenant_id;
INSERT INTO workspaces(id,tenant_id,name,organization_id) SELECT ownstate_stable_uuid('legacy-organization-workspace:'||tenant_id),tenant_id,'Legacy organization',ownstate_stable_uuid('legacy-organization:'||tenant_id) FROM projects WHERE ownership_domain='ORGANIZATION' GROUP BY tenant_id;
INSERT INTO project_ownership(tenant_id,project_id,workspace_id,ownership_domain) SELECT tenant_id,id,ownstate_stable_uuid('legacy-organization-workspace:'||tenant_id),'ORGANIZATION' FROM projects WHERE ownership_domain='ORGANIZATION';

-- Defense in depth for deployments whose runtime role is not the migration
-- owner. Services SET LOCAL ownstate.tenant_id on scoped transactions; absent
-- context sees no rows. Table owners retain migration/bootstrap access.
ALTER TABLE project_ownership ENABLE ROW LEVEL SECURITY;
ALTER TABLE principals ENABLE ROW LEVEL SECURITY;
ALTER TABLE principal_credentials ENABLE ROW LEVEL SECURITY;
ALTER TABLE authorization_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_executions ENABLE ROW LEVEL SECURITY;
ALTER TABLE agent_grants ENABLE ROW LEVEL SECURITY;
ALTER TABLE policy_decisions ENABLE ROW LEVEL SECURITY;
CREATE POLICY project_ownership_tenant ON project_ownership USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY principals_tenant ON principals USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY credentials_tenant ON principal_credentials USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY grants_tenant ON authorization_grants USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY executions_tenant ON agent_executions USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY agent_grants_tenant ON agent_grants USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY decisions_tenant ON policy_decisions USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
