# Pending implementation — architecture-constrained task graph

This document is an execution map, not target authority. Root `ARCHITECTURE.md`,
root `REQUIREMENTS.md`, and root `README.md` control scope. If this file conflicts
with them, repair this file before changing code.

## Architecture traceability gate

No model may implement a feature merely because it appears useful, modern, or
common in similar products. Before editing code, write a batch contract containing:

| Required field | Rule |
|---|---|
| Root clause | Quote or precisely cite the root architecture/requirement section. |
| Existing gap | Cite the ledger row and source/test evidence proving the gap. |
| Smallest behavior | State the minimum behavior that satisfies the clause. |
| Changed paths | List owned files before edits; avoid unrelated refactors. |
| Acceptance | Name deterministic tests and real infrastructure proof. |
| Non-goals | List adjacent features that the root docs mark optional/future or that are unnecessary for this batch. |

Reject or remove a proposed change when it lacks this trace. Do not add a new
service, database, broker, framework, protocol, SDK, abstraction, configuration
surface, or dependency unless a cited root clause requires it and the existing
implementation cannot satisfy the clause more simply.

Treat the modal language and context in the root documents literally:

- **must/shall/required** and an unconditional product **should** create target
  scope;
- **may/optional/where useful** is conditional scope, and **future/later** work is
  implemented only when the root-stated prerequisite is now true;
- examples explain a required capability but do not require every named vendor,
  integration, entity-specific workflow, or deployment topology;
- an architectural seam is complete when the required contract and one
  production-credible implementation prove replacement; do not build duplicate
  providers merely to demonstrate replaceability.

If a requested or proposed feature cannot be mapped to a mandatory root clause,
record it as out of scope and do not implement it. If two root clauses appear to
conflict, stop that batch, cite both clauses in `STATE.md`, and obtain an
independent architecture interpretation before editing. Never resolve ambiguity
by expanding scope.

Use the root documents' dependency order. Preserve proven behavior and extend it
additively. Do not rewrite accepted evidence, institutional-state, query, or
operability foundations simply to make the code resemble a new design.

## Root requirement execution map

This map prevents a ledger row from being lost between batches. The ledger gives
the clause-level status; this table assigns the remaining proof to a batch.

| Root requirement | Owning batch(es) |
|---|---|
| R1–R2 independence and durable knowledge | P6, P13 |
| R3 invariants | Every batch; final audit in P13 |
| R4 technology foundation | P3, P7, P12 |
| R5.1 ownership hierarchy | P0–P1 |
| R5.2 evidence/source records | P6 |
| R5.3–R5.7 institutional model/history | Preserve accepted foundation; remaining conflict/authority work P4; P13 regression |
| R6 provenance | P3, P4, P6 |
| R7 source authority | P4 |
| R8 entity resolution/merge | P4 |
| R9 artifacts | P3 |
| R10 freshness | P4 |
| R11 typed learning pipeline | P6 |
| R12 retrieval | P4, P7; preserve accepted query/retrieval foundation |
| R13 query planner | Accepted foundation; P13 regression and scope verification |
| R14–R15 Context Compiler/audit | P2 |
| R16 MCP | P1 authorization and P6 feedback |
| R17 ModelRuntime | P7 |
| R18 Experience Store/lineage | P9 |
| R19 HarnessRuntime/UHP boundary | P7 |
| R20 agents/child grants | P1, P8 |
| R21 optional connectors | P10 |
| R22–R23 authorization/security | P0–P1, P12 |
| R24 object storage | P3 |
| R25 events/workers | P5 |
| R26–R27 scale seams/keys | P5, P12 |
| R28 observability | P11, P12 |
| R29 API direction | P1 protected adapters, P12 secure admin/debug; preserve accepted entity routes |
| R30 operability | P3, P7, P10, P12 |
| R31 tests | Every batch; final matrix in P13 |
| R32–R33 evaluation/acceptance | P11, P13 |

## P0 — restore a trustworthy build without reopening bypasses

The current all-target build fails because the authorization boundary was made
private before all tests were migrated. The repair must preserve the private
boundary.

### P0.1 Finish opaque principal ownership

