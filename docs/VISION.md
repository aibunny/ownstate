# VISION — Ownstate v0.1 foundational vertical slice

## Goal (machine-checkable definition of done)

The implementation is done when ALL of the following hold:

1. `cargo fmt --check` is clean across the workspace.
2. `cargo check --workspace` succeeds.
3. `cargo clippy --workspace --all-targets` reports no warnings that we have not
   explicitly justified in code.
4. `cargo test --workspace` passes, including integration tests against a real
   PostgreSQL 18 + pgvector instance, covering:
   - event persistence + ordering
   - evidence immutability (DB-enforced)
   - candidate persistence + deterministic promotion
   - knowledge versioning (supersede, never overwrite)
   - provenance retrieval
   - project-scoped + classification-scoped hybrid retrieval
   - revoked/superseded knowledge exclusion
   - context packet creation + max_items budget
   - MCP tool behavior via a real in-process MCP client/server round-trip
5. The end-to-end flow works against the running stack:
   create project → create session → append events → propose candidate
   (referencing those events) → promote to canonical → embedding job processed →
   hybrid search finds it → `POST /context/compile` returns it with provenance →
   a ContextPacket row is recorded → the same knowledge is retrievable through
   the MCP server.
6. The versioning demo works: promote v1, promote an update for the same
   (kind, subject) → v1 = SUPERSEDED, v2 = ACTIVE, both retain evidence.
7. `README.md` and `docs/ARCHITECTURE.md` describe what was actually built.

## Non-goals for this slice

LLM extraction, novelty engine, OpenRouter, provider adapters, org RBAC, object
storage, contradiction auto-resolution, dashboards. Leave clean seams only.

## Architectural invariants (never compromise)

- PostgreSQL is canonical; pgvector is an index.
- Raw evidence / candidate knowledge / canonical knowledge are separate concepts.
- Raw evidence is append-only (DB triggers enforce it).
- Canonical knowledge is versioned; content of a written version is immutable.
- Every canonical version supports provenance.
- Extraction/models never write canonical state directly; deterministic Rust
  promotion logic is the only path.
- Authorization (tenant + project scope, classification ceiling) before retrieval.
- Parameterized SQL only. No secrets in logs or source.
