# Security and institutional production acceptance

This is the required verification rubric for the user's requested secure,
institutional production-readiness claim. It does not authorize deployment,
external publication, destructive testing, secret access, or installation of
unavailable tools. Root architecture and requirements remain authoritative for
product features. Operational controls in this rubric must mitigate a documented
threat or production failure mode and must not expand product scope.

## Required review roles

Use separate passes with bounded evidence:

1. **Correctness maker:** implements one architecture-traced batch.
2. **Correctness checker:** tries to falsify invariants and acceptance behavior.
3. **Security reviewer:** uses `$security-best-practices` only for languages that
   skill supports, and separately reviews Rust trust boundaries, authn/authz,
   egress, secrets, logs, dependencies, SQL, unsafe code, and runtime behavior.
4. **Threat-model reviewer:** runs `$security-threat-model` against the settled
   repository and maps every abuse path to code/test evidence.
5. **Operations reviewer:** verifies install, upgrade, rollback/forward recovery,
   backup/restore, observability, limits, and failure behavior.
6. **Documentation reviewer:** checks that README and operations claims match the
   demonstrated artifact.

The maker cannot self-accept. Reviewers receive the root clauses, artifact,
changed paths, tests, and rubric; they do not receive persuasive maker reasoning.
Any missing evidence defaults to fail.

If the Codex Security plugin is installed, use its repository scanning and review
capabilities as an additional checker. It is not currently installed in this
workspace, so its absence must never be reported as a passing scan. Do not install
it unless the user explicitly requests that plugin.

## Tool inventory rule

Before invoking an optional scanner, verify it exists and record its version:

```bash
rustc --version
cargo --version
docker --version
docker compose version
psql --version
command -v cargo-audit
command -v cargo-deny
command -v gitleaks
command -v trivy
command -v syft
command -v grype
command -v k6
```

An absent command is “not run,” never “passed.” Installing a tool or downloading
advisory/vulnerability databases requires network access and may require user
approval. Record the exact version and database timestamp because findings change
over time.

## Deterministic repository gates

All must pass on one stable source manifest:

```bash
cargo fmt --check
cargo check --workspace
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s scripts/tests -v
python3 scripts/implementation_cycle.py --phase verify --change-id <active-change>
```

Real PostgreSQL tests must run against PostgreSQL with pgvector and disposable
migrated databases. Do not replace SQL concurrency, locking, trigger, RLS,
constraint, migration, or query-plan tests with mocks.

## Threat model scope

The threat model must identify assets, actors, trust boundaries, entry points,
privilege levels, and abuse paths for at least:

- bearer credentials, credential digests, admin/migration/runtime DB credentials,
  model/object-store credentials, and rotation/revocation;
- Personal fallback, Business default deny, users, organizations, memberships,
  workspaces, repositories, projects, principals, grants, and child executions;
- HTTP, MCP stdio, workers, internal service calls, model runtimes, harnesses,
  object storage, connectors, and PostgreSQL;
- untrusted model output, prompt injection, documents, email/chat/source text,
  tool responses, connector records, metadata, filenames, and URLs;
- raw evidence, canonical versions, institutional history, ContextPackets,
  artifacts, experiences, audit records, and backups;
- cross-tenant/workspace/project access, confused deputy, ID enumeration, forged
  identity, privilege escalation, stale grants, revocation races, pooled-context
  leakage, and unsafe historical/as-of reads;
- model egress, SSRF, endpoint substitution, tool escalation, budget exhaustion,
  data exfiltration, malicious embeddings, and provider response injection;
- SQL injection, dynamic identifiers, migration tampering, RLS bypass, owner-role
  bypass, transaction races, replay, duplicate delivery, stale worker leases, and
  denial of service;
- dependency/build compromise, model-file integrity, container escape, writable
  filesystem abuse, overbroad network exposure, unsafe logs/telemetry, and secret
  leakage.

For each abuse path record: prerequisite, impact, current controls, source/test
evidence, residual risk, required mitigation, owner, and acceptance status.

## Authentication and authorization acceptance

- Authenticated principals are opaque and storage-created. No public constructor,
  deserializer, request field, MCP parameter, or metadata can create identity.
- Bearer tokens are never stored or logged. Digests are resolved server-side on
  every request; expired, disabled, revoked, and unknown credentials fail before
  resource identity or content is returned.
- Business mode has no owner fallback. Membership alone grants nothing. Every
  action needs an explicit current grant for exact tenant/workspace/project,
  action, classification, destination, budget, and execution context.
