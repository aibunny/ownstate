# Ownership and principal-policy batch contract

> **Interrupted checkpoint (2026-09-20):** this task list is not accepted and the
> current all-target build is broken during a security-boundary refactor. Read
> root `ARCHITECTURE.md`, `REQUIREMENTS.md`, and `README.md`, then
> `CONTINUATION_PROMPT.md`, `STATE.md`, and `docs/PENDING_IMPLEMENTATION.md` before
> editing. Do not mark tasks complete from the historical 102-test receipt, and
> do not restore public raw `AppServices` methods to make tests compile.

Root `ARCHITECTURE.md`, `REQUIREMENTS.md`, and `README.md` remain target authority.
This is an additive compatibility-preserving security batch, not a product-completion claim.

- [ ] Define closed typed identities, actions, resource/context scopes, grants, destinations, budgets, and privately constructed authenticated/authorized values.
- [ ] Add migration0010 ownership/principal/grant/execution/policy-decision schema with immutability and legacy project bindings that preserve all existing IDs/content/provenance.
- [ ] Implement deterministic default-deny policy evaluation with enabled/valid/scope/action/classification/destination/budget/delegation checks.
- [ ] Resolve credential digests server-side and create explicit Personal owner, API, curator, worker, and MCP/service grants; infer no Business membership or authority.
- [ ] Add principal-scoped services and enforce admission before project identity, evidence, knowledge, query, context, curation, embedding/model egress, and final sensitive loads/commits.
- [ ] Bind HTTP and MCP to server-derived principals; reject forged caller identity and separate read/propose from curation.
- [ ] Add child execution creation with explicit grant attenuation, no parent permission inheritance, bounded expiry/classification/destination/budget, and no delegated policy/human-attribution authority.
- [ ] Add append-only redacted policy-decision audit and revocation-safe final rechecks.
- [ ] Verify legacy migration, Personal compatibility, organization default denial, same/cross-tenant denial, credential lifecycle, pre-embedding denial, final-load revocation, remote-egress denial, child attenuation, and pooled-scope isolation with real PostgreSQL.
- [ ] Run four full Cargo gates and implementation-loop Python tests on stable settled sources.
- [ ] Obtain independent artifact review, repair all findings, and rerun affected gates.
- [ ] Update README/current docs/ledger/STATE with exact evidence and remaining gaps.
- [ ] Apply OpenSpec and advance to the next unmet root requirement.

## Ownership and changed paths

Policy maker owns `crates/domain/src/policy.rs`, typed IDs/exports,
`migrations/0010_ownership_and_policy.sql`, `crates/storage/src/policy.rs`, storage
exports, and `crates/storage/tests/ownership_policy.rs`. Root owns service policy
integration, runtime/bootstrap, HTTP/MCP/worker adapters and their tests, shared
exports, docs, state, and coordination. The independent checker owns no source
files. Makers are not alone in the repository and must preserve concurrent edits.

## Acceptance limits

Membership alone grants no organization access. Historical/as-of reads never
restore revoked authorization. Untrusted request metadata never establishes
identity. Remote model egress is denied unless an exact destination grant permits
it. RLS is defense in depth only when the runtime role and pooled request context
make the test meaningful; existing SQL scope/classification/provenance predicates
remain mandatory. A child can only receive an explicit subset of a delegable
parent grant. Existing public Personal behavior may be preserved only through
server-created grants, never through a Business fallback.
