//! HTTP-layer integration tests: routing, auth, status-code mapping, and the
//! full end-to-end flow through the real router against real PostgreSQL.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use ownstate_api::{AppState, build_router};
use ownstate_domain::TenantId;
use ownstate_embeddings::DeterministicProvider;
use ownstate_services::AppServices;
use ownstate_storage::test_support::{TestDb, fresh_db};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app(api_token: Option<&str>) -> (Router, Arc<AppServices>, TestDb) {
    app_with_admin(api_token, None).await
}

async fn app_with_admin(
    api_token: Option<&str>,
    admin_token: Option<&str>,
) -> (Router, Arc<AppServices>, TestDb) {
    let db = fresh_db().await.unwrap();
    let services = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
    ));
    let state = AppState::new(
        services.clone(),
        api_token.map(String::from),
        admin_token.map(String::from),
    )
    .await
    .unwrap();
    (build_router(state), services, db)
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(req).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    // Axum's own extractor rejections (e.g. enum deserialization failures)
    // return plain-text bodies; keep them as strings.
    let body: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, body)
}

fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(path: &str) -> Request<Body> {
    Request::builder().uri(path).body(Body::empty()).unwrap()
}

fn authorized(mut req: Request<Body>, token: &str) -> Request<Body> {
    req.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );
    req
}