- Raw unscoped use-case methods are not callable outside the services crate.
  Storage APIs are not exposed directly to HTTP/MCP/worker adapters.
- Reads, canonical writes, evidence writes, curation, context delivery, object
  access, tools, and model egress hold authorization through the sensitive load or
  commit boundary.
- Historical/as-of endpoints apply current authorization in addition to temporal
  validity; history cannot resurrect revoked access.
- Child agents use an active bound execution and explicit attenuated grants. They
  cannot inherit or delegate parent authority or claim human attribution.
- Policy decisions are append-only, redacted, and contain enough identifiers and
  reason codes to reconstruct why access was allowed or denied without storing
  content or credentials.

Required adversarial tests include cross-tenant, same-tenant wrong workspace,
wrong project, unknown/unowned organization, guessed IDs, forged fields, wrong
credential channel, classification boundaries, exact model destination, budget
overflow, concurrent revocation, expired execution, pooled connection reuse, and
disabled principal.

## Database and migration acceptance

- Test fresh migration and upgrade from each supported schema boundary with a
  data-bearing fictional fixture. Verify IDs, evidence, canonical content,
  provenance, status, and valid-time history are preserved.
- Migration files already applied to persistent environments are immutable. Fix
  forward with a new numbered migration.
- Production uses separate migration-owner and non-owner runtime roles. Runtime
  cannot create/alter/drop schema, bypass RLS, rewrite evidence/version history,
  mutate grant content, or delete audit decisions.
- RLS tests execute as a non-owner. Verify absent context returns no policy rows,
  transaction-local context scopes one tenant, commit clears it, and pool reuse
  does not leak it.
- Every ownership and agent/grant relationship has same-tenant composite foreign
  keys or an equally strong database invariant.
- Grant/credential revocation is irreversible and monotonic. Content fields are
  immutable. Project ownership cannot be reassigned in place.
- Concurrency tests cover promotion, supersession, event ordering, idempotency,
  revocation, leases/fencing, and outbox publication.
- Query plans use static parameterized SQL, bounded results, correct indexes, and
  explainable scope predicates. Run `EXPLAIN (ANALYZE, BUFFERS)` only on fictional
  disposable data and record plans without content/secrets.

## Untrusted content and model/runtime acceptance

- Treat every external string and model/tool result as data. It cannot mutate
  policy, permissions, prompts outside its quoted data section, SQL, tools,
  destinations, canonical state, or runtime configuration.
- Extraction/generation runtimes receive only authorized, classified, bounded
  inputs and explicit capabilities. They have no DB credentials or unrestricted
  shell/network/secrets.
- Validate typed outputs with closed enums, deny unknown authority fields, strict
  size/count/range limits, and deterministic policy before persistence.
- Model endpoints are allowlisted exact destinations with TLS requirements and
  SSRF-safe URL handling. Redirects cannot escape the authorized destination.
- Timeouts, cancellation, retry classification, response/body limits,
  concurrency, and token/item budgets are enforced outside the model.
- Model/provider failures do not silently switch to a weaker provider, bypass
  egress policy, or promote partial output.

Use malicious fictional fixtures for prompt injection, oversized values, nested
JSON, Unicode/control characters, URL redirects, tool-call escalation, false
human attribution, policy text, and secret-exfiltration requests.

## Supply-chain and secret scanning

When installed and current, run:

```bash
cargo audit
cargo deny check
gitleaks detect --source . --no-git
trivy fs --scanners vuln,secret,misconfig .
```

Use Syft to produce an SBOM and Grype or Trivy to scan final images when those
tools are installed. Because this checkout lacks Git metadata, use filesystem
scan modes and inspect exclusions. Review `Cargo.lock`, build scripts, enabled
features, licenses, abandoned/yanked advisories, and model/runtime binary sources.

Production acceptance requires all of these categories even when the named tools
are unavailable: (1) a dependency advisory check using a current RustSec or
equivalent advisory database, (2) a complete dependency/license policy review,
(3) a repository secret scan plus manual match review, and (4) a vulnerability
and configuration scan of every final container image. Use an equivalent reviewed
tool if necessary and record its version/database timestamp. If no current path
exists for any category, production acceptance is blocked. Extra scanners may be
recorded as `not run`; a mandatory category may not.

Do not suppress an advisory by identifier without a written exploitability
analysis, expiry/review date, and independent approval. No unresolved exploitable
Critical/High finding may remain at production acceptance.