- Keep `AuthenticatedPrincipal` in `crates/storage/src/policy.rs` with private
  fields and a private constructor.
- Do not reintroduce `AuthenticatedPrincipal::from_verified_record` in the domain
  or expose a public constructor, `Deserialize`, `Default`, or public fields.
- Adapters obtain principals only from credential resolution, Personal bootstrap,
  or child-execution creation.
- Run `rg -n "AuthenticatedPrincipal|from_verified_record" --glob '*.rs'` and
  verify that only storage constructs the value.

### P0.2 Finish the principal-only service boundary

- Keep raw use-case methods on `AppServices` `pub(crate)` or narrower in:
  `projects.rs`, `sessions.rs`, `knowledge.rs`, `search.rs`, `context.rs`,
  `query.rs`, `bootstrap.rs`, `institutional.rs`, and the insecure raw job path.
- Keep public use cases on `PrincipalServices` in `policy.rs`.
- Do not expose `PrincipalServices::app()` or the embedding provider publicly.
- Complete `TestServices` in `crates/services/tests/common/mod.rs`; it should own
  an `Arc<AppServices>` for database inspection and dereference to an owner-scoped
  `PrincipalServices` for use cases.
- Convert `institutional_state.rs`, `typed_queries.rs`, remaining
  `retrieval_and_context.rs` helpers, and `policy_boundary.rs` to scoped services.
- Convert API and MCP fixtures that seed state through raw services. Obtain a
  Personal owner/worker actor and call the principal facade instead.
- Remove the old raw `AppServices` worker execution methods once tests use the
  principal worker path. Do not silence dead-code warnings for a security-bypass
  path.
- Add a compile-time regression test or API-surface check if practical. At
  minimum, an external crate test must be unable to call raw promotion/query
  methods, while HTTP/MCP/worker tests succeed through principals.

### P0.3 Repair and verify migration 0011

`migrations/0011_policy_boundary_hardening.sql` is unverified.

- Update `ownstate_policy_immutable` so an exact no-op update returns unchanged.
  This is required for idempotent service bootstrap and for re-registering an
  already revoked credential without resurrecting it.
- Permit only the first grant/credential revocation transition from `NULL` to a
  timestamp at or after `created_at`. Reject changing, clearing, or repeating a
  different revocation timestamp.
- Preserve the new tenant-composite foreign keys for parent executions and child
  and parent grants.
- Preserve immutable project ownership; allow only the exact no-op used by
  idempotent `bind_project`.
- Keep the `SECURITY DEFINER` credential resolver narrowly scoped, schema
  qualified, with a fixed safe `search_path`, and returning only principal ID,
  tenant ID, and kind. It must never return the credential digest.
- Add upgrade coverage from migration 0010 to 0011 and fresh-database coverage.
- Test that a revoked credential cannot be resurrected, project ownership cannot
  be reassigned, and cross-tenant agent-grant links fail at the database layer.
- Do not edit migration 0010; it has already been applied to preserved data.

### P0.4 Establish a real non-owner runtime/RLS path

- In a disposable PostgreSQL database, create a non-owner role with only the
  privileges required by runtime queries.
- Prove credential resolution works through the narrow resolver before a tenant
  session variable exists.
- Prove policy tables return no rows without transaction-local tenant context,
  return only the selected tenant with context, and return no previous tenant
  after commit and pooled connection reuse.
- Prove the role cannot alter migrations, policy decisions, immutable evidence,
  canonical versions, ownership, or grant content.
- Produce a table-by-table enforcement matrix for every tenant-bearing read/write
  path. For each table, record whether isolation is enforced by non-owner RLS,
  static parameterized tenant/workspace/project predicates, or both, and cite its
  test. A table without RLS is acceptable only where the root says RLS is
  conditional and the static scoped query plus least-privilege role prevents
  bypass. Test representative evidence, canonical, retrieval/context, artifact,
  experience, job/outbox, and audit paths as they are added.
- Separate migration-owner and runtime database credentials/configuration for
  Business deployments. The API may migrate with the migration URL, then must
  serve through the restricted runtime pool. Worker and MCP use only runtime
  credentials. Personal development may retain a clearly documented simpler
  owner connection.
