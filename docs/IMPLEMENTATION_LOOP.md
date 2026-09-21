# Ownstate implementation loop

## Goal contract

- **Goal:** fulfill every applicable requirement in [ARCHITECTURE.md](ARCHITECTURE.md), [REQUIREMENTS.md](REQUIREMENTS.md), [AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md](AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md), and root [README.md](../README.md), with implementation and acceptance-test evidence, while preserving raw evidence → candidate knowledge → canonical knowledge → retrieval/context → agent boundaries, history, provenance, and deterministic authorization. A scoped batch is a unit of progress, not the product stop condition.
- **Context:** the active target documents are under `docs/`; `docs/archive/` is historical only. Read the target documents, `STATE.md`, the current OpenSpec proposal, repository rules, and relevant code/tests before each cycle. Read `.env.example`, never secret `.env` files.
- **Actions:** establish the baseline, record intent with `./opsx propose`, and write a batch contract that cites the exact mandatory root clause, demonstrated implementation gap, smallest satisfying behavior, changed paths, acceptance tests, and explicit non-goals. Reject work justified only by examples, optional/future language, existing speculative code, or general best practice. Then make the smallest tested batch, repair failed verification, obtain independent review, update documentation/state, and record `./opsx apply`. No commit or push without explicit authorization. Do not remove evidence immutability or canonical versioning safeguards, expose unrestricted SQL, or add mandatory optional infrastructure.
- **Feedback:** the four fixed workspace checks, regression tests for the batch, an independent checker given the artifact and acceptance rubric, and a documentation review.
- **Stop:** every applicable target clause and every feature/security acceptance case has direct implementation and evidence, all checks pass on stable implementation files, no unresolved substantive independent findings remain, documentation describes demonstrated behavior, and the OpenSpec receipt and `STATE.md` record the result. Explicit non-goals and optional future deployments require source-backed classification rather than placeholder implementation. Cargo success alone never completes a batch or the whole target architecture. A passing batch advances to the next dependency-ordered gap. If a review is externally blocked, its acceptance stays pending while other authorized implementation proceeds.

## Loop

- **Trigger:** immediate execution or a manual command in the current task. No scheduler is installed.
- **State:** consult `STATE.md` at discovery; write verified facts, open failures, rejected attempts, and the next cycle entry point before stopping. Workflow records remain separate from tested implementation files.
- **Roles:** maker performs the scoped change; checker tries to refute acceptance using the artifact and rubric; documentation review confirms setup and capability claims.
- **Failure/block handling:** a nonzero Cargo result is a failed verification gate pending triage, not proof of a code defect. Determine whether it is an implementation failure or a confirmed external block before retrying. Missing Cargo and deterministic unsafe/missing filesystem/change metadata are recorded as blocked. Do not infer blocks by pattern matching arbitrary output, skip tests, or blindly retry. For confirmed network, permission, dependency, or PostgreSQL blocks, manually record a sanitized exact diagnostic and next viable route in `STATE.md`; obtain required approval or restore the dependency before resuming.
- **Memory write-back:** permanent rules require reproduction and verification. Keep guesses under open failures. Use explicit changed paths in OpenSpec because this checkout may lack Git; `opsx apply` alone does not enumerate untracked files or prove verification.

## Fixed verification runner

Python 3's standard library is sufficient:

```bash
python3 scripts/implementation_cycle.py --phase baseline
python3 scripts/implementation_cycle.py --phase verify --change-id your-change-slug
python3 -m unittest discover -s scripts/tests -v
```

Baseline must pass before changing implementation. A previously established baseline can be documented in `STATE.md`; the runner does not require inventing a retrospective source snapshot. Verify requires an existing `openspec/changes/<slug>` directory; omitting `--change-id` resolves `.opsx-current`. Slugs permit lowercase letters, digits, and single hyphen separators.

The runner executes these commands in order, stopping at the first nonzero result:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The Clippy command adds a stricter warning failure gate to the documented workspace command. There is no arbitrary command override. Console output stays in the console; receipts contain no raw logs, environment values, credentials, or content.

Integration tests use real PostgreSQL + pgvector, create disposable migrated databases, and require a role permitted to create databases. Start the current local dependency with `docker compose up -d postgres`. The harness defaults to the fictional local credentials in `.env.example`; override `OWNSTATE_TEST_DATABASE_URL` when needed. It opportunistically drops internally named test databases older than one hour. A database outage cannot count as a passing or skipped integration gate.

## Receipt and stop semantics

The runner atomically writes `target/implementation-loop/latest.json`, rejecting symlinks in the receipt directories or destination. Exit statuses are:

| Receipt status | Exit code | Meaning |
| --- | --- | --- |
| `checks_passed` | 0 | All fixed gates passed on stable implementation files; independent and documentation review remain pending. |
| `failed` | 1 | A gate returned nonzero, or implementation files changed during checks; triage is required. |
| `blocked` | 2 | A required tool, safe receipt path, change metadata, or filesystem operation is unavailable. If the path is unsafe, no receipt is written there. |
| `interrupted` | 130 | Keyboard interruption; unfinished gates have no passing evidence. |

Receipts include timestamps, phase/change id, fixed command exit codes, explicit next action, source hash manifest, optional baseline comparison, source stability, and pending review gates. `batch_complete` always remains false: completion is a separate reviewed decision recorded in OpenSpec and state.

The source manifest hashes a whitelist of application/crate sources, migrations, docs, scripts, CI, and root source/docs/config files using SHA-256. It includes root `.env.example` only; excludes secret `.env` files, symlinks, caches, attachments, build output, `STATE.md`, `.opsx-current`, and OpenSpec workflow records. This is a tested implementation snapshot, not a complete changed-file inventory. Workflow write-back after verification does not alter that snapshot.

A baseline run preserves its manifest through later verify receipts. Without a recorded baseline manifest, `baseline_source_manifest` and `changed_paths` are null, accurately indicating that comparison is unavailable. Actual changed paths still belong in the primary-owned OpenSpec record.

## First cycle

1. **DISCOVER:** read target/current docs, invariants, state, implementation, and tests; establish a real PostgreSQL baseline.
2. **PLAN:** select a dependency-ordered gap, write scoped acceptance cases, and record the OpenSpec proposal.
3. **EXECUTE:** preserve working systems, implement the bounded change, and add meaningful regression coverage.
4. **VERIFY:** run fixed gates and independent artifact review; confirm documentation and configuration accuracy.
5. **ITERATE/STOP:** fix verified failures or record confirmed blocks; write state and OpenSpec receipt with the exact next cycle. After a reviewed batch passes, continue to the next applicable requirement. Stop only when every applicable clause in the active architecture, requirements, feature requirements, and security contract has evidence and the final state/OpenSpec reconciliation passes, or when genuinely blocked under the persistent goal's blocked-audit rules.
