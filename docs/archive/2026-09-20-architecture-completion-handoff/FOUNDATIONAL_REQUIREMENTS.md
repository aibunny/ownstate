# Ownstate — Foundational Product and Engineering Requirements Specification

Version: 0.1
Status: Initial implementation specification

Authority note (2026-09-15): this is the earlier foundational implementation
specification. Root ARCHITECTURE.md, REQUIREMENTS.md and README.md define the
current target, as explicitly clarified by the user. Consult
[the completion ledger](REQUIREMENT_LEDGER.md) for clause-level progress and
[current architecture](ARCHITECTURE.md) for demonstrated implementation.
The institutional batch adds typed entities/relationships/claims, scoped
provenance, valid/observed/recorded history and safe identifier resolution;
it does not complete the full target architecture.

Current query/operability note (2026-09-16): migration0009 and common typed query
services provide deterministic structured aggregates, scoped graph/semantic
associations and temporal interval changes. Runtime validates startup configuration;
Docker/Compose/CI configuration includes the application processes. Settled full
gates and independent acceptance passed; a fresh isolated Compose stack verified
default Fastembed initialization, a completed real embedding job, and MCP stdio.
Hosted CI and semantic-quality evaluation remain unproved. The larger older
specification below is preserved as implementation history, not target authority.

## 1. Product definition

Ownstate is a sovereign, model-agnostic and agent-agnostic knowledge layer for individuals and organizations using AI.

Its purpose is to ensure that useful context, knowledge, decisions and experience created while interacting with AI do not remain trapped inside a particular AI vendor, model, agent or conversation.

A user should be able to work with:

* ChatGPT
* Codex
* Claude
* Claude Code
* Kimi
* MiniMax
* DeepSeek
* OpenRouter models
* Cursor
* cowork-style agents
* local/open-source models
* future AI systems

while maintaining one persistent knowledge state that the user or organization owns.

The model is replaceable.
The agent is replaceable.
The accumulated knowledge is not.

## 2. Primary product objective

Ownstate must capture the observable context generated during AI work and continuously transform useful portions of that activity into durable, searchable and versioned knowledge.

The system must preserve enough evidence that knowledge can later be:

* verified
* corrected
* superseded
* traced to its source
* re-extracted using better models
* removed
* exported
* selectively exposed to another AI

The system must NOT simply save chat history and call it memory.

The architecture must distinguish between:

1. raw interaction evidence
2. candidate knowledge
3. canonical knowledge
4. compiled context delivered to AI systems

## 3. Fundamental invariant

The core lifecycle is:

```text
AI Interaction
      ↓
Raw Evidence
      ↓
Knowledge Extraction
      ↓
Candidate Knowledge
      ↓
Novelty + Validation + Policy
      ↓
Canonical Knowledge
      ↓
Context Compiler
      ↓
Next AI
      ↓
New Interaction
      ↓
Learning continues
```

No extraction model may write directly to canonical knowledge.
The LLM proposes knowledge.
Ownstate decides whether it becomes trusted state.

## 4. Scope of captured information

Ownstate should capture all useful externally observable information available from an integrated AI system, including where supported:

* user messages
* assistant messages
* agent task descriptions
* tool calls
* tool results
* shell commands
* command output
* source files inspected
* source files modified
* Git diffs
* Git commits
* test execution
* test results
* search queries
* search results
* documents inspected
* external API responses
* artifacts produced
* generated summaries
* task completion results
* model metadata
* project/workspace metadata
* timestamps
* provider/session identifiers

Ownstate must not depend on or attempt to obtain a provider's hidden chain-of-thought.
Only observable events and available artifacts are captured.

## 5. Product modes

### 5.1 Personal

The user continues using their own AI applications and subscriptions.

Examples:

```text
Codex
Claude Code
Cursor
ChatGPT
other MCP-compatible clients
        │
        ▼
     Ownstate
```

Ownstate provides persistent knowledge through MCP and other adapters.
Ownstate does not need to pay for the user's main agent inference.

