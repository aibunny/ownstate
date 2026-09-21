# Ownstate Requirements Specification

## 1. Product Definition

Ownstate is a vendor-neutral, model-agnostic, agent-agnostic sovereign knowledge layer for individuals and organizations.

It must retain and organize useful information created across AI conversations, coding agents, cowork agents, files, decks, contracts, reports, generated artifacts, explicit user knowledge, and optional external application connectors.

The canonical institutional state must remain independent of any AI vendor, model, agent, connector, or inference runtime.

## 2. Primary Objective

Ownstate must continuously transform authorized activity into durable, current, searchable, versioned, provenance-backed institutional knowledge.

The system must preserve enough evidence that knowledge can later be verified, corrected, superseded, traced to sources, re-extracted, exported, deleted according to policy, and selectively exposed to models and agents.

## 3. Foundational Invariants

1. Raw evidence is separate from derived knowledge.
2. Candidate knowledge is separate from canonical knowledge.
3. Models cannot directly mutate canonical knowledge.
4. Connectors cannot directly mutate canonical knowledge.
5. Agents cannot directly declare canonical truth.
6. Canonical knowledge is versioned, not overwritten.
7. Every important canonical item has provenance.
8. Conflicts remain visible until resolved.
9. Authorization happens before retrieval or model egress.
10. Models, providers, agents, and protocols remain replaceable.
11. Database schema is independent of AI vendors.
12. Current authoritative sources outrank stale inference.
13. Raw evidence is reprocessable with future extractors.
14. Context delivery is auditable.
15. Optional connectors remain optional.
16. Open-source/self-hosted operation must not depend on hidden hosted services.

## 4. Technology Foundation

Preferred foundation:

- Rust 2024 edition
- Tokio
- Axum
- SQLx
- PostgreSQL 18
- pgvector
- S3-compatible object storage
- FastEmbed or equivalent local embedding runtime
- MCP through the official Rust ecosystem
- tracing + OpenTelemetry
- BLAKE3 for content addressing
- typed environment configuration

Start as a modular monolith. Do not begin with microservices.

Avoid mandatory Kafka, Redis, Elasticsearch, Neo4j, Qdrant, Kubernetes, or specialized graph/vector infrastructure unless measurements later justify them.

## 5. Canonical Data Model

### 5.1 Ownership hierarchy

Support User, Organization, Workspace, Project, Repository, and Session. Ownership domains should include PERSONAL and ORGANIZATION.

### 5.2 Evidence

Normalized source records and interaction events should support event types such as USER_MESSAGE, ASSISTANT_MESSAGE, TOOL_CALL, TOOL_RESULT, COMMAND, COMMAND_RESULT, FILE_READ, FILE_WRITE, SEARCH, SEARCH_RESULT, DOCUMENT_READ, ARTIFACT_CREATED, GIT_DIFF, TEST_RESULT, AGENT_RESULT, and SYSTEM_EVENT.

Evidence should be append-oriented.

### 5.3 Entities

Initial conceptual entity types include Person, Organization, LegalEntity, Jurisdiction, Customer, Investor, Partner, Vendor, Project, Product, Repository, Contract, Artifact, Deck, Fundraise, Requirement, Risk, Issue, and Decision.

Do not implement every type immediately if the current repo does not require it.

### 5.4 Relationships

Relationships must support source entity, relation type, target entity, valid_from, valid_until, status, confidence, and provenance.

### 5.5 Claims

Claims represent atomic assertions and should support subject, predicate, object/value, valid_from, valid_until, observed_at, recorded_at, confidence, trust/source authority, status, and provenance.

### 5.6 Semantic knowledge

Initial kinds include FACT, ARCHITECTURE, DECISION, RATIONALE, CONSTRAINT, REQUIREMENT, PROCEDURE, PREFERENCE, FAILURE, OUTCOME, GOAL, RISK, RELATIONSHIP, DEFINITION, and OPEN_QUESTION.

### 5.7 Knowledge versioning

Separate KnowledgeItem from KnowledgeVersion. Statuses should support ACTIVE, STALE, SUPERSEDED, CONFLICT, QUARANTINED, and REVOKED. Never overwrite historical knowledge content.

## 6. Provenance

Every canonical knowledge version, claim, and important relationship must be traceable to evidence.

