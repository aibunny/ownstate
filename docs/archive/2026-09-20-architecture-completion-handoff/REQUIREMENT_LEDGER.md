# Whole-architecture completion ledger

Authority: root ARCHITECTURE.md, REQUIREMENTS.md, README.md, user correction and
whole-product persistence instruction dated 2026-09-15. `docs/` is implementation
documentation. A section marked PARTIAL does **not** satisfy all its clauses.
VERIFY_PENDING means implemented work still needs settled full gates and
independent acceptance. GAP means implementation/evidence is missing. Completion
requires every applicable clause below, including testing and acceptance rows;
green Cargo gates alone cannot close the ledger.

## Current interrupted checkpoint (2026-09-20)

The tables below combine the last accepted query/operability evidence with open
target work. They are not a current green-build receipt. The active
`implement-ownership-and-default-deny-principal-policy` change is partially
implemented, unaccepted, and the all-target build currently fails while tests
are migrated to the private principal-scoped service boundary. Migration 0011 is
also unverified. Treat ownership, principal policy, model-egress timing, child
execution, RLS/runtime roles, and related adapter rows as `VERIFY_PENDING` until
P0/P1 in `docs/PENDING_IMPLEMENTATION.md` pass on one stable manifest. Never
reopen raw service methods or cite the historical 102-test result as evidence for
the current tree.

## Requirement coverage and remaining clauses

