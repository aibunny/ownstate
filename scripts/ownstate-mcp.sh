#!/usr/bin/env bash
# Launcher for the Ownstate MCP server (stdio).
# MCP clients (Claude Code / Claude Desktop) start servers from arbitrary
# working directories, so everything here is absolute and self-contained.
set -euo pipefail

OWNSTATE_DIR="/Users/aibunny/ownstate"

export OWNSTATE_DATABASE_URL="${OWNSTATE_DATABASE_URL:-postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate}"
export OWNSTATE_TENANT_ID="${OWNSTATE_TENANT_ID:-00000000-0000-0000-0000-000000000001}"
export OWNSTATE_EMBEDDING_PROVIDER="${OWNSTATE_EMBEDDING_PROVIDER:-fastembed}"
export OWNSTATE_EMBEDDING_CACHE_DIR="${OWNSTATE_EMBEDDING_CACHE_DIR:-$OWNSTATE_DIR/.fastembed_cache}"
# The API owns schema migrations; the MCP server only needs to read/propose.
export OWNSTATE_AUTO_MIGRATE="${OWNSTATE_AUTO_MIGRATE:-false}"
export RUST_LOG="${RUST_LOG:-warn}"

if [[ -x "$OWNSTATE_DIR/target/release/ownstate-mcp" ]]; then
    exec "$OWNSTATE_DIR/target/release/ownstate-mcp"
else
    exec "$OWNSTATE_DIR/target/debug/ownstate-mcp"
fi