- Do not claim RLS defense while all production processes connect as the table
  owner.

## P1 — close the active ownership/default-deny policy batch

Root trace: `REQUIREMENTS.md` sections 5.1, 20, 22, 23 and testing section 31;
`ARCHITECTURE.md` authorization, classification, model-egress, and subagent
sections. Active OpenSpec: `implement-ownership-and-default-deny-principal-policy`.

### P1.1 Bind authorization to actual model egress

- Search, semantic query, hybrid query, temporal-semantic query, context compile,
  project bootstrap, and embedding worker must hold an allowed
  `GENERATE_EMBEDDING` and exact-destination `MODEL_EGRESS` policy lease while the
  provider call executes.
- A revocation update must either win before admission and prevent the call, or
  serialize after an already admitted call. It must never overtake admission
  while the provider is receiving data.
- Do not treat a post-call denial as egress prevention.
- Add a blocking embedding provider test: verify the revocation transaction
  cannot complete while the authorized provider call holds the grant lock; after
  release, both operations complete in a deterministic order.
- Test missing egress action, wrong local provider, remote destination without an
  exact grant, classification above ceiling, and token/item budget overflow.
- Worker content must not load before read and egress admission; its final
  transaction must re-resolve resource scope and authorization before embedding
  persistence.

### P1.2 Finish mutation/read lease coverage

- Hold policy leases through project/session/evidence/proposal/promotion/rejection
  commits and sensitive reads. Do not authorize in one transaction, release the
  grant lock, and commit the sensitive operation later.
- Keep denied decisions append-only and redacted. A denied request must not lose
  its audit row through rollback.
- Add tests for grant and principal changes racing a sensitive read and write.
- Test same-tenant wrong-workspace, cross-tenant, unowned organization project,
  expired credential, disabled principal, revoked credential, revoked grant,
  classification, destination, and budget denial.

### P1.3 Finish child-execution attenuation

- Every agent service must carry a verified execution ID. A bare AGENT principal
  is unusable.
- Child actions, destinations, project/workspace, classification, budget, and
  expiry must be subsets of a currently delegable parent grant.
- Prohibit child `DELEGATE_AGENT`, human attribution, policy/grant mutation, and
  any capability absent from the explicit child grant.
- Persist parent grant, child grant, execution task, harness, destination, expiry,
  and status with same-tenant foreign keys.
- Test no inheritance, wrong execution ID, completed/revoked/expired execution,
  parent revocation, budget escalation, cross-project scope, and prohibited
  human attribution.

### P1.4 Adapter and deployment acceptance

- Complete the required User → Organization → Workspace → Project → Repository →
  Session ownership/scope hierarchy without replacing accepted IDs. Personal and
  Organization ownership domains must remain explicit. Add repository binding,
  organization/workspace authorization, wrong-workspace, wrong-repository, and
  cross-organization denial tests.
- HTTP resolves the bearer digest on every request; JSON/query fields never
  establish identity. Unknown identity/tenant/grant fields are rejected.
- API and admin credentials have separate explicit grants when configured.
- MCP and worker use distinct service principals and no Personal owner shortcut.
- Business mode has no fallback and fails startup if configured credentials do
  not resolve to provisioned principals.
- Personal compatibility exists only through server-created explicit grants.
- Add API, MCP, worker, and real PostgreSQL acceptance for every case listed in
  the active OpenSpec tasks.

### P1.5 Review and close

- Run targeted tests, four fixed Cargo gates, Python runner tests, and a stable
  implementation-loop verify receipt.
- Obtain fresh independent correctness and security review. The prior strict
  review failed on raw service bypass, egress timing, incomplete context audit,
  and non-owner RLS evidence; do not reuse it as acceptance.
- The active policy proposal explicitly leaves complete ContextPacket audit for a
  later batch. Record that honestly; do not claim whole R14/R15 completion here.
- Update README, current architecture, operations, ledger, state, and OpenSpec
  with exact evidence, then apply the change.

## P2 — complete ContextPacket and authorization audit

