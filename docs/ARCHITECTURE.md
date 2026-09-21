# Ownstate Architecture

## Product Goal

Ownstate is a sovereign institutional cognition layer for individuals and organizations.

The core idea is simple:

> AI models, agents, vendors, and applications should be replaceable. The knowledge accumulated while using them should belong to the individual or organization.

Today, knowledge created while working with AI is fragmented across AI conversations, coding agents, cowork agents, documents, decks, email, Slack, GitHub, CRM systems, and other applications. Each new AI session often has to reconstruct context that another AI already learned.

Ownstate exists to prevent that loss.

It should continuously build and maintain a durable representation of:

- what an organization knows
- what has happened
- what is currently true
- what used to be true
- what changed
- what decisions were made
- why those decisions were made
- what projects exist
- how systems are architected
- who customers, investors, partners, and vendors are
- what commitments have been made
- what requirements exist
- what artifacts have been created or exchanged
- what failed
- what succeeded
- what risks remain
- what work is currently happening
- how the organization prefers to work

The long-term goal is for Ownstate to become the institutional context substrate on top of which humans, personal AI models, individual agents, and agent swarms can work.

---

## 1. Architectural North Star

```text
                              OWNSTATE

┌──────────────────────────────────────────────────────────────┐
│                       SOURCES                                │
│                                                              │
│ AI chats   Coding agents   Cowork agents   Files / Artifacts │
│                      Optional connectors                     │
└──────────────────────────────┬───────────────────────────────┘
                               ▼
                    NORMALIZED EVIDENCE GATEWAY
                               │
               immutable + content-addressed + ACL
                               ▼
                     EVIDENCE / EVENT LAYER
                               │
             ┌─────────────────┴─────────────────┐
             ▼                                   ▼
      KNOWLEDGE PIPELINE                  EXPERIENCE PIPELINE
             │                                   │
             ▼                                   ▼
     Candidate Knowledge                    Experience Store
             │
             ▼
  Novelty / Validation / Policy
             │
             ▼
      CANONICAL INSTITUTIONAL STATE
             │
   ┌─────────┼────────────┬──────────────┐
   ▼         ▼            ▼              ▼
Entities  Relations      Claims      Semantic Knowledge
   │         │            │              │
   └─────────┴────────────┴──────────────┘
                       │
                       ▼
             QUERY / RETRIEVAL PLANE
                       │
      SQL + FTS + vectors + graph + temporal
                       │
                       ▼
                 Query Planner
                       │
                       ▼
                Context Compiler
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
      Humans         Models         Agents
                                        │
                                        ▼
                                  Agent Swarms
                                        │
                                        ▼
                                       Work
                                        │
                                        └──► Evidence
```

Ownstate should become more accurate, more connected, and more useful as this loop repeats.

---

## 2. Core Product Principle

Ownstate is not a chatbot, a model, a vector database, or an agent framework.

Ownstate is the persistent institutional state that those systems use.

The core learning loop is:

```text
Human / AI / Agent Activity
            ↓
       Raw Evidence
            ↓
    Knowledge Extraction
            ↓
    Candidate Knowledge
            ↓
 Novelty / Validation / Policy
            ↓
    Canonical Knowledge
            ↓
     Context Compiler
            ↓
       Next AI / Agent
            ↓
         New Work
            └──────────► Ownstate learns again
```

---

## 3. Sovereignty

Ownstate is designed around four forms of sovereignty.

### Knowledge sovereignty

Canonical knowledge belongs to the individual or organization. It must not depend on any particular model provider, model, agent, framework, SaaS application, or inference vendor.

### Model sovereignty

Models must remain interchangeable. GPT, Claude, Kimi, MiniMax, DeepSeek, local models, or future models should be replaceable without rebuilding organizational knowledge.

### Agent sovereignty

Codex, Claude Code, Cursor, cowork agents, custom Ownstate agents, and future harnesses should consume Ownstate knowledge rather than maintain incompatible long-term company memories.

### Infrastructure sovereignty

Ownstate must support hosted and customer-controlled deployments. An institution should eventually be able to operate its own PostgreSQL, object storage, Ownstate services, embedding runtime, model runtime, KMS, and agent infrastructure.

---

## 4. Memory Types

Ownstate distinguishes four memory classes.

### Episodic memory

What happened: messages, tool calls, agent sessions, artifacts created, commands, tests, customer interactions, and other observable events.

### Semantic memory

What is known: architecture, business facts, customer requirements, decisions, rationale, constraints, risks, procedures, and lessons.

