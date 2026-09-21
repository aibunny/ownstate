# Ownstate v0.1 — Architecture (as implemented)

This document describes what the foundational vertical slice actually does,
not what the product will eventually do. Root [ARCHITECTURE.md](../ARCHITECTURE.md),
[REQUIREMENTS.md](../REQUIREMENTS.md), and [README.md](../README.md) define target
intent and take precedence for the intended evolution.

## Implementation verification

The [manual implementation loop](IMPLEMENTATION_LOOP.md) runs fixed workspace
checks and stores a source snapshot receipt, while independent acceptance review
and documentation review remain separate completion gates. Compose now defines
API/worker/PostgreSQL and optional stdio MCP images. A fresh isolated stack has
verified migration, non-root API/worker startup, readiness, a real Fastembed job,
cache persistence, and the MCP stdio handshake; details are in OPERATIONS.md.

Evidence-linked proposals and promotions enforce the evidence classification floor.
Migration 0007 adds a safe evidence-read view and an insertion guard; retrieval
rechecks project scope, classification, ACTIVE status, and validity when loading
ranked versions. These checks preserve the existing evidence/candidate/canonical
flow and do not introduce new APIs or fine-grained principal authorization.

## Component boundaries

```text
                 ┌─────────────────────────────────────────────┐
                 │                interface adapters           │
                 │  apps/api (Axum)   apps/mcp (rmcp, stdio)   │
                 │           apps/worker (job loop)            │
                 └───────────────┬─────────────────────────────┘
                                 │ calls (no business logic above this line)
                 ┌───────────────▼─────────────────────────────┐
                 │        crates/services (AppServices)        │
                 │ ingestion · proposal · promotion · hybrid   │
                 │ search · context compiler · bootstrap · jobs│
                 └───────┬──────────────────┬──────────────────┘
                         │                  │
        ┌────────────────▼───┐   ┌──────────▼──────────────┐
        │  crates/storage    │   │  crates/embeddings      │
        │  SQLx repositories │   │  EmbeddingProvider      │
        │  migrations        │   │  fastembed | determin.  │
        └────────┬───────────┘   └─────────────────────────┘
                 │ depends on domain types only
        ┌────────▼───────────┐
        │  crates/domain     │  pure types, enums, ids, limits,
        │  (no infra deps)   │  deterministic promotion rules
        └────────────────────┘
```

- **crates/domain** has no Axum/SQLx/MCP/fastembed dependencies. It owns the
  typed vocabulary (`InteractionEventType`, `KnowledgeKind`, `KnowledgeStatus`,
  `TrustLevel`, `OwnershipDomain`, `SecurityClassification`, …), UUIDv7 id
  newtypes, size limits, BLAKE3 content hashing, token estimation, and
  `decide_promotion` — the pure rule function for semantic knowledge promotion.
  Institutional candidates use the typed institutional service/storage curation
  path; both paths preserve immutable canonical history and provenance.
