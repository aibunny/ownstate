# Host capture capability matrix

This is a required evidence record. Do not fill a cell from memory or assumption.
For each host, cite primary documentation or a reproducible local test, including
host version and verification date. `Unknown` is preferable to a false claim.

| Host/integration | Version/date | User messages | Assistant messages | Tool calls/results | Shell commands/results | File reads/writes/diffs | Artifacts | Start/end | Stable source conversation/event IDs | Repository context | Capture mode justified | Evidence/limitations |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Generic MCP | Unverified | Unknown | Unknown | Ownstate tool calls only | Unknown | Unknown | Explicit tool arguments only | Transport-dependent | Unknown | Client-supplied evidence only | `MCP_ONLY` maximum until verified | MCP installation alone does not expose host-private activity. |
| Codex Desktop/CLI | Unverified | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | `UNKNOWN` | Verify supported hooks/APIs and user authorization. |
| Claude / Claude Code | Unverified | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | `UNKNOWN` | Verify supported hooks/APIs and user authorization. |
| OpenCode | Unverified | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | Unknown | `UNKNOWN` | Verify supported hooks/APIs and user authorization. |
| API gateway | Implementation-specific | Depends on gateway | Depends on gateway | Depends on gateway | Not implied | Not implied | Explicit payloads only | Depends on gateway | Depends on gateway | Explicit payloads only | `PARTIAL` until contract proves more | Document concrete deployment adapter. |
| Transcript import | Format-specific | Format-dependent | Format-dependent | Format-dependent | Format-dependent | Format-dependent | Format-dependent | Import boundary only | Format-dependent | Imported metadata only | `IMPORTED` | Parser must report omitted/unsupported channels. |

## Verification checklist per adapter

- What documented, permissioned integration surface is used?
- Can it observe events automatically, or only when the model calls a tool?
- Which channels are complete for a precisely bounded session?
- Which events may be dropped, redacted, sampled, or reordered?
- Are stable source IDs provided? What is their scope and lifetime?
- Can duplicate/replayed delivery occur?
- Can repository root/remotes be verified locally without trusting model text?
- How are secrets, private content, host permissions, and opt-in handled?
- What happens on crash, reconnect, partial failure, or host upgrade?
- Which contract and end-to-end tests reproduce the claimed guarantees?

