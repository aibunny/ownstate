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
volumes so later deployments can reuse them. The Railway image initializes the
cache mount while privileged, then permanently drops to the dedicated
`ownstate` user before starting the selected process.

After deployment:

1. Open the `API` service and copy its public Railway domain.
2. Open the `API` service variables and copy `OWNSTATE_API_TOKEN` and
   `OWNSTATE_MCP_TOKEN` into a secret manager. Do the same with
   `OWNSTATE_ADMIN_TOKEN`; keep the admin token away from agent integrations.
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

The API service exposes authenticated Streamable HTTP MCP at
`https://YOUR-DOMAIN/mcp`. In the base template it accepts the configured
`OWNSTATE_MCP_TOKEN`. With account signup enabled it also accepts active
user-owned connections; API and admin tokens are rejected. PostgreSQL remains private.

### Account signup and user-owned connections

The template above remains a single-user starting point. The API can also host
the multi-user account site and OAuth MCP connections without changing the
existing Personal worker or its credentials. After backing up the database and
deploying the account migrations, set these **API service** variables. Railway
Hobby blocks outbound SMTP, so use a verified Resend domain and its HTTPS API
on that plan:

```text
OWNSTATE_PUBLIC_URL=https://YOUR-DOMAIN
OWNSTATE_SMTP_FROM=ownstate@theaibunny.com
OWNSTATE_RESEND_API_KEY=<set privately as a Railway secret>
```

If the Railway workspace is upgraded to Pro, Lark SMTP can be used instead:

```text
OWNSTATE_PUBLIC_URL=https://YOUR-DOMAIN
OWNSTATE_SMTP_HOST=smtp.larksuite.com
OWNSTATE_SMTP_PORT=587
OWNSTATE_SMTP_FROM=ownstate@theaibunny.com
OWNSTATE_SMTP_USERNAME=ownstate@theaibunny.com
OWNSTATE_SMTP_PASSWORD=<set privately as a Railway secret>
```

The API prefers Resend when `OWNSTATE_RESEND_API_KEY` is set. Otherwise it uses
Lark STARTTLS on port 587. The account site remains disabled until the public
URL and a supported email credential are configured. Railway Pro requires a
redeploy after upgrade to allow SMTP. A new signup receives a new personal
tenant; it does not automatically take ownership of the original Personal mode
tenant or its historical data. A guarded one-time owner claim for the existing
deployment is in `scripts/claim_legacy_personal_account.sql`; back up and
verify the target account before using it. Keep the original deployment
credentials until replacement account connections are verified.

Users sign up at `/signup`, verify their email, and can create organizations and
invite members from `/account`. The account page issues per-user manual MCP
tokens and allows revocation. OAuth clients discover the authorization server
through `/.well-known/oauth-protected-resource/mcp`. The authenticated
`/capture/conversations` endpoint accepts events from local host adapters.
See [the capture strategy](CHAT_CAPTURE_AND_MULTI_TENANT_STRATEGY.md) and
[adapter setup](../integrations/README.md) for coverage and limitations.

For Claude, add `https://YOUR-DOMAIN/mcp` as a custom Web connector. Choose
**Sign in now** and **Register automatically** for the OAuth client, leave fixed
request headers empty, then sign in to Ownstate and choose the intended space.
The connector persists in the Claude account across chats; enable it for each
conversation where it should be available. A manually copied token is a
different connection route. Verify actual OAuth registration, authorization,
token exchange, and refresh with the live client before calling this complete.

Do not advertise the site or submit connector listings until email delivery,
OAuth interoperability with each target client, broader traffic controls, backup/restore,
and the privacy and retention policy have been verified in the deployed
environment. MCP access does not provide a passive full chat transcript.

## Template service contract

The published template is expected to keep this exact topology. If you create
the services manually, use these settings.

### `pgvector`

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
OWNSTATE_DATABASE_URL=${{pgvector.DATABASE_URL}}
OWNSTATE_DEPLOYMENT_MODE=personal
OWNSTATE_API_TOKEN=${{secret(48)}}
OWNSTATE_ADMIN_TOKEN=${{secret(48)}}
OWNSTATE_MCP_TOKEN=${{secret(48)}}
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
OWNSTATE_DATABASE_URL=${{pgvector.DATABASE_URL}}
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