| Root requirement | Current evidence | Remaining required work / status |
|---|---|---|
| R1 product independence | Domain types, common services, provider-neutral embedding interface | Sources across conversations/files/artifacts and optional ingestion interface; PARTIAL |
| R2 activity to durable knowledge | Evidence → proposal → promotion → search lifecycle tests | Re-extraction, export, policy deletion/revocation, selective principal/egress exposure, continuous typed pipeline; PARTIAL |
| R3 invariants | Detailed invariant table below | All rows must pass; PARTIAL |
| R4 foundation | Cargo workspace Rust2024/Tokio/Axum/SQLx, PG18/pgvector, local fastembed/MCP/tracing/BLAKE3/typed config | S3-compatible object storage; telemetry export where useful; full config externalization; PARTIAL |
| R5.1 ownership | Tenant, ownership domain, project, session; ownership/policy migration work exists in the interrupted active batch | User/organization/workspace/repository identities and scope require settled migrations, full gates and review; VERIFY_PENDING |
| R5.2 evidence | All 16 event kinds; append-only/order/storage regressions | Normalized source-record ingestion and ACL provenance; PARTIAL |
| R5.3 entities | `domain/institutional.rs`, `services/institutional.rs`, migration0008, 15 independently passing institutional integration tests | VERIFIED temporal foundation, aliases, stable external IDs. Every entity-specific workflow is not implied by vocabulary support |
| R5.4 relationships | Typed endpoints/type/confidence/status/validity/provenance, scoped SQL, endpoint/history visibility regression | VERIFIED scoped temporal foundation |
| R5.5 claims | VERIFIED typed subject/predicate/value, confidence/trust, valid/observed/recorded/status/provenance | Central type-dependent source authority remains required; PARTIAL |
| R5.6 semantic kinds | All 15 enums and SQL constraints, domain roundtrip | VERIFIED in accepted baseline |
| R5.7 history/status | Item/version separation, immutable triggers, supersession/history/lifecycle regressions | Conflict policy for semantic knowledge and temporal metadata; PARTIAL |
| R6 provenance | Event/hash/repo/commit/file/line fields; scoped DB evidence guard; institutional event references | Objects/artifact versions/explicit statements/source authority; typed semantic admission requires raw-event evidence; mandatory global canonical provenance and legacy admission remain PARTIAL |
| R7 source authority | Existing trust multiplier; institutional human-vs-agent conflict safeguard | Central knowledge-type-dependent policy, verified legal/repository/decision sources and currentness precedence; GAP |
| R8 entity resolution | VERIFIED stable exact external IDs, aliases/name alternatives, no uncertain destructive merge tests | Attribute/relationship/semantic comparison seam where needed; auditable reversible explicit merge operations; PARTIAL |
| R9 artifacts | Event type only | Artifact identity/version/hash/media/creator/project/entities/times/provenance; SENT_TO/REVIEWED_BY/SUPERSEDES/CREATED_FOR; deduplicated object storage; GAP |
| R10 freshness | Current-version selection, final retrieval revalidation, institutional validity intervals | Typed eight novelty outcomes; affected-state tracking; source change/document-version staleness; projection/context invalidation; PARTIAL |
| R11 pipeline | Knowledge proposal and deterministic promotion | Typed classification/normalization/extraction/resolution/novelty/verification/policy stages; untrusted extractor interface without credentials/capabilities; GAP |
| R12 retrieval | FTS+dense+RRF, tenant/project/kind/classification/current validity; temporal edges SQL | Typed exact/relationship/temporal/hybrid foundation implemented and focused tests passed; workspace scope; source authority/recency; sparse only where useful; PARTIAL |
| R13 query planner | Typed QueryPlan, static scoped SQL relations, protected common-service /query; 12 query storage regressions plus typed service/HTTP tests; settled full gates and independent acceptance | VERIFIED scoped typed execution; workspace principal scope and authority ranking remain tracked under R12/R22 |
| R14 context compiler | Search/trust/budget/categories/context packet | Principal auth, scope/plan, semantic dedup, entities/claims/artifacts, total serialized packet budget, freshness-aware packet invalidation; PARTIAL |
| R15 packet audit | Tenant/project/session/provider/model/agent/task/budget/version IDs/tokens/created | Principal, exact entity/claim/evidence IDs and authorization decision context, queries and freshness; PARTIAL |
| R16 MCP | Single common-service server; baseline bootstrap/search/get/propose; new get_entity/get_relationships/compile_context; interrupted principal binding exists | Feedback is missing; principal authorization/egress needs settled full gates and review; VERIFY_PENDING/PARTIAL |
| R17 ModelRuntime | Provider-neutral embedding interface only | Provider-neutral generation/runtime capability/config; OpenAI-compatible/selfhost adapter (OpenRouter may share adapter); GAP |
| R18 experience | None | Separate experience/task/packet/provider/model/output/tools/outcome/feedback/quality/class/provenance/trainability; versioned dataset/model-adapter lineage; GAP |
| R19 HarnessRuntime | None | Internal abstraction and personal implementation independent of optional UHP; GAP |
| R20 agents | Session agent text plus interrupted child-execution/policy foundation | Settle child attenuation first, then durable task/packet/tools/model/harness lifecycle and output evidence; VERIFY_PENDING/PARTIAL |
| R21 connectors | Session external ID, event external ID | Optional generic source interface initial/incremental sync, cursor/pagination/times/tombstones/rate limit/backoff/ACL/idempotency; no canonical writes; GAP |
| R22 authorization | Tenant/project scope, server classification ceiling, API/admin credential gate; interrupted principal/action/resource/context policy implementation | Settle private service boundary, provider-call egress lease, adapter grants and non-owner RLS proof; VERIFY_PENDING |
| R23 capability security | No SQL/model canonical tool, immutable history, untrusted MCP proposer; interrupted capability/grant implementation | Settle fine-grained capability/egress boundaries, child execution and sensitive error-log audit; VERIFY_PENDING/PARTIAL |
| R24 objects | BLAKE3 hashes on events | S3-compatible immutable content-addressed object store, PG metadata/provenance and binary dedup; GAP |
| R25 events/workers | PG job queue/retries/SKIP LOCKED, embedding job | Transactional outbox/EventBus, idempotency keys, claim-ownership fencing, stateless scalable retries; PARTIAL |
| R26 scalability | Modular monolith, API/MCP/worker/PG | Object seam and clean scale boundaries; measured distributed deployments optional; PARTIAL |
| R27 scale keys | Tenant/time fields on evidence/packets, pool injection | Scope all high-growth/new records, dedicated-tenant routing seam/config; actual partitioning is optional future infrastructure; PARTIAL |
| R28 observability | Structured operation IDs/counts/job status/errors | Request correlation, latency, extraction/model/token metrics, safe telemetry export where useful; PARTIAL |
| R29 API | Existing requested health/project/session/evidence/knowledge/context routes tested | Entity and entity-relationship routes VERIFIED; admin debug secure policy; PARTIAL |
| R30 operability | README/current docs/env example, typed config, multistage non-root API/worker/MCP images, PG18 Compose, migrations, CI and local fresh-stack Fastembed/MCP proof | Hosted CI execution and future object-store profile/config; details below; PARTIAL |
| R31 testing | Settled full gates: 102 Rust +10 Python tests; 12 query storage and 5 service query tests independently passed | Explicit matrix and broader target scenarios below; PARTIAL |
| R32 evaluation | None | Precision/recall/entity/conflict/stale/provenance/structured/temporal/leakage/packet/agent/rediscovery metrics and reproducible fixtures; GAP |
| R33 acceptance | Semantic and basic learning lifecycle regressions only | Six complete end-to-end scenarios below; PARTIAL |

