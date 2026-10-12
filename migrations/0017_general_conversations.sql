-- Non-project conversations are first-class evidence. Existing project-scoped
-- sessions and interaction_events remain intact and readable.
CREATE TABLE conversation_threads (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    workspace_id uuid NOT NULL,
    owner_principal_id uuid NOT NULL,
    source text NOT NULL CHECK (char_length(source) BETWEEN 1 AND 100),
    source_conversation_id text NOT NULL CHECK (char_length(source_conversation_id) BETWEEN 1 AND 500),
    title text,
    capture_mode text NOT NULL CHECK (capture_mode IN ('MCP_ONLY','PARTIAL_ADAPTER','NATIVE_COMPLETE','IMPORTED','UNKNOWN')),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (tenant_id,workspace_id,owner_principal_id,source,source_conversation_id),
    UNIQUE (tenant_id,id),
    FOREIGN KEY (tenant_id,workspace_id) REFERENCES workspaces(tenant_id,id),
    FOREIGN KEY (tenant_id,owner_principal_id) REFERENCES principals(tenant_id,id)
);

CREATE TABLE conversation_events (
    id uuid PRIMARY KEY,
    tenant_id uuid NOT NULL,
    thread_id uuid NOT NULL,
    source_event_id text,
    sequence bigint NOT NULL CHECK (sequence>=0),
    event_type text NOT NULL CHECK (event_type IN ('USER_MESSAGE','ASSISTANT_MESSAGE','TOOL_CALL','TOOL_RESULT','SYSTEM_EVENT','ARTIFACT_CREATED')),
    actor_type text NOT NULL CHECK (actor_type IN ('USER','ASSISTANT','TOOL','SYSTEM')),
    content text,
    security_classification text NOT NULL DEFAULT 'INTERNAL' CHECK (security_classification IN ('PUBLIC','INTERNAL','CONFIDENTIAL','RESTRICTED','SECRET')),
    occurred_at timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (thread_id,sequence),
    FOREIGN KEY (tenant_id,thread_id) REFERENCES conversation_threads(tenant_id,id)
);
CREATE UNIQUE INDEX conversation_event_source_id ON conversation_events(thread_id,source_event_id)
    WHERE source_event_id IS NOT NULL;
CREATE INDEX conversation_events_thread ON conversation_events(thread_id,sequence);

CREATE TRIGGER conversation_events_append_only BEFORE UPDATE OR DELETE ON conversation_events
    FOR EACH ROW EXECUTE FUNCTION ownstate_forbid_event_mutation();