Evidence may reference events, objects, artifact versions, repositories, commits, files, line ranges, explicit user statements, source types, and content hashes.

Knowledge must be able to answer: **Why do we believe this?**

## 7. Source Authority

Do not use one universal confidence score as the only truth mechanism.

Authority may depend on knowledge type.

Examples:

- legal entity existence: verified legal record > authorized explicit statement > AI inference
- current code architecture: current repository > recent verified architecture > historic discussion
- decision rationale: explicit decision record > contemporaneous discussion > later recollection > inference

Implement source-authority policy centrally.

## 8. Entity Resolution

Resolution should consider exact identifiers, aliases, structured attributes, relationship context, semantic similarity, and model-assisted comparison when needed.

Uncertain entities must not be destructively merged. Entity merges should be auditable and reversible.

## 9. Artifacts

Artifacts must be first-class, versioned objects.

Track artifact identity, version, content hash, media type, creator, project, associated entities, timestamps, provenance, and relationships such as SENT_TO, REVIEWED_BY, SUPERSEDES, and CREATED_FOR.

Use content-addressed object storage to avoid duplicated binaries.

## 10. Knowledge Freshness

When new evidence arrives:

1. identify affected entities/claims/knowledge
2. compare with canonical state
3. classify as NEW / DUPLICATE / CONFIRMS / UPDATES / SUPERSEDES / CONTRADICTS / STALE / UNRESOLVED
4. preserve historical state
5. create new current state where appropriate
6. invalidate affected retrieval projections and cached context

For code-derived knowledge, relevant source changes should trigger staleness checks. For document-derived knowledge, newer authoritative versions should be able to supersede prior current-state claims.

## 11. Learning Pipeline

```text
Source Record
    ↓
Classification
    ↓
Chunk / Normalize
    ↓
Entity Extraction
    ↓
Entity Resolution
    ↓
Knowledge Extraction
    ↓
Candidate Claims / Knowledge
    ↓
Novelty Search
    ↓
Verification
    ↓
Policy
    ↓
Promotion
```

Each stage should consume and emit typed records.

Extraction models are untrusted and must not have database credentials, canonical-write capability, policy mutation, permission mutation, unrestricted shell, or unrestricted secrets/network access.

## 12. Retrieval Architecture

Ownstate must support hybrid temporal GraphRAG-style retrieval using exact identifiers, PostgreSQL FTS, dense vector similarity, sparse lexical retrieval where useful, relationship traversal, temporal filtering, project/workspace scope, knowledge kind, trust/source authority, security classification, and recency.

Use an explainable fusion strategy such as Reciprocal Rank Fusion initially. Optionally rerank a small candidate set locally.

Vectors are indexes, not canonical state.

## 13. Query Planner

Support Structured, Semantic, Relationship, Temporal, and Hybrid plans.

A model may assist planning but must not receive unrestricted SQL. Use a validated typed query representation. Simple structured queries must not require an LLM.

## 14. Context Compiler

Flow:

```text
Request
 ↓
Authentication
 ↓
Authorization
 ↓
Scope resolution
 ↓
Query planning
 ↓
Retrieval
 ↓
Freshness / trust filtering
 ↓
Deduplication
 ↓
Reranking
 ↓
Token/item budget
 ↓
ContextPacket
```

Context packets may include project summary, architecture, decisions, rationale, constraints, requirements, previous failures, outcomes, recent changes, open questions, entities, claims, and sources.

## 15. ContextPacket Audit

Every context delivery must record tenant, project, session, principal, agent, provider, model, query/task, token budget, knowledge versions, claims/entities/evidence included, authorization context, estimated tokens, and created_at.

## 16. MCP

Maintain one canonical Ownstate MCP server.

Tool direction:

- bootstrap_project
- search_knowledge
- get_knowledge
- get_entity
- get_relationships
- compile_context
- propose_knowledge
- feedback

No model-specific MCP servers. MCP handlers must call the same application services used by HTTP.

Do not expose raw SQL, policy mutation, permission mutation, or canonical force-write.

## 17. Model Runtime

Define a provider-neutral ModelRuntime.

Possible implementations include OpenRouterRuntime, OpenAICompatibleRuntime, and SelfHostedRuntime.

Model identifiers and providers must be configuration. OpenRouter is an adapter, not a domain dependency. Self-hosted OpenAI-compatible endpoints must remain possible.

