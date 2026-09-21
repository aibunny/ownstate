# RULES — constraints for any agent working in this repo

1. Never rewrite raw interaction evidence. The DB forbids it; do not remove the
   triggers to make a test pass.
2. Never UPDATE the content of an existing knowledge version. Supersede it.
3. Never expose SQL execution, canonical force-writes, or permission changes
   through MCP or HTTP.
4. Parameterized SQL exclusively. sqlx 0.9 requires `&'static str` queries;
   do not wrap dynamically built strings in `AssertSqlSafe` to get around it.
5. No `unwrap()`/`expect()` in request/runtime paths unless genuinely impossible,
   with a comment saying why.
6. Do not log event content, knowledge content, or tokens. IDs and counts only.
7. Do not add infrastructure (Redis, Kafka, NATS, ES, Qdrant, k8s) — Postgres
   and object storage are the only stores this product starts with.
8. Domain crate stays free of axum/sqlx/rmcp/fastembed dependencies.
9. Tests that exercise PostgreSQL semantics run against real PostgreSQL — do not
   mock the database for those behaviors.
10. Do not commit or push unless the user explicitly asks.
11. Record every change through opsx (`./opsx propose` before, `./opsx apply` after).
12. MCP/model-controlled interfaces are non-destructive. Never expose delete,
    purge, erase, retention execution, raw SQL, canonical force-write, grant,
    credential, policy, or permission mutation through MCP. Legitimate retention
    belongs to a separate explicitly authorized administrative plane.
13. Scope handles and provider/MCP session IDs are references, not identity,
    authentication, authorization, tenant selection, or grants. Authenticate and
    authorize every use server-side.
14. Capture completeness is factual. Never label MCP-only or partially observed
    activity as full/lossless capture.