### 5.2 Business

Business mode adds:

* organizational workspaces
* organization-owned knowledge
* RBAC/ABAC
* data classification
* audit trails
* model/provider policies
* managed agent execution
* UHP integration
* BYOK
* centralized model billing
* retention controls
* enterprise deployment options

Business functionality must build on the same knowledge core used by Personal mode.

## 6. Technology requirements

Use Rust for the backend.
Use a Rust workspace and modular monolith initially.
Do not begin with microservices.
Use current stable releases at implementation time.

Foundation:

```text
Rust
Axum
Tokio
SQLx
PostgreSQL
pgvector
S3-compatible object storage
fastembed
rmcp
```

Do not introduce unnecessary infrastructure.

In particular, do not start with:

* Kafka
* NATS
* Elasticsearch
* Neo4j
* Qdrant
* Kubernetes
* multiple independently deployed services

unless actual requirements later justify them.

## 7. Canonical database

PostgreSQL is the canonical database.

PostgreSQL owns:

* users
* organizations
* workspaces
* projects
* repositories
* sessions
* events
* knowledge
* provenance
* permissions
* classifications
* context packets
* audit data
* jobs
* vector indexes

Use pgvector for semantic retrieval.
Vector storage must remain an index over canonical data, not the canonical knowledge store itself.

## 8. Large-object storage

Large immutable content must be stored in S3-compatible object storage rather than unnecessarily bloating PostgreSQL.

Examples:

* imported conversation archives
* large transcripts
* large tool results
* generated artifacts
* document originals
* attachments
* large diffs

Content should be content-addressed.
Use BLAKE3 or another suitable cryptographic content digest.

Store in PostgreSQL:

```text
object_id
content_hash
size
media_type
storage_location
classification
encryption metadata
```

The same content should not be stored multiple times when the hash is identical.
Compression should be used where appropriate.

## 9. Identity hierarchy

The foundational ownership hierarchy must support:

```text
User
Organization
Workspace
Project
Repository
Session
```

Knowledge must always have an explicit ownership domain.

Initial ownership domains:

```text
PERSONAL
ORGANIZATION
```

Personal knowledge must never silently become organizational knowledge.
Organizational knowledge must never silently become personal knowledge.

## 10. Projects

A Project represents a durable logical body of work.

A project may contain:

* repositories
* documents
* conversations
* coding sessions
* research sessions
* cowork sessions
* decisions
* architecture
* people
* goals
* requirements

Projects are the primary knowledge scope for the initial product.

## 11. Source adapters

Provider-specific implementations must only normalize external data into Ownstate's internal event representation.

Examples:

```text
CodexAdapter
ClaudeCodeAdapter
ChatImportAdapter
McpAdapter
OpenRouterAdapter
UhpAdapter
GenericAdapter
```

Adapter code must NOT contain canonical knowledge rules.
Adapters are replaceable.

## 12. Normalized interaction event

Define a provider-neutral `InteractionEvent`.

It must be capable of representing:

```text
USER_MESSAGE
ASSISTANT_MESSAGE

TOOL_CALL
TOOL_RESULT

COMMAND
COMMAND_RESULT

FILE_READ
FILE_WRITE

SEARCH
SEARCH_RESULT

DOCUMENT_READ

ARTIFACT_CREATED

GIT_DIFF

TEST_RESULT

AGENT_RESULT

SYSTEM_EVENT
```

Important fields should include:

```text
id

tenant_id
workspace_id
project_id
session_id

source
source_session_id
source_event_id

event_type

actor_type
actor_id

sequence
occurred_at

model_provider
model_name

content
content_object_id
content_hash

tool_name
tool_call_id

repository_id
branch
commit_sha

metadata

security_classification

created_at
```

Events must be append-oriented.
Existing source evidence should not be rewritten because an extraction model later changes its interpretation.

## 13. Sessions

A session represents one bounded AI interaction/work period.

Examples:

* ChatGPT conversation
* Codex session
* Claude Code session
* cowork task
* UHP task
* imported historical conversation

