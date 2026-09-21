- [x] Migration 0006: unique (tenant_id, lower(name)) — case-insensitive name identity
- [x] Storage: find_by_name case-insensitive; find_by_git_origin; find_by_root_path
- [x] Services: multi-factor ensure_project (origin → root_path → compatible name → create); root_path recorded/updated; origin backfilled, never clobbered on name match; same-name different-repo gets deterministic suffixed project
- [x] MCP: workspace detection includes absolute root_path; bootstrap returns git_origin + root_path
- [x] Tests: 52 green (dlala clone/client dedup, case-insensitivity, path resolution, name-collision separation); clippy 0
- [x] Live-verified against the real dlala project: bootstrap from a different clone location resolved to the existing id, still one row
- [x] Both Claude Code and Codex serve the new build via the shared launcher (release binary rebuilt)
- [x] Ops note: migration was blocked by a leaked idle-in-transaction connection; terminated backend 51480 to unblock

## Applied Updates

### 2026-09-02T10:02:13Z
- (no local file changes detected)