### Structured institutional state

Facts that should be queryable deterministically: legal entities, jurisdictions, customers, investors, contracts, projects, products, people, artifacts, and relationships.

### Procedural memory

How the user or organization works: preferred debugging approaches, research methodology, coding conventions, successful workflows, tool-use strategies, and writing preferences. This may later contribute to personal-model adaptation.

---

## 5. Knowledge Layers

Ownstate must preserve strict separation between:

```text
RAW EVIDENCE
    ↓
CANDIDATE KNOWLEDGE
    ↓
CANONICAL KNOWLEDGE
```

Raw evidence is append-oriented and represents observable source material. Candidate knowledge is information a model or deterministic extractor believes may be worth remembering. Canonical knowledge is Ownstate's accepted institutional state.

Canonical state must support provenance, ownership, temporal validity, versioning, contradiction, supersession, and authorization.

Models cannot directly write canonical knowledge.

---

## 6. Institutional Knowledge Model

Ownstate represents the organization with five complementary concepts:

- Entities
- Relationships
- Claims
- Semantic Knowledge
- Evidence

### Entities

Durable identities such as Person, Organization, LegalEntity, Jurisdiction, Customer, Investor, Partner, Vendor, Project, Product, Repository, Contract, Artifact, Deck, Fundraise, Requirement, Risk, Issue, and Decision.

### Relationships

Examples:

```text
Person WORKS_AT Organization
LegalEntity INCORPORATED_IN Jurisdiction
Customer CONTRACTS_WITH LegalEntity
Investor RECEIVED Deck
Customer REQUESTED Requirement
Project USES Repository
Decision AFFECTS Project
```

Relationships should support temporal validity and provenance.

### Claims

Atomic assertions such as:

```text
Customer X STATUS PILOT
Project X TARGET_LAUNCH_DATE 2026-11-01
Product X USES PASETO
```

Claims should support `valid_from`, `valid_until`, `observed_at`, `recorded_at`, trust, confidence, status, and provenance.

### Semantic Knowledge

Meaning-rich information such as architecture, decision rationale, lessons learned, procedures, failure analysis, strategy, technical explanations, customer context, constraints, and requirements.

---

## 7. Temporal Knowledge and Freshness

Ownstate must understand that organizations change.

Example:

```text
Customer X

PROSPECT
Jan → Apr

PILOT
Apr → Sep

ACTIVE CUSTOMER
Sep →
```

Ownstate should eventually answer:

- What is true now?
- What was true six months ago?
- What changed?

Knowledge transitions should support:

- NEW
- DUPLICATE
- CONFIRMS
- UPDATES
- SUPERSEDES
- CONTRADICTS
- STALE
- UNRESOLVED

Newer evidence is not automatically more authoritative.

When new evidence arrives, Ownstate should identify affected state, compare it against current knowledge, preserve historical truth, create new current state where appropriate, and invalidate stale retrieval projections/context.

Code-derived architecture should be capable of becoming stale when relevant code changes. New artifact versions should supersede current-state claims without erasing history.

---

## 8. Provenance and Source Authority

Ownstate must be able to answer: **Why do we believe this?**

Every important piece of canonical state should be connected to supporting evidence.

Authority should depend on knowledge domain.

Examples:

```text
current repository
>
old AI conversation describing current code
```

```text
verified incorporation record
>
AI speculation about company structure
```

```text
explicit decision record
>
contemporaneous discussion
>
later recollection
>
AI inference
```

---

## 9. Entity Resolution

Different references may describe the same entity. Resolution should consider exact identifiers, known aliases, structured attributes, relationships, semantic similarity, and model-assisted comparison only when needed.

Uncertain entities must not be destructively merged. Merges must be auditable and reversible.

---

## 10. Artifacts

Artifacts are first-class institutional objects: decks, contracts, technical specs, research reports, financial models, presentations, spreadsheets, code artifacts, proposals, and architecture diagrams.

Artifacts must support versions and relationships such as SENT_TO, REVIEWED_BY, CREATED_FOR, and SUPERSEDES.

Artifact content should be content-addressed so identical binaries are not repeatedly stored.

---

## 11. Sources and Optional Connectors

Ownstate must work without external application connectors.

Minimum sources:

- AI interactions
- agent interactions
- explicit user knowledge
- uploaded files
- generated artifacts

Optional connectors may include Slack, Email, Teams, GitHub, GitLab, Jira, Linear, Google Drive, SharePoint, OneDrive, Dropbox, Box, Notion, Confluence, Salesforce, HubSpot, Calendar, Microsoft 365, and internal systems.

