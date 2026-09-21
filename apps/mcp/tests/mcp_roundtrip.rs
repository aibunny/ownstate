//! Real MCP round-trip: an rmcp client talks to the Ownstate server over an
//! in-process duplex transport. Proves the wire protocol, tool registration
//! and that MCP goes through the same services (and scoping) as HTTP.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use ownstate_domain::TenantId;
use ownstate_embeddings::DeterministicProvider;
use ownstate_mcp::OwnstateMcp;
use ownstate_services::AppServices;
use ownstate_services::knowledge::ProposeKnowledge;
use ownstate_services::policy::PrincipalServices;
use ownstate_services::projects::CreateProject;
use ownstate_storage::test_support::{TestDb, fresh_db};
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use serde_json::Value;

async fn setup() -> (Arc<AppServices>, PrincipalServices, TestDb) {
    let db = fresh_db().await.unwrap();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
    ));
    let owner = app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = app.for_principal(owner).unwrap();
    (app, s, db)
}

fn text_content(result: &rmcp::model::CallToolResult) -> Value {
    let text = result
        .content
        .first()
        .and_then(|c| c.as_text())
        .map(|t| t.text.clone())
        .expect("tool result should carry text content");
    serde_json::from_str(&text).expect("tool result text should be JSON")
}

#[tokio::test]
async fn mcp_client_retrieves_the_same_knowledge_as_http_services() {
    let (app, s, _db) = setup().await;

    // Seed a project with promoted knowledge through the application services.
    let project = s
        .create_project(CreateProject {
            name: "MCP Project".into(),
            description: Some("proves MCP retrieval".into()),
            ownership_domain: None,
            metadata: None,
        })
        .await
        .unwrap();
    let candidate = s
        .propose_knowledge(ProposeKnowledge {
            project_id: project.id,
            session_id: None,
            kind: ownstate_domain::KnowledgeKind::Architecture,
            subject_key: "authentication-architecture".into(),
            content: "Authentication flows through middleware into the policy service.".into(),
            structured_content: None,
            confidence: Some(0.9),
            proposed_by: ownstate_domain::ProposerKind::Agent,
            source: Some("seed".into()),
            evidence_event_ids: vec![],
            security_classification: None,
        })
        .await
        .unwrap();
    let (item, _version) = s.promote_candidate(candidate.id).await.unwrap();
    s.drain_jobs(10).await.unwrap();
    let session = s
        .create_session(ownstate_services::sessions::CreateSession {
            project_id: project.id,
            source: "fictional-register".into(),
            source_session_id: None,
            agent: None,
            provider: None,
            model: None,
            repository: None,
            initial_commit: None,
            metadata: None,
        })
        .await
        .unwrap();
    let event = s
        .append_events(
            session.id,
            vec![
                serde_json::from_value(serde_json::json!({
        "event_type":"DOCUMENT_READ","actor_type":"USER","content":"Fictional Atlas register"}))
                .unwrap(),
            ],
        )
        .await
        .unwrap()
        .remove(0);
    let candidate=s.propose_institutional(ownstate_services::institutional::ProposeInstitutional {
        project_id:project.id,proposal:serde_json::from_value(serde_json::json!({"type":"ENTITY","kind":"LEGAL_ENTITY",
            "name":"Fictional Atlas Ltd","external_ids":{"register":"ATLAS-001"},"aliases":["Atlas"]})).unwrap(),
        evidence_event_ids:vec![event.id],security_classification:None,observed_at:None,proposed_by:ownstate_domain::ProposerKind::Human,confidence:Some(0.9)}).await.unwrap();
    let entity = s.promote_institutional(candidate.id).await.unwrap();

    // In-process duplex wire: server on one end, client on the other.
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let server_services = app.clone();
    let server_task = tokio::spawn(async move {
        let workspace = ownstate_mcp::Workspace {
            name: "mcp-workspace-project".to_string(),
            root_path: "/tmp/mcp-workspace-project".to_string(),
            git_origin: Some("https://github.com/example/mcp-workspace-project.git".to_string()),
        };
        let actors = server_services
            .bootstrap_personal_policy_actors(None, None)
            .await
            .unwrap();
        let scoped = server_services.for_principal(actors.mcp).unwrap();
        let server = OwnstateMcp::new(scoped, Some(workspace))
            .serve(server_io)
            .await
            .unwrap();
        let _ = server.waiting().await;
    });

    let client = ().serve(client_io).await.unwrap();

    // Tools are advertised.
    let tools = client.list_tools(Default::default()).await.unwrap();
    let names: Vec<_> = tools.tools.iter().map(|t| t.name.as_ref()).collect();
    for expected in [
        "bootstrap_project",
        "search_knowledge",
        "get_knowledge",
        "propose_knowledge",
        "get_entity",
        "get_relationships",
        "compile_context",
    ] {
        assert!(names.contains(&expected), "missing tool {expected}");
    }
    for forbidden in [
        "execute_sql",
        "promote_candidate",
        "set_policy",
        "grant_permission",
    ] {
        assert!(!names.contains(&forbidden));
    }
    let result = client
        .call_tool(
            CallToolRequestParams::new("get_entity").with_arguments(
                serde_json::json!({
        "project_id":project.id.to_string(),"entity_id":entity.entity_id.unwrap().to_string()})
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    let retrieved = text_content(&result);
    assert_eq!(retrieved["versions"][0]["id"], entity.id.to_string());
    assert_eq!(
        retrieved["versions"][0]["evidence_event_ids"][0],
        event.id.to_string()
    );
    let result = client
        .call_tool(
            CallToolRequestParams::new("get_relationships").with_arguments(
                serde_json::json!({
        "project_id":project.id.to_string(),"entity_id":entity.entity_id.unwrap().to_string()})
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(text_content(&result).as_array().unwrap().is_empty());
    let result = client
        .call_tool(
            CallToolRequestParams::new("compile_context").with_arguments(
                serde_json::json!({
        "project_id":project.id.to_string(),"task":"authentication middleware"})
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(
        text_content(&result)["context_packet_id"]
            .as_str()
            .is_some()
    );

    // bootstrap_project returns the project summary + knowledge.
    let result = client
        .call_tool(
            CallToolRequestParams::new("bootstrap_project").with_arguments(
                serde_json::json!({ "project_id": project.id.to_string() })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let bootstrap = text_content(&result);
    assert_eq!(bootstrap["name"], "MCP Project");
    assert_eq!(bootstrap["active_knowledge_count"], 1);
    assert_eq!(
        bootstrap["knowledge"][0]["subject_key"],
        "authentication-architecture"
    );

    // search_knowledge finds the promoted knowledge.
    let result = client
        .call_tool(
            CallToolRequestParams::new("search_knowledge").with_arguments(
                serde_json::json!({
                    "project_id": project.id.to_string(),
                    "query": "authentication middleware"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    let hits = text_content(&result);
    assert_eq!(hits.as_array().unwrap().len(), 1);
    assert_eq!(hits[0]["knowledge_item_id"], item.id.to_string());
    // Agent proposal without evidence -> EXTERNAL_UNTRUSTED, by the domain rules.
    assert_eq!(hits[0]["trust_level"], "EXTERNAL_UNTRUSTED");

    // get_knowledge returns version history.
    let result = client
        .call_tool(
            CallToolRequestParams::new("get_knowledge").with_arguments(
                serde_json::json!({ "knowledge_item_id": item.id.to_string() })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let detail = text_content(&result);
    assert_eq!(detail["versions"].as_array().unwrap().len(), 1);

    // propose_knowledge creates a PENDING candidate -- never canonical state.
    let result = client
        .call_tool(
            CallToolRequestParams::new("propose_knowledge").with_arguments(
                serde_json::json!({
                    "project_id": project.id.to_string(),
                    "kind": "DECISION",
                    "subject_key": "database-choice",
                    "content": "PostgreSQL is the canonical store.",
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await
        .unwrap();
    let proposed = text_content(&result);
    assert_eq!(proposed["status"], "PENDING");
    let candidate_id: ownstate_domain::CandidateId =
        proposed["candidate_id"].as_str().unwrap().parse().unwrap();
    let stored = s.get_candidate(candidate_id).await.unwrap();
    // MCP proposals are always AGENT -- a model cannot claim human trust.
    assert_eq!(stored.proposed_by, ownstate_domain::ProposerKind::Agent);
    assert_eq!(stored.status, ownstate_domain::CandidateStatus::Pending);

    // Workspace resolution: bootstrap with NO project argument resolves the
    // project from the (injected) workspace directory name, creates it on
    // first use, and records the git origin.
    let result = client
        .call_tool(
            CallToolRequestParams::new("bootstrap_project").with_arguments(serde_json::Map::new()),
        )
        .await
        .unwrap();
    let ws_bootstrap = text_content(&result);
    assert_eq!(ws_bootstrap["name"], "mcp-workspace-project");
    assert_eq!(
        ws_bootstrap["git_origin"],
        "https://github.com/example/mcp-workspace-project.git"
    );
    let ws_project_id: ownstate_domain::ProjectId = ws_bootstrap["project_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let ws_project = s.get_project(ws_project_id).await.unwrap();
    assert_eq!(
        ws_project.metadata["git_origin"],
        "https://github.com/example/mcp-workspace-project.git"
    );

    // A second workspace-resolved call reuses the same project (get, not create).
    let result = client
        .call_tool(
            CallToolRequestParams::new("search_knowledge").with_arguments(
                serde_json::json!({ "query": "anything" })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let ws_hits = text_content(&result);
    assert!(ws_hits.as_array().unwrap().is_empty());
    let same_project = s
        .get_project(ws_project_id)
        .await
        .expect("workspace project persists across calls");
    assert_eq!(same_project.id, ws_project_id);

    // Unknown project id -> MCP error, same authorization path as HTTP.
    let err = client
        .call_tool(
            CallToolRequestParams::new("bootstrap_project").with_arguments(
                serde_json::json!({ "project_id": "00000000-0000-0000-0000-00000000dead" })
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await;
    match err {
        Err(_) => {}
        Ok(result) => assert_eq!(result.is_error, Some(true), "expected an MCP tool error"),
    }

    client.cancel().await.unwrap();
    server_task.abort();
}
