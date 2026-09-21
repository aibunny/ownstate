# Ownstate implementation state

## Active authority

- Target architecture: `docs/ARCHITECTURE.md`
- Target requirements: `docs/REQUIREMENTS.md`
- Automatic-capture requirements: `docs/AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`
- Automatic-capture execution map: `docs/AUTOMATIC_CAPTURE_IMPLEMENTATION_PLAN.md`
- Mandatory feature security: `docs/MCP_CAPTURE_SECURITY.md`
- Runnable OpenCode prompt: `OPENCODE_CAPTURE_SCOPE_PROMPT.md`
- Files under `docs/archive/` are historical and not active authority.

## Current verified baseline (2026-09-21)

- `cargo fmt --check` passed;
- `cargo check --workspace --all-targets` passed;
- `cargo clippy --workspace --all-targets -- -D warnings` passed;
- `cargo test --workspace` passed 130+ tests with zero failures;
- `python3 -m unittest discover -s scripts/tests -v` passed 10 tests;
- OpenSpec change `implement-automatic-capture-and-scope-resolution` created.

## Automatic capture progress

### Completed (Phases 0–2, partial 3–6)

**Phase 0 — Baseline and reuse audit**
- Full baseline established: 130+ Rust tests, 10 Python tests, all gates green.
- Gap analysis produced: 51 rows across capture, scoping, resolution, repository
  normalization, security, and integration.
- Historical OpenSpec changes audited and reused where correct.

**Phase 1 — Closed domain contracts**
- Added `CaptureMode` enum (NATIVE_COMPLETE, PARTIAL_ADAPTER, MCP_ONLY, IMPORTED,
  UNKNOWN) with `is_definitively_complete()` — MCP_ONLY never reports complete.
- Added `ScopeKind` enum (ORGANIZATION, PROJECT, REPOSITORY, RELATIONSHIP, ARTIFACT,
  THREAD, PERSONAL).
- Added `ResolutionState` enum (EXACT_STABLE, KNOWN_ALIAS, PROBABLE_ASSOCIATION,
  PROVISIONAL_SCOPE, GENERAL_SCOPE).
- Added `ChannelCompleteness` struct with 12 tracked channels, `mcp_only()` factory,
  `all_complete()`, `completed_count()`.
- Added `ScopeHandle`, `RepositoryLocator`, `RepositoryProvider`, `CaptureAssessment`,
  `ResolvedScope`, `ResolutionAnchor`, `ResolutionAnchorType`, `BootstrapRequest`,
  `BootstrapResponse`, `CompactContext` types.
- Added `ScopeHandleId`, `CaptureAssessmentId` ID types.
- Pure tests: MCP_ONLY never reports complete, native complete is definitive, scope
  handle usability (revoked/expired), channel completeness counting.

**Phase 2 — Repository identity and durable schema**
- Pure remote normalization (`normalize_remote`): SCP-style SSH, SSH URL, HTTPS, local
  paths. Credential stripping, terminal .git removal, host normalization, control
  character rejection, traversal prevention.
- Provider detection: GitHub, GitLab, Bitbucket, SelfHosted, Unknown.
- Internal key computation via BLAKE3 content hash.
- Tests: GitHub HTTPS/SSH/SSH-URL equivalence, credential stripping, trailing .git
  removal, control character rejection, traversal rejection, provider detection.
- Migration 0012 applied: extended `repositories` with provider, owner_name, repo_name,
  internal_key, fork_of_repository_id, metadata. Added `repository_aliases`,
  `scope_handles`, `capture_assessments`, `recording_idempotency` tables with RLS,
  append-only triggers, and unique indexes.
- Migrations 0007–0011 applied to running database (were previously missing).

**Phase 3 (partial) — Storage layer for scope resolution**
- `repositories.rs`: `upsert_repository` (transactional upsert by internal key),
  `find_by_canonical_origin`, `find_by_internal_key`, `insert_alias`.
