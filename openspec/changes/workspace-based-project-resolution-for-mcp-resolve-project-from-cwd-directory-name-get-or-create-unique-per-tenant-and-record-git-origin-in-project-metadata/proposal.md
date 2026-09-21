## Why

- Agents shouldn't need to know or pass a project UUID: the directory they
  work in identifies the project, and its git origin is provenance worth
  recording (spec §26 repository awareness, §34 zero-friction integration).

## What Changes

- Migration 0005: UNIQUE (tenant_id, name) on projects — names become
  identities, enabling race-safe get-or-create.
- services: `ensure_project` (get-or-create by name; records/backfills
  `git_origin` in project metadata); create_project maps duplicate names to
  Conflict; bootstrap response now includes git_origin.
- apps/mcp: workspace detection at startup (cwd dir name + `git config
  remote.origin.url`); bootstrap/search/propose take optional
  project_id/project and default to the workspace project.
- Tests: ensure_project semantics, duplicate-name conflict, MCP round-trip
  workspace resolution (50 green, clippy 0).

## Impact

- Zero-config MCP usage from any directory in Claude Code and Codex; explicit
  ids still work. Live-verified from a git repo: project auto-created with
  origin recorded, idempotent across binaries.
