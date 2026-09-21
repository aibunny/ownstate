# API Reference

Ownstate exposes an HTTP API for managing evidence, knowledge, and context.

## Authentication

Protected endpoints require a Bearer token:

```bash
Authorization: Bearer YOUR_TOKEN
```

Set `OWNSTATE_API_TOKEN` in your `.env` file. If unset, the API runs unauthenticated (development only).

## Base URL

```
http://localhost:8080
```

## Endpoints

### Health

```
GET /health
```

Returns 200 if the service is running. No authentication required.

### Ready

```
GET /ready
```

Returns 200 if the service is ready to accept requests. No authentication required.

---

### Projects

#### Create Project

```
POST /projects
```

```json
{
  "name": "my-project",
  "description": "Optional description"
}
```

Response: `201 Created` with project object.

#### Get Project

```
GET /projects/{id}
```

Response: `200 OK` with project object.

---

### Sessions

#### Create Session

```
POST /sessions
```

```json
{
  "project_id": "uuid",
  "source": "claude-code",
  "agent": "optional-agent-name"
}
```

Response: `201 Created` with session object.

#### Get Session

```
GET /sessions/{id}
```

Response: `200 OK` with session object.

---

### Events

#### Append Events

```
POST /sessions/{id}/events
```

```json
[
  {
    "event_type": "USER_MESSAGE",
    "actor_type": "USER",
    "content": "What is our auth architecture?",
    "source_event_id": "optional-idempotency-key"
  }
]
```

Response: `201 Created` with event objects.

#### List Events

```
GET /sessions/{id}/events
```

Response: `200 OK` with event array.

---

### Knowledge

#### Propose Knowledge

```
POST /knowledge/proposals
```

```json
{
  "project_id": "uuid",
  "kind": "ARCHITECTURE",
  "subject_key": "auth-architecture",
  "content": "We use JWT with RS256 signing...",
  "confidence": 0.9
}
```

Response: `201 Created` with candidate object.

#### Promote Candidate

```
POST /knowledge/proposals/{id}/promote
```

Response: `200 OK` with promoted knowledge item and version.

#### Reject Candidate

```
POST /knowledge/proposals/{id}/reject
```

```json
{
  "reason": "Outdated information"
}
```

Response: `200 OK`.

#### Search Knowledge

```
GET /knowledge/search?project_id=uuid&query=architecture&limit=10
```

Response: `200 OK` with search hits.

#### Get Knowledge

```
GET /knowledge/{id}
```

Response: `200 OK` with item, versions, and evidence.

---

### Context

#### Compile Context

```
POST /context/compile
```

```json
{
  "project_id": "uuid",
  "task": "Implement authentication",
  "token_budget": 4000
}
```

Response: `200 OK` with compiled context packet.

---

### Query

#### Execute Query

```
POST /query
```

```json
{
  "type": "STRUCTURED",
  "constraints": {
    "project_id": "uuid",
    "max_classification": "INTERNAL",
    "entity_kind": "LEGAL_ENTITY",
    "limit": 10
  },
  "operation": "COUNT"
}
```

Query types: `STRUCTURED`, `SEMANTIC`, `RELATIONSHIP`, `TEMPORAL`, `HYBRID`

Response: `200 OK` with query results.

---

### Institutional

#### Propose Entity

```
POST /institutional/proposals
```

Response: `201 Created` with graph candidate.

#### Promote Entity

```
POST /institutional/proposals/{id}/promote
```

Response: `200 OK` with institutional version.

#### List Entities

```
GET /entities?project_id=uuid
```

Response: `200 OK` with entity list.

#### Resolve Entity

```
GET /entities/resolve?project_id=uuid&value=Acme+Corp
```

Response: `200 OK` with resolution result.

#### Get Entity

```
GET /entities/{id}
```

Response: `200 OK` with entity details.

#### Get Entity Relationships

```
GET /entities/{id}/relationships
```

Response: `200 OK` with relationship list.

#### Entity History

```
GET /institutional/{id}/history
```

Response: `200 OK` with version history.

---

## Error Responses

All errors return:

```json
{
  "error": "description of the error"
}
```

Common status codes:

- `400` — Invalid request
- `401` — Missing or invalid authentication
- `403` — Authorization denied
- `404` — Resource not found
- `409` — Conflict (e.g., duplicate name)
- `500` — Internal error

## Rate Limiting

No rate limiting is currently implemented. This is planned for production deployments.

## Body Size Limit

Request body limit: 4 MiB.
