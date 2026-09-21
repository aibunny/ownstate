# MCP capture and scope-resolution security contract

This contract is mandatory for the automatic-capture feature. It supplements the
repository-wide rules and existing default-deny policy.

## 1. Threat model

Assume a malicious or compromised model can call every advertised MCP tool with
arbitrary JSON repeatedly and concurrently. Assume task text, remote URLs,
working directories, Git metadata, client metadata, imported transcripts,
artifact names, source IDs, and scope handles are attacker-controlled.

Protect at least:

- append-only raw evidence and its source ordering;
- immutable canonical versions and temporal history;
- artifacts/object metadata and provenance;
- tenant, organization, workspace, project, repository, thread, and global scope;
- principals, credential digests, grants, classifications, decisions, and audit;
- model/object-store/runtime credentials and logs;
- availability of PostgreSQL, workers, context compilation, and MCP.

Model prompts and MCP instructions are usability controls, not security controls.

## 2. Permanently forbidden MCP capabilities

The model-controlled MCP surface must contain no operation that can:

- delete, purge, erase, truncate, or hard-delete evidence, sessions, artifacts,
  repositories, projects, scopes, knowledge, versions, claims, relationships,
  policy decisions, or audit records;
- update raw evidence or existing canonical-version content;
- force canonical promotion, supersession, merge, or conflict resolution;
- create/change/revoke principals, credentials, grants, memberships, policy, RLS,
  retention rules, or data classification authority;
- execute raw SQL, arbitrary commands, arbitrary filesystem reads, arbitrary
  network requests, or unrestricted model/tool calls;
- select a tenant/principal through arguments or turn a scope handle into
  authorization.

Enforce this structurally, not only through tool descriptions:

- no destructive MCP tool registration;
- MCP handlers depend only on restricted principal-scoped service interfaces;
- those interfaces expose append evidence, bounded retrieval, proposal, and
  feedback behavior but no destructive/admin storage port;
- database triggers/permissions retain append-only and version immutability;
- integration tests enumerate advertised tools and attempt direct/indirect
  destructive payloads.

## 3. Legitimate retention and deletion

Existing product requirements may require authorized policy deletion, retention,
or data-subject workflows. Those are administrative operations, not model tools.
If implemented, they require a separate authenticated administrative plane,
explicit narrow action, current grant, human/operator intent, reason, preview,
append-only audit, concurrency-safe transaction, and recovery/retention semantics.

No MCP scope handle, model claim, prompt, feedback, artifact metadata, or source
content may trigger administrative deletion. A model may propose that an operator
review retention; it cannot execute it.

## 4. Authentication and authorization

- Resolve bearer/transport credentials server-side on every MCP operation.
- Request bodies and client metadata cannot construct a principal.
- Business mode defaults deny; Personal convenience uses explicit server-created
  grants, not a hidden bypass.
- Authorize before resolving sensitive identity, returning existence, reading
  content, appending evidence, issuing object access, or invoking a model.
- Hold authorization through the sensitive load/commit/provider-call boundary.
- Current authorization applies to historical/as-of reads.
- Audit allow/deny decisions with IDs/reason codes only; never content or tokens.

## 5. Scope-handle security

Scope handles are opaque references only. Requirements:

- generated with cryptographically strong randomness or equivalent unguessable
  server identity;
- stored/compared safely and never used as a credential;
- bound to ownership-domain and resolved resource records;
- independently authenticated and re-authorized on every use;
- status/expiry/revocation checked where the design uses them;
- no embedded tenant, grant, credential, secret, or predictable sequence;
- cross-principal, cross-tenant, wrong-workspace/project, disabled/revoked, and
  stale-handle tests return non-enumerating errors.

Leaking a handle must not grant access.

## 6. Untrusted Git and environment evidence

Do not trust client-supplied `working_directory` as a server filesystem path. A
local host adapter may inspect its own repository using fixed Git arguments after
validating the root. A remote MCP server treats supplied repository facts as
claims until corroborated by a trusted adapter or existing authorized alias.

Remote normalization must strip credentials before logging, error construction,
metrics, persistence, and hashing. Cover userinfo, percent encoding, SCP syntax,
query/fragment tokens, control characters, Unicode confusables, extremely long
values, nested schemes, local/file URLs, and malformed ports.

