//! Ownstate MCP server: the canonical (and only) MCP interface.
//!
//! Read tools (`bootstrap_project`, `search_knowledge`, `get_knowledge`) and
//! one write tool (`propose_knowledge`) that can only create PENDING
//! candidates — MCP clients get no path to canonical state, SQL, or policy.
//! All tools call the same application services as the HTTP API, with the
//! same tenant scoping.

pub mod server;
pub mod workspace;

pub use server::OwnstateMcp;
pub use workspace::Workspace;