- `scope_handles.rs`: `insert_handle`, `find_handle` (with expiry/revocation check),
  `revoke_handle`, `find_handle_for_scope`.
- `capture.rs`: `insert_assessment`, `latest_for_session`, `insert_idempotency_key`,
  `find_idempotency_key`.

**Phase 5 — MCP ownstate.record tool**
- Added `RecordParams` schema (event_type, actor_type, source_event_id, content,
  repository, branch, commit_sha, file_path, tool_name, metadata, etc.).
- Added `record` tool: creates session, appends event as raw evidence via
  `PrincipalServices`, returns event_id and session_id. No promotion, no canonical
  state changes, no deletion capability.

### Not yet implemented

- **Phase 3 (service layer)**: `ScopeResolver` service method on `PrincipalServices`
  that orchestrates repository upsert, project association, scope handle creation,
  and bootstrap response.
- **Phase 4**: Non-code and multi-scope continuity (thread, artifact, relationship,
  organization scopes).
- **Phase 5 (complete)**: Full bootstrap with scope handles and compact context.
- **Phase 6 (complete)**: Capture adapter/source boundary (the recording_idempotency
  table exists; adapter abstraction is not yet formalized).
- **Phase 7**: Verified host integrations and capability matrix.
- **Phase 8**: Bounded progressive context with Layer 0/1/2/3.
- **Phase 9**: Production hardening, adversarial tests, documentation reconciliation.

## Files changed

### New files
- `crates/domain/src/capture.rs` — CaptureMode, ScopeKind, ResolutionState,
  ChannelCompleteness, ScopeHandle, RepositoryLocator, CaptureAssessment, etc.
- `crates/domain/src/remote.rs` — Pure repository remote normalization.
- `crates/storage/src/repositories.rs` — Repository identity storage.
- `crates/storage/src/scope_handles.rs` — Scope handle storage.
- `crates/storage/src/capture.rs` — Capture assessment and idempotency storage.
- `migrations/0012_automatic_capture_and_scope_resolution.sql` — Schema migration.
- `openspec/changes/implement-automatic-capture-and-scope-resolution/` — OpenSpec.

### Modified files
- `crates/domain/src/lib.rs` — Added capture, remote modules and re-exports.
- `crates/domain/src/enums.rs` — Added CaptureMode, ScopeKind, ResolutionState,
  ChannelCompleteness.
- `crates/domain/src/ids.rs` — Added ScopeHandleId, CaptureAssessmentId.
- `crates/domain/src/error.rs` — Added InvalidRemoteUrl variant.
- `crates/storage/src/lib.rs` — Added capture, repositories, scope_handles modules.
- `apps/mcp/src/server.rs` — Added ownstate.record tool and RecordParams.
- `apps/mcp/Cargo.toml` — Added chrono dependency.

## Known prerequisites and open risks

- Non-owner runtime-role and meaningful RLS/static-scope evidence remains open.
- Model-egress authorization must remain held through the actual provider call;
  a post-call denial is not prevention.
- Complete ContextPacket principal/authorization/exact-delivery audit remained
  incomplete in the last review.
- The ScopeResolver service method (Phase 3 service layer) needs implementation
  to complete the bootstrap flow.
- Host capture adapters (Phase 7) remain unimplemented — only the generic MCP
  recording path exists.
- MCP must not gain any delete, purge, erase, raw SQL, force-canonical, grant,
  policy, credential, retention, or permission-mutation capability.
- The existing `ownstate-postgres` service and named volume contain developer
  state and must not be deleted. Use disposable databases and isolated Compose
  projects for tests.

## Next implementation steps

1. Implement `ScopeResolver` service method on `PrincipalServices` (Phase 3).
2. Implement full bootstrap with scope handles (Phase 5 complete).
3. Add integration tests for repository normalization, concurrent bootstrap,
   idempotency, and scope handle security.
4. Run adversarial security tests from `MCP_CAPTURE_SECURITY.md`.
5. Reconcile documentation with demonstrated behavior.
