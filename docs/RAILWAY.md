# Deploy Ownstate on Railway

The Ownstate Railway template provisions one isolated project with three
services:

```text
Internet
   |
   v
Ownstate API  ---- private network ---->  PostgreSQL 18 + pgvector
   ^                                      (persistent volume)
   |
Ownstate worker ---- private network -----+
```

Only the API receives a public domain. PostgreSQL and the worker remain on
Railway's private network. The template builds the repository `Dockerfile` for
both application services and selects the process with `OWNSTATE_PROCESS`.

## One-click deployment

Use the **Deploy on Railway** button in the repository README. Railway will
create all three services, generate independent API, admin, and database
credentials, attach persistent volumes, and deploy the stack.

The first API and worker starts can take several minutes while each service
downloads the local Fastembed model. The model caches are stored on persistent
volumes so later deployments can reuse them.

After deployment:

1. Open the `API` service and copy its public Railway domain.
2. Open the `API` service variables and copy `OWNSTATE_API_TOKEN` into a secret
   manager. Do the same with `OWNSTATE_ADMIN_TOKEN`; keep the admin token away
   from routine agent integrations.
3. Verify the unauthenticated liveness and readiness endpoints:

   ```bash
   curl https://YOUR-DOMAIN/health
   curl https://YOUR-DOMAIN/ready
   ```

4. Verify an authenticated endpoint:

   ```bash
   curl -H "Authorization: Bearer YOUR_API_TOKEN" \
     https://YOUR-DOMAIN/projects
   ```

The Railway deployment exposes Ownstate's HTTP API. The current MCP binary is a
stdio process and is started by a local MCP client; it is not exposed as a
public network service by this template.

## Template service contract

The published template is expected to keep this exact topology. If you create
the services manually, use these settings.

### `Postgres`

| Setting | Value |
|---|---|
| Source | Docker image `pgvector/pgvector:pg18` |
| Public networking | Disabled |
| Volume mount | `/var/lib/postgresql` |
| Restart policy | Always |

Variables:

```text
POSTGRES_USER=ownstate
POSTGRES_DB=ownstate
POSTGRES_PASSWORD=${{secret(48)}}
DATABASE_URL=postgresql://${{POSTGRES_USER}}:${{POSTGRES_PASSWORD}}@${{RAILWAY_PRIVATE_DOMAIN}}:5432/${{POSTGRES_DB}}
```

Ownstate migrations execute `CREATE EXTENSION vector`, so a plain PostgreSQL
image is insufficient. The pgvector image provides the required extension.

### `API`

| Setting | Value |
|---|---|
| Source | GitHub repository `aibunny/ownstate` |
| Dockerfile | `/Dockerfile` (final `railway` stage) |
| Public networking | Generate a Railway domain |
| Health check | `/ready` |
| Health check timeout | 600 seconds |
| Volume mount | `/app/.fastembed_cache` |
| Restart policy | On failure, 10 retries |

Variables:

```text
OWNSTATE_PROCESS=api
OWNSTATE_DATABASE_URL=${{Postgres.DATABASE_URL}}
OWNSTATE_DEPLOYMENT_MODE=personal
OWNSTATE_API_TOKEN=${{secret(48)}}
OWNSTATE_ADMIN_TOKEN=${{secret(48)}}
OWNSTATE_MAX_CLASSIFICATION=CONFIDENTIAL
OWNSTATE_EMBEDDING_PROVIDER=fastembed
OWNSTATE_EMBEDDING_CACHE_DIR=/app/.fastembed_cache
OWNSTATE_AUTO_MIGRATE=true
OWNSTATE_MAX_DB_CONNECTIONS=5
RUST_LOG=info
```

Railway injects `PORT`. Ownstate binds to `0.0.0.0:$PORT` when
`OWNSTATE_HTTP_ADDR` is absent.

### `Worker`

| Setting | Value |
|---|---|
| Source | GitHub repository `aibunny/ownstate` |
| Dockerfile | `/Dockerfile` (final `railway` stage) |
| Public networking | Disabled |
| Volume mount | `/app/.fastembed_cache` |
| Restart policy | Always |

Variables:

```text
OWNSTATE_PROCESS=worker
OWNSTATE_DATABASE_URL=${{Postgres.DATABASE_URL}}
OWNSTATE_API_SERVICE_HOST=${{API.RAILWAY_PRIVATE_DOMAIN}}
OWNSTATE_DEPLOYMENT_MODE=personal
OWNSTATE_MAX_CLASSIFICATION=CONFIDENTIAL
OWNSTATE_EMBEDDING_PROVIDER=fastembed
OWNSTATE_EMBEDDING_CACHE_DIR=/app/.fastembed_cache
OWNSTATE_AUTO_MIGRATE=false
OWNSTATE_MAX_DB_CONNECTIONS=5
OWNSTATE_WORKER_POLL_MS=1000
RUST_LOG=info
```

`OWNSTATE_API_SERVICE_HOST` is a deployment dependency reference. The worker
does not use it at runtime; it makes Railway deploy the healthy API before the
worker when the full template is deployed as a batch.

## Security and operations

- Do not make PostgreSQL public. The API and worker use Railway's private
  network and a generated database password.
- Give routine clients only `OWNSTATE_API_TOKEN`. Reserve
  `OWNSTATE_ADMIN_TOKEN` for explicit human curation operations.
- Store tokens in a password manager. Rotating a token requires updating the
  service variable and every authorized client.
- Enable Railway backups for the PostgreSQL volume before storing important
  data. Test restores into a separate project.
- Review Railway deployment logs after upgrades and verify `/ready` before
  directing clients to a new deployment.
- The template uses `personal` mode. Business mode is default-deny and requires
  explicit principal provisioning before switching the variable.
- Pin image or repository revisions when operating under a formal change
  control policy.

The database volume is durable, but the container and application deployments
remain replaceable. Model caches are performance data and may be rebuilt; the
PostgreSQL volume contains the authoritative Ownstate state.

## Updating

Redeploy the `API` service from the desired repository revision. It runs pending
database migrations before becoming ready. After the API is healthy, redeploy
the `Worker`. Back up PostgreSQL before upgrades and read `CHANGELOG.md` for any
release-specific migration notes.

## Removing a deployment

Deleting a Railway project or its PostgreSQL volume destroys the only database
copy. Export or back up PostgreSQL first. Ownstate intentionally exposes no MCP
or model-controlled operation that can delete recorded evidence.
