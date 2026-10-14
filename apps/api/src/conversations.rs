use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use ownstate_services::conversations::{
    CaptureConversation, CapturedEvent, ConversationEvent, ConversationSummary,
};
use ownstate_services::policy::AuthenticatedPrincipal;

use crate::error::ApiError;
use crate::state::AppState;

pub async fn capture(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(input): Json<CaptureConversation>,
) -> Result<(StatusCode, Json<CapturedEvent>), ApiError> {
    let captured = state
        .services
        .for_verified_principal(principal)?
        .capture_conversation(input)
        .await?;
    let status = if captured.duplicate {
        StatusCode::OK
    } else {
        StatusCode::CREATED
    };
    Ok((status, Json(captured)))
}

pub async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
) -> Result<Json<Vec<ConversationSummary>>, ApiError> {
    Ok(Json(
        state
            .services
            .for_verified_principal(principal)?
            .list_conversations()
            .await?,
    ))
}

pub async fn events(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(thread_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<ConversationEvent>>, ApiError> {
    Ok(Json(
        state
            .services
            .for_verified_principal(principal)?
            .conversation_events(thread_id)
            .await?,
    ))
}