Search manually for repository-specific leaks even when scanners pass:

```bash
rg -n --hidden --glob '!target/**' --glob '!.git/**' \
  '(BEGIN (RSA|OPENSSH|EC) PRIVATE KEY|Bearer [A-Za-z0-9._-]+|password\s*=|secret\s*=|api[_-]?key\s*=)'
rg -n 'tracing::|println!|dbg!|error!|warn!|info!|debug!' apps crates
```

Review matches; these patterns contain false positives. Never print `.env`, live
environment variables, credentials, tokens, or secret files into tool output.

## Container and network acceptance

- Build final API, worker, and MCP images from a clean context. Final images run
  as non-root, contain no compiler/cache/source secrets, use minimal packages,
  have bounded writable paths, and respond correctly to shutdown signals.
- Pin and document base-image/toolchain versions. Generate and scan final-image
  SBOMs when tooling is available.
- API and PostgreSQL bind loopback by default in local Compose. MCP remains stdio,
  not an unauthenticated network daemon.
- Business production documents TLS termination, PostgreSQL TLS verification,
  object/model endpoint TLS, network allowlists, proxy trust, request IDs, and
  trusted-forwarded-header behavior.
- Health proves process liveness. Readiness checks required dependencies without
  exposing sensitive diagnostics. Neither endpoint bypasses protected data APIs.
- Test request/body/header limits, slow clients, connection exhaustion,
  concurrent jobs, graceful drain, restart recovery, and dependency outage.

Use unique Compose project names, containers, loopback ports, and volumes for
smoke tests. Never reuse or delete the preserved developer database volume.

## Backup, restore, retention, and disaster recovery

- Define RPO/RTO and supported backup method for PostgreSQL and object storage.
- Encrypt backups, restrict backup credentials, and document retention/deletion.
- Run a restore drill into an isolated environment. Verify schema version,
  evidence/canonical hashes, provenance, object hashes, audit decisions, grants,
  and ability to serve authorized current and historical queries.
- Test database restored with a missing object, object restored with missing
  metadata, partial outbox replay, credential rotation, and compromised principal
  revocation.
- Do not claim backup readiness from successful backup creation alone; restoration
  and integrity verification are mandatory.

## Performance, resilience, and SLO evidence

Define measurable SLOs from root product behavior before load tests, including
API/query/context latency, worker throughput/backlog age, error rate, database
connections, object/model timeouts, and recovery time. Then test realistic
fictional datasets at documented scale.

If `k6` is installed, use it for bounded HTTP load. Otherwise use a checked-in
Rust or Python harness with deterministic scenarios; do not install a large tool
only to satisfy this document. Test soak, burst, large authorized context,
cross-tenant concurrency, retry storms, worker crashes, PostgreSQL restart,
object-store outage, and model timeout. Record hardware, dataset, concurrency,
duration, percentiles, errors, and resource limits.

Performance failure must degrade safely: no scope widening, skipped provenance,
silent provider substitution, unbounded retries, duplicate canonical writes, or
lost audit/outbox records.

## Observability acceptance

- Structured logs include correlation/request/job/execution IDs and safe reason
  codes, never content, prompts, outputs, bearer values, digests, or secrets.
- Metrics cover auth denials, egress denials, latency, errors, queue age/retries,
  stale state, packet sizes, and root evaluation metrics without high-cardinality
  sensitive labels.
- Audit and operational logs have documented retention and access controls.
- Alerts map to an operator action and are tested. Avoid placeholder dashboards
  and unused telemetry infrastructure.
- OpenTelemetry export is conditional on actual deployment value; safe internal
  metrics and correlation are required regardless.

## Final evidence bundle

Production acceptance requires one reviewable bundle containing:

- source-manifest/implementation-loop receipt;
- full gate and focused test counts;
- migration fresh/upgrade/non-owner RLS evidence;
- threat model and resolved findings;
- dependency advisory, license, secret, filesystem/configuration, and final-image
  scan versions/results; `not run` is allowed only for supplementary tools after
  every mandatory category above has current evidence;
- SBOMs when tooling is available;
- isolated fresh-stack API/worker/MCP/model/object proof;
- load/resilience results against stated SLOs;
- backup/restore integrity report;
- six product-scenario and evaluation results;
- independent correctness, security, operations, and docs verdicts;
- exact remaining optional/conditional items and why the root docs permit them.

No single scanner, test command, review, container startup, or happy-path demo is
sufficient by itself.
