//! Ownstate domain layer.
//!
//! Pure domain types and invariant logic. This crate must stay free of
//! infrastructure dependencies (Axum, SQLx, MCP, embedding runtimes) so that
//! every adapter and storage backend depends on the domain, never the reverse.

pub mod capture;
pub mod entities;
pub mod enums;
pub mod error;
pub mod hash;
pub mod ids;
pub mod institutional;
pub mod limits;
pub mod policy;
pub mod promotion;
pub mod query;
pub mod remote;
pub mod tokens;

pub use capture::*;
pub use entities::*;
pub use enums::*;
pub use error::DomainError;
pub use hash::content_hash;
pub use ids::*;
pub use institutional::*;
pub use policy::*;
pub use promotion::{PromotionDecision, decide_promotion};
pub use query::*;
pub use remote::normalize_remote;
pub use tokens::estimate_tokens;