Normalization cannot perform network requests. Later provider verification, if
required, uses allowlisted HTTPS APIs, server-owned credentials, DNS/IP/redirect
SSRF controls, timeouts, and response limits.

Never invoke a shell with concatenated Git/path/remote input. Prevent path
traversal and symlink escape in local adapters.

## 7. Input, resource, and denial-of-service controls

- Closed schemas reject unknown authority/identity fields.
- Bound content, strings, lists, nesting, metadata keys, artifacts, source refs,
  token/item budgets, and batch sizes before allocation or database work.
- Apply per-principal request/concurrency/rate limits and database timeouts.
- Use bounded retries/backoff and idempotency; no retry amplification.
- Bootstrap exact-match paths are indexed and model-free.
- Avoid high-cardinality/sensitive metric labels.
- Cancellation and client disconnect cannot leave partial canonical writes or
  orphaned authorization leases.

## 8. Evidence and idempotency integrity

- Raw events are append-only and ordered under existing constraints.
- Source idempotency is ownership-scoped; one tenant cannot collide with another.
- Replays return the existing event outcome without duplicating downstream work.
- Content fingerprints never include unredacted secrets in logs/metrics.
- Same content at different legitimate source positions remains distinguishable.
- Adapters, imports, and MCP record calls cannot assert human attribution or a
  stronger capture mode than their verified channel permits.

## 9. Context and exfiltration controls

- Apply scope, current authorization, classification, source ACL, freshness, and
  total packet budgets before delivery.
- A scope handle does not bypass query predicates or RLS/static scope controls.
- Evidence drill-down is separately authorized and bounded.
- No prompt, context, log, error, trace, or policy decision includes bearer
  tokens, credential digests, secret-bearing remotes, raw secrets, or unrelated
  tenant content.
- Model egress uses exact destination policy and holds the lease through the
  provider call. No silent fallback to a weaker/unapproved provider.

## 10. Required adversarial tests

Add deterministic tests for:

1. tool enumeration contains no delete/admin/raw-SQL/force-write operation;
2. guessed, leaked, stale, cross-principal, cross-tenant, and wrong-workspace
   handles cannot reveal existence or data;
3. JSON fields cannot forge principal, tenant, ownership, grant, capture mode,
   human attribution, or canonical status;
4. record/feedback/proposal cannot update or delete existing evidence or versions;
5. SQL/database roles and triggers reject direct mutation by runtime/MCP roles;
6. destructive language embedded in content, prompt injection, tool results, and
   metadata remains inert data;
7. remote URLs with tokens/passwords never persist or appear in logs/errors;
8. shell metacharacters, path traversal, symlink escape, malformed URLs, Unicode,
   oversized/nested JSON, duplicate keys, and control characters fail safely;
9. concurrent bootstrap/record requests preserve uniqueness and idempotency;
10. authorization revocation racing read/write/egress cannot overtake the held
    policy boundary;
11. MCP-only capture cannot claim FULL or complete channels;
12. retention/admin endpoints, if any, are unreachable with MCP credentials and
    absent from the MCP service interface;
13. context remains bounded and contains only exact authorized delivered records;
14. denial and validation errors do not enumerate resources or expose internals.

## 11. Production verification tools

Run on one stable source manifest:

```bash
cargo fmt --check
cargo check --workspace
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s scripts/tests -v
```

Use real PostgreSQL for migration, constraint, trigger, RLS/static-scope,
concurrency, and idempotency behavior. Run the repository implementation-cycle
verification for the active change.

Production acceptance also requires current evidence for dependency advisories,
dependency/license policy, repository secrets, filesystem/container
misconfiguration, and final-image vulnerabilities. Prefer established tools such
as `cargo audit`, `cargo deny`, `gitleaks`, Trivy, Syft, and Grype when available;
record exact versions and advisory database timestamps. An absent mandatory
category is blocked, not passed.

Create a repository-grounded threat model and give the settled artifact to an
independent reviewer who attempts to bypass identity, authorization, immutability,
idempotency, and capture claims.

## 12. Security completion condition

The feature cannot be called production secure while any model-controlled path
can delete or rewrite recorded state, mint authority, cross scope, leak remote
credentials, overstate capture completeness, bypass egress policy, or cause
unbounded work. No unresolved exploitable Critical or High finding may remain.

