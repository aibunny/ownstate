# Ownstate continuation prompt

Use this file as the primary handoff prompt for the next implementation model.
It describes an interrupted working tree. It is not a statement that the current
source compiles or that the active policy batch is accepted.

## Mission

Continue the existing Ownstate repository until every applicable requirement in
root `ARCHITECTURE.md`, root `REQUIREMENTS.md`, and root `README.md` has direct
implementation, real acceptance evidence, accurate operating documentation, and
independent review. Do not stop after repairing the current batch. A passing batch
advances to the next open row in `docs/REQUIREMENT_LEDGER.md`.

Apply these skills when they are available:

- `$loop-engineer` for the persistent discover/plan/execute/verify/iterate loop.
- `$orchestrate-agent-graphs` for bounded maker/checker work with independent
  security and correctness reviewers.
- `$security-best-practices` for its supported Python/JavaScript/TypeScript
  support code where applicable; review Rust with the repository-specific rubric,
  Clippy, dependency scanners, tests, and an independent Rust security reviewer.
- `$security-threat-model` for a repository-grounded threat model before the
  production-readiness claim.

The exact stop condition is stated under **Completion contract** below. Do not
mark the persistent goal complete merely because Cargo is green.

Strict scope rule: every implementation batch must cite a mandatory clause in
one of the three root documents and must implement the smallest behavior that
satisfies it. Root language such as `optional`, `may`, `where useful`, `future`,
and `long-term` does not authorize current feature work. Do not infer product
scope from examples, the ledger, OpenSpec, existing partial code, common industry
practice, or reviewer suggestions. Those sources may identify a gap or a
mitigation, but the root documents alone define the target. Record and reject any
feature proposal that lacks that trace.

## Authority and mandatory first reads

Read these in order before editing:

1. `ARCHITECTURE.md`, `REQUIREMENTS.md`, and `README.md` — target authority.
2. `docs/RULES.md` — non-negotiable repository constraints.
3. `STATE.md` — verified facts and the interrupted checkpoint.
4. `docs/REQUIREMENT_LEDGER.md` — whole-product coverage and dependency order.
5. `docs/PENDING_IMPLEMENTATION.md` — exact pending task graph.
6. `docs/SECURITY_AND_PRODUCTION_ACCEPTANCE.md` — required security and
   production proof.
7. `docs/IMPLEMENTATION_LOOP.md` — fixed gates and receipt semantics.
8. `.opsx-current` and the matching `openspec/changes/<id>/proposal.md` and
   `tasks.md`.

Files under `docs/` describe the current implementation. They do not override the
three root target documents. This checkout has no usable Git metadata, so do not
use an empty Git diff as evidence that nothing changed. OpenSpec changed paths,
source inspection, and the implementation-loop manifest are authoritative.

## Repository invariants

- Preserve raw evidence. Never weaken its append-only database triggers.
- Never edit canonical knowledge-version content in place. Supersede versions.
- Models, agents, connectors, and external text may propose only. Deterministic
  Rust authorization and validation control canonical writes and model egress.
- Use static parameterized SQL. Do not use dynamic SQL or `AssertSqlSafe` to
  bypass SQLx query safety in application/storage code.
- Never expose SQL execution, force-canonical writes, credentials, grants, or
  permission mutation through HTTP or MCP.
- Never log raw evidence, knowledge content, bearer tokens, credential digests,
  secrets, or model prompts. Log identifiers, reason codes, and bounded counts.
- Keep the domain crate free of Axum, SQLx, RMCP, Fastembed, and vendor runtime
  dependencies.
- PostgreSQL-specific behavior requires real PostgreSQL + pgvector tests.
- Preserve existing database volumes and data. Use disposable databases and
  isolated Compose project/volume names for tests.
- Do not add Kafka, Redis, Elasticsearch, Qdrant, Kubernetes, or NATS merely to
  satisfy a diagram. PostgreSQL and S3-compatible object storage are the initial
  stores; NATS remains conditional on measured need.
- Do not commit, push, publish, deploy externally, or delete persistent volumes
  without explicit user authorization.

## Interrupted checkpoint — do not assume a green baseline

The previous accepted query/operability batch had 102 Rust tests and 10 Python
runner tests passing, plus a fresh Fastembed Compose proof. That receipt is for
the previous source snapshot and does not validate the current tree.

The active OpenSpec change is
`implement-ownership-and-default-deny-principal-policy`. Its task list is still
unchecked and it has not been applied. Most policy foundations exist, including:

- typed policy actions, scopes, budgets, destinations, grants, and agent IDs;
- migrations `0010_ownership_and_policy.sql` and the unverified follow-up
  `0011_policy_boundary_hardening.sql`;
- storage credential resolution, default-deny grants, decisions, child execution,
  ownership binding, RLS policies, and PostgreSQL acceptance tests;
