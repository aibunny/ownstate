use axum::Json;
use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use ownstate_domain::{
    CandidateId, CandidateKnowledge, InteractionEvent, KnowledgeItemId, KnowledgeKind, Project,
    ProjectId, SecurityClassification, Session, SessionId,
};
use ownstate_services::context::CompiledContext;
use ownstate_services::knowledge::ProposeKnowledge;
use ownstate_services::policy::AuthenticatedPrincipal;
use ownstate_services::projects::CreateProject;
use ownstate_services::search::SearchParams;
use ownstate_services::sessions::CreateSession;
use serde_json::{Value as Json_, json};

use crate::dto::*;
use crate::error::ApiError;
use crate::state::AppState;

type ApiResult<T> = Result<T, ApiError>;

pub async fn health() -> Json<Json_> {
    Json(json!({ "status": "ok" }))
}

pub async fn ready(State(state): State<AppState>) -> ApiResult<Json<Json_>> {
    state.services.ping().await?;
    Ok(Json(json!({ "status": "ready" })))
}

pub async fn create_project(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<CreateProjectRequest>,
) -> ApiResult<(StatusCode, Json<Project>)> {
    let project = state
        .services
        .for_principal(principal)?
        .create_project(CreateProject {
            name: req.name,
            description: req.description,
            ownership_domain: req.ownership_domain,
            metadata: req.metadata,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(project)))
}

pub async fn get_project(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<ProjectId>,
) -> ApiResult<Json<Project>> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .get_project(id)
            .await?,
    ))
}

pub async fn create_session(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<CreateSessionRequest>,
) -> ApiResult<(StatusCode, Json<Session>)> {
    let session = state
        .services
        .for_principal(principal)?
        .create_session(CreateSession {
            project_id: req.project_id,
            source: req.source,
            source_session_id: req.source_session_id,
            agent: req.agent,
            provider: req.provider,
            model: req.model,
            repository: req.repository,
            initial_commit: req.initial_commit,
            metadata: req.metadata,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(session)))
}

pub async fn get_session(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<SessionId>,
) -> ApiResult<Json<Session>> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .get_session(id)
            .await?,
    ))
}

pub async fn append_events(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<SessionId>,
    Json(req): Json<AppendEventsRequest>,
) -> ApiResult<(StatusCode, Json<Vec<InteractionEvent>>)> {
    let events = state
        .services
        .for_principal(principal)?
        .append_events(id, req.events)
        .await?;
    Ok((StatusCode::CREATED, Json(events)))
}

pub async fn list_events(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<SessionId>,
) -> ApiResult<Json<Vec<InteractionEvent>>> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .list_events(id)
            .await?,
    ))
}

pub async fn propose_knowledge(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<ProposeKnowledgeRequest>,
) -> ApiResult<(StatusCode, Json<CandidateKnowledge>)> {
    let candidate = state
        .services
        .for_principal(principal)?
        .propose_knowledge(ProposeKnowledge {
            project_id: req.project_id,
            session_id: req.session_id,
            kind: req.kind,
            subject_key: req.subject_key,
            content: req.content,
            structured_content: req.structured_content,
            confidence: req.confidence,
            proposed_by: req.proposed_by,
            source: req.source.or_else(|| Some("api".to_string())),
            evidence_event_ids: req.evidence_event_ids,
            security_classification: req.security_classification,
        })
        .await?;
    Ok((StatusCode::CREATED, Json(candidate)))
}

pub async fn promote_candidate(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<CandidateId>,
) -> ApiResult<Json<PromotionResponse>> {
    let (item, version) = state
        .services
        .for_principal(principal)?
        .promote_candidate(id)
        .await?;
    Ok(Json(PromotionResponse { item, version }))
}

pub async fn reject_candidate(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<CandidateId>,
    Json(req): Json<RejectCandidateRequest>,
) -> ApiResult<StatusCode> {
    if req.reason.trim().is_empty() {
        return Err(ApiError::bad_request("reason is required"));
    }
    state
        .services
        .for_principal(principal)?
        .reject_candidate(id, &req.reason)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn search_knowledge(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<SearchHitResponse>>> {
    let kinds = match &q.kinds {
        Some(raw) => {
            let parsed: Result<Vec<KnowledgeKind>, _> = raw
                .split(',')
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().parse::<KnowledgeKind>())
                .collect();
            Some(parsed.map_err(|e| ApiError::bad_request(e.to_string()))?)
        }
        None => None,
    };

    let hits = state
        .services
        .for_principal(principal)?
        .search_knowledge(SearchParams {
            project_id: q.project_id,
            query: q.q,
            kinds,
            limit: q.limit.unwrap_or(10),
            max_classification: q
                .max_classification
                .unwrap_or(SecurityClassification::Confidential),
        })
        .await?;

    Ok(Json(
        hits.into_iter()
            .map(|h| SearchHitResponse {
                score: h.score,
                item: h.item,
                version: h.version,
                evidence: h.evidence,
            })
            .collect(),
    ))
}

pub async fn get_knowledge(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Path(id): Path<KnowledgeItemId>,
) -> ApiResult<Json<KnowledgeDetailResponse>> {
    let detail = state
        .services
        .for_principal(principal)?
        .get_knowledge(id)
        .await?;
    Ok(Json(KnowledgeDetailResponse {
        item: detail.item,
        versions: detail.versions,
        evidence: detail.evidence,
    }))
}

pub async fn compile_context(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(req): Json<CompileContextRequest>,
) -> ApiResult<Json<CompiledContext>> {
    let compiled = state
        .services
        .for_principal(principal)?
        .compile_context(ownstate_services::context::CompileRequest {
            project_id: req.project_id,
            task: req.task,
            session_id: req.session_id,
            provider: req.provider,
            model: req.model,
            agent: req.agent,
            max_items: req.max_items,
            token_budget: req.token_budget,
            max_classification: req.max_classification,
        })
        .await?;
    Ok(Json(compiled))
}

/// Models and clients submit typed constraints, never SQL or canonical writes.
pub async fn execute_query(
    State(state): State<AppState>,
    Extension(principal): Extension<AuthenticatedPrincipal>,
    Json(plan): Json<ownstate_domain::QueryPlan>,
) -> ApiResult<Json<ownstate_services::query::QueryResult>> {
    Ok(Json(
        state
            .services
            .for_principal(principal)?
            .execute_query(plan)
            .await?,
    ))
}
