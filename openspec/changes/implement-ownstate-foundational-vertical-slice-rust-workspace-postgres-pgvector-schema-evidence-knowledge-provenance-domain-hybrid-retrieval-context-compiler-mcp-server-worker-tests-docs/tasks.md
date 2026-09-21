- [x] Rust workspace + toolchain + compose + env scaffolding
- [x] Domain crate: ids (UUIDv7), enums, entities, limits, hashing, promotion rules (13 unit tests)
- [x] Migrations 0001–0004: schema + append-only/immutability triggers + FTS + HNSW
- [x] Storage crate: repositories, tx primitives, disposable-DB test harness
- [x] Embeddings crate: provider trait, deterministic + fastembed (feature-gated)
- [x] Services crate: ingestion, proposal, promotion tx, hybrid RRF search, context compiler, bootstrap, jobs
- [x] apps/api (Axum, bearer auth, error mapping), apps/worker, apps/mcp (rmcp stdio)
- [x] Tests: 44 passing (schema invariants, lifecycle, retrieval/context, HTTP, MCP round-trip)
- [x] Gates: cargo fmt --check, cargo check --workspace --all-targets, clippy (0 warnings), cargo test --workspace
- [x] Live acceptance run: project → session → events → propose → promote → worker embeds (MiniLM) → hybrid + semantic search → context compile → v1 SUPERSEDED / v2 ACTIVE → MCP third-agent retrieval
- [x] Docs: README.md, docs/ARCHITECTURE.md, docs/VISION.md, docs/RULES.md
- [x] Independent adversarial review round 1 → 4 findings (C1 classification ceiling caller-chosen + get_knowledge history leak; M1 client-supplied proposed_by mints HUMAN_EXPLICIT + same token promotes; M2 sequence poisoning via i64::MAX)
- [x] Fixes: server-side classification ceiling (OWNSTATE_MAX_CLASSIFICATION, clamped on every read path incl. get_knowledge; REVOKED/QUARANTINED content withheld), curation credential (OWNSTATE_ADMIN_TOKEN gates promote/reject + HUMAN attribution), bounded sequence gaps (MAX_SEQUENCE_GAP + checked arithmetic)
- [x] Regression tests added (clamping, history filtering, curation gating, sequence guards); all gates green again
- [x] Verification round 2: fixes A-D all EFFECTIVE; 5 new findings (N1 evidence-read ceiling gap; N2 stranded RUNNING jobs; N3 promotion race → 500; N4 query text in trace spans; N5 misleading auth warning) — ALL FIXED with gates re-run green (48 tests, clippy 0)

## Applied Updates

### 2026-09-01T16:01:13Z
- (no local file changes detected)

## Applied Updates

### 2026-09-01T16:15:01Z
- (no local file changes detected)

## Applied Updates

### 2026-09-01T16:25:01Z
- (no local file changes detected)
- [x] Cutover: dev DB migrated brew→docker compose Postgres (pg_dump/pg_restore, all rows + migration state); brew PG stopped
- [x] MCP registered with Claude Code (user scope) via scripts/ownstate-mcp.sh — claude mcp list: Connected

## Applied Updates

### 2026-09-01T17:57:04Z
- (no local file changes detected)
- [x] MCP registered with Codex CLI (global) — live verified: codex exec called bootstrap_project and retrieved the demo knowledge. Milestone 6 proof complete: two different MCP clients (Claude Code + Codex) retrieve the same project knowledge

## Applied Updates

### 2026-09-01T18:04:59Z
- (no local file changes detected)