Track:

```text
id
project_id
source
source_session_id

agent
provider
model

started_at
ended_at

repository
initial_commit
final_commit

status
metadata
```

## 14. Raw evidence layer

Raw interaction events are evidence.

The raw layer exists so Ownstate can:

* audit a fact
* reproduce extraction
* reprocess historical sessions
* improve extraction later
* detect errors
* retain provenance

Raw evidence is NOT automatically trusted knowledge.

## 15. Knowledge extraction

After meaningful interaction boundaries, Ownstate should determine whether useful durable knowledge may have been created.

Extraction triggers can include:

* meaningful completed turn
* task completion
* agent session completion
* significant code change
* significant tool activity
* explicit user request to remember
* imported conversation processing

Do not run extraction after every trivial event.
Avoid extraction for events containing no meaningful new context.

The extractor must be behind a Rust interface similar to:

```rust
trait KnowledgeExtractor {
    async fn extract(
        &self,
        input: ExtractionInput,
    ) -> Result<Vec<CandidateKnowledge>>;
}
```

Implementations may include:

```text
LocalExtractor
OpenRouterExtractor
EnterpriseExtractor
```

The rest of Ownstate must not depend on a specific model.

## 16. Extraction security

Treat extraction models as untrusted.

They must not have:

* PostgreSQL credentials
* direct canonical-knowledge writes
* unrestricted shell access
* authorization administration
* policy administration
* unrestricted network access
* secret-management access

Extractor output must be typed structured data.

For example:

```json
{
  "kind": "ARCHITECTURE",
  "subject": "authentication",
  "claim": "...",
  "evidence_event_ids": ["..."],
  "confidence": 0.91
}
```

Rust validates the result before anything is persisted as canonical knowledge.

## 17. Candidate knowledge

Candidate knowledge represents something that may deserve to become durable state.

Initial knowledge kinds:

```text
FACT
ARCHITECTURE
DECISION
RATIONALE
CONSTRAINT
REQUIREMENT
PROCEDURE
PREFERENCE
FAILURE
OUTCOME
GOAL
RISK
RELATIONSHIP
DEFINITION
OPEN_QUESTION
```

The schema must allow new kinds later.

## 18. Canonical knowledge model

Separate a semantic knowledge identity from its versions.

Conceptually:

```text
KnowledgeItem
      │
      ├── KnowledgeVersion 1
      ├── KnowledgeVersion 2
      └── KnowledgeVersion 3
```

Example:

```text
Authentication Architecture

v1:
Middleware → Handler

v2:
Middleware → Policy Service → Handler

v3:
Gateway → Policy Service → Handler
```

Old versions must not simply disappear.

## 19. KnowledgeItem

Represents the durable conceptual identity of knowledge.

Suggested fields:

```text
id

tenant_id
workspace_id
project_id

ownership_domain

kind
subject_key

created_at
created_by
```

## 20. KnowledgeVersion

Suggested fields:

```text
id
knowledge_item_id

content
structured_content

status

trust_level
confidence

valid_from
valid_until

created_at
created_by

supersedes_version_id
```

Initial statuses:

```text
CANDIDATE
ACTIVE
STALE
SUPERSEDED
CONFLICT
QUARANTINED
REVOKED
```

## 21. Provenance

Every canonical knowledge version must retain evidence.

Knowledge must be able to answer:
Why does Ownstate believe this?

Evidence can reference:

* interaction events
* documents
* files
* source lines
* Git commits
* diffs
* explicit user statements
* tool output
* external sources
* artifacts

Suggested evidence fields:

```text
knowledge_version_id

event_id
object_id

repository_id
commit_sha

file_path
line_start
line_end

source_type
content_hash

created_at
```

Knowledge without provenance should generally receive lower trust.

## 22. Trust levels

Initial trust levels:

```text
HUMAN_EXPLICIT
SOURCE_VERIFIED
REPO_VERIFIED
AGENT_DERIVED
EXTERNAL_VERIFIED
EXTERNAL_UNTRUSTED
```

