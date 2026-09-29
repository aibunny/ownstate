## Why

- Users need a one-click Railway deployment that provisions the public API,
  private worker, and durable PostgreSQL with the pgvector extension. The
  deployment must use generated credentials and must not expose PostgreSQL to
  the public internet.

## What Changes

- Add a unified Railway container target that selects the API or worker at
  runtime while preserving the existing dedicated Docker targets.
- Accept Railway's injected `PORT` when no explicit Ownstate bind address is
  configured.
- Document the exact Railway services, secret references, volumes, health
  checks, deployment order, and verification steps.
- Publish a Railway template and add its one-click deployment button to the
  repository README.

## Impact

- Makes a secure personal deployment reproducible from the public repository.
- Keeps PostgreSQL and the worker private, stores database/model data on
  persistent volumes, and preserves existing local Compose behavior.
