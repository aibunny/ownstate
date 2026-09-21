# Ownstate

> Open infrastructure for sovereign AI knowledge and institutional cognition.

Ownstate is a sovereign knowledge and agent-context layer for individuals and organizations.

The core idea is simple: **models, agents, vendors, and applications should be replaceable. The knowledge accumulated while using them should belong to the individual or organization.**

Ownstate continuously builds and maintains a durable, permission-aware representation of what an organization knows, what happened, what is currently true, what used to be true, what changed, what decisions were made, why they were made, how systems are architected, who customers/investors/partners/vendors are, what commitments and requirements exist, what artifacts have been exchanged, what failed, what succeeded, what risks remain, and what work is happening now.

Ownstate is not a chatbot, model, vector database, or agent framework. It is the persistent institutional state that humans, models, agents, and agent swarms can use.

## Documentation

- [Target architecture](docs/ARCHITECTURE.md)
- [Target requirements](docs/REQUIREMENTS.md)
- [Automatic capture and scope requirements](docs/AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md)
- [Automatic capture implementation plan](docs/AUTOMATIC_CAPTURE_IMPLEMENTATION_PLAN.md)
- [MCP capture security contract](docs/MCP_CAPTURE_SECURITY.md)
- [Host capture capability matrix](docs/HOST_CAPTURE_CAPABILITY_MATRIX.md)
- [OpenCode implementation prompt](OPENCODE_CAPTURE_SCOPE_PROMPT.md)
- [Implementation loop](docs/IMPLEMENTATION_LOOP.md)
- [Operating instructions](docs/OPERATIONS.md)
- [Repository rules](docs/RULES.md)

The architecture, requirements, automatic-capture requirements, and this README
define target intent. Current capability claims below do not imply that every
target feature already exists. Files under `docs/archive/` are historical and
must not be treated as active requirements or current evidence.

> **Current development checkpoint (2026-09-21):** the prior model reported that
> the private principal-service test migration and migration 0011 no-op trigger
> repair pass 116 Rust tests and 10 implementation-loop tests. The next model must
> establish a fresh baseline. Non-owner runtime/RLS evidence and holding model-
> egress authorization through the actual provider call remain unresolved
> prerequisites. Automatic capture and durable scope resolution are not yet
> implemented beyond the existing project/workspace resolution foundations.

## Current capabilities

**Implemented:** a Rust workspace with shared HTTP/MCP application services,
append-only interaction evidence, candidate promotion, immutable canonical
versions and provenance, supersession, PostgreSQL full-text plus pgvector/RRF
retrieval, classification ceilings, tenant/project filtering, audited context
packets with budgets, embedding jobs with retries, and workspace project resolution.
Temporal institutional entities, aliases/external identifiers, relationships and
claims use evidence-gated candidates, immutable history and scoped as-of reads.
A protected `POST /query` accepts validated Structured, Semantic, Relationship,
Temporal and Hybrid plans. Exact counts and jurisdiction grouping run in
PostgreSQL; hybrid retrieval uses canonical entity/knowledge associations.
Temporal queries return snapshots and a bounded change log with provenance.

**Experimental/local:** static API/admin credentials and a fixed deployment
tenant support personal development; local fastembed provides semantic embeddings,
while the deterministic test provider exercises mechanics without semantic quality.
The manual implementation loop records checks and requires separate review.
The active requirements, security contract, state, and OpenSpec receipts track
the full target rather than treating a passing batch as product completion.
Institutional source authority currently protects explicit
human state from lower-trust contradictions; the full type-dependent policy and
authorized lifecycle review are planned.

**Planned:** automatic cross-host capture with factual completeness, canonical
repository identity, provisional non-code scopes, opaque MCP scope handles,
compact bootstrap/record/search behavior, artifact versioning/object storage,
reversible explicit entity merges,
source authority by knowledge type, workspace-wide query scope, fine-grained principal and model-egress policies,
provider-neutral model/harness runtimes, permission-scoped agent execution and
swarms, Experience Store lineage, optional connectors and transactional outbox/EventBus. Preserve the target architecture while
adding these in small verified batches.

## Product goals

Ownstate should eventually answer questions such as:

- What is our wallet recovery architecture?
- Why was it designed this way?
- How many legal entities do we operate and in which jurisdictions?
- Which entity contracts with Customer X?
- What did we promise Investor Y?
- Which investors received the latest deck?
- Which customers requested feature X?
- What changed across the company this week?
- What are our biggest unresolved risks?
- What work is blocked and why?

Answers should be current, permission-aware, and grounded in evidence.

## Core architecture

Ownstate distinguishes:

1. **Evidence** — what actually happened.
2. **Entities** — durable identities such as companies, people, projects, products, customers, investors, and artifacts.
3. **Relationships** — how entities connect.
4. **Claims** — atomic assertions that may change over time.
5. **Semantic knowledge** — architecture, rationale, decisions, procedures, failures, requirements, and lessons.
6. **Experience** — high-quality work traces that may later train or adapt a personal model.

```text
Raw Evidence
    ↓
Candidate Knowledge
    ↓
Novelty / Validation / Policy
    ↓
Canonical Knowledge
    ↓
Query / Retrieval
    ↓
Context Compiler
    ↓
Human / Model / Agent
    ↓
New Work
    ↓
Raw Evidence
```

## Product modes