- HTTP/MCP/worker principal binding, Personal actors, Business-mode configuration,
  policy leases, forged-field rejection, and adversarial policy tests;
- an opaque `AuthenticatedPrincipal` recently moved from the domain crate to the
  storage policy module so other crates cannot construct it.

The tree is currently **not compiling in all-target mode**. The last command was:

```bash
cargo check --workspace --all-targets
```

It failed with `E0624` and `E0308` because raw `AppServices` use-case methods were
changed to `pub(crate)` to close an in-process authorization bypass, while several
integration tests still call those private methods. Partial test migration exists
in `crates/services/tests/common/mod.rs` (`TestServices`) and
`retrieval_and_context.rs`; `institutional_state.rs`, `typed_queries.rs`, API/MCP
fixtures, and some helpers still need conversion. Do not “fix” this by making the
raw methods public again.

`migrations/0011_policy_boundary_hardening.sql` was added after the last passing
PostgreSQL suites and has not been executed or verified. Its replacement
`ownstate_policy_immutable` trigger must first be adjusted to permit an exact
no-op update, including idempotent bootstrap of an already revoked credential,
while still allowing only the first `NULL -> timestamp` revocation transition.
Never edit migration `0010` now; it has already been applied to a preserved local
database. Put corrections in `0011` until it has been validated, or in a new
forward migration after validation.

The existing `ownstate-postgres` Compose service and its named volume must be
preserved. Check its state with `docker compose ps postgres`; start only the
service if required. Never run `docker compose down --volumes` against it.

## Required recovery sequence

1. Reproduce the all-target compile failure and record the exact files/errors.
2. Repair migration `0011` revocation/no-op semantics and add its real PostgreSQL
   tests before trusting it.
3. Finish the principal-only service boundary. Production adapters and external
   crates must be unable to invoke unscoped project, evidence, knowledge,
   promotion, query, context, institutional, or worker methods.
4. Bind model-egress authorization to the actual embedding operation by holding
   an authorized policy lease across every provider call in search, semantic and
   hybrid query, context compilation, bootstrap, and worker paths.
5. Add a non-owner RLS/credential-resolution test and then establish separate
   migration-owner and runtime-role behavior for production Business mode.
6. Rerun focused policy/API/MCP/worker tests. Repair failures; do not weaken tests.
7. Run the four fixed Cargo gates and Python runner tests on stable sources.
8. Give the settled artifact and the active OpenSpec rubric to independent
   correctness and security reviewers. Repair all High/Critical findings and all
   Medium findings that violate an explicit requirement.
9. Update current-implementation docs, the requirement ledger, `STATE.md`, and
   OpenSpec only with demonstrated evidence. Apply the OpenSpec change only after
   review accepts it.
10. Continue through every dependency-ordered batch in
    `docs/PENDING_IMPLEMENTATION.md` until the whole completion contract passes.
11. Perform P13 ledger reconciliation and remove every unsupported completion
    claim before reporting the architecture fulfilled.

## Verification commands

Use targeted commands while iterating, then the stable runner:

```bash
docker compose up -d postgres
cargo fmt --all
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s scripts/tests -v
python3 scripts/implementation_cycle.py --phase verify \
  --change-id implement-ownership-and-default-deny-principal-policy
```

Database tests may require authorized access to the local PostgreSQL socket. A
sandbox `PermissionDenied` is an environment block, not a passing test and not a
product defect. Rerun with the required local permission. Never skip or mock a
PostgreSQL semantic test.

Before each new logical batch:

```bash
./opsx propose "precise batch description"
```

After implementation, stable gates, independent review, and documentation:

```bash
./opsx apply
```

Do not apply the current change merely to clear `.opsx-current`.

## Completion contract

The task is complete only when all of the following are true:

- every applicable root requirement has a direct code path and acceptance proof;
- every ledger row is `VERIFIED`, or is explicitly optional/conditional in the
  root documents with evidence explaining why no implementation is required;
- all six root end-to-end scenarios work with fictional fixtures, exact
  provenance, temporal behavior, authorization, and bounded context;
- Personal and Business deployment modes pass fresh-install and upgrade tests;
- the four Cargo gates, Python loop tests, migration tests, API/MCP/worker tests,
  security acceptance, backup/restore drill, and isolated Compose smoke pass on
  one stable source manifest;
- no unresolved Critical/High security finding remains, and the threat model has
  evidence-backed mitigations for every in-scope abuse path;
- independent correctness, security, and documentation reviewers accept the
  final artifact;
- README/current docs/operations/ledger/STATE describe only demonstrated
  behavior and known limits;
- no secret, private organization data, generated credential, or raw sensitive
  content is committed or logged.

If any item lacks evidence, keep the goal active and record it under `STATE.md`
Open Failures. Two or three consecutive “nothing new” review cycles may close a
discovery loop, but they cannot waive an unmet root requirement.
