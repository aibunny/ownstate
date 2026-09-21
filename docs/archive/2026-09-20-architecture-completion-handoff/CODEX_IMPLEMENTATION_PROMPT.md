# Codex Implementation Prompt: Evolve Existing Ownstate Repository

> **Legacy brief — do not execute this file by itself.** The user corrected its
> original authority paths: root `ARCHITECTURE.md`, root `REQUIREMENTS.md`, and
> root `README.md` define the target; files under `docs/` describe the current
> implementation. The source is currently interrupted and not green. Start with
> `CONTINUATION_PROMPT.md`, `STATE.md`, and `docs/PENDING_IMPLEMENTATION.md`.
> Those files override sequencing, current evidence, and scope in this legacy
> brief. Implement only work traced to a mandatory root clause.

You are working in the existing Ownstate repository.

Ownstate is NOT a greenfield project anymore.

Do not start over.

Do not rewrite working systems merely to conform to this prompt.

Your first responsibility is to deeply inspect what already exists and understand the current architecture before making changes.

## Read first

Inspect and read:

- ARCHITECTURE.md
- REQUIREMENTS.md
- README.md
- CONTINUATION_PROMPT.md
- STATE.md
- docs/PENDING_IMPLEMENTATION.md
- docs/SECURITY_AND_PRODUCTION_ACCEPTANCE.md
- docs/ARCHITECTURE.md and docs/REQUIREMENTS.md as current-state references only
- Cargo workspace
- migrations/schema
- domain models
- storage layer
- knowledge pipeline
- retrieval implementation
- embeddings
- model runtime
- MCP implementation
- agent-related code
- authorization/security code
- Docker / Compose configuration
- .env.example
- CI configuration
- tests

Run the existing test suite and relevant checks before changing anything so you know the baseline.

Then compare the current implementation against root `ARCHITECTURE.md`, root
`REQUIREMENTS.md`, and root `README.md`. Do not treat same-named files under
`docs/` as target authority.

Use your own engineering judgment. Preserve anything that already implements these requirements cleanly.

Do not perform a wholesale rewrite.

## Product objective

Ownstate is a sovereign institutional cognition layer.

It should continuously build a temporal, provenance-backed representation of:

- what a person or organization knows
- what happened
- what is currently true
- what used to be true
- what changed
- what decisions were made
- why they were made
- what systems/products/projects exist
- what entities and relationships exist
- what customers/investors/partners exist
- what commitments and requirements exist
- what artifacts were created or exchanged
- what succeeded
- what failed
- what remains unresolved

It must support AI chats, coding agents, cowork-style agents, files, and generated artifacts as first-class sources.

External connectors are optional enrichment sources.

The same state must later support personal models, agents, and permission-scoped agent swarms.

## Before changing code

Produce a concise architecture gap analysis:

1. What is already implemented correctly?
2. What is partially implemented?
3. What is missing?
4. What is coupled too tightly to a model/provider/agent?
5. What schema changes are required?
6. What security risks exist?
7. What scaling risks exist?
8. What documentation/open-source workflow gaps exist?
9. What should be implemented first?

Then begin implementation immediately in small, tested batches.

## Non-negotiable architecture

Preserve strict separation:

```text
Raw Evidence
→ Candidate Knowledge
→ Canonical Knowledge
→ Retrieval / Context Compilation
→ AI / Agent
```

Also maintain a separate Experience Store for future personal-model adaptation.

Models never directly mutate canonical knowledge.

Connectors never directly mutate canonical knowledge.

Agents never directly declare canonical truth.

PostgreSQL remains canonical unless the current implementation demonstrates an objectively better design while preserving all invariants.

Vectors are indexes, not truth.

## Institutional knowledge model

Evolve or preserve the domain around:

- Evidence
- Entities
- Relationships
- Claims
- Semantic Knowledge
- Artifacts
- Experience

Do not force all institutional information into embeddings.

Structured questions must be answerable deterministically.

Examples:

```text
How many legal entities do we have in each jurisdiction?
```

should use structured state.

```text
Why is the recovery architecture designed this way?
```

should use semantic knowledge, decisions, temporal state, and provenance.

```text
Which European customers requested settlement automation and what did they ask for?
```

should combine structured filtering, relationships, semantic retrieval, and temporal information.

## Temporal knowledge

Implement or improve:

- valid_from
- valid_until
- observed_at
- recorded_at
- status
- trust/source authority
- confidence
- provenance

Knowledge transitions should support:

- NEW
- DUPLICATE
- CONFIRMS
- UPDATES
- SUPERSEDES
- CONTRADICTS
- STALE
- UNRESOLVED

Do not overwrite historical state.

## Source authority

Do not use a single global confidence score as truth.

Allow source authority to depend on knowledge type.

Examples:

```text
legal entity existence:
verified legal record > authorized explicit statement > AI inference
```