### Personal

Individuals can use existing AI tools while Ownstate preserves knowledge across models and agents.

### Business

Business mode extends the same knowledge core with organization-wide institutional state, fine-grained authorization, model-egress policy, UHP-managed agent execution, future agent swarms, audit, data classification, BYOK, and self-hosted inference.

## Open-source philosophy

Ownstate is designed to run locally, in your own cloud, in a private institutional environment, or as a hosted service. Hosted convenience must not become a hidden dependency.

## Quick start

The target local experience is:

```bash
git clone <your-ownstate-repo>
cd ownstate
cp .env.example .env
docker compose up --build
```

The default local stack should remain minimal and should not require Kubernetes, Terraform, paid model APIs, external managed databases, optional connectors, or external message brokers.

Compose defines PostgreSQL 18 + pgvector, API and worker images, plus an optional
stdio MCP profile. The API owns startup migrations and the worker waits for API
readiness. Images compile Fastembed by default; the local model downloads on first
startup. See [operating instructions](docs/OPERATIONS.md) for cache persistence,
credentials, container options, and a development mode without model downloads.
The previously accepted images were verified in a fresh isolated stack with nine migrations,
non-root API and worker processes, readiness checks, an actual Fastembed-backed
embedding job, persistent model cache, and an MCP stdio handshake. Hosted CI
execution and semantic-quality evaluation remain separate evidence gaps. That
historical smoke does not cover the unverified migrations 0010/0011 or the
current interrupted tree.

## Configuration

All runtime configuration should come from typed configuration backed by environment variables.

```bash
cp .env.example .env
```

Do not commit `.env` files or secrets.

## Development

Typical native development:

```bash
docker compose up -d postgres
cargo run -p ownstate-api
```

In separate terminals, run the worker and an MCP-capable client if needed:

```bash
cargo run -p ownstate-worker
cargo run -p ownstate-mcp
```

The binaries load `.env` locally. Fastembed downloads its local model on first
startup; choose `OWNSTATE_EMBEDDING_PROVIDER=deterministic` for development without
a model download (semantic retrieval quality is not provided by that test adapter).
The MCP server uses stdio and shares the same application services as HTTP.

## Quality checks

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Tests require a real PostgreSQL + pgvector server and a role with database creation
privileges. `OWNSTATE_TEST_DATABASE_URL` overrides the harness's local default;
tests create migrated disposable databases and clean up their own databases older
than one hour. No paid model API is needed. CI is configured with the same gates
and a separate container smoke; hosted execution is not implied by local checks.

For recorded verification:

```bash
python3 scripts/implementation_cycle.py --phase verify --change-id your-change-slug
```

Its fixed Clippy gate adds `-- -D warnings`;
check success leaves independent and documentation review pending. See the
[loop contract](docs/IMPLEMENTATION_LOOP.md) for baseline, receipts, and block handling.

## Documentation rule

`README.md` must always reflect the current implementation. When a change affects setup, architecture, environment variables, Docker, APIs, MCP, model runtimes, infrastructure, capture guarantees, or supported capabilities, update the README and relevant docs in the same change.

Clearly distinguish Implemented, Experimental, and Planned functionality.

## Security

Ownstate assumes all AI output and external content is untrusted. Models may propose; deterministic Rust code controls authorization, policy, canonical writes, tool access, model egress, and data classification.

## License

Choose an explicit open-source license before public release and include a `LICENSE` file.

## Contributing

The repository should remain understandable and runnable without access to private company systems, private repositories, paid AI APIs, production infrastructure, or private credentials. Use fictional examples and fixtures in public documentation and tests.

## Typed queries

Exact reports require no model call. For example, send this authenticated JSON to
`POST /query`, replacing `project_id` with your project UUID:

```json
{
  "type": "STRUCTURED",
  "constraints": {
    "project_id": "00000000-0000-0000-0000-000000000001",
    "max_classification": "INTERNAL",
    "entity_kind": "LEGAL_ENTITY",
    "limit": 10
  },
  "operation": "COUNT"
}
```

`GROUP_BY_JURISDICTION` uses authorized `REGISTERED_IN`, `INCORPORATED_IN`, or
`JURISDICTION` edges (case-insensitive grouping labels); unmatched entities have
an explicit unknown-jurisdiction group. Counts and groups aggregate all matching
entities before response limits and include source event IDs.

Semantic results use bounded lexical/dense candidates and trust-weighted RRF, so
the ranking is approximate. Responses expose `semantic_candidate_limit`; each leg
admits at most four times the requested item limit. Hybrid entity samples obey the
response limit and include an exact count and `has_more_entities`, while SQL still
considers all authorized matching entities for semantic associations.

Temporal plans use `start`, `end` and a typed structured or semantic `subject`. They
return endpoint snapshots plus recorded/validity transitions in `(start, end]`,
including intermediate superseded versions. Change logs expose `total`, `has_more`
and the semantic candidate limit per leg; semantic totals cover that bounded fused
population, while structured totals cover all matching transitions. Scoped source
references accompany returned changes. These APIs remain subject to deployment
tenant/classification boundaries; fine-grained business principal policy is planned. Typed semantic query admission
requires scoped raw-event evidence. Global canonical provenance enforcement for
legacy storage and promotion remains required work under the active requirements
and must remain recorded in `STATE.md` until verified.