## 18. Personal Model and Experience Store

Keep factual/current knowledge external to model weights.

Maintain a separate Experience Store for future procedural learning.

A training experience may include task, ContextPacket reference, model/provider, response/output, tool summary, objective outcome, user feedback, quality signals, classification, provenance, and trainability status.

Trainability statuses: TRAINABLE, NOT_TRAINABLE, NEEDS_REVIEW.

Do not continuously retrain after every interaction. Future training should use versioned datasets and tracked model/adapter lineage.

## 19. UHP / Harness Runtime

Introduce a HarnessRuntime abstraction. UHP may be an adapter for managed/business agent execution. Do not make Personal mode depend on UHP. The domain must not depend directly on UHP.

## 20. Agent Architecture and Swarms

Every agent/subagent must have identity, authorization scope, task, ContextPacket, tools/capabilities, model/harness, and execution record.

Parent permissions do not automatically transfer to children.

Agent output returns through the normal evidence and knowledge pipeline. Agents do not own separate canonical institutional memories.

Future swarm orchestration should support multiple permission-scoped agents working from one canonical institutional state.

Do not require A2A unless independently deployed agents actually need a vendor-neutral peer protocol.

## 21. Optional Connectors

Connector categories may include Slack/Teams/Email, GitHub/GitLab/Jira/Linear, Drive/SharePoint/OneDrive/Dropbox/Box/Notion/Confluence, Salesforce/HubSpot/CRM, Calendar/Microsoft 365, and internal systems.

Connectors are optional. Ownstate must remain useful with zero external connectors.

Connector requirements include initial sync, incremental sync, stable external IDs, source timestamps, pagination/cursors, tombstones where available, rate-limit handling, retries/backoff, source ACL preservation where available, and idempotency.

No connector writes canonical truth directly.

## 22. Authorization and Data Classification

Initial classifications:

- PUBLIC
- INTERNAL
- CONFIDENTIAL
- RESTRICTED
- SECRET

Authorization should evaluate principal, action, resource, and context. Default deny. Cedar or an equivalent mature policy engine may be used. PostgreSQL RLS should provide defense in depth where appropriate.

Model egress must be policy-controlled.

## 23. Security

Assume prompt injection can succeed at the model level. Security must rely on capability boundaries.

All external content is untrusted, including AI output, email, Slack, documents, source comments, issue text, web content, and tool responses.

Models may propose. Rust code controls authorization, promotion, policy, canonical writes, tool access, and model egress.

Never log sensitive raw content by default. Never commit secrets.

## 24. Object Storage

Use S3-compatible object storage for large immutable content. Use content addressing with BLAKE3 or another suitable digest. Store metadata and provenance in PostgreSQL. Do not repeatedly store the same binary when hashes match.

## 25. Event Architecture

Start with a PostgreSQL transactional outbox. Define an EventBus abstraction. Use NATS JetStream only when throughput/service decoupling makes it valuable.

Workers must be idempotent, stateless where practical, horizontally scalable, and retry-safe.

## 26. Scalability

Simple deployment:

- API
- worker
- PostgreSQL + pgvector
- object storage when needed

Scaled deployment:

- API × N
- MCP × N
- specialized workers × N
- agent runtime × N
- NATS JetStream
- read replicas
- tenant partitions
- dedicated large-tenant databases
- specialized retrieval projections
- self-hosted models
- large object archives

Do not prematurely convert the system into microservices.

## 27. PostgreSQL Partitioning and Retrieval Scale

High-growth tables may eventually include source_records, interaction_events, knowledge_evidence, context_packets, audit_events, embeddings, and agent_events.

Design keys so tenant/time partitioning is possible later. Do not partition everything immediately.

Large/regulatory tenants should be able to move to dedicated partitions or databases without changing domain semantics.

## 28. Observability

Use structured tracing. Track request IDs, operation type, latency, retrieval counts, extraction counts, model usage, token usage, worker/job status, and errors.

Use tracing with OpenTelemetry export where useful. Do not emit confidential content into telemetry by default.

## 29. API Direction

Health:

- GET /health
- GET /ready

Projects:

- POST /projects
- GET /projects/:id

Sessions/Evidence:

- POST /sessions
- POST /sessions/:id/events
- GET /sessions/:id