- **crates/storage** owns every SQL statement (static, parameterized strings —
  sqlx 0.9's `SqlSafeStr` enforces this at the type level). Domain invariants
  PostgreSQL can enforce live in migrations as triggers/constraints.
- **crates/embeddings** abstracts embedding generation behind
  `EmbeddingProvider`. Two implementations: `fastembed` (all-MiniLM-L6-v2,
  384 dims, local ONNX, feature-gated) and `deterministic` (FNV-hash bags for
  tests/dev). Embedding rows carry (model, model_version, dimensions) so
  re-embedding never rewrites knowledge.
- **crates/services** is the single implementation of every use case. HTTP,
  MCP and the worker all call it; none of them duplicate retrieval, promotion
  or authorization logic.
- **crates/runtime** is shared process bootstrap (env config, tracing to
  stderr, pool + migrations + provider construction) for the three binaries.

## Data flow (the learning loop, v0.1)

```text
POST /sessions/:id/events        (or a future adapter)
  → validated, BLAKE3-hashed, appended in one tx under a session row lock
    (sequence assignment is race-free; UNIQUE(session_id, sequence))

POST /knowledge/proposals        (or MCP propose_knowledge)
  → CandidateKnowledge (PENDING). Evidence event ids are verified to exist
    inside the same tenant + project before the candidate is accepted.

POST /knowledge/proposals/:id/promote
  → one transaction:
      lock candidate (FOR UPDATE) → lock item identity (project, kind,
      subject_key) → domain::decide_promotion → supersede old ACTIVE version
      (status + valid_until only) → insert new ACTIVE version → copy evidence
      links (re-verified in-tx) → mark candidate PROMOTED → enqueue
      GENERATE_EMBEDDING job
  → trust is derived, never claimed: HUMAN → HUMAN_EXPLICIT;
    AGENT/EXTRACTOR with evidence → AGENT_DERIVED; without → EXTERNAL_UNTRUSTED.

worker (or AppServices::drain_jobs in tests)
  → claims jobs FOR UPDATE SKIP LOCKED, embeds version content, upserts into
    the embeddings index. Retries with linear backoff until max_attempts.

GET /knowledge/search
  → hybrid retrieval (see below), evidence attached to each hit.

POST /context/compile
  → deterministic pipeline: validate → authorize (tenant + project) →
    hybrid retrieval over ACTIVE knowledge under the classification ceiling →
    trust-weighted ranking → greedy fill under max_items and token_budget →
    ContextPacket audit row (exact version ids) → sectioned response with
    per-item provenance. No generative model anywhere on this path.
```

## Hybrid retrieval

Promotion uses PostgreSQL time after acquiring the item lock. One timestamp
closes the previous version's validity and starts the next version, so there is
no clock-dependent gap between versions. Current retrieval applies database
time to validity filtering; application and database clocks need not be identical.

Two legs, both fully scoped inside SQL before any row leaves PostgreSQL:

1. **Full-text**: generated `tsvector` column + GIN index,
   `websearch_to_tsquery`, ranked by `ts_rank`.
2. **Vector**: pgvector HNSW (default parameters), cosine distance, filtered
   by the current provider's model name.

Fusion is Reciprocal Rank Fusion (k = 60) multiplied by a deterministic trust
weight (HUMAN_EXPLICIT 1.2 … EXTERNAL_UNTRUSTED 0.7). The vector leg degrades
gracefully: if query embedding fails, lexical retrieval still answers.

## Database model

Fifteen tables (see `migrations/`):

| table | role | mutability |
|---|---|---|
| projects | ownership scope (tenant_id on every row for future RLS) | app-managed |
| sessions | bounded AI work periods | app-managed |
| interaction_events | raw evidence | **append-only (trigger)** |
| candidate_knowledge | untrusted proposal layer | status transitions only |
| knowledge_items | conceptual identity, UNIQUE(project, kind, subject_key) | insert-only in practice |
| knowledge_versions | immutable content; UNIQUE partial index = one ACTIVE per item | **content frozen (trigger); only status/valid_until may change** |
| knowledge_evidence | provenance links | **append-only (trigger)** |
| embeddings | derived index, UNIQUE(entity, model) | upsert allowed |
| context_packets | audit of context handed to AIs | **append-only (trigger)** |
| jobs | durable queue (FOR UPDATE SKIP LOCKED) | worker-managed |
| institutional_candidates | typed evidence-backed entity/relationship/claim proposals | payload frozen; one terminal decision/result |
| institutional_records | scoped durable entity/assertion identities | immutable |
| institutional_versions | valid/observed/recorded content and provenance | only status/valid_until change |
| institutional_identifiers | scoped stable external identity registry | immutable across lifecycle changes |
| knowledge_entity_links | canonical semantic version/entity version associations with endpoint evidence | immutable |

Enums are TEXT + CHECK constraints (extensible by migration), converted to
Rust enums at the storage boundary. Ids are UUIDv7 (time-ordered).

## Key invariants and where they are enforced

| invariant | enforcement |
|---|---|
| Raw evidence never rewritten | DB trigger (UPDATE/DELETE raise) + no code path |
| Version content immutable | DB trigger allowing only status/valid_until changes |
| One ACTIVE version per item | partial unique index |
| Supersede, never overwrite | promotion tx + tests |
| Every version can carry provenance | knowledge_evidence FK'd to events; evidence verified in-project at propose AND promote |
| Extractors can't write canonical state | only `promote_candidate` creates versions; MCP/HTTP expose proposal + explicit promotion only; MCP proposals are forced to `AGENT` |
| Authorization before retrieval | tenant + project + classification filters inside every retrieval SQL statement |
| Trust ≠ confidence | trust derived by `decide_promotion` from proposer + evidence; confidence stored separately |
| Sequence integrity | session row lock + UNIQUE(session_id, sequence) |

## Security posture (v0.1)

- Single-tenant "personal mode": a fixed tenant id from config stamps every
  row, so PostgreSQL RLS and real tenancy can be added without reshaping data.
- HTTP auth: optional static bearer token (BLAKE3-digest comparison); health
  endpoints open; everything else 401s without it. Unset = dev mode with a
  loud startup warning.
- **Curation capability boundary**: an optional second credential
  (`OWNSTATE_ADMIN_TOKEN`). When configured, candidate promotion/rejection
  and HUMAN-attributed proposals require it — agents holding the regular
  token can read and propose but cannot decide what becomes canonical or
  claim human trust for their own claims. Unset (pure personal mode), the
  single token holder is the human and holds both capabilities.
- **Server-side classification ceiling** (`OWNSTATE_MAX_CLASSIFICATION`,
  default CONFIDENTIAL): request ceilings are clamped to it before any
  retrieval — search, context compilation, bootstrap AND version history
  (`get_knowledge`), so a superseded higher-classification version cannot
  leak through history reads. Content of REVOKED/QUARANTINED versions is
  withheld on read paths (identity, status and provenance stay visible).
- The ceiling applies to raw evidence too: `GET /sessions/{id}/events`
  withholds the content of events classified above the server ceiling (rows,
  sequences and content hashes stay visible for audit continuity).
- Event sequences: explicit sequences may only jump a bounded distance ahead
  (`MAX_SEQUENCE_GAP`), so a crafted value cannot exhaust a session's
  sequence space and poison future appends.
- Jobs orphaned by a crashed/stopped worker (stuck RUNNING) are requeued
  after 5 minutes; a concurrent first-promotion race for the same subject
  surfaces as a retryable 409, backstopped by the DB's unique constraints.
- HTTP trace spans record method + path only — query strings (which carry
  user search text) never reach logs.
- Input limits everywhere (content 256 KiB, batches ≤ 500, subjects ≤ 200
  chars, tasks ≤ 8 KiB); body cap 4 MiB.
- Content is data: nothing retrieved is ever interpreted as instructions or
  policy; logs carry ids and counts, never content or tokens.
- MCP exposes no SQL, no force-writes, no permission or policy surface, and
  hard-codes `proposed_by = AGENT` for its proposal tool.

### Known v0.1 trust-boundary limitations (deliberate, documented)

- There is no per-principal identity: authorization is two static tokens.
  Real RBAC arrives with business mode; the schema (tenant_id everywhere)
  and the capability split above are the seams it attaches to.
- Evidence references are validated for existence and project scope, not
  relevance: an agent can attach an unrelated in-project event to a claim and
  receive AGENT_DERIVED rather than EXTERNAL_UNTRUSTED trust. Relevance
  checking belongs to the novelty/verification milestone (M7).

## Temporal institutional state

An additive migration introduces scoped institutional candidates, durable records,
and immutable versions for entities, relationships, and atomic claims. Proposals
carry typed content, bounded evidence, confidence and observed time; services fix
tenant/project scope, validate endpoint access, derive the evidence classification
floor and assign trust. PostgreSQL repeats provenance/scope/classification checks,
checks typed record endpoints, and freezes candidate payloads and canonical content.

Versions separate observed, recorded, and valid time. Recording/transition time is
sampled from PostgreSQL after locking the record. Supersession uses one exact old-end/
new-start boundary; as-of retrieval can recover the prior version. Exact external
identifiers resolve existing entity identity; names/aliases return explicit ambiguity
without merging. Conflicting stable identifiers are rejected transactionally.
Duplicate confirmations retain the original canonical version/trust and record the
confirming candidate's result. Lower-classification proposals cannot alter a more
restricted current identity. Lower-trust contradictions of explicit human state
remain CONFLICT versions in visible history, leaving current state intact. This
guard is narrower than the still-planned central type-dependent authority policy.

An immutable scoped identifier registry survives staleness/revocation/quarantine.
Canonical history sets the identity's classification floor. Stale confirmations
retain stale status/trust; they do not reactivate knowledge. Changed explicit human
curation may supersede stale state. Generic promotion cannot revive revoked or
quarantined entity, claim or relationship identities; explicit authorized lifecycle
review remains part of the policy/freshness work. Supersession shortens future end
dates to its boundary and preserves already-closed intervals. Historical assertions
are withheld if their subject or target is currently hidden or inaccessible, while
the database retains their immutable history.

HTTP supports institutional proposals/curation, entities, relationships, claims,
resolution and scoped history. Curation and human attribution use the existing
admin capability gate. MCP adds entity/relationship reads and context compilation
through the same services. Temporal/classification filters and endpoint visibility
apply before relationship result limits. Dedicated real-PostgreSQL regressions
cover these behaviors; the complete product requirements remain in the ledger.

## Semantic context packet audit

Every compiled context (including MCP `bootstrap_project`) writes a
`context_packets` row recording task, budgets, classification ceiling and the
exact `knowledge_version_ids` returned. This is the substrate for future
novelty detection ("what did this agent already know?").

## Testing

- Domain rules: unit tests (promotion, enums, hashing, budget estimation, RRF).
- PostgreSQL semantics: integration tests against real PostgreSQL
  (disposable `ownstate_test_*` databases): append-only triggers, version
  immutability, one-ACTIVE index, ordering, conflicts.
- Services: full lifecycle tests (propose→promote→supersede with evidence),
  project isolation, classification ceilings, revoked/superseded exclusion,
  budget enforcement, packet audit, job processing.
- HTTP: router-level tests (auth, status codes, full E2E flow).
- MCP: real client↔server round-trip over an in-process duplex transport.

## Intentionally not implemented (clean seams left)

- Automatic LLM extraction (`KnowledgeExtractor` seam: candidates +
  promotion rules already treat extractors as untrusted).
- Novelty engine (context packets already record what agents were told).
- Object storage for large content (content_hash + size limits point there).
- Organizations/workspaces/RBAC/RLS (tenant_id everywhere; single fixed
  tenant today).
- Git-aware staleness (repository/branch/commit fields captured on events and
  evidence; no verification job yet).
- Provider adapters (Codex/Claude/imports), OpenRouter, model runtimes,
  contradiction auto-resolution (CONFLICT status exists), audit-log table
  beyond context packets, dashboards.

## Typed query execution

`domain/query.rs` defines closed Structured, Semantic, Relationship, Temporal and
Hybrid plans with bounded project/time/classification/kind/exact-ID/trust/source
and relationship constraints. Unknown fields and executable SQL are rejected.
`services/query.rs` performs project admission before retrieval or embedding calls;
`POST /query` is a thin authenticated adapter. Current queries sample PostgreSQL
time, while explicit as-of queries use the requested valid-time instant.

Migration0009 centralizes scoped SQL relations for graph and knowledge rows.
Counts and relation-backed jurisdiction groups aggregate all authorized matches
before limiting. Relationship and change-log trust/source predicates apply to
returned assertions, with endpoint kind/identity constraints separately evaluated.
Typed semantic admission requires scoped raw-event evidence; legacy zero-evidence
canonical storage/promotion still exists and remains a target gap. Semantic FTS and
vector legs each admit at most 4*limit candidates; every admitted
fused ID is revalidated before trust weighting and final truncation. Ranking is
approximate, with candidate limits exposed, unlike exact structured totals.

Knowledge metadata may contain a validated `entity_ids` UUID array. Proposal and
promotion repeat accessible entity scope/classification checks; promotion creates
immutable canonical associations. PostgreSQL independently verifies typed metadata,
tenant/project, current endpoint validity, classification floors and exact endpoint
provenance, locking endpoint rows. The shared safe-evidence view prevents linked
inactive, revoked, hidden or higher-classification endpoints from leaking through
legacy current search/final loads/history as well as the typed path. Hybrid legs
use SQL semijoins over all constrained entities; bounded output samples never
restrict the candidate association population.

Temporal results separate endpoint differences from recorded/validity transitions
in `(start,end]`; transient superseded rows are retained. A structured transition
total covers the full scoped interval population, while a semantic total covers
its bounded FTS/dense fused population. Domain trust weights are applied before
limiting; `has_more` and per-leg candidate limits disclose truncation. Returned
semantic change versions include separately keyed immutable evidence references.
Full source invalidation, ownership/workspace queries, principal/egress policy and
complete context compilation remain required target work.

## Startup configuration and packaging

Runtime configuration parses a socket bind address, strict migration boolean,
positive worker interval and database pool maximum. Invalid enum/config values
are rejected without echoing supplied content; Config debug output redacts database
credentials and token values. Pure lookup tests avoid mutating process environment.
Multistage Docker targets share compilation, run as UID10001 and keep cache/compiler
artifacts out of final images. API owns migrations; worker/MCP await readiness and
skip migrations. CI declares the four fixed Cargo gates, real PostgreSQL18/pgvector
preflight, Python runner checks and isolated deterministic container smoke. A local
fresh-stack run also proved the default Fastembed image can initialize and complete
an embedding job. Workflow syntax and local execution do not prove hosted CI or
semantic quality.