Connectors are sensors, not Ownstate itself. No connector may directly write canonical knowledge.

Source ACLs should propagate through evidence and derived knowledge. If a user cannot access a source, Ownstate must not expose knowledge derived exclusively from that source.

---

## 12. Retrieval: Temporal Hybrid GraphRAG

Ownstate should not depend solely on vector search.

The query system should combine:

- structured SQL
- exact identifiers
- PostgreSQL full-text search
- dense vector retrieval
- sparse lexical retrieval where useful
- relationship traversal
- temporal filtering
- source authority
- security classification
- recency
- reranking

GraphRAG is a retrieval methodology, not Ownstate's canonical storage layer.

The canonical state remains Ownstate's Rust/PostgreSQL domain model.

---

## 13. Query Planner

Different questions require different plans.

Structured:

```text
How many legal entities do we have in each jurisdiction?
```

should resolve through structured state and SQL.

Semantic:

```text
Why did we choose the current recovery architecture?
```

should resolve through semantic and temporal knowledge with provenance.

Hybrid:

```text
Which European customers requested settlement automation,
and what exactly did they ask for?
```

should combine structured filtering, relationships, semantic retrieval, and temporal data.

The planner should support StructuredQuery, SemanticQuery, RelationshipQuery, TemporalQuery, and HybridQuery. A model may help create the plan but must never receive unrestricted SQL execution.

---

## 14. Context Compiler

The Context Compiler turns institutional knowledge into the smallest useful authorized context for a model or agent.

```text
Task
 ↓
Principal
 ↓
Authorization
 ↓
Entity / project / relationship resolution
 ↓
Query planning
 ↓
Retrieval
 ↓
Freshness filtering
 ↓
Trust filtering
 ↓
Deduplication
 ↓
Reranking
 ↓
Token budget
 ↓
ContextPacket
```

Every ContextPacket should record the requesting principal, agent/model, task, project/scope, knowledge versions, entities, claims, evidence, permissions, classifications, token budget, and timestamp.

---

## 15. Canonical Storage

PostgreSQL remains the canonical database for identity, organizations, projects, entities, relationships, claims, knowledge, versions, provenance, permissions, temporal state, audit data, jobs, and retrieval metadata.

Use pgvector for semantic retrieval. Vectors are indexes, not truth.

Large binary/raw content should live in S3-compatible object storage using content addressing such as BLAKE3.

---

## 16. Event Architecture and Learning Workers

Request handling should not wait for expensive AI processing.

Start with a transactional PostgreSQL outbox:

```text
database transaction
    ↓
canonical write + outbox event
    ↓
commit
    ↓
workers
```

Define an internal EventBus abstraction. NATS JetStream may later sit behind it when scale justifies it.

Asynchronous workers may handle ingestion, artifact processing, classification, embedding generation, entity extraction, entity resolution, knowledge extraction, novelty detection, verification, connector sync, and freshness evaluation.

Workers should be stateless, idempotent, and horizontally scalable.

---

## 17. Model Runtime

Models are infrastructure adapters.

```rust
trait ModelRuntime {
    async fn infer(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse>;

    fn capabilities(&self) -> ModelCapabilities;
}
```

Possible implementations include OpenRouterRuntime, OpenAICompatibleRuntime, and SelfHostedRuntime.

OpenRouter may provide hosted model access, but the domain must not depend on OpenRouter. Institutions must be able to use self-hosted inference through OpenAI-compatible systems such as vLLM.

---

## 18. Personal Model and Experience Store

The personal model should consume Ownstate context rather than encode rapidly changing institutional facts permanently into model weights.

Keep current factual knowledge in Ownstate and procedural learning in a separate Experience Store.

A training experience may include task, ContextPacket, teacher model, response, tool usage, artifact/output, objective result, user feedback, quality signals, classification, and provenance.

The Experience Store can later produce versioned training datasets for user-owned or institution-owned models. Do not continuously fine-tune after every interaction.

---

## 19. MCP, UHP, and Agents

### MCP

Ownstate exposes one canonical MCP server. MCP is the universal context/tool interface. Do not build model-specific MCP servers.

Native MCP clients use it directly. Tool-calling models can use a bridge. Models without tools can receive controlled ContextPacket injection.

### UHP

UHP belongs in the managed agent-execution layer behind an internal `HarnessRuntime` abstraction. Personal mode does not require UHP.

### Agent architecture

Agents do not own long-term institutional memory.