```text
current code architecture:
current repository > recent verified architecture > historic discussion
```

```text
decision rationale:
explicit decision record > contemporaneous discussion > later recollection > inference
```

Centralize these rules rather than scattering precedence logic.

## Entity resolution

Implement or improve safe entity resolution using:

1. exact identifiers
2. aliases
3. structured attributes
4. relationship context
5. semantic similarity
6. model-assisted comparison only when needed

Never destructively merge uncertain entities.

Merges must be auditable and reversible.

## Artifacts

Treat artifacts as first-class versioned objects.

Track:

- identity
- version
- hash
- creator
- project
- associated entities
- timestamps
- provenance
- relationships such as SENT_TO / REVIEWED_BY / SUPERSEDES / CREATED_FOR

Use content-addressed object storage where appropriate.

## Retrieval

Implement or improve hybrid temporal GraphRAG-style retrieval.

Do not depend solely on vectors.

Combine:

- structured SQL
- exact identifiers
- PostgreSQL FTS
- dense vectors
- sparse lexical retrieval where useful
- graph/relationship traversal
- temporal filtering
- trust/source authority
- security classification
- recency
- reranking

Use a simple explainable fusion strategy such as RRF initially.

Keep embeddings model/versioned and replaceable.

Do not bind canonical knowledge to one embedding model.

## Query planner

Introduce or improve a provider-neutral query-planning abstraction.

Support conceptual plan types:

- Structured
- Semantic
- Relationship
- Temporal
- Hybrid

A model may assist planning but must never receive unrestricted SQL.

Use a validated typed query representation.

Simple structured queries should not require a model.

## Context Compiler

The Context Compiler must consume the query/retrieval plane instead of talking directly to raw vectors.

It must:

1. authenticate principal
2. authorize scope
3. resolve relevant entities/projects/time range
4. plan query
5. retrieve
6. filter stale/inappropriate state
7. rank
8. deduplicate
9. respect token/item budget
10. produce provenance-backed ContextPackets

Every ContextPacket records what was exposed, to whom, for which task, under what authorization.

## Authorization

Evaluate Cedar only if the repository does not already have a comparably strong system.

Ownstate needs fine-grained decisions for users, organizations, agents, subagents, projects, knowledge, artifacts, tools, model egress, and classifications.

Use deterministic authorization. Default deny. Use PostgreSQL RLS as defense in depth where appropriate.

## Model runtime

Keep a provider-neutral ModelRuntime abstraction.

Support OpenRouter as an adapter without coupling the domain to it.

Model selection must be configuration.

Keep the same interface compatible with future/self-hosted OpenAI-compatible endpoints such as vLLM.

Do not hard-code Kimi, MiniMax, Claude, GPT, DeepSeek, or any specific model.

## Personal model and Experience Store

Keep changing factual knowledge in Ownstate retrieval, not in model weights.

Maintain a separate Experience Store for future procedural learning.

Training candidates may contain task, ContextPacket reference, model/provider, response, tool summary, objective outcome, feedback, quality signals, classification, and provenance.

Do not build continuous online training now. Build the user-owned dataset and lineage foundation first.

## MCP

Maintain one canonical Ownstate MCP server.

MCP handlers must call the same application services as HTTP.

No model-specific MCP implementations.

Never expose raw SQL, policy mutation, permission mutation, or canonical force-write.

## UHP / HarnessRuntime

Keep UHP behind a replaceable internal HarnessRuntime.

Use it for managed/business agent execution where valuable.

Do not make personal mode depend on UHP.

## Agent swarms

Design the agent layer so future swarms work over one Ownstate institutional state.

Every agent/subagent must have:

- identity
- authorization scope
- task
- ContextPacket
- tool capabilities
- model/harness
- execution record

Parent permissions must not automatically transfer to children.

Agent output returns through the evidence/knowledge pipeline.

Do not add A2A unless independently deployed agents genuinely require it.

## Optional connectors

External connectors remain optional.

Do not implement all connectors.

Establish or preserve a generic connector boundary.

A connector should support initial sync, incremental sync, stable external IDs, provenance, pagination/cursors, tombstones where available, retries/backoff, ACL metadata where available, and idempotency.

No connector directly writes canonical knowledge.

## Event architecture

Preserve or introduce a transactional outbox.

Define an EventBus boundary.

Start with PostgreSQL-backed processing if sufficient.

Use NATS JetStream only where throughput and decoupling justify it.

Workers should be idempotent and horizontally scalable.

## Open-source and operability requirements

Ownstate is intended to be open source. Treat the repository experience as part of the product.

### README

For each implementation batch, determine whether the change affects architecture, setup, dependencies, environment variables, Docker, APIs, MCP, model runtime, infrastructure, or supported capabilities.

If yes, update README.md and/or relevant docs in the same batch.

README must describe what actually works and clearly mark implemented, experimental, and planned features.

### Environment configuration