Root trace: `ARCHITECTURE.md` Context Compiler and audit sections;
`REQUIREMENTS.md` sections 14 and 15.

- Extend ContextPacket persistence with requesting principal, execution, grant or
  decision references, requested and effective scope/classification,
  destination/provider/model, query/task, token and item budgets, exact knowledge
  versions, entities, claims, artifacts, and evidence actually delivered.
- Store stable IDs/digests, never raw bearer tokens or confidential prompt text in
  logs. Packet content remains governed by classification and retention policy.
- Enforce total serialized packet budget, not content-only estimates.
- Include semantic deduplication, freshness state, and invalidation references.
- Ensure bootstrap/context/query deliveries create an audit only after the final
  delivered set is known and after final authorization.
- Test that audit equals the delivered payload exactly, revoked/hidden material is
  absent, pooled principal context does not leak, and failed delivery does not
  create a misleading successful audit.

## P3 — artifacts and content-addressed object storage

Root trace: requirements 5.2, 6, 9, and 24.

- Add provider-neutral `ObjectStore` with an S3-compatible implementation and a
  deterministic local test implementation. Do not make cloud credentials
  mandatory for zero-connector Personal startup.
- Add immutable artifact identity/version metadata, BLAKE3 content address,
  media type, size, creator/principal, project/workspace ownership, timestamps,
  provenance, entity links, and SENT_TO/REVIEWED_BY/SUPERSEDES/CREATED_FOR
  relations.
- Store large immutable bytes once per hash; PostgreSQL remains canonical for
  metadata/provenance.
- Authorize metadata retrieval and object egress before issuing object-store
  reads or signed URLs.
- Test deduplication, version history, wrong-tenant/project denial, tampered hash,
  missing object behavior, retention/deletion policy, and backup/restore.

## P4 — central source authority, novelty, and freshness

Root trace: requirements 7, 8, 10, and 12; architecture source-authority,
entity-resolution, freshness, and retrieval sections.

- Implement one deterministic policy for source authority by knowledge type,
  classification, verification, trust, and currentness. Preserve the accepted
  institutional human-vs-agent safeguard.
- Implement typed novelty outcomes: new, confirmation, update, contradiction,
  duplicate, stale-source impact, insufficient evidence, and needs review (use
  the exact root vocabulary if it differs after rereading).
- Track source/document versions and affected canonical state. Source changes or
  revocation must mark dependent projections/context stale without rewriting
  history.
- Require evidence for every new canonical semantic write; close the legacy
  zero-evidence promotion path.
- Add explicit entity-merge proposals only where the root entity-resolution
  contract needs them. A merge must be reviewable, auditable, reversible, and
  preserve both original identities and provenance. Uncertain matches remain
  separate.
- Make semantic contradictions visible until a deterministic, authorized
  resolution supersedes them; never erase the losing history.
- Support workspace-scoped retrieval across projects only when the requesting
  principal has an explicit workspace grant. Preserve project boundaries and
  rank by source authority and recency as required by R12.
- Test type-specific precedence, lower-authority contradictions, source
  revocation, stale propagation, re-verification, historical visibility,
  ambiguous merge rejection, merge reversal, workspace isolation, and
  authority/recency ranking.

## P5 — transactional outbox, idempotency, and worker fencing

Root trace: requirements 25–27.

- Add PostgreSQL transactional outbox writes in the same transaction as state
  changes and an internal `EventBus` contract backed initially by PostgreSQL.
- Add stable external idempotency keys, replay-safe ingestion, claim ownership
  tokens/leases, fencing, bounded retry/backoff, dead-letter/admin inspection, and
  stateless worker behavior.
- Do not add NATS unless measured throughput or decoupling evidence requires it.
- Test crash after commit/before publish, duplicate delivery, stale worker
  completion, lease expiry, retry exhaustion, ordering constraints, and multiple
  workers.

## P6 — typed ingestion and learning pipeline

Root trace: requirements 1, 2, 5.2, 6, 11, and the missing `feedback` operation
from requirement 16.

- Add normalized source records and typed stages for classification,
  normalization, extraction, entity resolution, novelty, verification, policy,
  proposal, and deterministic promotion.