```text
Agent starts
    ↓
authenticate
    ↓
receive authorization scope
    ↓
Context Compiler
    ↓
ContextPacket
    ↓
perform task
    ↓
tools / artifacts / results
    ↓
Raw Evidence
    ↓
Ownstate learning pipeline
```

Agents can learn but cannot directly declare canonical truth.

### Agent swarms

Future swarms should allow permission-scoped finance, engineering, legal, research, sales, product, and operations agents to work over one canonical institutional state.

Each subagent must have its own identity, permissions, ContextPacket, tools, task, runtime, and execution record. Parent-agent permissions must not automatically transfer to children.

A2A or similar protocols should only be introduced where independently deployed agents genuinely need vendor-neutral peer communication.

---

## 20. Authorization and Security Classification

Authorization is a core system, not model behavior.

Preferred policy shape:

```text
principal
action
resource
context
```

Use deterministic Rust-side authorization. Cedar or an equivalent mature policy system may be used where valuable. PostgreSQL RLS should provide defense in depth where appropriate.

Knowledge and evidence should support PUBLIC, INTERNAL, CONFIDENTIAL, RESTRICTED, and SECRET classifications.

Model-egress policy should be enforceable. Models never decide whether they may receive information.

Assume prompt injection can succeed at the model level. All AI output, emails, Slack messages, documents, web pages, source comments, issue descriptions, tool results, and connector records are untrusted content. Models may propose; Rust code controls authorization, promotion, policy, canonical writes, tool access, and model egress.

---

## 21. Scalability

Ownstate should begin deployable as:

```text
Rust API
Rust worker
PostgreSQL + pgvector
S3-compatible object storage
```

and evolve into:

```text
API × N
MCP × N
Workers × N
Agent runtime × N
NATS JetStream
PostgreSQL replicas
Tenant partitions
Dedicated large-tenant databases
Specialized read/search projections
Large object archives
```

without changing canonical domain semantics.

Small tenants may share infrastructure. Large tenants may receive dedicated partitions, vector indexes, databases, or deployments.

Canonical storage should remain independent of retrieval technology. SQL, vector, search, graph, and analytics systems should be treated as projections where specialized systems become necessary.

---

## 22. Evaluation

Ownstate must measure knowledge quality, not memory volume.

Important metrics include retrieval precision/recall, entity-resolution accuracy, structured-query accuracy, temporal-query accuracy, contradiction detection, stale knowledge rate, provenance coverage, cross-tenant leakage, ContextPacket size, agent task success, and rediscovery tokens avoided.

### Rediscovery Cost

> How many tokens or how much work would a new AI have required to reconstruct information Ownstate already knew?

Ownstate should drive this number downward over time.

---

## 23. Technology Direction

Preferred direction where appropriate:

- Rust, Tokio, Axum
- PostgreSQL 18 + pgvector
- S3-compatible object storage
- FastEmbed and modern BGE-family embedding/reranking models
- transactional outbox
- NATS JetStream when scale requires it
- MCP
- UHP behind HarnessRuntime
- OpenRouter behind ModelRuntime
- OpenAI-compatible self-hosted inference such as vLLM
- Cedar or equivalent policy engine
- tracing + OpenTelemetry

Useful methodologies include RAG, hybrid retrieval, temporal GraphRAG, knowledge graphs, reranking, context engineering, and eval-driven retrieval.

Do not adopt agent frameworks merely because they are popular.

Ownstate should not initially depend on a graph database, Microsoft GraphRAG runtime, Graphiti runtime, LangChain, LangGraph, CrewAI, AutoGen, a dedicated vector database, or Redis as canonical storage.

Their concepts may be useful; the Ownstate Rust domain remains authoritative.

---

## 24. Open Source and Operability

Ownstate is intended to be open source. The repository itself is a product surface.

### README

`README.md` must remain current. Any change that materially affects setup, configuration, environment variables, Docker, APIs, MCP, model runtimes, infrastructure, or supported capabilities must update the README and relevant docs in the same change.

### Environment-based configuration

Maintain an accurate root `.env.example`. All runtime settings should be parsed centrally into typed Rust configuration. Production must not depend on `.env` files, but environment variables should work with Docker, Kubernetes, Nomad, systemd, and cloud platforms.

### Docker-first development

Target:

```bash
cp .env.example .env
docker compose up --build
```

Keep the default stack light. Optional infrastructure should use Compose profiles or equivalent.

```bash
docker compose --profile nats up
docker compose --profile local-ai up
```