All runtime configuration must be externalized through typed configuration populated from environment variables.

Maintain `.env.example`.

Do not scatter environment lookups throughout the codebase.

When adding configuration:

1. add typed config
2. validate/default it
3. update `.env.example`
4. update README where relevant
5. update Docker/Compose where relevant
6. add tests for non-trivial behavior

Never log secrets.

### Docker

Target local experience:

```bash
cp .env.example .env
docker compose up --build
```

Keep the default stack small.

Optional systems should use profiles or equivalent.

```bash
docker compose --profile nats up
docker compose --profile local-ai up
```

Do not make NATS, local models, paid providers, observability stacks, or optional connectors mandatory for basic startup.

### Progressive infrastructure

Keep a simple deployment viable:

- Ownstate API
- Ownstate worker
- PostgreSQL + pgvector
- object storage when required

Allow later evolution to many API/MCP nodes, NATS, specialized workers, read replicas, dedicated tenant databases, specialized retrieval projections, and self-hosted inference without changing domain semantics.

### Docker images

Use multi-stage builds, small runtime images, non-root user where practical, health checks, graceful shutdown, no baked secrets, and no unnecessary compilers/build caches in runtime images.

### Database

A fresh DB must be fully constructible from version-controlled migrations.

No undocumented manual SQL.

### CI alignment

Keep documented local commands aligned with CI:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

### Contributor friendliness

Do not assume contributors have private company infrastructure, paid model APIs, private repositories, or production credentials.

Provide local/no-op/test implementations where appropriate.

Use fictional public examples.

## Implementation priority

After auditing the repository, prioritize dependencies in this order where missing:

1. temporal entities / aliases / relationships / claims + provenance
2. artifact identity/versioning
3. source authority + freshness/supersession
4. transactional outbox/EventBus boundary
5. safe entity extraction/resolution
6. hybrid SQL + FTS + pgvector + relationship/temporal retrieval
7. QueryPlan + Context Compiler integration
8. fine-grained policy improvements
9. ModelRuntime hosted/self-hosted boundary
10. MCP alignment
11. HarnessRuntime/UHP boundary
12. agent identity/execution foundation
13. Experience Store and evals
14. optional connector examples only when useful

Do not implement an item if the existing repository already has a better implementation.

## Testing

Before changing code, reproduce the known interrupted baseline described in
`CONTINUATION_PROMPT.md`; do not assume a passing baseline or weaken the private
principal boundary to obtain one.

For every logical batch:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

plus repository-specific checks.

Add strong tests for temporal supersession, history preservation, provenance, entity resolution, uncertain merge behavior, source precedence, structured queries, semantic retrieval, hybrid queries, temporal queries, tenant/project isolation, authorization, model-egress policy, ContextPacket construction, event idempotency, worker retry behavior, stale knowledge invalidation, and agent/subagent authorization.

Never weaken tests to make architectural changes pass.

## Security

Treat every AI response, file, email, Slack message, web result, document, comment, issue description, and connector record as untrusted content.

Security relies on deterministic capability boundaries.

Models may propose. Rust code authorizes and commits.

Do not give models raw SQL, DB credentials, policy mutation, permission mutation, canonical force-write, or unrestricted tools.

Do not log confidential content or secrets.

## Constraints

Do not:

- rewrite the repository from scratch
- add a graph database merely because the domain is graph-shaped
- make Microsoft GraphRAG or Graphiti a core dependency
- add LangChain/LangGraph/CrewAI/AutoGen unless benchmarked and clearly justified
- add Redis as canonical state
- create model-specific knowledge paths
- let agents maintain separate canonical company memory
- implement every connector
- build a GPU fine-tuning platform yet
- commit or push unless explicitly instructed

## Definition of success

The architecture should support these classes of tasks without separate knowledge systems:

### Structured

```text
How many legal entities do we have and in which jurisdictions?
```

→ deterministic structured result.

### Semantic

```text
What is our recovery architecture and why is it designed this way?
```

→ evidence-backed semantic/temporal answer.

### Hybrid

```text
Which customers have asked for feature X and what did we promise them?
```

→ entity/relationship + semantic + temporal answer.

### Temporal

```text
What changed about Project X in the last month?
```

→ temporal delta.

### Agent

```text
Prepare an investor update for Investor Y.
```

→ permission-scoped institutional context.

### Swarm

```text
Prepare us for tomorrow's board meeting.
```

→ multiple permission-scoped agents working from one canonical institutional state, with outputs captured back into Ownstate.

This is a future/conditional architecture example. It does not authorize a swarm
or A2A implementation unless the current root-stated prerequisite is met. The
required present work is the individual permission-scoped agent and child-grant
foundation in `docs/PENDING_IMPLEMENTATION.md`.

Begin now by inspecting the existing repository and documentation thoroughly.

Report a concise architecture gap analysis and implementation plan.

Then proceed in small, safe, tested batches.