## R3 invariant audit

| Invariant | Evidence/status |
|---|---|
| 1 raw separate from derived | interaction_events vs candidates/items/versions; VERIFIED |
| 2 candidate separate from canonical | Pending persistence/promotion tests; VERIFIED |
| 3 models cannot directly mutate canonical | No model write credentials/tool; MCP only proposes; VERIFIED for existing adapters, future runtime must preserve |
| 4 connectors cannot directly mutate canonical | Connector pipeline/interface missing; GAP |
| 5 agents cannot declare canonical truth | MCP forced AGENT + admin curation test; scoped agent runtime missing; PARTIAL |
| 6 canonical versioned never overwritten | PostgreSQL immutable semantic and institutional history/content tests; VERIFIED |
| 7 important canonical provenance | Classification/scope guard; zero-evidence semantic promotion still exists; PARTIAL |
| 8 conflicts visible until resolved | Institutional conflict history test VERIFIED; semantic conflict resolution missing; PARTIAL |
| 9 authorize before retrieval/egress | Fixed tenant/ceiling scoped SQL; perprincipal/egress absent; PARTIAL |
| 10 replaceable models/providers/agents/protocols | Embedding seam/common services; model/harness seams missing; PARTIAL |
| 11 vendor-independent schema | No vendor-specific tables; VERIFIED for current schema |
| 12 current authority outranks stale inference | Current-validity/final recheck tests; central authority absent; PARTIAL |
| 13 evidence reprocessable | Append-only raw content/hashes preserved; re-extraction pipeline absent; PARTIAL |
| 14 auditable context | Exact semantic version packet tests; entity/claim/evidence/principal audit absent; PARTIAL |
| 15 optional connectors | No external connector startup dependency; generic optional interface absent; PARTIAL |
| 16 selfhost without hidden services | Fresh local Docker stack verified PG/API/worker/MCP, nine migrations, default Fastembed cache and a real job without paid APIs; object storage and full target capabilities remain PARTIAL |

## R30 artifact and command gates

Every passing result in this table is historical evidence for the last accepted
query/operability source manifest. It does not describe the current interrupted
tree. Re-run and replace these entries after migrations 0010/0011 and the active
policy change settle.

