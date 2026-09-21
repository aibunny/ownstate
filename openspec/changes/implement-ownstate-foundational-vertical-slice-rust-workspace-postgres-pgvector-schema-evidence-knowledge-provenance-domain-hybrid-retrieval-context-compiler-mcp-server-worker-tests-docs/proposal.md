## Why

- Ownstate had no implementation. The foundational vertical slice from
  docs/REQUIREMENTS.md (§45–46) must exist before any automated learning,
  adapters or business features: evidence capture → candidate knowledge →
  canonical versioned knowledge → hybrid retrieval → context compilation →
  MCP retrieval, with provenance and versioning correct from day one.

## What Changes

- New Rust workspace (edition 2024): crates/domain, crates/storage,
  crates/embeddings, crates/services, crates/runtime; binaries apps/api
  (Axum), apps/worker (PG job queue), apps/mcp (rmcp stdio).
- PostgreSQL 18 + pgvector schema (migrations/0001–0004): projects, sessions,
  append-only interaction_events, candidate_knowledge, knowledge_items,
  immutable knowledge_versions (+ one-ACTIVE partial index), append-only
  knowledge_evidence, embeddings index, append-only context_packets, jobs.
  Append-only/immutability enforced by triggers.
- Deterministic promotion rules (domain), hybrid FTS+vector retrieval with
  RRF and trust weighting, deterministic context compiler with packet audit,
  embedding jobs via FOR UPDATE SKIP LOCKED.
- MCP tools: bootstrap_project, search_knowledge, get_knowledge,
  propose_knowledge (candidates only, AGENT-forced).
- 44 tests (unit + integration against real PostgreSQL + MCP wire round-trip),
  README.md, docs/ARCHITECTURE.md, docs/VISION.md, docs/RULES.md,
  docker-compose.yml, .env.example.

## Impact

- Establishes the production baseline schema and domain boundaries all future
  versions build on. Local infra: docker compose OR Homebrew postgresql@18
  (used here because the host's Docker Desktop content store is corrupted).
