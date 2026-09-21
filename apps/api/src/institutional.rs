//! Thin institutional HTTP adapter; all scope and lifecycle rules are services.
use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::StatusCode,
};
use ownstate_domain::*;
use ownstate_services::institutional::{
    InstitutionalQuery, InstitutionalSnapshot, ProposeInstitutional,
};
use ownstate_services::policy::AuthenticatedPrincipal;
use serde::Deserialize;

use crate::{error::ApiError, state::AppState};

pub async fn propose(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<ProposeInstitutional>,
) -> Result<(StatusCode, Json<GraphCandidate>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .services
                .for_principal(principal)?
                .propose_institutional(req)
                .await?,
        ),
    ))
}

pub async fn promote(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<GraphCandidateId>,
) -> Result<Json<InstitutionalVersion>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .promote_institutional(id)
            .await?,
    ))
}

pub async fn entities(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Query(q): Query<InstitutionalQuery>,
) -> Result<Json<InstitutionalSnapshot>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .institutional_snapshot(q, None, Some("ENTITY"))
            .await?,
    ))
}
pub async fn relationships(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Query(q): Query<InstitutionalQuery>,
) -> Result<Json<InstitutionalSnapshot>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .institutional_snapshot(q, None, Some("RELATIONSHIP"))
            .await?,
    ))
}

pub async fn entity_relationships(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<EntityId>,
    Query(q): Query<InstitutionalQuery>,
) -> Result<Json<Vec<InstitutionalVersion>>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .entity_relationships(q.project_id, id, q.limit.unwrap_or(100), q.as_of)
            .await?,
    ))
}
pub async fn claims(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Query(q): Query<InstitutionalQuery>,
) -> Result<Json<InstitutionalSnapshot>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .institutional_snapshot(q, None, Some("CLAIM"))
            .await?,
    ))
}
pub async fn get_entity(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<EntityId>,
    Query(q): Query<InstitutionalQuery>,
) -> Result<Json<InstitutionalSnapshot>, ApiError> {
    let snapshot = state
        .services
        .for_principal(principal)?
        .institutional_snapshot(q, Some(id.as_uuid()), Some("ENTITY"))
        .await?;
    if snapshot.versions.is_empty() {
        return Err(ownstate_services::ServiceError::NotFound("entity").into());
    }
    Ok(Json(snapshot))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveQuery {
    pub project_id: ProjectId,
    pub value: String,
    pub namespace: Option<String>,
}
pub async fn resolve(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Query(q): Query<ResolveQuery>,
) -> Result<Json<EntityResolution>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .resolve_entity(q.project_id, &q.value, q.namespace.as_deref(), None)
            .await?,
    ))
}

pub async fn history(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<EntityId>,
    Query(q): Query<InstitutionalQuery>,
) -> Result<Json<Vec<InstitutionalVersion>>, ApiError> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .institutional_history(q.project_id, id.as_uuid())
            .await?,
    ))
}
