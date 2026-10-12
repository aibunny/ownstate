# Ownstate: conversation capture and multi-tenant access strategy

Status: implementation in progress, 2026-10-12. Account, OAuth, and capture code was deployed to the Railway Ownstate API and migrations 0013–0018 applied. Public signup is paused while HTTPS email delivery is configured: Railway Hobby blocks outbound SMTP. No connector directory submission has been made. This extends the active [automatic capture requirements](AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md) and [security contract](MCP_CAPTURE_SECURITY.md).

## Current implementation

- A signup site at `/signup` creates a personal tenant after email verification. Login uses an HttpOnly cookie. Set `OWNSTATE_PUBLIC_URL` and either `OWNSTATE_RESEND_API_KEY` or `OWNSTATE_SMTP_PASSWORD` to enable it in the existing Personal deployment or in Business mode. Resend uses HTTPS and requires a verified sender domain. Lark SMTP defaults to `smtp.larksuite.com:587` with STARTTLS and `ownstate@theaibunny.com` as sender and username; it requires Railway Pro or another host with SMTP egress. Keep credentials in the deployment secret store. Existing Personal mode project data remains in its original tenant until a separate owner migration is performed.
- Signed-in users can create organization spaces, invite members by email, create scoped manual MCP connections, and revoke active connections from `/account`. Each personal space and organization has a distinct tenant. The server resolves the credential's principal and tenant for every `/mcp` request.
- `/mcp` supports OAuth authorization code with PKCE and rotating refresh tokens. Discovery is at `/.well-known/oauth-protected-resource/mcp` and `/.well-known/oauth-authorization-server`; dynamic registration and authorization are at `/oauth/register` and `/oauth/authorize`. A user chooses a space during consent. Access tokens expire in one hour; refresh tokens rotate and expire after 365 days of inactivity. Revocation disables both.
- `record_conversation` stores explicit non-project chat events in append-only `conversation_threads` and `conversation_events` with source conversation and event IDs. `POST /capture/conversations` accepts the same user-owned MCP bearer for local host adapters. Repeated source event IDs return the original event. These paths do not pass captured chats through the project knowledge pipeline yet.
- Local Codex and Claude Code hook scripts and an OpenCode plugin are included under `integrations/`; they have syntax checks but have not yet been tested in running host clients. Capture remains **partial**. OAuth MCP clients do not grant transcript access by themselves. No connector marketplace listing or hosted service rollout is complete.

### Host adapter request

Send `POST /capture/conversations` with `Authorization: Bearer <user-owned connection token>` and JSON such as:

```json
{"source":"opencode","source_conversation_id":"stable-chat-id","source_event_id":"stable-message-id","title":"Research","capture_mode":"PARTIAL_ADAPTER","event_type":"USER_MESSAGE","actor_type":"USER","content":"Question text"}
```

Use stable source IDs on retries. `MCP_ONLY` describes explicit model tool calls. `PARTIAL_ADAPTER` describes observed local host events. `IMPORTED` describes user-supplied exports. Do not mark a stream complete without a verified host contract. The API currently accepts one event per request and has no queue, export, retention settings, or capture traffic quota. Signup, login, verification resend, and OAuth dynamic registration have basic database-backed attempt limits. It remains a pilot interface rather than an open public ingest service.

## Product promise and boundary

Ownstate should preserve authorized AI activity from ordinary chats, coding tasks, research, and strategy work, then make useful knowledge available across clients. Each person should have a private space; an organization should have its own space with explicit membership and grants. A conversation can belong to a personal or organization space without belonging to a project or repository.

The product must distinguish **using Ownstate through MCP** from **capturing a host conversation**. A remote MCP server observes its own tool requests and responses; it does not receive a full transcript merely because it is connected. OpenAI's plugin submission guidance also prohibits tools that request full conversation history or raw chat transcripts just in case. For supported local clients, a separate, permissioned adapter may capture more. For hosted chat products without a supported event feed, provide a user-initiated export/import path and label its coverage honestly. Never advertise universal or lossless capture without per-host, per-channel proof. This matches the existing [capability matrix](HOST_CAPTURE_CAPABILITY_MATRIX.md).

## Baseline found before this implementation

