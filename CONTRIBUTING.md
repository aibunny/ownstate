# Contributing to Ownstate

Thank you for your interest in contributing to Ownstate. This guide will help you get started.

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to uphold its standards.

## Getting Started

### Prerequisites

- Rust 1.96+ (see `rust-toolchain.toml`)
- PostgreSQL 18+ with pgvector extension
- Docker and Docker Compose (for the quick start path)
- Python 3 (for test utilities)

### Local Development

```bash
git clone https://github.com/anthropics/ownstate.git
cd ownstate
cp .env.example .env
docker compose up -d postgres
cargo run -p ownstate-api
```

In separate terminals:

```bash
cargo run -p ownstate-worker
cargo run -p ownstate-mcp
```

### Running Tests

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 -m unittest discover -s scripts/tests -v
```

Tests require a running PostgreSQL + pgvector server. The test harness creates disposable databases and cleans up after itself.

## How to Contribute

### Reporting Bugs

Open an issue with:

- A clear description of the problem
- Steps to reproduce
- Expected vs actual behavior
- Your environment (OS, Rust version, PostgreSQL version)

### Suggesting Features

Open an issue with:

- The problem you're trying to solve
- Your proposed solution
- Alternatives you considered

### Submitting Code

1. Fork the repository
2. Create a feature branch from `main`
3. Make your changes
4. Run the full test suite
5. Submit a pull request

### Branch Naming

Use descriptive names:

- `fix/session-timeout-handling`
- `feat/entity-relationship-query`
- `docs/api-endpoint-reference`

### Commit Messages

Write clear commit messages:

```
Add session timeout handling for idle MCP connections

- Set default timeout to 30 minutes
- Return proper MCP error on timeout
- Add configuration option OWNSTATE_MCP_TIMEOUT_MS

Fixes #42
```

### Pull Request Expectations

Your PR should:

- Pass all CI checks (fmt, clippy, tests)
- Include tests for new functionality
- Update documentation if behavior changes
- Be focused on a single change
- Have a clear description of what changed and why

### Adding Migrations

Migration files go in `migrations/` with the pattern `NNNN_description.sql`:

1. Never modify an applied migration
2. Add only additive changes when possible
3. Test against a fresh database
4. Document any irreversible changes

### Adding Dependencies

Before adding a dependency:

1. Check if existing dependencies already provide the functionality
2. Prefer well-maintained, widely-used crates
3. Check the license compatibility (Apache-2.0)
4. Add it to `Cargo.toml` at the workspace level when possible

## Security-Sensitive Areas

The following areas require extra care:

- **Authorization and policy** — never bypass authorization checks
- **Tenant isolation** — never leak data across tenants
- **Model egress** — never send data to unauthorized destinations
- **Credential handling** — never log or persist secrets
- **Input validation** — validate and bound all external input

If you find a security vulnerability, please report it privately. See [SECURITY.md](SECURITY.md).

## Architecture Boundaries

Ownstate has clear layer boundaries:

- **Domain** (`crates/domain/`) — pure types, no infrastructure dependencies
- **Storage** (`crates/storage/`) — PostgreSQL, SQLx, migrations
- **Services** (`crates/services/`) — application logic, authorization
- **API** (`apps/api/`) — Axum HTTP handlers
- **MCP** (`apps/mcp/`) — MCP protocol server
- **Worker** (`apps/worker/`) — background job processing

Do not add infrastructure dependencies (axum, sqlx, rmcp) to the domain crate.

## Documentation

- Update `README.md` if setup or capabilities change
- Update relevant `docs/` files for architectural changes
- Use clear, concise language
- Include examples where helpful

## Questions?

Open an issue for questions about the codebase or contribution process.
