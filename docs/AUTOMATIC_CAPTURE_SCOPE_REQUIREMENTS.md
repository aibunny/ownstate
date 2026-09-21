# Automatic capture, scope resolution, and MCP context protocol

Status: normative feature requirements.

These requirements extend `ARCHITECTURE.md` and `REQUIREMENTS.md`. They do not
replace existing evidence, candidate, canonical, temporal, provenance, context,
authorization, or model-egress invariants.

## 1. User experience

Connecting Ownstate MCP must be sufficient to gain persistent context and
knowledge capabilities. Normal use must not require users or models to create a
project row, remember a UUID, select a database row, or associate every session
manually.

Ownstate must resolve institutional scope from observable evidence, retrieve the
smallest useful authorized context, record what the integration can actually
observe, and feed that evidence through the existing knowledge pipeline.

Project/workspace registration is an internal Ownstate concern.

## 2. MCP and capture boundary

MCP is the universal context and knowledge interface. Installing an MCP server
does not imply that a host sends every private message, tool call, shell command,
file event, or artifact.

The implementation must distinguish:

- MCP context integration;
- model-initiated MCP recording;
- native or hook-based host capture;
- imported evidence;
- complete versus incomplete observation.

MCP-mediated knowledge capture must never be described as lossless host capture
unless the host integration proves completeness for each channel.

