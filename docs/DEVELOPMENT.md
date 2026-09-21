# Development Guide

## Prerequisites

- **Rust** 1.96+ (see `rust-toolchain.toml`)
- **PostgreSQL** 18+ with pgvector extension
- **Docker** and Docker Compose (for the quick start path)
- **Python 3** (for test utilities)

## Quick Start

```bash
git clone https://github.com/anthropics/ownstate.git
cd ownstate
cp .env.example .env
docker compose up -d postgres
cargo run -p ownstate-api
```

## Native Development

### Start the database

```bash
docker compose up -d postgres
```

This starts PostgreSQL 18 + pgvector on `localhost:5432`.

### Run the API

```bash
cargo run -p ownstate-api
```

The API starts on `http://localhost:8080` and runs migrations automatically.

### Run the worker

```bash
cargo run -p ownstate-worker
```

The worker processes background jobs (embedding generation).

### Run the MCP server

```bash
cargo run -p ownstate-mcp
```

The MCP server uses stdio and is typically started by an MCP client.

## Docker Development

### Full stack

```bash
docker compose up --build
```

### Database only

```bash
docker compose up -d postgres
```

### Without model download

```bash
OWNSTATE_IMAGE_FASTEMBED=false docker compose build
docker compose up
```

## Configuration

All configuration comes from environment variables. See [.env.example](../.env.example) for the complete list.

Key variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `OWNSTATE_DATABASE_URL` | `postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate` | PostgreSQL connection |
| `OWNSTATE_HTTP_ADDR` | `127.0.0.1:8080` | API bind address |
| `OWNSTATE_DEPLOYMENT_MODE` | `personal` | `personal` or `business` |
| `OWNSTATE_EMBEDDING_PROVIDER` | `fastembed` | `fastembed` or `deterministic` |
| `OWNSTATE_AUTO_MIGRATE` | `true` | Run migrations on startup |
| `RUST_LOG` | `info` | Log filter |

## Migrations

Migrations are in `migrations/` and use SQLx's embedded migration system.

- Migrations run automatically on API startup when `OWNSTATE_AUTO_MIGRATE=true`
- Never modify an applied migration
- Add only additive changes when possible
- Test against a fresh database

### Adding a migration

1. Create `migrations/NNNN_description.sql`
2. Use the next sequence number
3. Test against a clean database
4. Document any irreversible changes

## Testing

### Rust tests

```bash
cargo test --workspace
```

Tests require a running PostgreSQL + pgvector server. The harness creates disposable databases and cleans up after itself.

### Python tests

```bash
python3 -m unittest discover -s scripts/tests -v
```

### Full quality suite

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s scripts/tests -v
```

## Useful Commands

```bash
# Format code
cargo fmt

# Check compilation
cargo check --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Run API
cargo run -p ownstate-api

# Run worker
cargo run -p ownstate-worker

# Run MCP server
cargo run -p ownstate-mcp

# Start database
docker compose up -d postgres

# Build Docker images
docker compose build

# Full Docker stack
docker compose up --build
```

## Debugging

### Logs

Set `RUST_LOG` for more verbose output:

```bash
RUST_LOG=debug cargo run -p ownstate-api
```

### Database inspection

```bash
docker compose exec postgres psql -U ownstate -d ownstate
```

### Test database

The test harness creates databases named `ownstate_test_*` and cleans them up automatically.

## Architecture

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full architecture.

Key boundaries:

- **Domain** (`crates/domain/`) — pure types, no infrastructure
- **Storage** (`crates/storage/`) — PostgreSQL, SQLx
- **Services** (`crates/services/`) — application logic, authorization
- **API** (`apps/api/`) — Axum HTTP handlers
- **MCP** (`apps/mcp/`) — MCP protocol server
- **Worker** (`apps/worker/`) — background jobs

Do not add infrastructure dependencies to the domain crate.
