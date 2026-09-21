# Operating Ownstate

The local stack contains API, worker, and PostgreSQL 18 with pgvector. MCP is an
optional stdio process. No paid model API, GPU, connector, external database,
message broker, or hosted Ownstate service is needed. S3-compatible object
storage remains planned; adding an unused object-service container would not
provide an implemented ObjectStore.

## Start and inspect

```bash
cp .env.example .env
docker compose up --build
```

Fastembed is compiled into the default images and downloads the local
all-MiniLM-L6-v2 model on first startup. This needs network access to the model
host, but no model API account. Readiness can take several minutes during this
download. The API applies version-controlled migrations before becoming healthy;
the worker starts after API readiness. PostgreSQL jobs provide the durable queue.

For detached startup and verification:

```bash
docker compose up --build -d --wait --wait-timeout 600
curl --fail http://127.0.0.1:8080/health
curl --fail http://127.0.0.1:8080/ready
docker compose ps
docker compose logs --tail 100 api worker
```

`/health` proves the API is serving; `/ready` also checks the database. The release
verification additionally queued a fictional event and observed a `SUCCEEDED`
embedding job using `all-minilm-l6-v2`; ordinary status alone does not prove this.

API and PostgreSQL publish only on host loopback, at ports 8080 and 5432 by
default. Set API and separate curation credentials before allowing access from
other machines. Compose forwards the HTTP credentials only to the API.

## Environment and image options

Native binaries load `.env` and use `OWNSTATE_DATABASE_URL` and
`OWNSTATE_HTTP_ADDR`. Compose reads `.env` for interpolation and supplies explicit
container environment variables; it does not copy the file into images.

| Setting | Compose behavior |
| --- | --- |
| `OWNSTATE_CONTAINER_DATABASE_URL` | Overrides the internal database URL; default `postgres://ownstate:ownstate_dev@postgres:5432/ownstate` uses fictional local credentials. |
| `OWNSTATE_HTTP_PORT` | Host loopback API port, default `8080`; internal bind remains `0.0.0.0:8080`. |
| `OWNSTATE_POSTGRES_PORT` | Host loopback database port, default `5432`. |
| `OWNSTATE_POSTGRES_CONTAINER_NAME` | Default `ownstate-postgres`; use a distinct name for a second stack. |
| `OWNSTATE_IMAGE_FASTEMBED` | Build argument, default `true`; `false` omits the ONNX dependency and requires the deterministic provider. |
| `OWNSTATE_EMBEDDING_PROVIDER` | Runtime provider, default `fastembed`; `deterministic` exercises mechanics without semantic retrieval quality. |
| `OWNSTATE_EMBEDDING_CACHE_DIR` | Native cache setting; Compose fixes the container cache at `/app/.fastembed_cache` on a named volume. |
| `OWNSTATE_AUTO_MIGRATE` | API migration ownership, default `true`; worker/MCP always receive `false`. |
| `OWNSTATE_MAX_DB_CONNECTIONS` | Maximum connections per application process, default `10`. |
| `OWNSTATE_WORKER_POLL_MS` | Worker polling interval, default `1000`. |

Tenant, classification ceiling, logging, and API/admin token settings also come
from the central typed runtime configuration documented in `.env.example`.
Changing database credentials requires updating the PostgreSQL service and the
internal/native connection URLs consistently. Changing bootstrap environment
variables does not change credentials in an already initialized database volume.

For mechanics without a model download, using the normal images:

```bash
OWNSTATE_EMBEDDING_PROVIDER=deterministic docker compose up --build
```

To avoid the ONNX binary download during image compilation as well:

```bash
OWNSTATE_IMAGE_FASTEMBED=false OWNSTATE_EMBEDDING_PROVIDER=deterministic docker compose up --build
```

Keep both settings on later builds/runs of that image. Selecting `fastembed` in an
image compiled without it fails explicitly; it never silently substitutes hashes.
Switching providers leaves existing knowledge intact, but model-specific indexes
are not interchangeable. Historical items need jobs to build embeddings for a new
provider; changing the environment alone does not reindex them.

## MCP clients

Build the MCP image, then let the client launch the following stdio command from
the repository directory:

```bash
docker compose build mcp
docker compose run --rm -T mcp
```

