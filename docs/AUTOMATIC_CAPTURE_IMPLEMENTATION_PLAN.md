# Automatic capture and scope-resolution implementation plan

This is an execution map. Normative behavior lives in
`AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`; security acceptance lives in
`MCP_CAPTURE_SECURITY.md`.

For each phase, create an OpenSpec batch contract listing exact requirement
clauses, current source evidence, changed paths, migration impact, focused tests,
full gates, and explicit non-goals. Do not assume the suggested type names or
file locations are correct before auditing existing code.

## Phase 0 — baseline and reuse audit

- Run the current full gates and real PostgreSQL suites.
- Inspect historical OpenSpec changes for workspace resolution and multi-factor
  identity. Map every existing implementation path and test.
- Audit unresolved authorization work recorded in `STATE.md`. This feature must
  not build on a public/unscoped service bypass or model-egress gap.
- Produce the requirement-by-requirement gap table and a migration compatibility
  plan. Preserve all existing IDs, evidence, canonical versions, and provenance.
- Verify the existing developer database is preserved; use disposable databases
  for migration/concurrency tests.

Exit: current baseline recorded, reuse decisions cited, implementation OpenSpec
created, and no proposed change duplicates accepted behavior.

## Phase 1 — closed domain contracts

- Add closed typed capture mode/completeness, scope kind, scope evidence,
  resolution result/state, repository locator/alias, and opaque handle records.
- Keep provider, Git, MCP, SQLx, Axum, and runtime-specific types out of domain.
- Define deterministic precedence and ambiguity behavior. Models cannot mint
  exact identity or authority.
- Add pure tests for state transitions, completeness invariants, and forbidden
  full-capture claims.

Exit: domain rules compile independently and cannot represent MCP-only as full.

## Phase 2 — repository identity and durable schema

- Implement pure safe remote parsing/normalization with credential redaction.
- Add forward-only migrations for repository identity, aliases, fork/upstream
  lineage, project association, provisional identity, context scopes, handles,
  capture assessments, adapter/source sessions, and idempotency keys only where
  the audit proves existing tables cannot support them.
- Add ownership-domain composite keys, immutable/history constraints, bounded
  fields, and uniqueness for deterministic identities.
- Use transactional upserts and test concurrent creation in real PostgreSQL.
- Migrate existing project metadata additively without deleting or re-keying
  evidence/history.

Exit: equivalent remotes converge, forks stay distinct, provisional upgrade is
history-preserving, and concurrency cannot duplicate exact identities.

## Phase 3 — resolver and automatic project association

- Implement the provider-neutral resolver in application services using storage
  ports and deterministic rules.
- Add local Git evidence collection at the host/adapter boundary. Never make the
  remote MCP server inspect an arbitrary client-provided filesystem path.
- Resolve or create a repository and default project idempotently. Keep project
  and repository distinct.
- Record branch, commit, and working-tree state as session/evidence context.
- Add Personal/Organization ownership and explicit principal authorization to
  every resolver load/create operation.

Exit: cross-clone/worktree/branch/model acceptance passes without project IDs.

## Phase 4 — non-code and multi-scope continuity

- Add general, thread, artifact, relationship, project, repository, and
  organization scopes using existing entity/artifact/thread models where present.
- Implement deterministic non-code anchors first and provisional fallback.
- Use entity/relationship/semantic signals only as bounded association evidence.
- Add auditable authorized consolidation/linking that preserves original scopes
  and provenance; no destructive semantic-only merge.

Exit: ambiguous work remains safe, and the fundraise/deck/investor continuity
fixture associates across hosts without a user-managed UUID.

## Phase 5 — secure MCP bootstrap and compact intent tools

- Add or evolve bootstrap so observable evidence replaces project IDs on the
  normal path. Return an opaque non-authoritative scope handle and compact Layer
  0/1 context.
- Reuse compatible search/context/propose/feedback tools. Add generic append-only
  record/artifact intent tools only where missing.
- Publish concise MCP instructions and tool descriptions.
- Authenticate and authorize every call through principal-scoped services.
- Remove/deprecate model-facing implementation tools only through a documented
  compatibility plan; do not break existing clients silently.
- Do not expose any destructive or administrative operation.

Exit: MCP clients operate without project IDs and all security-contract tests
pass, including absence and denial of deletion paths.

## Phase 6 — normalized capture and idempotency

- Implement the capture adapter/source boundary using the existing normalized
  event model.
- Persist source session/event locators, capture assessment, per-channel
  completeness, and idempotency.
- Import/replay the same transcript without duplicates while preserving distinct
  same-content events.
- Feed evidence into the existing asynchronous proposal pipeline. Do not let an
  adapter or model write canonical state.

Exit: native/import/MCP-only behavior is factual, replay-safe, and provenance
complete.

## Phase 7 — verified host integrations

- Complete `HOST_CAPTURE_CAPABILITY_MATRIX.md` from primary documentation and
  local tests for each named host/version.
- Implement only stable accessible adapters. Keep Generic MCP and Import fallback
  behavior explicit.
- Add fixture/contract tests shared by every adapter.
- Never claim a channel or host is fully captured without proof.

Exit: every shipped adapter has an evidence-backed capability row and contract
test; unavailable adapters remain documented, not stubbed as working.

## Phase 8 — bounded progressive context

- Compile Layer 0/1 bootstrap through the existing retrieval/context services.
- Add Layer 2 search and authorized Layer 3 evidence drill-down.
- Enforce total serialized token/item limits, scope, classification, freshness,
  authority, deduplication, and audit of exact delivered records.
- Cache only safe stable context with authorization/freshness invalidation.

Exit: large-history budget, deduplication, freshness, leakage, and audit-equals-
delivery tests pass.

## Phase 9 — production hardening and closure

- Run all adversarial cases in `MCP_CAPTURE_SECURITY.md`.
- Test fresh migration and upgrades from every supported existing boundary.
- Test restart, concurrency, replay, malformed input, rate limits, oversized
  payloads, provider outage, and partial adapter failure.
- Run format/check/all-target check/strict Clippy/workspace tests/Python loop and
  isolated Compose smoke on one stable manifest.
- Run current dependency advisory, license, secret, and final-image scans with
  versions/database timestamps; missing mandatory evidence blocks production.
- Obtain independent correctness and security reviews and repair findings.
- Reconcile README, architecture, requirements, operations, capability matrix,
  `.env.example`, `STATE.md`, and OpenSpec with actual behavior.

Exit: every normative acceptance case passes and no unsupported capture or
security claim remains.

