# Query and operability batch contract

Root ARCHITECTURE.md, REQUIREMENTS.md and README.md remain target authority.
Scope: validated typed query execution with scoped graph-semantic links and
real temporal transition history, plus full local Compose and CI packaging.
Passing this batch advances the active whole-architecture goal.

- [x] Define typed query input/output contract; reject SQL and unknown authority fields.
- [x] Execute exact counts and jurisdiction groups in PostgreSQL before response limits.
- [x] Add immutable, scope-checked canonical knowledge/entity associations and final retrieval revalidation.
- [x] Integrate common service and protected HTTP query adapter.
- [x] Separate temporal snapshots from recorded/validity transitions; expose truncation.
- [x] Validate startup configuration without leaking credentials or malformed values.
- [x] Verify fictional graph, semantic, hybrid, temporal, provenance and isolation scenarios against PostgreSQL.
- [x] Verify multistage images, non-root runtime, full Compose startup and MCP handshake in an isolated test project.
- [x] Run four full Cargo gates and implementation-loop Python tests on stable settled sources.
- [x] Obtain independent artifact review; fix all unresolved findings and rerun appropriate gates.
- [x] Update implementation docs, requirement ledger, README and STATE with scoped evidence and remaining gaps.
- [x] Apply OpenSpec; advance to next unmet architecture requirement.

## Ownership and changed paths

Query maker owns crates/domain/src/query.rs, crates/domain/src/lib.rs,
crates/storage/src/query.rs, crates/storage/src/lib.rs,
migrations/0009_query_graph_links.sql and crates/storage/tests/query_plans.rs.
Root owns crates/services/src/query.rs, crates/services/src/lib.rs,
crates/services/src/knowledge.rs, crates/services/src/institutional.rs,
crates/services/tests/typed_queries.rs,
apps/api/src/handlers.rs, apps/api/src/router.rs, apps/api/src/main.rs,
apps/api/tests/http_api.rs, crates/runtime/src/lib.rs, .env.example, README.md,
docs/ARCHITECTURE.md, docs/REQUIREMENTS.md, docs/REQUIREMENT_LEDGER.md,
scripts/implementation_cycle.py, scripts/tests/test_implementation_cycle.py and STATE.md.
Operability maker owns Dockerfile, .dockerignore, docker-compose.yml,
.github/workflows/ci.yml and docs/OPERATIONS.md.

## Acceptance limits

Exact aggregates use all authorized matches. Bounded semantic/entity lists and
recorded transition pages must explicitly expose limits or has_more, never imply
exhaustive monthly histories from endpoint differences. PostgreSQL aggregation
and traversal cannot be replaced by model-generated SQL. Semantic quality is
not established by the deterministic embedding adapter. CI configuration is
locally checked; no hosted CI execution is implied. Existing database containers
and data are preserved; Docker smoke uses a unique project, containers and volumes.
Source authority, ownership/principal policy, artifacts, agents, Experience Store,
connectors, outbox and complete acceptance evaluations remain separate unmet work.

## Applied Updates

### 2026-09-16T08:31:42Z
- (no local file changes detected)
