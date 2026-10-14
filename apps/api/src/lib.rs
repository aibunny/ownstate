//! Ownstate HTTP API. Thin adapter over `ownstate-services`: DTO mapping,
//! bearer-token auth, error → status-code mapping. No business logic here.

pub mod auth;
pub mod conversations;
pub mod dto;
pub mod error;
pub mod handlers;
pub mod institutional;
pub mod oauth;
pub mod router;
pub mod site;
pub mod state;

pub use router::build as build_router;
pub use state::AppState;