Knowledge:

- GET /knowledge/search
- GET /knowledge/:id
- POST /knowledge/proposals

Context:

- POST /context/compile

Entities:

- GET /entities/:id
- GET /entities/:id/relationships

Keep admin/debug endpoints secure.

## 30. Open-Source and Operability Requirements

The repository is a product surface.

Maintain README.md, docs/ARCHITECTURE.md, docs/REQUIREMENTS.md, `.env.example`, Dockerfile, Compose configuration, and CI configuration.

### README

Must remain accurate and distinguish implemented, experimental, and planned functionality.

### Environment configuration

All runtime configuration should be externalized through environment variables and loaded into typed Rust configuration at startup. Do not scatter direct environment lookups throughout the code.

### Local Docker

Target:

```bash
cp .env.example .env
docker compose up --build
```

Default startup must not require paid model APIs, NATS, local GPU inference, optional connectors, or Kubernetes.

Optional components should use Compose profiles or equivalent.

### Secrets

`.env.example` contains placeholders only. Never commit `.env` or production secrets.

### Database migrations

A fresh database must be constructible entirely from version-controlled migrations.

### CI

At minimum:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets
cargo test --workspace
```

### Public examples

Do not use private organizational information in public examples, fixtures, docs, or seed data.

## 31. Testing Requirements

At minimum test:

- project/tenant isolation
- evidence immutability
- event ordering
- candidate persistence
- candidate promotion
- temporal supersession
- history preservation
- contradiction handling
- provenance
- entity resolution
- uncertain merge behavior
- source precedence
- structured queries
- semantic retrieval
- hybrid queries
- temporal queries
- revoked/stale knowledge handling
- context packet creation
- MCP authorization
- model-egress policy
- event idempotency
- worker retry behavior
- stale knowledge invalidation
- agent/subagent authorization

Use real PostgreSQL integration tests for PostgreSQL-specific behavior.

## 32. Evaluation

Track retrieval precision/recall, entity-resolution accuracy, contradiction detection, stale knowledge rate, answer provenance coverage, structured-query accuracy, temporal-query accuracy, cross-tenant leakage, ContextPacket size, agent task success, and rediscovery tokens avoided.

Do not optimize for number of memories stored.

## 33. Foundational Acceptance Tests

Structured question:

> How many legal entities do we have and in which jurisdictions?

returns a deterministic structured/relationship-backed result.

Semantic question:

> What is our recovery architecture and why is it designed this way?

returns a semantic + temporal + evidence-backed answer.

Hybrid question:

> Which customers requested feature X and what did we promise them?

returns an entity/relationship + semantic + temporal answer.

Temporal question:

> What changed about Project X in the last month?

returns a temporal delta over structured and semantic knowledge.

Agent task:

> Prepare an investor update for Investor Y.

compiles permission-scoped context from organization state, investor relationship, relevant projects, artifacts, and prior commitments.

Learning loop:

1. AI/agent work creates evidence.
2. Important knowledge is proposed.
3. Validation promotes current knowledge.
4. A fresh agent retrieves it.
5. New work adds evidence.
6. Ownstate updates/supersedes knowledge without losing history.

## 34. Automatic Capture, Scope Resolution, and MCP Context Protocol

Ownstate must provide persistent context without requiring normal users or models
to create projects manually, remember internal UUIDs, or understand storage
rows. It must resolve authorized scope from observable stable evidence and use
provisional/general scopes where confidence is insufficient.

The complete normative specification for this capability is
[`AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`](AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md).
Every requirement and acceptance test in that document is part of this
requirements specification.

Security requirements are defined in
[`MCP_CAPTURE_SECURITY.md`](MCP_CAPTURE_SECURITY.md). In particular, no
model-controlled MCP operation may delete, purge, erase, rewrite, or force-promote
already recorded Ownstate data; mutate credentials, grants, or policy; execute
raw SQL; or turn a scope handle into authority. Legitimate policy/retention
deletion must use a separate explicit administrative plane that MCP models cannot
reach.

The architectural invariant is:

> No model owns project identity, conversational state, or institutional state.
> Ownstate resolves identity from stable external evidence and stores it
> independently from the model runtime.

Connecting Ownstate MCP must provide persistent context capabilities, while the
system accurately distinguishes MCP-mediated observation from complete host-level
capture.
