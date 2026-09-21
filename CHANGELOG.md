# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Apache-2.0 license
- CONTRIBUTING.md with development setup and PR guidelines
- SECURITY.md with vulnerability reporting process
- CODE_OF_CONDUCT.md (Contributor Covenant 2.0)
- CHANGELOG.md
- GitHub issue and PR templates
- Domain types for automatic capture: CaptureMode, ScopeKind, ResolutionState, ChannelCompleteness
- Domain types for scope handles: ScopeHandle, RepositoryLocator, CaptureAssessment
- Pure repository remote normalization with credential stripping (SCP, SSH, HTTPS)
- Migration 0012: repository identity extensions, scope handles, capture assessments, idempotency keys
- Storage modules: repositories, scope_handles, capture
- MCP `ownstate.record` tool for append-only evidence recording

### Changed
- README.md rewritten for public release clarity
- .gitignore expanded to exclude IDE files and common artifacts
- Cargo.toml license field updated to Apache-2.0

### Removed
- .idea/ directory from version control

## [0.1.0] - 2026-09-21

### Added
- Initial public release
- Rust workspace with 8 crates
- HTTP API (Axum) with evidence, knowledge, context, and query endpoints
- MCP server with bootstrap, search, propose, and record tools
- Background worker for embedding jobs
- PostgreSQL + pgvector storage layer
- 12 migrations covering evidence, knowledge, retrieval, institutional state, and policy
- Personal and business deployment modes
- Typed query API (Structured, Semantic, Relationship, Temporal, Hybrid)
- Docker multi-stage build with non-root runtime
- Docker Compose for local development
- CI with native tests and container smoke tests
