# MCP Server

Ownstate includes an MCP (Model Context Protocol) server that provides AI models with access to persistent knowledge and context.

## Overview

The MCP server is one interface into Ownstate — not the whole product. It allows AI models to:

- Bootstrap project context at session start
- Search existing knowledge
- Record observable events as evidence
- Propose new knowledge candidates
- Query institutional entities and relationships

## Running

### With Docker

```bash
docker compose --profile mcp up --build mcp
```

The MCP server uses stdio transport and is typically started by an MCP client.

### Locally

```bash
cargo run -p ownstate-mcp
```

## Client Configuration

### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "ownstate": {
      "command": "path/to/ownstate-mcp",
      "env": {
        "OWNSTATE_DATABASE_URL": "postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate",
        "OWNSTATE_DEPLOYMENT_MODE": "personal"
      }
    }
  }
}
```

### Claude Code

```bash
claude mcp add ownstate -- path/to/ownstate-mcp
```

## Tools

### bootstrap_project

Load a project's current knowledge state at session start.

**Input:**

```json
{
  "project_id": "optional-uuid",
  "project": "optional-project-name",
  "max_items": 20,
  "agent": "optional-agent-name"
}
```

**Behavior:**

- If no `project_id` or `project` is given, resolves from the current workspace directory
- Creates the project on first use
- Records Git origin if available
- Returns knowledge summaries, pending candidate counts, and a context packet

**Output:** JSON with project info, knowledge items, and audit trail.

---

### search_knowledge

Hybrid search (full-text + semantic) over a project's knowledge.

**Input:**

```json
{
  "query": "authentication architecture",
  "project_id": "optional-uuid",
  "project": "optional-project-name",
  "limit": 10,
  "kinds": ["ARCHITECTURE", "DECISION"]
}
```

**Output:** Ranked knowledge items with trust levels and evidence references.

---

### get_knowledge

Fetch one knowledge item with full version history and provenance.

**Input:**

```json
{
  "knowledge_item_id": "uuid"
}
```

**Output:** Item details, all versions, and evidence links.

---

### compile_context

Compile a budgeted context packet for a specific task.

**Input:**

```json
{
  "project_id": "uuid",
  "task": "Implement authentication",
  "max_items": 20,
  "token_budget": 4000
}
```

**Output:** Context packet with knowledge items, token estimate, and audit record.

---

### propose_knowledge

Propose new candidate knowledge. Proposals are reviewed by deterministic promotion rules.

**Input:**

```json
{
  "project_id": "optional-uuid",
  "project": "optional-project-name",
  "kind": "FACT",
  "subject_key": "auth-mechanism",
  "content": "We use JWT with RS256 signing",
  "confidence": 0.9
}
```

**Output:** Candidate ID and status.

**Note:** MCP callers are AI agents by default. Proposals are always attributed as `AGENT` trust level, never `HUMAN_EXPLICIT`.

---

### record

Record an observable event as append-only evidence. This is the non-negotiable write path for capture.

**Input:**

```json
{
  "project_id": "optional-uuid",
  "project": "optional-project-name",
  "event_type": "FILE_WRITE",
  "actor_type": "ASSISTANT",
  "content": "Updated auth middleware",
  "file_path": "src/auth/middleware.rs",
  "source_event_id": "optional-idempotency-key"
}
```

**Output:** Event ID, session ID, project ID.

**Behavior:**

- Creates raw evidence only — does not promote knowledge
- Supports idempotency via `source_event_id`
- Cannot delete, modify, or rewrite existing evidence

---

### get_entity

Get one institutional entity with its current version and provenance.

**Input:**

```json
{
  "project_id": "uuid",
  "entity_id": "uuid"
}
```

**Output:** Entity details with version history and evidence.

---

### get_relationships

Get institutional relationships involving one entity.

**Input:**

```json
{
  "project_id": "uuid",
  "entity_id": "uuid",
  "limit": 100
}
```

**Output:** Relationship list with provenance.

## Security

- All tools require authorization through `PrincipalServices`
- Models are untrusted — their proposals are candidates, not truth
- Scope handles are opaque references, not credentials
- The MCP interface cannot delete, modify, or rewrite existing data
- Classification ceilings are enforced on all retrieval

See [MCP_CAPTURE_SECURITY.md](MCP_CAPTURE_SECURITY.md) for the full security contract.

## Limitations

- MCP context integration is not proof of complete transcript capture
- The server does not inspect client filesystems
- Model-generated names are never proof of identity
- Semantic similarity alone cannot merge entities