- The database already has `users`, `organizations`, `organization_memberships`, `workspaces`, `principals`, `principal_credentials`, `authorization_grants`, and policy decisions (`migrations/0010_ownership_and_policy.sql`). These are an authorization foundation, not a signup or OAuth product.
- Evidence already models `USER_MESSAGE`, `ASSISTANT_MESSAGE`, tool activity, files, artifacts, and other event types (`migrations/0002_evidence.sql`). Sessions and events currently require `project_id`; non-project chats need a new scope model and an additive migration.
- `record` currently creates a fresh session for each call and resolves a project first (`apps/mcp/src/server.rs`). Durable chat capture needs a stable source conversation/session association and replay-safe event ingestion. A client-supplied `source_event_id` alone does not make a complete conversation.
- The running application fixes one `tenant_id` when `AppServices` is built (`crates/services/src/lib.rs`, `crates/runtime/src/lib.rs`). The HTTP API resolves token credentials, but `/mcp` additionally accepts only the one configured MCP principal and constructs one principal-scoped MCP service at router startup (`apps/api/src/auth.rs`, `apps/api/src/router.rs`). The Railway template starts in personal mode with a single MCP token (`docs/RAILWAY.md`). Multi-tenant request routing is therefore not ready.
- The existing [state file](../STATE.md) marks host capture adapters, non-code continuity, and the full scope resolver as unfinished. Existing work in progress should be incorporated rather than replaced.

## Target model

```text
Person signs in -> personal workspace
                -> optional organization memberships -> chosen organization workspace

Codex / Claude / OpenCode -- OAuth or scoped token --> Ownstate MCP (context and explicit records)
Local host adapter / approved import -------------> capture API (normalized events)
                                                   -> append-only evidence
                                                   -> candidates and canonical knowledge
                                                   -> bounded, authorized context
```

Keep `tenant` as the hard isolation boundary and `workspace` as the personal or organization container. A practical SaaS mapping is one tenant for each personal space and one tenant for each organization, with a global login identity mapped to tenant-local user and principal records. This avoids treating the current tenant-local `users` table as a cross-tenant account directory. Resolve a tenant and principal from the authenticated credential on **every request**; never accept tenant, user, organization, workspace, or role from model-controlled arguments as authority. The user chooses a workspace during authorization or in the account UI. A connection receives only the workspace and actions approved there. For someone in two organizations, use separate grants/connections or an explicit workspace switch that issues a differently scoped credential. Retain a server-side source identity (`provider`, `source_account`, `source_conversation_id`) and store thread/event IDs under that ownership domain; a host conversation ID is never globally unique.

Represent chats as first-class **threads** with zero or more project/repository associations. A thread can start in a general personal or organization scope, then acquire verified links to a project, artifact, entity, or other thread later. Preserve the original source identity and evidence; do not silently move or merge it. Avoid a fake `general` project and avoid making `project_id` mandatory on new evidence. For compatibility, keep old project-linked sessions readable and migrate additively.

## Capture routes by client

| Surface | Proposed route | Coverage claim |
|---|---|---|
| Codex local desktop/CLI | Trusted lifecycle hook or local companion that records supported turn events and selected tool outcomes; MCP for retrieval | Potentially richer local capture. Validate hook payloads and lifecycle on the supported version. The documented `transcript_path` format is unstable and should not be the durable parser contract. |
| Claude Code | Supported hooks such as `SessionStart`, `UserPromptSubmit`, `PostToolUse`, `Stop`, and `SessionEnd`, or a supported SDK event stream; MCP for retrieval | Hook-observed channels only, with per-channel completeness. Verify exact event payloads and retries. |
| OpenCode | Global plugin subscribing to message, part, tool, and session events; MCP for retrieval | Strong candidate for automatic capture. Fetch final messages from the supported API where necessary and verify ordering/replay. |
| Claude hosted chat / ChatGPT hosted chat | Remote MCP for context and explicit `record` actions; user-initiated data export/import where supported | MCP-only is partial. Imports cover only what the export contains and are marked `IMPORTED`. Do not promise passive full capture from a connector. |
| Any custom app or agent using an API | Instrument the app's own message/event stream and send it to the capture API | Can approach complete capture for the app's documented channels when the application controls the full exchange. |

All adapters use one normalized ingestion contract: source, source account, source conversation, source event ID or stable replay key, event type, actor, sequence/time, content or artifact reference, classification, and capture assessment. Use per-source, per-conversation idempotency constraints. Queue and retry ingestion locally where appropriate, report gaps, and keep exact captured evidence separate from summaries. Add consent, exclusion, redaction, retention, and export controls before inviting external users to sync private chats.

## Sign-up and MCP authentication

Build a small account site for registration, sign-in, verified email, personal workspace, organization creation/invitations, membership management, connection management, capture settings, and export. The site login is the human identity layer. **Publishing the MCP endpoint still requires a separate OAuth authorization flow** for clients to act on that signed-in user's behalf.