| Artifact / command | Historical accepted evidence / current gap |
|---|---|
| README.md accurate implemented/experimental/planned | Updated institutional foundation and broader planned capability distinction; reviewed |
| docs/ARCHITECTURE.md and docs/REQUIREMENTS.md | Updated current institutional architecture and root-authority distinction; reviewed; continue synchronization with later batches |
| .env.example placeholders, no secrets | Accepted baseline; extend only typed runtime config |
| Dockerfile | Multistage Trixie-based API/worker/MCP, UID10001; default images built and locally started from settled source |
| Compose API +worker +PG, optional object profile | Historical nine-migration stack passed health/readiness, UID10001, default Fastembed real job and MCP handshake; migrations 0010/0011 and object adapter/profile remain unverified/GAP |
| CI fmt/check/clippy/test using real PG18/pgvector | Workflow declares four gates, real PG preflight, Python and isolated container smoke; syntax checked, hosted execution not claimed |
| cp .env.example .env; docker compose up --build | Equivalent clean isolated project invocation verified locally with default Fastembed; actual clone command is documentation-only |
| cargo fmt --check | Historical query/operability source passed; current tree must rerun |
| cargo check --workspace | Historical query/operability source passed; current all-target build is known broken |
| cargo clippy --workspace --all-targets -- -D warnings | Historical query/operability source passed with warnings denied; current tree must rerun |
| cargo test --workspace | Historical query/operability source passed 102 Rust tests, zero failed/skipped; current tree must rerun |
| Fresh migrations | Historical harness tested first-six→migration 0009 and a nine-migration fresh stack; migrations 0010/0011 require fresh and upgrade proof |
| Typed runtime config, no scattered env reads | Typed socket address/strict boolean/positive interval/pool maximum; redacted Debug and invalid-value errors; 2 pure tests passed; future adapter config remains |
| Public fictional examples/fixtures | Institutional Atlas/Mercury examples fictional; no private organizational seed data |

## R31 explicit test matrix

| Required test | Evidence/status |
|---|---|
| Project/tenant isolation | lifecycle/retrieval tests VERIFIED; institutional test VERIFIED |
| Evidence immutability | schema_invariants VERIFIED |
| Event ordering | schema_invariants VERIFIED |
| Candidate persistence | knowledge_lifecycle VERIFIED; institutional VERIFIED |
| Candidate promotion | knowledge_lifecycle VERIFIED; institutional VERIFIED |
| Temporal supersession | exact PG transition regression VERIFIED; institutional as-of VERIFIED |
| History preservation | knowledge_lifecycle VERIFIED; institutional VERIFIED |
| Contradiction handling | institutional current+conflict regression VERIFIED; semantic GAP |
| Provenance | schema/classification/lifecycle VERIFIED; artifact GAP |
| Entity resolution | institutional exact ID/alias regression VERIFIED |
| Uncertain merge behavior | institutional ambiguity/no automatic merge VERIFIED; reversible explicit merge GAP |
| Source precedence | institutional human guard VERIFIED; central knowledge-type policy GAP |
| Structured queries | Exact count/provenance and jurisdiction group tests passed real PG, settled gates and independent review |
| Semantic retrieval | retrieval_and_context VERIFIED |
| Hybrid queries | Constrained canonical entity links +semantic SQL semijoin, bounded outputs/full association population passed real PG and independent review |
| Temporal queries | Scoped snapshots +structured/FTS/dense interval transitions including transient and trust/source cases passed real PG and independent review |
| Revoked/stale knowledge | retrieval invalidation VERIFIED; endpoint hiding VERIFIED |
| Context packet creation | retrieval_and_context VERIFIED; complete audit GAP |
| MCP authorization | Forced proposer/shared ceiling existing; explicit principals/capabilities/transport tests GAP |
| Model-egress policy | GAP |
| Event idempotency | Existing event sequence conflict insufficient for external replay; GAP |
| Worker retry behavior | knowledge_lifecycle existing basic retries; fencing/idempotent outbox GAP |
| Stale source invalidation | Manual lifecycle invalidation VERIFIED; automatic source-change invalidation GAP |
| Agent/subagent authorization | GAP |

## R33 acceptance and R32 evaluation

