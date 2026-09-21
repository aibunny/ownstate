- [x] Establish passing four-gate baseline (62 Rust tests).
- [x] Define typed institutional proposals/candidates/entity/assertion versions with confidence and valid/observed/recorded time.
- [x] Add scoped additive PostgreSQL migration, immutable history/provenance, pending-candidate canonical guard, endpoint checks and classification floor.
- [x] Implement shared proposal/promotion, exact external identifier/alias resolution, scoped temporal snapshots/history and relationship filtering before limits.
- [x] Preserve uncertainty, stable identifiers, duplicate trust and human current state; audit confirming candidates and reject restricted-identity downgrade.
- [x] Add HTTP routes/admin curation and MCP entity/relationship/context tools through shared services.
- [x] Add real PostgreSQL regressions, HTTP curation test, MCP wire coverage and pure proposal-authority tests.
- [x] Expand whole-architecture ledger and stop condition per explicit user instruction.
- [x] Four full workspace gates on settled current source:80Rust tests, stable final receipt finished2026-09-15T16:30:06.535450+00:00.
- [x] Independent substantive review and corrected documentation acceptance;15institutional tests independently rerun, all five findings resolved. Prior capacity errors did not block the fresh reviewer.
- [x] State/OpenSpec write-back; apply receipt follows.

Acceptance: scoped institutional batch COMPLETE. Python runner regressions10/10
also passed. The runner correctly leaves batch_complete=false because reviewed
acceptance is recorded here, not inferred from Cargo. No broad product completion.

Changed paths (explicit because this workspace has no Git metadata):
crates/domain/src/ids.rs, crates/domain/src/lib.rs, crates/domain/src/institutional.rs,
crates/storage/src/lib.rs, crates/storage/src/institutional.rs,
crates/services/src/lib.rs, crates/services/src/institutional.rs,
crates/services/tests/institutional_state.rs,
migrations/0008_institutional_state.sql,
apps/api/src/lib.rs, apps/api/src/handlers.rs, apps/api/src/router.rs,
apps/api/src/institutional.rs, apps/api/tests/http_api.rs,
apps/api/Cargo.toml, Cargo.lock,
apps/mcp/src/server.rs, apps/mcp/tests/mcp_roundtrip.rs,
README.md, docs/ARCHITECTURE.md, docs/IMPLEMENTATION_LOOP.md,
docs/REQUIREMENT_LEDGER.md, docs/REQUIREMENTS.md, STATE.md.

Product scope is the full root architecture/requirements. This batch contributes
institutional foundations; typed exact counts/query plans, reversible merges,
type-dependent authority and the rest of the ledger remain required next work.
No commit/push or live canonical force-write.

## Applied Updates

### 2026-09-15T16:30:58Z
- (no local file changes detected)