Protocol transport identity is not institutional identity. The deployed server
currently uses MCP revision `2025-11-25`; later protocol revisions may remove
transport sessions. In all supported revisions, durable application state must
use server-owned records and explicit server-minted handles, never a transport
session ID. See the official MCP changelog for the active revision before making
protocol assumptions: [MCP specification changelog](https://modelcontextprotocol.io/specification/draft/changelog).

## 3. Capture quality

Define a closed capture-mode type. The exact names may follow established domain
conventions but must distinguish at least:

- native/complete adapter capture;
- partial adapter capture;
- MCP-only observation;
- imported capture;
- unknown capability.

Store per-channel completeness separately from the summary mode. At minimum:

- user messages;
- assistant messages;
- tool calls and results;
- commands and results;
- file reads and writes;
- diffs/tests;
- artifacts;
- start/end lifecycle;
- source conversation/session identifiers.

Completeness is factual metadata, not a confidence guess. `MCP_ONLY` cannot imply
complete messages, tools, or files. An adapter may mark a channel complete only
when its documented host surface provides every event in that channel for the
bounded session.

Capture metadata is append-oriented and auditable. Later capability discovery
may add a new assessment; it must not silently rewrite what was observable at the
time.

## 4. Scope-resolution domain

Introduce or preserve a provider-neutral `ScopeResolver` abstraction. Its input
is typed observable evidence; its output is an authorized resolved scope plus
resolution state and evidence.

Resolution state must distinguish at least:

- exact stable identity;
- known alias match;
- probable association;
- provisional scope;
- general personal/organization scope.

The output must identify which anchors and deterministic rules produced the
result. A model-generated name is never proof of identity.

Support a generic context-scope model above project identity. Required scope
kinds are organization, project, repository, relationship, artifact, thread, and
personal/organization general scope. A session may reference multiple scopes.

## 5. Git repository identity

For coding work, repository identity is the strongest deterministic anchor.
Resolve in this order where evidence exists:

1. canonical remote repository identity;
2. known current or historical remote aliases;
3. stable repository metadata;
4. provisional local repository identity.

Directory name, absolute path, branch, agent name, model name, chat title, and a
model-generated project name are not durable repository identity.

Normalize equivalent SCP-style SSH, SSH URL, HTTPS, and other supported remote
forms into one canonical locator. Normalization must:

- remove user information, passwords, tokens, query strings, and fragments;
- normalize recognized host names safely;
- normalize separators and unnecessary trailing slashes;
- remove a terminal `.git` where appropriate;
- preserve provider-specific owner/repository case semantics;
- reject control characters, malformed URLs, ambiguous host/path forms, and
  values exceeding configured bounds;
- never log or persist the original secret-bearing remote.

Persist the canonical repository URI, provider, owner, repository name, current
and historical aliases, and a stable internal repository key derived from the
canonical identity. The key is internal and never a user-managed identifier.

Normalization must be pure and deterministic. Do not run a shell with
concatenated remote text. When a local adapter invokes Git, use fixed executable
arguments and an explicitly validated repository working directory.

## 6. Forks, clones, worktrees, and branches

A fork and its upstream are separate repository identities. Persist origin and
upstream separately and represent verified lineage with `FORK_OF` or the existing
typed relationship mechanism. Never merge them because names or commit history
look similar.

Clones and Git worktrees of the same canonical repository resolve to one
repository identity and default project association. Branch and commit belong to
session/evidence context, with working-tree state where safely observable; they
do not create repository identities.

Repository identity must remain the same across Codex, Claude Code, OpenCode, and
other hosts when canonical evidence is equivalent.

## 7. Repositories without remotes

A repository without a remote receives a provisional identity without requiring
manual registration. Use the strongest available combination of repository root
metadata, root/initial commit when available, and a persisted creation
fingerprint. Do not rely only on absolute path.

When a canonical remote is later discovered, link or upgrade the provisional
identity transactionally. Preserve the provisional locator, evidence, sessions,
project association, and history. Do not create a duplicate or rewrite evidence.

## 8. Rename and transfer

A remote rename or ownership transfer does not automatically create a new
repository. Preserve historical locators and associate a new canonical locator
only when deterministic provider evidence, trusted explicit evidence, or an
authorized review supports continuity. Similar names alone never merge records.

## 9. Project and repository relationship

Project and repository are distinct. A project may contain multiple repositories,
artifacts, conversations, relationships, and knowledge. A repository may
bootstrap a default project when no broader project is known, without asking the
user to create one.

Exact repository discovery and default-project association must be idempotent
and concurrency-safe. Enforce deterministic uniqueness within the ownership
domain in PostgreSQL and use a transactional upsert or equivalent. Application
`find then insert` alone is insufficient.

## 10. Non-code scope resolution

Support research, fundraising, customer, legal, product, strategy, personal,
document, and other non-repository work without mandatory manual registration.

Resolution may use, in descending determinism:

- authenticated organization/workspace identity;
- explicit stable host/source identifiers;
- existing artifact identity and version lineage;
- prior server-minted scope handle or ContextPacket reference;
- known entities and relationships;
- thread/task lineage;
- recent authorized associations;
- bounded semantic similarity as supporting evidence only.

When evidence is insufficient, use a provisional thread/scope or the authorized
general personal/organization scope. Do not invent a confident project.

Later consolidation must preserve original thread, source, artifact, entity, and
provenance links. Semantic similarity alone cannot perform a destructive merge.

## 11. Global knowledge

Support personal-global and organization-global scope for preferences, policies,
legal entities, organization identity, contacts, and genuinely cross-project
knowledge. Do not create fake projects merely to store global knowledge.

Global retrieval still requires explicit current authorization and classification
checks. Organization-global does not mean visible to every organization member.

## 12. Bootstrap and scope handles

Expose a compact high-value MCP bootstrap operation using existing naming
conventions. It accepts observable task/environment evidence and must not require
`project_id` on the normal path.

Repository evidence may include sanitized remote locators, branch, commit, and
working-directory hints. Client evidence may include name/version/capabilities.
Every field is untrusted and bounded.

Bootstrap returns:

- a short opaque `scope_handle`;
- resolved scope summary and resolution state;
- capture limitations known for the caller/adapter;
- a compact authorized Layer 0/1 context;
- pending candidate counts or freshness warnings where relevant.

A scope handle is a server-minted convenience reference. It is not canonical
identity, authentication, authorization, or a bearer capability. Bind it to an
ownership domain and resolved records; on every use, authenticate independently,
check expiry/status if applicable, and authorize the referenced resources. Never
embed secrets or predictable internal IDs in a handle.

## 13. MCP instructions and compact tools

Publish concise server instructions through supported MCP descriptions,
prompts/resources, and installation documentation. Tell compatible clients to
bootstrap at meaningful work boundaries, retrieve before rediscovery, record
meaningful observable outcomes, avoid secrets/filler, and never invent IDs.

Do not depend on instructions for capture completeness.

Keep the model-facing interface small and intention-oriented. A target surface is
bootstrap, search, record, artifact, feedback, and a bounded evidence drill-down
where the existing architecture requires it. Reuse compatible existing tools
rather than adding aliases.

Do not expose create-project, database row insertion, embedding creation,
knowledge-version mutation, policy/grant mutation, canonical force-write, raw
SQL, or any delete/purge/erase operation.

## 14. Recording and evidence idempotency

Provide a generic append-only recording operation that accepts a scope handle,
typed event, bounded content/reference fields, artifact references, source
references, and safe metadata. The model does not decide storage tables,
canonical status, supersession, or permanent entity merges.

Recording creates raw evidence. Existing typed extraction, novelty, policy, and
promotion processes determine derived and canonical state.

Every captured event supports source-level idempotency. Prefer source,
source-session ID, and source-event ID when stable. Otherwise use a documented
bounded fingerprint that includes enough ordering/context to avoid collapsing
distinct equal-content events. Re-importing the same transcript must not
duplicate all evidence.

## 15. Capture adapters

Define one provider-neutral `CaptureAdapter` boundary only if the existing source
adapter boundary cannot express capture. All adapters emit the same normalized
evidence and capture-quality model. Provider-specific types stop at the adapter.

Investigate Codex Desktop/CLI, Claude/Claude Code, OpenCode, Generic MCP,
API-gateway capture, and imports using primary documentation and testable local
surfaces. Record the date, version, source, observable channels, guarantees, and
limitations in `HOST_CAPTURE_CAPABILITY_MATRIX.md`.

Implement only adapters with available, stable, safe, testable integration
surfaces. Unknown or unavailable surfaces remain documented as unverified. Do not
scrape private application storage, bypass host permissions, or infer capability.

Native capture is preferred over model self-report when both are available.

## 16. Threads and provider sessions

Provider session/conversation IDs are source locators, not global project or
thread identity. An Ownstate thread may link multiple provider sessions when
evidence supports continuation. Uncertainty preserves separate/provisional
threads.

Never merge threads across principals, tenants, or classifications because task
text is similar.

## 17. Context delivery

Storage volume and context delivery are separate. Preserve captured evidence
according to retention policy; never preload full histories by default.

Compile progressive context layers:

- Layer 0: identity, authorized organization/scope, repository, current task;
- Layer 1: small stable project purpose, architecture, constraints, major state;
- Layer 2: task-specific decisions, failures, requirements, and recent changes;
- Layer 3: exact evidence drill-down only when requested and authorized.

Bootstrap should normally stay within hundreds to low thousands of tokens. All
layers enforce max tokens, max items, classification, scope, freshness, knowledge
types, evidence depth, and exact authorization.

Ranking combines task relevance, scope, source authority, freshness, trust,
relationship relevance, and recency. Vector similarity alone is insufficient.

Deduplicate canonical/current meaning. Prefer active canonical knowledge and do
not include repeated old summaries. Include raw evidence only for useful detail
or provenance. Historical tasks may request superseded state explicitly;
ordinary current context must exclude superseded/revoked/stale material as
required by existing policy. Contradictions remain visible as conflict until
resolved.

## 18. Lossless evidence invariant

Summaries, embeddings, extracted knowledge, capture assessments, scope
associations, and ContextPackets are projections. They never replace captured
messages, tool results, artifacts, commands, or diffs. Original evidence remains
independently retrievable subject to authorization and legitimate retention
policy.

## 19. Configuration and performance

All applicable configuration is typed, centrally loaded, validated, redacted in
debug output, documented in `.env.example`, and usable without proprietary
Ownstate infrastructure.

Scope resolution and evidence storage require no paid or frontier model. The
bootstrap fast path uses normalized Git identity, exact aliases, indexed scope
associations, and cached stable context. Semantic/model assistance is bounded to
ambiguous cases and cannot finalize destructive merges.

`cp .env.example .env && docker compose up --build` remains the target local
experience. Preserve zero-connector and local/self-hosted operation.

## 20. Required acceptance tests

At minimum, prove:

1. equivalent GitHub SSH/SCP/HTTPS forms resolve to one repository and strip
   credentials;
2. different clone paths, branches, and worktrees resolve to one repository and
   default project;
3. fork and upstream remain distinct with an explicit relationship;
4. a repository without remote is provisional, then upgrades without duplicate
   state when a remote is discovered;
5. rename/transfer aliases preserve identity only with sufficient evidence;
6. two concurrent bootstrap requests cannot create duplicate repositories or
   projects;
7. Codex, Claude, and OpenCode inputs for the same canonical remote resolve to
   the same institutional scope;
8. ambiguous non-code work remains provisional; artifact/entity/thread evidence
   can later associate it without losing provenance;
9. personal-global and organization-global knowledge do not require fake projects;
10. repeated imports/capture events are idempotent without collapsing distinct
    equal-content events;
11. MCP-only capture is never marked full and each channel reports factual
    completeness;
12. a large history produces a bounded, deduplicated ContextPacket;
13. current retrieval omits superseded/revoked state while authorized historical
    retrieval preserves it;
14. scope handles cannot cross principals, tenants, workspaces, classifications,
    expiry/status, or grants;
15. arbitrary/credential-bearing remote strings cannot leak secrets, execute
    commands, traverse paths, trigger SSRF, or choose authority;
16. model-controlled MCP calls cannot delete or rewrite recorded evidence,
    canonical versions, artifacts, scope history, policy decisions, or audit;
17. provider session IDs never become global identity;
18. bootstrap uses the deterministic fast path without a model for exact Git and
    alias matches.

## 21. Completion condition

This feature is complete only when the normal cross-model repository flow works
without manual project registration, non-code ambiguity safely remains
provisional, capture limitations are explicit, context is bounded and fresh, all
security tests pass, and documentation describes only demonstrated host
capabilities.