- Cover the mandatory source classes without creating vendor-specific paths: AI
  interactions, agent interactions, explicit user knowledge, uploaded files, and
  generated artifacts. Each enters through the same normalized evidence gate.
- Extractors receive bounded untrusted content and no database credentials,
  canonical-write capability, permission mutation, unrestricted shell, secrets,
  or unrestricted network.
- Preserve source ACL and provenance where available. Derived knowledge must
  carry sufficient source-policy lineage so a principal who cannot access the
  sole supporting source cannot retrieve the derived item. Multiple-source items
  must have deterministic exposure semantics. No connector writes canonical
  truth directly.
- Add re-extraction/version lineage and policy-aware export/deletion/revocation.
- Add the missing MCP feedback operation only as an authenticated evidence or
  proposal input into this pipeline. Feedback cannot directly edit canonical
  knowledge, authority, permissions, or policy. Its principal, target version,
  classification, and provenance must be preserved.
- Test malformed/untrusted model output, prompt injection payloads, stage replay,
  evidence lineage, uploaded/generated source normalization, source ACL changes
  and exclusively-derived denial, feedback authorization/provenance, and
  deterministic policy rejection.

## P7 — provider-neutral model and harness execution

Root trace: requirements 12, 17, and 19, including replaceable embedding
projections and Personal-mode independence from UHP.

- Add an internal `ModelRuntime` capability interface for bounded generation and
  one OpenAI-compatible adapter that can target hosted or self-hosted endpoints.
  Keep provider-specific types out of domain.
- Add internal `HarnessRuntime`; Personal mode must work without UHP. UHP remains
  optional for managed Business execution.
- Keep embedding model/provider/version explicit and replaceable in retrieval
  metadata. Re-embedding creates a new projection; it never changes canonical
  knowledge or silently mixes incompatible vectors.
- Put policy admission around prompt/context construction, endpoint destination,
  tools, budgets, and response handling. Models never choose permissions.
- Test provider replacement, self-host endpoint configuration, cancellation,
  timeout, retry classification, egress denial, budget enforcement, and secret
  redaction.

## P8 — durable agents and canonical shared memory

Root trace: requirements 20.

- Extend the current child-execution foundation into durable agent execution with
  identity, task, ContextPacket, explicit tool/capability set, model, harness,
  budget, lifecycle, outcome, and evidence output.
- Agent output enters the normal evidence/candidate/policy/promotion pipeline.
  Agents do not own private canonical memories.
- Parent permissions never transfer automatically. Swarm orchestration and A2A
  remain future/conditional; do not add them merely to close this requirement.
- Test task cancellation, expired/revoked grants, tool denial, child isolation,
  output provenance, and recovery after worker/process failure.

## P9 — Experience Store and lineage

Root trace: requirements 18.

- Add separate experience records for task, ContextPacket, runtime/model, output,
  tools, objective outcome, feedback, quality, classification, provenance, and
  trainability status.
- Add versioned dataset and model/adapter lineage. Do not implement continuous
  retraining after every interaction.
- Keep experience authorization, retention, and export separate from canonical
  knowledge.
- Test classification/ownership, feedback changes, dataset reproducibility,
  lineage immutability, and exclusion of non-trainable records.

## P10 — optional connector contract, no mandatory connectors

Root trace: requirements 21.

- Implement a generic connector/source contract for initial and incremental sync,
  stable external IDs, source timestamps, cursors/pagination, tombstones,
  rate-limit handling, retry/backoff, ACL preservation, and idempotency.
- Implement only the smallest fictional/local adapter needed to prove the
  contract. Every named SaaS connector is optional; do not install or require
  third-party connector plugins to claim the core architecture.
- Test zero-connector startup, duplicate pages, cursor recovery, tombstones, ACL
  changes, rate limits, and canonical-write prohibition.

## P11 — observability, evaluation, and six product scenarios

Root trace: requirements 28, 32, and 33.

- Add safe structured metrics/correlation for retrieval precision/recall,
  entity-resolution accuracy, contradiction detection, stale rate, provenance
  coverage, structured/temporal accuracy, cross-tenant leakage, packet size,
  agent task success, and rediscovery tokens avoided.