Recommended flow:

1. Use an established OAuth/OIDC identity provider for user sign-in and an OAuth authorization server compatible with MCP. The MCP endpoint is an OAuth protected resource. Implement protected-resource metadata, authorization-server discovery, authorization-code with PKCE, refresh/revocation, and a token audience specific to Ownstate MCP. Prefer a maintained provider over writing an authorization server from scratch.
2. On connection, show the signed-in person the target personal/organization workspace and precise scopes (for example context read, evidence write). Check current organization membership before granting organization scope. Issue short-lived, audience-bound access tokens; store refresh credentials securely and support disconnect/rotation.
3. On every MCP and capture request, validate token issuer, audience, signature or introspection result, expiry, scopes, principal status, membership, and current grant. Bind `PrincipalServices` to that resolved tenant and principal **per request or per authenticated transport session**, not a singleton captured at server startup. Recheck authorization at each sensitive read/write and on long-lived connections.
4. Keep a separate scoped personal access token path for local CLI adapters if needed. Never distribute the deployment-wide `OWNSTATE_MCP_TOKEN` to multiple end users. Keep service/admin credentials separate from user access.
5. Test one user in two organizations, two users in one organization, revoked membership, disabled principal, expired/revoked token, workspace switching, concurrent sessions, guessed IDs, and cross-tenant reads/writes, including retrieval caches, object storage, worker jobs, and logs.

The MCP authorization specification requires protected-resource metadata and OAuth discovery for HTTP transport. Claude remote connectors and OpenCode support browser-based OAuth connection flows. OpenAI's plugin guidance calls for OAuth when the MCP server requires user authentication. The account site alone cannot identify a caller on `/mcp` without this token exchange and request-time authorization.

## Delivery order and acceptance gates

1. **Foundation:** preserve the current database; audit in-progress scope-handle work; define tenant-wide service construction and request-scoped authorization; add negative cross-tenant tests. Do not launch signup while one shared MCP identity remains.
2. **General threads:** additive schema for project-optional threads and events, personal/organization general scopes, source conversation IDs, event replay keys, and per-channel capture assessments. Preserve old project-linked records. Prove ordinary research chat can be stored and retrieved without a repository or fake project.
3. **Capture API and pilot:** normalize batch ingestion and import, then pilot OpenCode and Claude Code adapters. Add a Codex local hook pilot after verifying exact supported payloads. Repeated delivery must be idempotent; missing channels must remain visibly incomplete.
4. **Account and OAuth:** account UI, personal/organization onboarding, grants, OAuth metadata/flows, token revocation, and per-request principal resolution. Test with at least two isolated tenants and one shared organization before opening registration.
5. **Distribution:** publish an HTTPS MCP endpoint with a stable tool contract and privacy documentation. Test Claude custom connector, OpenCode, and OpenAI plugin/Codex connection separately. Publication review and client-specific OAuth interoperability are release gates. A marketplace listing distributes access; local capture adapters must still be installed and trusted separately.
6. **Scale and operations:** quotas, backpressure, encrypted backups and restore tests, retention/export processes, abuse controls, audit logs, and worker isolation. Reconcile the capability matrix with reproducible host/version tests before any “all chats” claim.

Suggested first release promise: **“Ownstate remembers the interactions you explicitly record or import, and automatically captures supported local-client events when its adapter is enabled.”** Expand that statement only as verified capture coverage grows.

## Primary documentation checked

- [MCP authorization specification, 2025-11-25](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization)
- [OpenAI Codex hooks](https://learn.chatgpt.com/docs/hooks) and [OpenAI plugin packaging](https://developers.openai.com/plugins/build/plugins)
- [OpenAI MCP server and plugin guidance](https://developers.openai.com/plugins/concepts/mcp-server), [plugin submission guidance](https://developers.openai.com/plugins/plugin-guidelines), and [ChatGPT data export](https://help.openai.com/en/articles/7260999-how-do-i-export-my-chatgpt-history-and-)
- [Claude Code hooks](https://code.claude.com/docs/en/hooks), [Claude remote connectors](https://support.claude.com/en/articles/11175166-get-started-with-custom-connectors-using-remote-mcp), and [Claude data export](https://support.claude.com/en/articles/9450526-export-your-claude-data)
- [OpenCode plugins](https://docs.opencode.ai/docs/plugins/), [server API](https://docs.opencode.ai/docs/server/), and [remote MCP OAuth](https://docs.opencode.ai/docs/mcp-servers/)
