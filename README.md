# Ownstate

> Infrastructure for preserving and reusing the context, evidence, and knowledge created while people and organizations work with AI.

Ownstate is a sovereign knowledge layer for individuals and organizations. The core idea: **models, agents, vendors, and applications should be replaceable. The knowledge accumulated while using them should belong to the individual or organization.**

Ownstate continuously builds and maintains a durable, permission-aware representation of what an organization knows, what happened, what decisions were made, and what is currently true. It is not a chatbot, model, vector database, or agent framework. It is the persistent institutional state that humans, models, and agents can use.

## Why Ownstate

Today, knowledge created while working with AI is fragmented across conversations, coding agents, documents, email, Slack, and CRM systems. Each new AI session reconstructs context that another AI already learned. Ownstate prevents that loss by providing:

- **Persistent context** that survives across models, agents, and sessions
- **Evidence-grounded knowledge** with full provenance and temporal history
- **Permission-aware retrieval** with data classification and tenant isolation
- **Provider independence** — swap models, agents, and tools without losing institutional knowledge

## What it does today

**Implemented:**

- Append-only interaction evidence with content hashing
- Candidate knowledge promotion with deterministic rules
- Immutable canonical knowledge versions with provenance
- PostgreSQL full-text + pgvector/RRF hybrid retrieval
- Classification ceilings and tenant/project filtering
- Audited context packets with token budgets
- Temporal institutional entities, aliases, relationships, and claims
- Typed query API (Structured, Semantic, Relationship, Temporal, Hybrid)
- Workspace project resolution from Git origin or directory name
- MCP server with bootstrap, search, propose, and record tools
- HTTP API with bearer token authentication
- Background worker for embedding jobs
- Docker multi-stage build with non-root runtime

**Experimental:**

- Local fastembed semantic embeddings (ONNX, downloaded on first run)
- Deterministic hash-based embeddings for development
- Personal mode with automatic local owner bootstrap
- Static API/admin credentials for development

**Planned:**

- Automatic cross-host capture with factual completeness
- Canonical repository identity with credential stripping
- Opaque MCP scope handles for secure context passing
- Provisional non-code scopes (threads, artifacts, relationships)
- Fine-grained principal and model-egress policies
- Provider-neutral model/harness runtimes
- Permission-scoped agent execution and swarms
- Optional connectors and transactional outbox/EventBus

## What it is not

- Not a chatbot or AI model
- Not a vector database (though it uses pgvector for retrieval)
- Not an agent framework (though agents can use it)
- Not a hosted service (though it can be deployed as one)
- Not a replacement for your database (it is a knowledge layer on top of PostgreSQL)

## Architecture at a glance

```text
Raw Evidence (append-only)
    ↓
Candidate Knowledge (untrusted proposals)
    ↓
Novelty / Validation / Policy
    ↓
Canonical Knowledge (immutable versions with provenance)
    ↓
Query / Retrieval (full-text + semantic + temporal)
    ↓
Context Compiler (budgeted, classified, audited)
    ↓
Human / Model / Agent
    ↓
New Work → Raw Evidence
```

Key architectural properties:

- **Evidence is append-only** — raw interaction events cannot be modified or deleted
- **Knowledge versions are immutable** — once promoted, content never changes
- **Authorization is default-deny** — every operation requires explicit grants
- **Models are untrusted** — AI output is treated as candidate knowledge, never direct truth
- **Scope handles are opaque** — server-minted references, not credentials

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the full architecture.

## Quick start

```bash
git clone https://github.com/anthropics/ownstate.git
cd ownstate
cp .env.example .env
docker compose up --build
```

The first startup downloads the Fastembed ONNX model (~50MB). Subsequent starts use the cached model.

**Verify it's running:**

```bash
curl http://localhost:8080/health
curl http://localhost:8080/ready
```

**Services:**

| Service | Port | Description |
|---------|------|-------------|
| API | 8080 | HTTP REST API |
| PostgreSQL | 5432 | Database (localhost only) |
| MCP | stdio | MCP protocol server (started by clients) |

## Configuration

Copy `.env.example` to `.env` and adjust as needed:

```bash
cp .env.example .env
```

Key configuration variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `OWNSTATE_DATABASE_URL` | `postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate` | PostgreSQL connection |
| `OWNSTATE_HTTP_ADDR` | `127.0.0.1:8080` | API bind address |
| `OWNSTATE_DEPLOYMENT_MODE` | `personal` | `personal` or `business` |
| `OWNSTATE_API_TOKEN` | (unset) | Bearer token for API auth |
| `OWNSTATE_EMBEDDING_PROVIDER` | `fastembed` | `fastembed` or `deterministic` |
| `OWNSTATE_AUTO_MIGRATE` | `true` | Run migrations on startup |

See [.env.example](.env.example) for the complete list.

**Personal vs Business mode:**

- **Personal** — automatically bootstraps local owner/service grants. Good for individual use.
- **Business** — defaults to deny, requires pre-provisioned principal credentials. For team/organization deployments.

## Running with Docker

The default Compose setup includes PostgreSQL 18 + pgvector, API, and worker:

```bash
docker compose up --build
```

Optional MCP server (stdio, started by clients):

```bash
docker compose --profile mcp up --build mcp
```

**Docker images:**

| Target | Description |
|--------|-------------|
| `api` | HTTP API server |
| `worker` | Background job processor |
| `mcp` | MCP protocol server |

Build without Fastembed (smaller image, no model download):

```bash
OWNSTATE_IMAGE_FASTEMBED=false docker compose build
```

## Running locally

**Prerequisites:**

- Rust 1.96+ (`rustup install 1.96.0`)
- PostgreSQL 18+ with pgvector
- Docker (for the database, or use a local PostgreSQL)

**Start the database:**

```bash
docker compose up -d postgres
```

**Run the API:**

```bash
cargo run -p ownstate-api
```

**Run the worker (separate terminal):**

```bash
cargo run -p ownstate-worker
```

**Run the MCP server (separate terminal):**

```bash
cargo run -p ownstate-mcp
```

All binaries load `.env` automatically.

## MCP / API usage

### MCP Tools

The MCP server exposes these tools:

| Tool | Description |
|------|-------------|
| `bootstrap_project` | Load project knowledge state at session start |
| `search_knowledge` | Hybrid search (full-text + semantic) |
| `get_knowledge` | Full version history and provenance |
| `compile_context` | Budgeted context packet for a task |
| `propose_knowledge` | Propose new candidate knowledge |
| `record` | Record observable events as append-only evidence |
| `get_entity` | Current institutional entity with provenance |
| `get_relationships` | Institutional relationships |

### HTTP API Examples

**Create a project:**

```bash
curl -X POST http://localhost:8080/projects \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OWNSTATE_API_TOKEN" \
  -d '{"name": "my-project"}'
```

**Search knowledge:**

```bash
curl "http://localhost:8080/knowledge/search?project_id=YOUR_PROJECT_ID&query=architecture"
```

**Typed query:**

```bash
curl -X POST http://localhost:8080/query \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer $OWNSTATE_API_TOKEN" \
  -d '{
    "type": "STRUCTURED",
    "constraints": {
      "project_id": "YOUR_PROJECT_ID",
      "max_classification": "INTERNAL",
      "entity_kind": "LEGAL_ENTITY",
      "limit": 10
    },
    "operation": "COUNT"
  }'
```

See [docs/API.md](docs/API.md) for the full API reference.

## Project structure

```text
ownstate/
├── apps/
│   ├── api/          # Axum HTTP API server
│   ├── worker/       # Background job processor
│   └── mcp/          # MCP protocol server
├── crates/
│   ├── domain/       # Pure domain types (no infrastructure)
│   ├── storage/      # PostgreSQL persistence
│   ├── services/     # Application logic and authorization
│   ├── embeddings/   # Embedding providers
│   └── runtime/      # Configuration and runtime setup
├── migrations/       # PostgreSQL migrations (SQLx)
├── scripts/          # Test utilities and implementation loop
├── docs/             # Architecture, requirements, and design docs
└── .github/          # CI workflows and templates
```

**Key boundaries:**

- `domain` — pure types, no framework dependencies
- `storage` — all SQL lives here, no business logic
- `services` — authorization-gated use cases
- `apps` — protocol adapters (HTTP, MCP, worker)

## Development

### Prerequisites

- Rust 1.96+ (see `rust-toolchain.toml`)
- PostgreSQL 18+ with pgvector
- Python 3 (for test utilities)

### Useful commands

```bash
# Format
cargo fmt

# Check
cargo check --workspace

# Lint
cargo clippy --workspace --all-targets -- -D warnings

# Test
cargo test --workspace

# Python tests
python3 -m unittest discover -s scripts/tests -v

# Run API locally
cargo run -p ownstate-api

# Run worker locally
cargo run -p ownstate-worker

# Run MCP server locally
cargo run -p ownstate-mcp

# Start database only
docker compose up -d postgres
```

### Database

Migrations run automatically on API startup when `OWNSTATE_AUTO_MIGRATE=true`.

To run manually:

```bash
cargo run -p ownstate-api  # migrations run at startup
```

Test databases are created and cleaned up automatically by the test harness.

## Testing

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s scripts/tests -v
```

Tests require a running PostgreSQL + pgvector server. The harness creates disposable databases and cleans up after itself. No paid model API is needed.

## Current status / roadmap

Ownstate is **early-stage** (v0.1.0). The core evidence-to-knowledge pipeline is functional. Interfaces may change before a stable release.

**Current focus:**

- Automatic capture and scope resolution
- Host adapter evidence and capability matrix
- Production hardening and security audit

**Roadmap:**

- Artifact versioning and object storage
- Fine-grained authorization policies
- Agent execution framework
- Hosted deployment option

See [CHANGELOG.md](CHANGELOG.md) for release history and [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md) for the full requirements.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, PR guidelines, and contribution expectations.

## Security

See [SECURITY.md](SECURITY.md) for vulnerability reporting and security design principles.

## License

Apache-2.0 — see [LICENSE](LICENSE) for details.