- Never emit content, tokens, secrets, or credential digests in logs/telemetry.
- Build reproducible fictional labeled fixtures and thresholds. A metric existing
  without a fixture and expected result is not acceptance.
- Implement the six root scenarios end to end: legal entities/jurisdictions;
  recovery architecture/rationale; customer requests/commitments; monthly
  project changes; investor update; and the six-step learning loop.
- Each scenario must prove current and historical answers, exact provenance,
  permissions, model egress, bounded audited context, and failure behavior.

## P12 — production operations and final hardening

Root trace: requirements 4, 23, 26–31 and architecture sections 20–24. Follow
`docs/SECURITY_AND_PRODUCTION_ACCEPTANCE.md`. The security, recovery, and
operational evidence also implements the user's 2026-09-20 requirement that the
result be secure and institutionally production ready. It is an acceptance gate,
not permission to add product features. Every mitigation must map to a documented
threat or production failure mode, and must use the smallest effective control.

- Fresh install and upgrade from every supported migration boundary.
- Separate migration/runtime/admin credentials and least-privilege roles.
- TLS, secret injection/rotation, request size/time/concurrency limits, graceful
  shutdown, health/readiness semantics, backup/restore and disaster recovery.
- Put every admin/debug route behind a distinct, explicit admin capability,
  redact its output, and disable it by default. Health/readiness never expose
  tenant data, credentials, SQL, configuration secrets, or dependency internals.
- Add the required dedicated-tenant routing seam and configuration validation;
  prove selection and isolation with tests. Do not provision a dedicated
  database or distributed control plane unless a root requirement and measured
  need require that deployment.
- Non-root minimal images, pinned dependencies, SBOM/vulnerability/license
  review, filesystem permissions, and isolated default network exposure.
- Add the object-store Compose profile and documented configuration after P3,
  while preserving a zero-connector startup path. Run the repository workflow in
  its hosted CI environment; a local reproduction alone does not satisfy R30.
- Verify every runtime setting is loaded once through typed startup configuration;
  update `.env.example`, Compose, and README together. Production must not depend
  on a checked-in or local `.env` file.
- Before public release, obtain the project owner's explicit license choice and
  add the matching `LICENSE`; do not invent a legal/license decision.
- Load/soak/failure tests for API, MCP, workers, PostgreSQL, object storage, and
  model runtime with explicit SLOs and capacity limits.
- Independent correctness, security, operational, and documentation review.

## P13 — final ledger reconciliation

This is an evidence audit, not a feature batch. Read every R1-R33 row, every R3
invariant, the R31 test matrix, R32 metrics, and R33 scenarios in
`docs/REQUIREMENT_LEDGER.md` against the settled source and the three root target
documents.

- For each mandatory clause, cite its production code path, database/migration
  invariant where applicable, deterministic test, and latest stable receipt.
- Change a row to `VERIFIED` only when all clauses in that row have evidence.
  Split mixed rows rather than hiding an unmet clause behind a passing one.
- Keep optional and conditional items explicitly classified with the exact root
  wording and the evidence showing that no current implementation is required.
- Search for capabilities implemented without a mandatory root trace. Remove
  dead or speculative code when safe; otherwise mark and isolate it from the
  supported surface until a separate authorized decision is made.
- Re-run the six scenarios, security/operations rubric, fresh-install and upgrade
  tests, and independent reviews on the same stable source manifest.
- Reconcile README, current implementation docs, operations material, OpenSpec,
  and `STATE.md`. No document may claim a feature from planned types, migrations,
  mocks, skipped tests, absent scanners, or historical receipts.

## Batch stop rule

A batch closes only when its root trace, code, database changes, focused tests,
four full Cargo gates, stable receipt, independent review, docs, ledger, state,
and OpenSpec record agree. Do not merge two dependency stages merely to move
faster. Do not start an adjacent stage while the current one leaves a known
Critical/High defect or a broken build.

The whole task stops only after P13 finds no mandatory `GAP`, `PARTIAL`, or
`VERIFY_PENDING` clause. A green build, a completed OpenSpec batch, or a security
scan by itself is never the stop condition.
