## Why

The completed architecture review used a now-superseded handoff spread across
root and `docs/` files. The next model needs one unambiguous task package for
automatic capture, stable repository/non-code scope resolution, compact MCP
context bootstrap, honest capture guarantees, and a hard prohibition on
model-controlled deletion of recorded Ownstate data.

## What Changes

- Move the target architecture and requirements from the repository root to their
  active `docs/` locations and remove the root duplicates.
- Move superseded handoff, ledger, gap, security, bundle, and prior current-doc
  files out of active paths into a dated recovery archive.
- Add a runnable OpenCode prompt, normative automatic-capture/scope requirements,
  a dependency-ordered implementation plan, MCP capture security contract, and a
  host capability evidence matrix.
- Update architecture, requirements, README, loop, rules, and state so the new
  authority and security boundaries are consistent.
- Correct the MCP session assumption: application scope uses explicit server
  records/handles and never transport session identity, across supported protocol
  revisions.

## Impact

- Documentation and task metadata only; no application, schema, migration,
  container, or runtime behavior changes in this handoff.
- Previous documents remain recoverable under `docs/archive/` because the
  workspace has no usable Git history.
- The future implementation must create its own OpenSpec change and verify the
  existing source baseline before editing code.

## Changed paths

Active additions/updates:

- `OPENCODE_CAPTURE_SCOPE_PROMPT.md`
- `README.md`
- `STATE.md`
- `docs/ARCHITECTURE.md`
- `docs/REQUIREMENTS.md`
- `docs/AUTOMATIC_CAPTURE_SCOPE_REQUIREMENTS.md`
- `docs/AUTOMATIC_CAPTURE_IMPLEMENTATION_PLAN.md`
- `docs/MCP_CAPTURE_SECURITY.md`
- `docs/HOST_CAPTURE_CAPABILITY_MATRIX.md`
- `docs/IMPLEMENTATION_LOOP.md`
- `docs/RULES.md`

Superseded active paths were moved to
`docs/archive/2026-09-20-architecture-completion-handoff/`; root
`ARCHITECTURE.md` and `REQUIREMENTS.md` moved to `docs/`.