Do not make paid AI APIs, NATS, local GPU inference, observability stacks, or external connectors mandatory for basic startup.

### Progressive infrastructure

Minimum deployment:

```text
Ownstate API
Ownstate Worker
PostgreSQL + pgvector
Object Storage when needed
```

Scaled deployments may add NATS, multiple workers, read replicas, dedicated tenant databases, self-hosted inference, and specialized retrieval projections.

### Docker images

Use multi-stage builds, minimal runtime images, non-root users where practical, health checks, graceful shutdown, no baked secrets, and no unnecessary compiler/build cache in final images.

### Migrations and CI

A fresh database must be constructible from version-controlled migrations. Local quality commands should stay aligned with CI:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

### Public examples

Never place confidential company information into public tests, seed data, docs, examples, screenshots, or fixtures.

### Extension points

Stable interfaces should eventually exist for Connector, ModelRuntime, EmbeddingProvider, HarnessRuntime, EventBus, ObjectStore, KnowledgeExtractor, Reranker, and Authorization provider.

Avoid a complicated plugin framework until real third-party extension needs justify one.

### Open-source architectural rule

A contributor must be able to understand and operate Ownstate without depending on undocumented hosted Ownstate services. Hosted services may improve convenience, but must not become required for core functionality.

---

## 25. Long-Term Vision

Ownstate should become the layer an authorized AI consults to understand:

```text
Who are we?
What do we do?
What do we own?
What do we know?
What happened?
What are we doing?
What changed?
Why did we make this decision?
Who are our customers?
Who are our investors?
What have we promised?
What relationships matter?
What risks exist?
What is unresolved?
What should this agent know before acting?
```

The organization should be able to replace the model, AI provider, coding agent, cowork agent, orchestration system, or inference infrastructure without losing accumulated understanding.

---

## 26. Engineering Priority

Optimize decisions in this order:

1. Correctness of institutional knowledge.
2. Security and authorization.
3. Provenance and explainability.
4. Freshness and temporal correctness.
5. Model and agent independence.
6. Data ownership and portability.
7. Retrieval quality.
8. Operational simplicity.
9. Scalability.
10. Performance.

Avoid prematurely optimizing infrastructure at the expense of the knowledge model. The canonical domain model should survive technologies changing around it.

---

## 27. Automatic Capture and Scope Resolution

Ownstate must hide storage identity from normal model and user workflows. Models
provide observable task, repository, artifact, thread, and source evidence;
Ownstate resolves durable institutional scope and authorization.

```text
Host / Adapter / MCP
        ↓
Normalized observable scope evidence
        ↓
Authenticated principal + deterministic ScopeResolver
        ↓
Organization / Project / Repository / Artifact / Thread / General scopes
        ↓
Opaque non-authoritative scope handle
        ↓
Authorized progressive ContextPacket
        ↓
Append-only captured evidence
        ↓
Existing extraction / novelty / policy / promotion pipeline
```

Repository identity uses normalized canonical remotes and aliases where
available. Clones, worktrees, branches, model changes, and directory moves do not
create new repositories. Forks remain separate and may link to upstream.
Repositories without remotes use provisional identities that can later associate
with a canonical remote without deleting history.

Projects and repositories remain distinct. Exact repository discovery may
bootstrap a default project, while broader projects can contain multiple
repositories, artifacts, threads, relationships, and knowledge. Non-code work
uses hierarchical scope evidence and remains provisional when identity is
uncertain. Global personal and organization knowledge does not require a fake
project.

MCP is the universal context/tool interface, not a promise of complete host
observation. Native adapters, hooks, imports, or gateways may provide stronger
capture. Every captured session records its factual capture mode and per-channel
completeness. Provider/transport session identity never becomes canonical
institutional identity.

Scope handles are explicit application references, not credentials or
authorization. Each operation resolves the authenticated principal and applies
current scope, classification, grant, freshness, and egress policy.

The model-controlled MCP surface is append/read/propose oriented. It cannot
delete or rewrite recorded evidence, canonical versions, artifacts, scope
history, grants, policy, or audit. Legitimate retention/deletion is a separate
administrative workflow and is not reachable through MCP.

Context delivery uses progressive layers: minimal identity, small stable
bootstrap, task-specific retrieval, then authorized evidence drill-down. Storage
may be large; default context remains bounded, deduplicated, fresh, explainable,
and exactly audited.

Detailed normative behavior is in
[`AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`](AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md).
Mandatory security acceptance is in
[`MCP_CAPTURE_SECURITY.md`](MCP_CAPTURE_SECURITY.md).
