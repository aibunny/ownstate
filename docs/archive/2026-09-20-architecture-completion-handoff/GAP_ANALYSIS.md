# Ownstate evolution audit — 2026-09-15

## Baseline and document authority

Before source edits, `cargo fmt --check`, `cargo check --workspace`,
`cargo clippy --workspace --all-targets`, and `cargo test --workspace` passed
(51 tests, no lint warnings). Integration tests used disposable databases on
the existing PostgreSQL 18 + pgvector Compose service. The sandbox initially
denied database connections; the authorized rerun with local database access
passed. No tests were skipped.

The authoritative target is in root `ARCHITECTURE.md`, `REQUIREMENTS.md`, and
`README.md` (confirmed by the user on 2026-09-15).
`docs/ARCHITECTURE.md` describes the implemented v0.1 slice, and
`docs/REQUIREMENTS.md` retains the earlier foundational specification. Preserve
these documents and distinguish target capabilities from demonstrated behavior.
This workspace has no Git metadata; do not infer changed files from an empty
`opsx apply` Git report, and do not initialize, commit, or push a repository.

## Architecture gaps

| Area | Implemented correctly | Partial or missing |
|---|---|---|
| Evidence / knowledge | Append-only hashed events, bounded ordered sessions, separate candidates/items/immutable versions, transactional promotion, evidence links | Sources are not replay-idempotent; some canonical versions have no evidence; relevance is not verified |
| Institutional domain | Projects, sessions, semantic knowledge kinds | No durable entities, aliases, structured claims, temporal relationships, reversible merge ledger, artifacts, or Experience Store |
| Temporal / authority | Version validity fields, status, confidence separate from derived trust, supersession preserves history | No observation/recording distinction, as-of/delta query, source freshness invalidation, type-specific authority, or novelty/contradiction decision |
| Retrieval / context | SQL-scoped FTS + pgvector, RRF, local ranking, content-token/item budget, context audit version IDs | No typed Structured/Relationship/Temporal plans or graph traversal; audit lacks principal/authorization decision/exact evidence IDs; token estimate excludes packet overhead |
| Authorization | Personal tenant scope, deployment classification ceiling, optional HTTP curation credential, MCP proposals forced AGENT | No per-principal policy, default-deny business grants, model-egress policy, RLS, or independently granted subagent scopes |
| Runtime / adapters | Shared services behind HTTP/MCP/worker; replaceable embedding trait; typed process config | No ModelRuntime, HarnessRuntime, extractor, connector, ObjectStore, or general EventBus; model/harness execution is absent rather than provider-coupled |
| Events / scale | Transactional embedding-job enqueue, SKIP LOCKED claims, bounded retries, stale-claim recovery | No domain outbox, claim lease fencing, ingestion replay keys, partition plan, or retrieval quality evals |
| Operability / OSS | Fresh databases built from migrations, health/readiness, stderr tracing | Compose starts PostgreSQL only; no application Dockerfile or CI; missing license decision; README claims target startup and has a broken implementation-prompt link |

## Schema and coupling

Extend the existing schema through new migrations. Preserve the existing
evidence/candidate/canonical pipeline and domain → storage → services → adapters
dependency direction. Initial schema work should add temporal entity identities,
aliases/identifiers, relationships, atomic claims, scoped provenance, and an
auditable resolution ledger. Later migrations add artifact identities/versions,
outbox events/lease fencing, principal grants/audit, agent executions, and
separate experience/dataset lineage. Do not create unused tables for every
concept at once.

The domain is already independent of Axum, SQLx, MCP, embedding runtime, and
model vendors. Preserve that boundary. The vector index is fixed at 384
dimensions and keyed/filtered by model name rather than full model version;
replacement dimensions/index coexistence need an index migration. Canonical
knowledge does not need to change when embeddings change.

## Risks and first batch

1. A proposal can be labeled below its supporting evidence. Derive the minimum
   classification from cited events at proposal and again within promotion.
   Enforce event provenance scope/classification on insertion in PostgreSQL.
2. Retrieval selects ACTIVE IDs then reloads content with only tenant filtering.
   Recheck project, classification, lifecycle and validity in the final SQL load.
   Hide unsafe legacy event-provenance links without rewriting historical rows.
3. Promotion always supersedes current ACTIVE state, including trusted state,
   without a contradiction/source-authority decision. Keep this documented and
   address centrally before automated extraction/promotion is introduced.
4. A slow worker can finish a newer reclaimed job because completion is keyed
   only by job ID/status. Add lease generation before expensive extraction jobs.
5. Independent ownership foreign keys do not enforce all tenant/project
   combinations. Add composite scope constraints and RLS before business mode.
6. Static personal-mode credentials and optional unauthenticated development
   mode do not satisfy business authorization. Do not claim business-mode
   security or model-egress control is implemented.

The first immediate cycle establishes a manual implementation loop and fixes
risks 1–2. This is an engineering dependency before exposing a richer domain,
not a replacement for the target priority order. It adds a classification-rank
function, safe event-provenance view, and evidence-insert guard through migration
0007; it does not modify historical content or add infrastructure/API endpoints.
The first full verification exposed a newly inserted test fixture on the current
validity boundary. Existing-knowledge fixtures now use an explicitly past start
while future/expired exclusion tests remain intact. Promotion now uses a single
PostgreSQL timestamp after the item lock for the old end/new start; a regression
checks exact boundary equality and database-clock bounds. Diagnostic samples did
not establish persistent clock skew; do not treat that hypothesis as a verified
environment fact.

## Dependency-ordered next cycles

1. Temporal entities/aliases/relationships/claims + scoped evidence; exact
   resolution, ambiguity without merging, deterministic jurisdiction counts.
2. Artifact identities/versions + content-addressed ObjectStore boundary and a
   local implementation; preserve creator/entity/project/provenance references.
3. Central type-specific source authority, novelty/confirmation/conflict,
   freshness/supersession and observation/recording time; preserve history.
4. Extend durable jobs into a PostgreSQL outbox/EventBus and lease-fenced,
   idempotent workers with replay-safe ingestion.
5. Safe extraction/resolution; validated typed query plans for structured,
   semantic, relationship, temporal and hybrid execution; integrate Context
   Compiler and exact provenance/audit.
6. Deterministic principal/action/resource/context authorization, model egress,
   explicit child grants and RLS defense in depth. Evaluate Cedar against actual
   requirements; no token-only system is a business-mode substitute.
7. Provider-neutral ModelRuntime with configuration-selected hosted and
   OpenAI-compatible adapters, canonical HTTP/MCP parity, HarnessRuntime,
   agent identity/capability/execution records with output captured as evidence.
8. Separate Experience Store and evaluation/dataset lineage; optional generic
   connector example only when useful. No online training or mandatory broker.

Keep README/config/migrations/tests current in each cycle. Add application
container builds, graceful worker SIGTERM handling and public CI as a scoped
operability batch; until then document native startup accurately.

See [IMPLEMENTATION_LOOP.md](IMPLEMENTATION_LOOP.md) for the five-part goal
contract, stage boundaries, stop condition, and runnable verification gate.
