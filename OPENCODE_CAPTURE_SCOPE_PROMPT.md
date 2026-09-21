# OpenCode implementation prompt — automatic capture and scope resolution

Work in the existing Ownstate repository. This is an implementation task, not a
greenfield design exercise. Multiple agents have already changed this codebase.
Inspect and preserve correct behavior; do not start over.

## Read before editing

Read these files in order:

1. `README.md`
2. `docs/ARCHITECTURE.md`
3. `docs/REQUIREMENTS.md`
4. `docs/AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`
5. `docs/AUTOMATIC_CAPTURE_IMPLEMENTATION_PLAN.md`
6. `docs/MCP_CAPTURE_SECURITY.md`
7. `docs/HOST_CAPTURE_CAPABILITY_MATRIX.md`
8. `docs/RULES.md`
9. `docs/IMPLEMENTATION_LOOP.md`
10. `docs/OPERATIONS.md`
11. `STATE.md`
12. the Cargo workspace, migrations, domain/storage/service boundaries, API,
    MCP server, evidence ingestion, project/workspace resolution, context
    compiler, retrieval, entity resolution, runtime adapters, tests, Docker/CI,
    and `.env.example`

Files under `docs/archive/` are historical records. Do not use them as target
requirements or current evidence.

The normative target is `docs/ARCHITECTURE.md`, `docs/REQUIREMENTS.md`, and
`docs/AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`. The security acceptance contract
is `docs/MCP_CAPTURE_SECURITY.md`.

## Required working method

1. Run the existing formatting, compile, lint, test, and PostgreSQL integration
   checks before modifying source. Record exact results; do not reuse historical
   counts as a current baseline.
2. Audit what already exists, including the historical OpenSpec changes for
   workspace/project resolution and multi-factor project identity.
3. Produce a short evidence-backed gap analysis. For every requested behavior,
   mark it implemented, partial, missing, or conflicting and cite source/tests.
4. Create a new implementation OpenSpec change with `./opsx propose` before code
   edits. Do not reuse the documentation-handoff change as implementation scope.
5. Execute `docs/AUTOMATIC_CAPTURE_IMPLEMENTATION_PLAN.md` in dependency order.
6. Implement the smallest compatible changes. Do not rewrite accepted evidence,
   knowledge, temporal, query, context, policy, or adapter foundations.
7. Add real PostgreSQL tests for constraints, concurrency, idempotency, RLS, and
   append-only behavior. Use unit tests for pure normalization and domain logic.
8. Run focused checks while iterating, then all stable gates. Obtain an
   independent correctness/security review when the environment supports it.
9. Update README, architecture, requirements, operations, host capability matrix,
   `.env.example`, `STATE.md`, and OpenSpec with demonstrated behavior only.
10. Keep working until every acceptance case in the feature requirements and
    security contract passes. Do not stop after planning, schema creation, happy
    paths, or a green compile.

## Product outcome

After a user connects Ownstate MCP, the normal experience is:

```text
Use Codex / Claude / OpenCode / another host normally
        ↓
Ownstate resolves durable scope from observable evidence
        ↓
Ownstate returns a compact authorized context bootstrap
        ↓
Observable work enters append-only evidence
        ↓
The existing knowledge pipeline proposes and updates institutional knowledge
```

The normal MCP path must not require a project UUID or manual project creation.
Models provide observable context and proposals. Ownstate resolves identity,
scope, authorization, idempotency, novelty, canonical promotion, and freshness.

## Non-negotiable security boundary

An MCP client or model must never be able to delete, purge, erase, rewrite, or
force-promote recorded Ownstate data. Do not expose deletion, grant mutation,
policy mutation, credential management, raw SQL, arbitrary filesystem access, or
canonical force-write through MCP.

`ownstate.record` appends authorized evidence. It does not update canonical
truth. Legitimate retention or data-subject deletion, if supported, belongs to a
separate explicitly authenticated administrative plane with policy, audit,
review, and database safeguards. It is never reachable through model-controlled
MCP tools or scope handles.

Scope handles are opaque references, not credentials or authority. Resolve the
authenticated principal server-side on every call and re-authorize the referenced
scope. Request fields, Git metadata, working directories, client names, provider
conversation IDs, and handles cannot choose a tenant or grant permissions.

Preserve the existing append-only evidence and immutable knowledge-version
database protections. Never weaken a trigger or authorization boundary to make a
test pass. Follow every control and adversarial test in
`docs/MCP_CAPTURE_SECURITY.md`.

## Scope limits

- Do not implement every host adapter speculatively. Verify actual host surfaces,
  record evidence and limitations in the capability matrix, and implement only
  integrations that are available and testable.
- MCP context integration is not proof of complete transcript capture. Persist
  capture mode and per-channel completeness. Never label MCP-only observation as
  full capture.
- Do not invoke a model on the deterministic bootstrap fast path.
- Do not use semantic similarity alone for destructive entity, repository,
  thread, scope, or project merges.
- Do not equate projects with repositories, branches, directories, provider
  sessions, model names, or chat titles.
- Do not add proprietary infrastructure, a new database, a graph database, a
  broker, or a plugin framework for this feature.
- Do not commit, push, publish, deploy, read secret `.env` values, or delete
  persistent volumes without explicit authorization.

## Required final evidence

The final report must include changed paths, migration behavior, exact focused
and full test results, concurrency/idempotency evidence, cross-model repository
continuity, non-code provisional-scope behavior, capture-completeness behavior,
bounded/fresh context behavior, and every destructive-MCP denial test. It must
also list host capture limits honestly and identify any unavailable external
integration as unverified rather than passed.