Start API/worker beforehand. `-T` prevents a pseudo-terminal from corrupting the
JSON-RPC stream. Protocol messages use stdout; logs use stderr. Do not run MCP as
a detached network daemon. The `mcp` profile keeps it out of ordinary startup.

Container working directory `/app` is not the client's project. Pass explicit
project IDs, or expose a chosen workspace and launch there:

```bash
docker compose run --rm -T --volume /absolute/project:/workspace:ro --workdir /workspace mcp
```

Only mount the intended workspace; repository metadata detection remains subject
to what is available inside the container. No host home directory is needed.

## Persistence, migrations and shutdown

`ownstate-pgdata` remains the existing Compose database-volume key, mounted at
`/var/lib/postgresql` for PostgreSQL 18. `ownstate-model-cache` contains downloaded
embedding assets and is initialized writable for application UID 10001. Compose
scopes these volumes by project name. Application containers use non-root users;
compiler artifacts and environment files are absent from final images.

```bash
docker compose stop
docker compose start
```

The API handles SIGTERM gracefully. The worker image uses SIGINT to reach its
existing graceful shutdown handler. Stop deadlines are 30 seconds. `docker
compose down` removes containers/network and preserves named volumes. Adding
`--volumes` deletes database and model-cache data; use it only for an explicitly
disposable stack. A second deployment needs a distinct project name, database
container name, and loopback ports.

For an existing externally managed schema, set `OWNSTATE_AUTO_MIGRATE=false`
only after pending migrations have been applied by an administrator. A fresh
database needs API auto-migration enabled. There is currently no separate
migration executable or automatic schema downgrade.

For local logical backups, write the output to an appropriately protected path:

```bash
docker compose exec -T postgres pg_dump -U ownstate -d ownstate -Fc > ownstate.dump
```

Verify restoration into a separate empty PostgreSQL 18/pgvector deployment before
relying on a backup. Preserve database provenance/history, and back up future
object storage alongside database metadata once that adapter exists. The model
cache can be downloaded again; it is not canonical evidence.

## CI and troubleshooting

CI uses the pinned Rust 1.96.0 toolchain and the same four Cargo gates as the
implementation loop, including Clippy with warnings denied. Its PostgreSQL
18/pgvector service proves extension availability and database-creation privileges
before tests create migrated disposable databases. PostgreSQL integration tests
are not mocked or skipped. Python implementation-runner tests run separately.

A separate container job builds API, worker, and MCP with Fastembed compiled in,
then proves fresh-database migration, API readiness, UID 10001, worker startup,
and a real MCP stdio initialize/tool-discovery exchange using the deterministic
provider. This container smoke avoids first-run model downloads and does not
prove default Fastembed startup or semantic quality. CI deletes only its own
disposable project volumes. The workflow's first hosted execution is separate
from local validation evidence.

The 2026-09-16 local release verification also built the default Fastembed images
from settled source and started a completely fresh isolated stack. It applied all
nine migrations, ran API and worker as UID 10001, populated the model cache,
completed a real `all-minilm-l6-v2` embedding job, and negotiated MCP protocol
`2025-11-25` with all required tools. This proves local packaging and default-model
startup mechanics; it does not measure semantic retrieval quality or prove the
workflow has run on the hosted CI service.

If startup fails, inspect service status and limited logs. An unavailable model
host can block Fastembed initialization; retry when connectivity returns or use
the explicit deterministic mechanics mode. Database readiness failures require
checking credentials, server reachability, migration errors, and pgvector
availability. Port/name collisions require a distinct stack configuration;
never delete an existing database volume to clear a collision. Build failures
may come from image/crate/ONNX download access before any application starts.

The builder uses Rust 1.96.0 on Debian Trixie and the runtime uses Trixie slim.
Both need the newer C++ runtime ABI used by the downloaded ONNX static library;
Bookworm failed at linking with missing C++ symbols. Trixie's
[libstdc++ package](https://packages.debian.org/trixie/libstdc%2B%2B6) comes from GCC 14.
Builder tag existence was verified against the Docker registry, including Linux
amd64 and arm64. The upstream [pgvector Docker instructions](https://github.com/pgvector/pgvector#docker)
and [PostgreSQL service guidance](https://github.com/pgvector/setup-pgvector#service)
describe PostgreSQL 18 image usage. Images use explicit release/platform tags;
mutable tags still require controlled updates and rebuilds for deployed systems.
