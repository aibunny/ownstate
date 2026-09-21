## Why

Ownstate currently scopes services to a fixed tenant and classification ceiling, but it does not know which user, service, or agent is calling. A bearer token admits the request without creating a server-derived principal, organization-labelled projects can be reached through the same service object, and workers load content without a principal decision. This cannot satisfy the root requirement that authorization precede retrieval and model egress or that Business mode default deny.

## What Changes

- Add durable users, organizations, memberships, workspaces, repositories, project ownership, principals, credential digests, explicit authorization grants, agent executions/grants, and append-only policy decisions.
- Add closed Rust policy types and a deterministic default-deny evaluator over principal, action, resource, and context.
- Preserve Personal mode through explicit bootstrap owner/API/curator/worker grants while denying unassigned organization state.
- Authenticate credentials into server-owned principal values. HTTP JSON and MCP arguments cannot choose identities or grants.
- Require principal-scoped admission before project/evidence/knowledge/query/context/curation work and before embedding/model egress, with final authorization rechecks for sensitive loads and commits.
- Add explicit child-agent grant attenuation and revocation/expiry/classification/destination/budget restrictions without claiming a complete agent runtime.
- Migrate existing projects additively without changing event, canonical, version, evidence, entity, or assertion identifiers and provenance.
- Add real PostgreSQL and adapter acceptance tests for unauthorized same-tenant access, cross-tenant access, forged identity, credential lifecycle, read-versus-curation separation, organization default denial, pre-embedding denial, revocation, child non-inheritance, remote-egress denial, and legacy preservation.

## Impact

This establishes the root ownership and authorization foundation while preserving existing personal workflows. Artifact storage, source authority, novelty/freshness, complete context audit, outbox, model/harness runtimes, agents, Experience Store, connectors, observability, evaluations, and complete product scenarios remain later batches.
