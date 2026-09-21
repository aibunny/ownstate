-- Automatic capture and scope resolution: repository identity extensions,
-- scope handles, capture assessments, and idempotency keys.
-- Additive only: no existing rows, columns, or constraints are modified.

-- Extend repositories with normalized identity fields.
ALTER TABLE repositories ADD COLUMN provider text NOT NULL DEFAULT 'UNKNOWN';
ALTER TABLE repositories ADD COLUMN owner_name text;
ALTER TABLE repositories ADD COLUMN repo_name text;
ALTER TABLE repositories ADD COLUMN internal_key text NOT NULL DEFAULT '';
ALTER TABLE repositories ADD COLUMN fork_of_repository_id uuid;
ALTER TABLE repositories ADD COLUMN metadata jsonb NOT NULL DEFAULT '{}';

-- Deterministic uniqueness within ownership domain.
CREATE UNIQUE INDEX repositories_internal_key ON repositories(tenant_id, internal_key) WHERE internal_key <> '';
CREATE UNIQUE INDEX repositories_canonical ON repositories(tenant_id, canonical_origin);

-- Repository aliases (historical remotes, renames).
CREATE TABLE repository_aliases(
  id uuid PRIMARY KEY,
  tenant_id uuid NOT NULL,
  repository_id uuid NOT NULL,
  alias_uri text NOT NULL,
  alias_kind text NOT NULL CHECK(alias_kind IN('HISTORICAL','FORK_UPSTREAM','PROVIDER_ALIAS')),
  recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  UNIQUE(tenant_id, repository_id, alias_uri),
  FOREIGN KEY(tenant_id, repository_id) REFERENCES repositories(tenant_id, id)
);
CREATE INDEX repository_aliases_lookup ON repository_aliases(tenant_id, repository_id);

-- Scope handles: server-minted opaque references for MCP operations.
CREATE TABLE scope_handles(
  id uuid PRIMARY KEY,
  tenant_id uuid NOT NULL,
  ownership_domain text NOT NULL CHECK(ownership_domain IN('PERSONAL','ORGANIZATION')),
  project_id uuid,
  repository_id uuid,
  workspace_id uuid,
  scope_kind text NOT NULL CHECK(scope_kind IN('ORGANIZATION','PROJECT','REPOSITORY','RELATIONSHIP','ARTIFACT','THREAD','PERSONAL')),
  resolution_state text NOT NULL CHECK(resolution_state IN('EXACT_STABLE','KNOWN_ALIAS','PROBABLE_ASSOCIATION','PROVISIONAL_SCOPE','GENERAL_SCOPE')),
  label text,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  expires_at timestamptz,
  revoked_at timestamptz,
  CHECK(expires_at IS NULL OR expires_at > created_at),
  FOREIGN KEY(tenant_id, project_id) REFERENCES projects(tenant_id, id),
  FOREIGN KEY(tenant_id, repository_id) REFERENCES repositories(tenant_id, id),
  FOREIGN KEY(tenant_id, workspace_id) REFERENCES workspaces(tenant_id, id)
);
CREATE INDEX scope_handles_tenant ON scope_handles(tenant_id);
CREATE INDEX scope_handles_project ON scope_handles(tenant_id, project_id) WHERE project_id IS NOT NULL;
CREATE INDEX scope_handles_repository ON scope_handles(tenant_id, repository_id) WHERE repository_id IS NOT NULL;

-- Capture assessments: append-only record of capture quality per session.
CREATE TABLE capture_assessments(
  id uuid PRIMARY KEY,
  tenant_id uuid NOT NULL,
  project_id uuid NOT NULL,
  session_id uuid,
  repository_id uuid,
  capture_mode text NOT NULL CHECK(capture_mode IN('NATIVE_COMPLETE','PARTIAL_ADAPTER','MCP_ONLY','IMPORTED','UNKNOWN')),
  channels jsonb NOT NULL DEFAULT '{}',
  source_adapter text NOT NULL,
  assessed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  assessed_by text,
  notes text,
  FOREIGN KEY(tenant_id, project_id) REFERENCES projects(tenant_id, id),
  FOREIGN KEY(session_id) REFERENCES sessions(id),
  FOREIGN KEY(tenant_id, repository_id) REFERENCES repositories(tenant_id, id)
);
CREATE INDEX capture_assessments_session ON capture_assessments(tenant_id, session_id) WHERE session_id IS NOT NULL;

-- Idempotency keys for evidence recording.
CREATE TABLE recording_idempotency(
  id uuid PRIMARY KEY,
  tenant_id uuid NOT NULL,
  project_id uuid NOT NULL,
  source text NOT NULL,
  source_session_id text,
  source_event_id text NOT NULL,
  event_id uuid NOT NULL,
  content_fingerprint text NOT NULL,
  recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  UNIQUE(tenant_id, project_id, source, source_event_id),
  FOREIGN KEY(tenant_id, project_id) REFERENCES projects(tenant_id, id),
  FOREIGN KEY(event_id) REFERENCES interaction_events(id)
);

-- Enable RLS on new tables.
ALTER TABLE scope_handles ENABLE ROW LEVEL SECURITY;
ALTER TABLE capture_assessments ENABLE ROW LEVEL SECURITY;
ALTER TABLE recording_idempotency ENABLE ROW LEVEL SECURITY;
ALTER TABLE repository_aliases ENABLE ROW LEVEL SECURITY;

CREATE POLICY scope_handles_tenant ON scope_handles USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY capture_assessments_tenant ON capture_assessments USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY recording_idempotency_tenant ON recording_idempotency USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);
CREATE POLICY repository_aliases_tenant ON repository_aliases USING(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid) WITH CHECK(tenant_id=nullif(current_setting('ownstate.tenant_id',true),'')::uuid);

-- Scope handles are append-only (no UPDATE, no DELETE).
CREATE FUNCTION ownstate_scope_handle_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'scope handles are append-only'; END IF;
 IF (to_jsonb(NEW)-'revoked_at') IS DISTINCT FROM (to_jsonb(OLD)-'revoked_at') THEN RAISE EXCEPTION 'scope handle content is immutable except revocation'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER scope_handles_immutable BEFORE UPDATE OR DELETE ON scope_handles FOR EACH ROW EXECUTE FUNCTION ownstate_scope_handle_immutable();

-- Capture assessments are append-only.
CREATE FUNCTION ownstate_capture_assessment_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'capture assessments are append-only'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER capture_assessments_immutable BEFORE UPDATE OR DELETE ON capture_assessments FOR EACH ROW EXECUTE FUNCTION ownstate_capture_assessment_immutable();

-- Recording idempotency is append-only.
CREATE FUNCTION ownstate_recording_idempotency_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'recording idempotency records are append-only'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER recording_idempotency_immutable BEFORE UPDATE OR DELETE ON recording_idempotency FOR EACH ROW EXECUTE FUNCTION ownstate_recording_idempotency_immutable();

-- Repository aliases are append-only.
CREATE FUNCTION ownstate_repository_alias_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN RAISE EXCEPTION 'repository aliases are append-only'; END IF;
 RETURN NEW;
END;$$;
CREATE TRIGGER repository_aliases_immutable BEFORE UPDATE OR DELETE ON repository_aliases FOR EACH ROW EXECUTE FUNCTION ownstate_repository_alias_immutable();