Trust level must influence retrieval and conflict resolution.
Trust is not the same as model confidence.

## 23. Novelty engine

The system must determine whether extracted candidate knowledge is genuinely new.

Candidate classifications:

```text
NEW
DUPLICATE
UPDATE
CONTRADICTION
UNCERTAIN
```

Processing should be staged for efficiency:

```text
candidate
   ↓
normalized fingerprint
   ↓
kind + subject filtering
   ↓
lexical/vector retrieval
   ↓
top related canonical items
   ↓
deterministic comparison where possible
   ↓
LLM comparison only when necessary
```

Do not run expensive model comparison when deterministic comparison is sufficient.

## 24. Knowledge updates

If newly observed information changes existing knowledge:

```text
old version
    ↓
SUPERSEDED
```

and create:

```text
new version
    ↓
ACTIVE
```

Do not overwrite history.

## 25. Contradictions

Conflicting evidence must not be silently resolved by whichever model ran last.

Store contradictions explicitly.

Resolution can consider:

* current authoritative source
* source freshness
* trust level
* explicit human decisions
* repository state
* document version
* temporal validity

When uncertainty remains:

```text
status = CONFLICT
```

## 26. Git-aware knowledge

Coding knowledge must understand repository state.

Code-derived knowledge should record where possible:

```text
repository
branch
commit
files
symbols
content hashes
```

If the relevant source changes later, Ownstate should be capable of identifying possibly stale knowledge.

Example:

```text
knowledge derived at abc123

relevant source changes at def456

knowledge → STALE or NEEDS_VERIFICATION
```

A future agent should be warned before receiving stale architecture as current truth.

## 27. Embeddings

Generate embeddings locally by default where practical.

Use an embedding abstraction:

```rust
trait EmbeddingProvider {
    async fn embed(
        &self,
        input: &[String],
    ) -> Result<Vec<Embedding>>;
}
```

Initial implementation may use `fastembed`.

Embedding records must contain:

```text
entity_id
entity_type

model
model_version
dimensions

embedding

created_at
```

Never couple canonical knowledge to one embedding model.
Re-embedding must be possible without rewriting knowledge.

## 28. Retrieval

Retrieval must be hybrid.
Do not use vector similarity alone.

Candidate generation should consider:

* exact identifiers
* PostgreSQL full-text search
* vector similarity
* project/workspace scope
* knowledge kind
* temporal validity
* trust
* security classification
* recency
* relationships

Use a simple fusion method initially, such as Reciprocal Rank Fusion.
Optionally rerank a small candidate set locally.

## 29. Context Compiler

The Context Compiler is the core read path.
It must turn a task into the smallest useful trusted context package.

Pipeline:

```text
Request
   ↓
Authentication
   ↓
Authorization
   ↓
Ownership scope
   ↓
Provider/model policy
   ↓
Task interpretation
   ↓
Hybrid knowledge retrieval
   ↓
Trust/staleness filtering
   ↓
Deduplication
   ↓
Token budgeting
   ↓
Context Package
```

A generative model must NOT be required for the basic compiler.
Models may later be used for optional compression.

## 30. Context categories

A project context packet may contain:

```text
PROJECT SUMMARY
RELEVANT ARCHITECTURE
RELEVANT DECISIONS
RATIONALE
CONSTRAINTS
REQUIREMENTS
PREVIOUS FAILURES
PREVIOUS OUTCOMES
RECENT CHANGES
OPEN QUESTIONS
SOURCES
```

Only categories relevant to the task should be included.

## 31. Token efficiency

Ownstate should optimize for:
the smallest context that gives the agent the required accumulated understanding.

Do not blindly send the entire knowledge base.

Separate:

```text
PROJECT BOOTSTRAP
+
TASK-SPECIFIC CONTEXT
```

A trivial task should not receive thousands of tokens of unrelated architecture.

## 32. Context packet audit

Every context packet sent to an AI must itself be recorded.

Suggested fields:

```text
id

tenant_id
project_id
session_id

provider
model
agent

query
token_budget

knowledge_version_ids

estimated_tokens

created_at
```

This allows Ownstate to know:
What did this agent already know before producing new knowledge?

This is important for novelty detection.

## 33. MCP interface

There must be one canonical Ownstate MCP server.
Do NOT build separate MCP servers for different models.

Initial read-oriented tools:

```text
bootstrap_project
search_knowledge
get_knowledge
```

Initial write-oriented interface:

```text
propose_knowledge
feedback
```

Models must not have:

```text
force_write_canonical_knowledge
execute_sql
change_permissions
change_policy
```

The MCP layer must use the same authorization and policy engine as the REST API.

## 34. Model and agent agnosticism

Ownstate must not require native MCP support from every model.

Support three integration paths:

Native MCP

```text
AI Client
   ↓ MCP
Ownstate
```

Tool-calling bridge

```text
Kimi / MiniMax / DeepSeek / etc.
          ↓
     tool calling
          ↓
Ownstate Gateway
          ↓
         MCP
          ↓
      Ownstate
```

Context pre-injection

For models without usable tool support:

```text
task
 ↓
Ownstate Context Compiler
 ↓
context packet
 ↓
model prompt
```

The knowledge system must remain identical across all three.

## 35. OpenRouter

OpenRouter is an optional model-runtime adapter.
Ownstate must not depend on it.

Define a provider-neutral interface such as:

```rust
trait ModelRuntime {
    async fn complete(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse>;

    fn capabilities(&self) -> ModelCapabilities;
}
```

Possible adapters:

```text
OpenRouterRuntime
LocalRuntime
DirectProviderRuntime
future runtimes
```

OpenRouter is useful for normalizing tool-capable models, but must remain replaceable.

## 36. Security classification

Initial data classifications:

```text
PUBLIC
INTERNAL
CONFIDENTIAL
RESTRICTED
SECRET
```

Classification must be evaluated before context leaves Ownstate.

Provider/model policies should eventually support:

```text
SECRET
→ local/private model only

RESTRICTED
→ approved enterprise providers

PUBLIC
→ any approved model
```

The AI model itself must never decide whether it is authorized to receive information.

## 37. Prompt-injection protection

Assume prompt injection will sometimes succeed.
Security must therefore rely on capability boundaries rather than perfect prompt detection.

Important requirements:

1. Raw retrieved content is treated as data, not instructions.
2. Extraction models cannot directly mutate canonical knowledge.
3. Models cannot create organization security policies automatically.
4. Models cannot grant themselves permissions.
5. Candidate knowledge from untrusted sources receives appropriate trust classification.
6. External instructions embedded in documents, source comments or web content must not automatically become persistent agent instructions.
7. Sensitive writes are controlled by deterministic Rust authorization/policy logic.

## 38. Encryption and secrets

Use:

* TLS
* encrypted infrastructure storage
* secure secret management
* application-level encryption where appropriate
* authenticated encryption
* strict secret redaction
* no sensitive content in ordinary logs

Use envelope encryption for high-sensitivity data.

Concept:

```text
KMS/HSM master key
       ↓
tenant/project data key
       ↓
encrypted records/objects
```

Never log:

* API keys
* access tokens
* plaintext encryption keys
* SECRET knowledge
* raw confidential context by default

## 39. Tenant isolation

Business mode must enforce tenant isolation at:

```text
application authorization
+
PostgreSQL Row Level Security
```

All tenant-owned records must include an appropriate tenant identifier.
Cross-tenant leakage is a critical security failure.

## 40. Audit logging

Audit security-sensitive activity.

Examples:

```text
knowledge viewed
knowledge exported
knowledge deleted

candidate promoted
candidate rejected

classification changed

permission changed

context sent to model

policy changed
```

Audit records must be append-oriented and tamper-evident.

## 41. Observability

Use structured tracing and OpenTelemetry-compatible instrumentation.

Record:

* request IDs
* operation type
* latency
* retrieval counts
* extraction counts
* model usage
* token usage
* errors
* job state