#[tokio::test]
async fn institutional_http_routes_gate_curation_and_return_scoped_entities() {
    let (app, services, db) = app_with_admin(Some("reader"), Some("curator")).await;
    let (status, p) = send(
        &app,
        authorized(
            post("/projects", json!({"name":"Fictional Atlas"})),
            "reader",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let project_id = p["id"].as_str().unwrap();
    let (status, session) = send(
        &app,
        authorized(
            post(
                "/sessions",
                json!({"project_id":project_id,"source":"fictional"}),
            ),
            "reader",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let(status,events)=send(&app,authorized(post(&format!("/sessions/{}/events",session["id"].as_str().unwrap()),json!({"events":[{"event_type":"DOCUMENT_READ","actor_type":"USER","content":"Fictional register"}]})),"reader")).await;
    assert_eq!(status, StatusCode::CREATED);
    let req = json!({"project_id":project_id,"proposal":{"type":"ENTITY","kind":"LEGAL_ENTITY","name":"Atlas Ltd","external_ids":{"register":"A"},"aliases":["Atlas"]},"evidence_event_ids":[events[0]["id"]],"proposed_by":"HUMAN"});
    assert_eq!(
        send(
            &app,
            authorized(post("/institutional/proposals", req.clone()), "reader")
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, c) = send(
        &app,
        authorized(post("/institutional/proposals", req), "curator"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let path = format!(
        "/institutional/proposals/{}/promote",
        c["id"].as_str().unwrap()
    );
    assert_eq!(
        send(&app, authorized(post(&path, json!({})), "reader"))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, v) = send(&app, authorized(post(&path, json!({})), "curator")).await;
    assert_eq!(status, StatusCode::OK);
    let id = v["entity_id"].as_str().unwrap();
    let path = format!("/entities/{id}?project_id={project_id}");
    assert_eq!(send(&app, get(&path)).await.0, StatusCode::UNAUTHORIZED);
    let (status, snapshot) = send(&app, authorized(get(&path), "reader")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(snapshot["versions"][0]["id"], v["id"]);
    assert_eq!(
        send(
            &app,
            authorized(
                get(&format!(
                    "/entities/{id}/relationships?project_id={project_id}"
                )),
                "reader"
            )
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(send(&app,authorized(get("/entities/00000000-0000-0000-0000-00000000dead?project_id=00000000-0000-0000-0000-00000000dead"),"reader")).await.0,StatusCode::NOT_FOUND);
    let owner = services
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = services.for_principal(owner).unwrap();
    let investor_req:ownstate_services::institutional::ProposeInstitutional=serde_json::from_value(json!({
        "project_id":project_id,"proposal":{"type":"ENTITY","kind":"INVESTOR","name":"Fictional Investor","external_ids":{"register":"I"},"aliases":[]},
        "evidence_event_ids":[events[0]["id"]],"proposed_by":"HUMAN"})).unwrap();
    let c = s.propose_institutional(investor_req).await.unwrap();
    let investor = s.promote_institutional(c.id).await.unwrap();
    let relation_req:ownstate_services::institutional::ProposeInstitutional=serde_json::from_value(json!({
        "project_id":project_id,"proposal":{"type":"RELATIONSHIP","source_entity_id":id,"target_entity_id":investor.entity_id.unwrap(),"relationship_type":"SENT_TO"},
        "evidence_event_ids":[events[0]["id"]],"proposed_by":"HUMAN"})).unwrap();
    let c = s.propose_institutional(relation_req).await.unwrap();
    let relationship = s.promote_institutional(c.id).await.unwrap();
    let (status, current) = send(
        &app,
        authorized(
            get(&format!(
                "/entities/{id}/relationships?project_id={project_id}"
            )),
            "reader",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(current.as_array().unwrap().len(), 1);
    let(status,past)=send(&app,authorized(get(&format!("/entities/{id}/relationships?project_id={project_id}&as_of=1970-01-01T00%3A00%3A00Z")),"reader")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(past.as_array().unwrap().is_empty());
    sqlx::query("UPDATE institutional_versions SET status='REVOKED' WHERE id=$1")
        .bind(investor.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    let (status, history) = send(
        &app,
        authorized(
            get(&format!(
                "/institutional/{}/history?project_id={project_id}",
                relationship.assertion_id.unwrap()
            )),
            "reader",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(history.as_array().unwrap().is_empty());
}

#[tokio::test]
async fn health_and_ready_work_without_auth() {
    let (app, _, _db) = app(Some("secret-token")).await;
    let (status, body) = send(&app, get("/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");

    let (status, body) = send(&app, get("/ready")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
}

#[tokio::test]
async fn protected_routes_require_bearer_token() {
    let (app, _, _db) = app(Some("secret-token")).await;

    // Without token → 401.
    let (status, body) = send(&app, post("/projects", json!({"name": "X"}))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthorized");

    // Wrong token → 401.
    let mut req = post("/projects", json!({"name": "X"}));
    req.headers_mut()
        .insert(header::AUTHORIZATION, "Bearer wrong-token".parse().unwrap());
    let (status, _) = send(&app, req).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Correct token → 201.
    let mut req = post("/projects", json!({"name": "X"}));
    req.headers_mut().insert(
        header::AUTHORIZATION,
        "Bearer secret-token".parse().unwrap(),
    );
    let (status, body) = send(&app, req).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "X");
}

#[tokio::test]
async fn request_identity_fields_are_rejected_and_business_has_no_fallback() {
    let (personal, services, _db) = app(Some("server-owned-token")).await;
    let forged = authorized(
        post(
            "/projects",
            json!({
                "name": "forged",
                "principal_id": "00000000-0000-0000-0000-000000000001",
                "tenant_id": "00000000-0000-0000-0000-000000000001"
            }),
        ),
        "server-owned-token",
    );
    assert_eq!(
        send(&personal, forged).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );

    let business = build_router(AppState::business(services));
    assert_eq!(
        send(&business, post("/projects", json!({"name": "no-fallback"})))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn unknown_ids_return_404_and_bad_input_400() {
    let (app, _, _db) = app(None).await;

    let (status, body) = send(&app, get("/projects/00000000-0000-0000-0000-00000000dead")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    let (status, _) = send(&app, post("/projects", json!({"name": ""}))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Invalid enum in event payload → 4xx from deserialization.
    let (status, _) = send(
        &app,
        post(
            "/projects",
            json!({"name": "ok", "ownership_domain": "NOT_A_DOMAIN"}),
        ),
    )
    .await;
    assert!(status.is_client_error());
}

#[tokio::test]
async fn curation_credential_gates_promotion_and_human_attribution() {
    let (app, _, _db) = app_with_admin(Some("user-token"), Some("curator-token")).await;
    let authed = |mut req: Request<Body>, token: &str| {
        let value = format!("Bearer {token}").parse().unwrap();
        req.headers_mut().insert(header::AUTHORIZATION, value);
        req
    };

    let (status, project) = send(
        &app,
        authed(post("/projects", json!({"name": "Curated"})), "user-token"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let project_id = project["id"].as_str().unwrap().to_string();

    // Agents (regular token) may propose as AGENT...
    let agent_proposal = json!({
        "project_id": project_id, "kind": "FACT", "subject_key": "db",
        "content": "PostgreSQL is canonical.", "proposed_by": "AGENT"
    });
    let (status, candidate) = send(
        &app,
        authed(
            post("/knowledge/proposals", agent_proposal.clone()),
            "user-token",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let candidate_id = candidate["id"].as_str().unwrap().to_string();

    // ...but cannot claim HUMAN attribution (that would mint HUMAN_EXPLICIT trust)...
    let mut human_proposal = agent_proposal.clone();
    human_proposal["proposed_by"] = json!("HUMAN");
    human_proposal["subject_key"] = json!("db-2");
    let (status, body) = send(
        &app,
        authed(
            post("/knowledge/proposals", human_proposal.clone()),
            "user-token",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    // ...and cannot promote their own candidates.
    let promote_path = format!("/knowledge/proposals/{candidate_id}/promote");
    let (status, _) = send(&app, authed(post(&promote_path, json!({})), "user-token")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The curation credential can do both.
    let (status, _) = send(
        &app,
        authed(
            post("/knowledge/proposals", human_proposal),
            "curator-token",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, promotion) = send(
        &app,
        authed(post(&promote_path, json!({})), "curator-token"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(promotion["version"]["status"], "ACTIVE");
}

#[tokio::test]
async fn full_flow_over_http() {
    let (app, services, _db) = app(None).await;

    // Create project.
    let (status, project) = send(&app, post("/projects", json!({"name": "E2E"}))).await;
    assert_eq!(status, StatusCode::CREATED);
    let project_id = project["id"].as_str().unwrap().to_string();

    // Create session.
    let (status, session) = send(
        &app,
        post(
            "/sessions",
            json!({"project_id": project_id, "source": "claude-code", "agent": "test"}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let session_id = session["id"].as_str().unwrap().to_string();

    // Append events.
    let (status, events) = send(
        &app,
        post(
            &format!("/sessions/{session_id}/events"),
            json!({"events": [
                {"event_type": "USER_MESSAGE", "actor_type": "USER",
                 "content": "How does auth work?"},
                {"event_type": "ASSISTANT_MESSAGE", "actor_type": "ASSISTANT",
                 "content": "Auth flows through middleware into the policy service."}
            ]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let event_ids: Vec<String> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(event_ids.len(), 2);
    assert_eq!(events[0]["sequence"], 0);
    assert_eq!(events[1]["sequence"], 1);

    // Duplicate explicit sequence → 409.
    let (status, body) = send(
        &app,
        post(
            &format!("/sessions/{session_id}/events"),
            json!({"events": [
                {"event_type": "USER_MESSAGE", "actor_type": "USER",
                 "sequence": 0, "content": "dup"}
            ]}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "conflict");

    // Propose knowledge with evidence.
    let (status, candidate) = send(
        &app,
        post(
            "/knowledge/proposals",
            json!({
                "project_id": project_id,
                "session_id": session_id,
                "kind": "ARCHITECTURE",
                "subject_key": "authentication-architecture",
                "content": "Authentication: middleware -> policy service -> handler.",
                "confidence": 0.9,
                "proposed_by": "AGENT",
                "evidence_event_ids": event_ids,
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(candidate["status"], "PENDING");
    let candidate_id = candidate["id"].as_str().unwrap().to_string();

    // Promote.
    let (status, promotion) = send(
        &app,
        post(
            &format!("/knowledge/proposals/{candidate_id}/promote"),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(promotion["version"]["status"], "ACTIVE");
    assert_eq!(promotion["version"]["version_number"], 1);
    assert_eq!(promotion["version"]["trust_level"], "AGENT_DERIVED");
    let item_id = promotion["item"]["id"].as_str().unwrap().to_string();

    // Process the embedding job (in production the worker does this).
    let owner = services
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = services.for_principal(owner).unwrap();
    s.drain_jobs(10).await.unwrap();

    // Hybrid search finds it.
    let (status, hits) = send(
        &app,
        get(&format!(
            "/knowledge/search?project_id={project_id}&q=authentication+middleware"
        )),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["item"]["id"].as_str().unwrap(), item_id);
    assert_eq!(hits[0]["evidence"].as_array().unwrap().len(), 2);

    // Get knowledge detail with provenance.
    let (status, detail) = send(&app, get(&format!("/knowledge/{item_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["versions"].as_array().unwrap().len(), 1);
    assert_eq!(detail["evidence"].as_array().unwrap().len(), 2);

    // Compile context.
    let (status, compiled) = send(
        &app,
        post(
            "/context/compile",
            json!({
                "project_id": project_id,
                "task": "Explain the authentication architecture",
                "max_items": 20
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let sections = compiled["sections"].as_array().unwrap();
    assert_eq!(sections[0]["category"], "RELEVANT ARCHITECTURE");
    let first = &sections[0]["items"][0];
    assert_eq!(first["trust_level"], "AGENT_DERIVED");
    assert_eq!(first["sources"].as_array().unwrap().len(), 2);
    assert!(compiled["context_packet_id"].as_str().is_some());
}

#[tokio::test]
async fn query_http_accepts_typed_reports_and_denies_sql_or_unauthenticated_plans() {
    let (app, services, _db) = app(Some("reader")).await;
    let owner = services
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = services.for_principal(owner).unwrap();
    let p = s
        .create_project(ownstate_services::projects::CreateProject {
            name: "Fictional typed reports".into(),
            description: None,
            ownership_domain: None,
            metadata: None,
        })
        .await
        .unwrap();
    let plan = json!({"type":"STRUCTURED","constraints":{"project_id":p.id,"max_classification":"INTERNAL","entity_kind":"LEGAL_ENTITY","limit":1},"operation":"COUNT"});
    assert_eq!(
        send(&app, post("/query", plan.clone())).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (status, result) = send(&app, authorized(post("/query", plan.clone()), "reader")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["bounded_by_limit"], false);
    assert_eq!(result["snapshot"]["result"]["count"], 0);
    let mut malformed = plan.clone();
    malformed["sql"] = json!("SELECT * FROM interaction_events");
    assert!(
        send(&app, authorized(post("/query", malformed), "reader"))
            .await
            .0
            .is_client_error()
    );
    let mut invalid = plan;
    invalid["constraints"]["limit"] = json!(0);
    assert_eq!(
        send(&app, authorized(post("/query", invalid), "reader"))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