| Scenario / metric | Proof required / status |
|---|---|
| Legal entities and jurisdictions | Exact scoped count and relation-backed grouping/provenance fixture passed real PG, settled gates and independent review |
| Recovery architecture and rationale | Current semantic/temporal/evidence combined answer fixture; partial search tests |
| Customer feature requests and commitments | Typed customer/relationships + linked semantic + valid-time fixture; GAP |
| Project monthly changes | Typed structured and semantic interval history implemented with explicit bounded semantic population and sources; integrated full scenario and source freshness remain PARTIAL |
| Investor update task | Authorized org/investor/project/artifact/prior commitment packet, complete audit and budget; GAP |
| Learning loop six steps | Existing evidence/propose/promote/search/supersede/history tests; freshness/policy/extractor coverage GAP |
| Retrieval precision/recall | Reproducible labeled retrieval fixtures and measurements; GAP |
| Entity-resolution accuracy | Exact/ambiguous labeled cases and metric; GAP |
| Contradiction detection | Labeled conflict/authority cases and metric; GAP |
| Stale knowledge rate | Active source-valid state ratio and source-change fixture; GAP |
| Answer provenance coverage | Delivered items with exact cited sources metric; GAP |
| Structured-query accuracy | Expected SQL-backed answers vs output; GAP |
| Temporal-query accuracy | Expected as-of/delta answers vs output; GAP |
| Cross-tenant leakage | Isolation fixture and zero leakage metric; GAP |
| ContextPacket size | Complete serialized token/item measurement; existing content-only estimate PARTIAL |
| Agent task success | Objective outcome fixture/experience metric; GAP |
| Rediscovery tokens avoided | Explicit baseline vs reused-knowledge token accounting; GAP |

## Architecture crosswalk

All root architecture sections remain in scope through these requirements.
Sections 1–3 → R1–4/22–24; 4–8 → R5–7/10/18; 9 → R8;
10 → R9/24; 11 → R21; 12 → R12; 13 → R13; 14 → R14–15;
15 → R4–6/24; 16 → R10–11/25; 17 → R17; 18 → R18;
19 → R16/19–20; 20 → R22–23; 21 → R25–27; 22 → R32;
23 → R4; 24 → R30; 25 → product scope and future exclusions below;
26 → dependency ordering, not permission to omit applicable requirements.
README target modes, knowledge layers, examples, quickstart, configuration,
extension points and quality checks map to the same rows; Business support
requires ownership/principal/egress/agent seams rather than a Personal-only claim.

## Explicit optional/future choices and non-goals

- R5.3 permits postponing entity-specific implementations; enum vocabulary is
  supported without claiming every specialized workflow.
- R12 local reranker and sparse retrieval are optional where useful; required
  typed seams/explainable hybrid retrieval cannot be omitted on this basis.
- R17 implementations are possibilities, not mandatory three duplicate SDKs;
  a configured provider-neutral OpenAI-compatible adapter must support selfhost.
- R18 explicitly prohibits continuous retraining after every interaction;
  versioned experience/dataset/model lineage is required, an actual training
  engine is future work.
- R19 UHP adapter is optional managed execution; internal HarnessRuntime and
  independent Personal mode are required.
- R20 swarm orchestrator/A2A are future/conditional; individual child identities,
  explicit attenuated scope and canonical shared memory are required now.
- R21 every named external connector is optional; generic safe sync/ACL/retry
  contracts are required and zero-connector startup must work.
- R25–27 NATS, microservices, replicas, actual partitioning, GPU inference,
  dedicated regulatory database deployment and specialized stores require
  measurements/need; clean tenant/time/storage/event/runtime seams are required.
- R28 OpenTelemetry exporter is conditional where useful; safe structured metrics
  and correlation are required.
- Root architecture long-term vision does not require replacing current models,
  training online, mandatory vendors/agents, or premature distributed infrastructure.
- No commit/push or deployment to external systems is authorized by this ledger.

## Next dependency order

Temporal domain and scoped exact queries → ownership/policy and artifacts →
central authority/novelty/source freshness → outbox/idempotency → typed learning
pipeline → typed query/context/audit → model/harness/agent capability execution →
experience/dataset lineage → evaluation and six acceptance scenarios. Docker/CI
and configuration can proceed alongside these when review capacity returns.

Historical temporal/institutional-batch note: an independent reviewer ran its
settled 15-test PostgreSQL suite, and repaired findings covered history endpoint
visibility, stable identifiers across inactive statuses, HTTP as-of handling,
stale human-trust preservation, and future validity-end overlap. That earlier
source passed 80 Rust and 10 Python tests. Later accepted query/operability
evidence superseded its gate count. Neither historical receipt validates the
current interrupted policy tree or closes broader PARTIAL/GAP rows.