Do not emit private context into observability systems by default.

## 42. Background jobs

Use PostgreSQL as the first durable job queue.

Possible jobs:

```text
EXTRACT_KNOWLEDGE
GENERATE_EMBEDDING
NOVELTY_CHECK
REEMBED
VERIFY_GIT_PROVENANCE
PROCESS_IMPORT
```

Workers should claim jobs transactionally using PostgreSQL locking semantics such as `FOR UPDATE SKIP LOCKED`.

Do not add Kafka/NATS until scale proves it necessary.

## 43. API boundaries

Initial HTTP API should include only what is necessary.

Health:

```text
GET /health
GET /ready
```

Projects:

```text
POST /projects
GET /projects/:id
```

Sessions:

```text
POST /sessions
POST /sessions/:id/events
GET /sessions/:id
```

Knowledge:

```text
GET /knowledge/search
GET /knowledge/:id
POST /knowledge/proposals
```

Context:

```text
POST /context/compile
```

Admin/debug endpoints must not expose raw secrets.

## 44. Initial Rust workspace

Recommended structure:

```text
ownstate/
│
├── Cargo.toml
├── rust-toolchain.toml
├── docker-compose.yml
│
├── apps/
│   ├── api/
│   └── worker/
│
├── crates/
│   ├── domain/
│   ├── storage/
│   ├── capture/
│   ├── knowledge/
│   ├── learning/
│   ├── retrieval/
│   ├── context/
│   ├── security/
│   ├── audit/
│   ├── embeddings/
│   ├── object-store/
│   ├── mcp/
│   └── model-runtime/
│
├── migrations/
│
└── docs/
```

Keep the domain layer independent of:

* Axum
* PostgreSQL
* OpenRouter
* MCP
* Codex
* Claude
* Kimi
* UHP

Infrastructure depends on domain abstractions, not vice versa.

## 45. Initial vertical slice

Do NOT implement the whole specification immediately.

The first production-grade vertical slice is:

```text
Project
  ↓
Session
  ↓
Interaction Events
  ↓
Candidate Knowledge
  ↓
Canonical Knowledge
  ↓
Embedding
  ↓
Hybrid Search
  ↓
Context Compilation
  ↓
MCP retrieval
```

For v0.1, knowledge extraction may initially be explicitly triggered rather than fully automatic.

## 46. Initial implementation milestones

### Milestone 1 - Foundation

Implement:

* Rust workspace
* configuration
* PostgreSQL connection
* pgvector
* migrations
* project model
* session model
* interaction-event model
* health/readiness
* structured errors
* tracing

### Milestone 2 - Evidence

Implement:

* append interaction events
* object references
* content hashing
* event retrieval
* source/session provenance

### Milestone 3 - Knowledge

Implement:

* candidate knowledge
* canonical knowledge
* versions
* evidence links
* statuses
* trust levels
* basic deterministic promotion

Do not integrate an LLM yet if it makes this milestone harder to validate.

### Milestone 4 - Retrieval

Implement:

* embeddings
* pgvector
* PostgreSQL FTS
* hybrid ranking
* project-scoped search

### Milestone 5 - Context Compiler

Implement:

```text
POST /context/compile
```

Input:

```json
{
  "project_id": "...",
  "task": "Explain the authentication architecture",
  "max_items": 20
}
```

Output must include:

* selected knowledge
* provenance
* trust
* staleness
* source identifiers

### Milestone 6 - MCP

Expose:

```text
bootstrap_project
search_knowledge
get_knowledge
```

through the official Rust MCP SDK.

Prove that two different MCP-capable AI clients can retrieve the same project knowledge.

### Milestone 7 - Automatic learning

Introduce:

* extraction model abstraction
* extraction jobs
* novelty detection
* NEW/DUPLICATE/UPDATE/CONTRADICTION
* candidate promotion rules

### Milestone 8 - First real adapter

Implement one coding-agent adapter, preferably Codex.

Capture real work and demonstrate:

```text
Codex learns project
        ↓
Ownstate records durable knowledge
        ↓
new AI session
        ↓
Ownstate supplies knowledge
        ↓
agent does not need full rediscovery
```

## 47. Testing requirements

Every major domain invariant requires tests.

At minimum test:

**Tenant/project isolation** — No project can retrieve another project's scoped knowledge unintentionally.

**Evidence immutability** — Knowledge changes cannot alter the raw source events they originated from.

**Knowledge versions** — Updating knowledge supersedes rather than overwrites.

**Contradiction handling** — Conflicting claims do not silently replace trusted knowledge.

**Candidate promotion** — Untrusted extraction output cannot bypass promotion rules.

**Retrieval** — Project and security scopes are applied before results are returned.

**Context compilation** — Token/item budgets are respected.

**MCP** — MCP requests are subject to the same authorization rules as HTTP.

## 48. Performance goals for initial architecture

The system should be engineered so that ordinary context retrieval does not require a generative LLM.

Target architecture:

```text
authorization
+
Postgres
+
pgvector
+
local ranking
```

Context retrieval should remain low-latency enough for interactive agent usage.

Batch:

* embedding generation
* extraction
* reprocessing
* imports

through workers where appropriate.

## 49. Data portability

Ownstate's purpose requires avoiding new lock-in.

Users must eventually be able to export:

* projects
* raw interactions
* canonical knowledge
* knowledge history
* provenance
* relationships
* metadata

using documented vendor-neutral formats.

The canonical schema must not use opaque provider-specific structures as its public representation.

## 50. Non-goals for initial implementation

Do NOT initially build:

* a replacement for ChatGPT
* a replacement for Codex
* a full autonomous coding agent
* a general model marketplace
* a graph database
* UHP execution
* enterprise SSO
* billing
* complicated dashboards
* mobile applications
* browser extensions
* automatic access to every AI provider

The first objective is proving that knowledge survives between AI sessions and vendors.

## 51. Foundational acceptance test

The foundational product is successful when this scenario works:

1. Create Project X.
2. Record a real AI/coding-agent session.
3. Store its raw observable interaction evidence.
4. Extract important architectural/project knowledge.
5. Store that knowledge with provenance.
6. Start a completely new AI session.
7. The new AI retrieves Ownstate context.
8. It can accurately explain relevant existing project architecture and decisions without rereading the complete historical interaction.
9. Ask a question outside the stored knowledge.
10. The new agent discovers the information.
11. Ownstate detects that the information is new.
12. The new knowledge becomes part of the canonical project state.
13. A third AI can retrieve it later.

That is the first complete Ownstate learning loop.

## 52. Core engineering rules

1. Prefer correctness over cleverness.
2. Prefer simple infrastructure.
3. PostgreSQL is the system of record.
4. Vectors are indexes, not truth.
5. Raw evidence is separate from derived knowledge.
6. Candidate knowledge is separate from canonical knowledge.
7. Extraction models are untrusted.
8. Canonical knowledge is versioned.
9. Every important fact has provenance.
10. Models are replaceable.
11. Agents are replaceable.
12. Model providers are replaceable.
13. MCP is an interface, not the knowledge model.
14. OpenRouter is an adapter, not a core dependency.
15. UHP will be an adapter, not a core dependency.
16. Authorization happens before retrieval.
17. Untrusted text never becomes policy automatically.
18. Current authoritative evidence outranks stale AI inference.
19. Do not optimize for number of memories stored.
20. Optimize for high-value knowledge accurately delivered when needed.

## 53. Product success metric

The product should eventually measure:

**Knowledge Retention** — How much useful AI-produced understanding survives between agents?

**Rediscovery Avoidance** — How much work/tokens would a new AI have required to reconstruct knowledge Ownstate already had?

A successful system should allow:

```text
Agent A learns
       ↓
Ownstate retains
       ↓
Agent B starts informed
       ↓
Agent B learns more
       ↓
Ownstate compounds
```

That compounding knowledge loop is the core product.
