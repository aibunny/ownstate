//! Retrieval scoping and Context Compiler tests: project isolation,
//! classification ceilings, status filtering, hybrid ranking, budgets, and
//! the context packet audit trail.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use ownstate_domain::*;
use ownstate_services::context::CompileRequest;
use ownstate_services::search::SearchParams;

/// Deterministically reproduces a lifecycle change after the lexical query
/// selected IDs, while semantic query embedding is still in progress.
struct InvalidateDuringQuery {
    pool: sqlx::PgPool,
    version_id: KnowledgeVersionId,
    status: &'static str,
    invalidated: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl ownstate_embeddings::EmbeddingProvider for InvalidateDuringQuery {
    fn model_name(&self) -> &str {
        "deterministic-hash"
    }
    fn model_version(&self) -> &str {
        "1"
    }
    fn dimensions(&self) -> usize {
        384
    }

    async fn embed(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, ownstate_embeddings::EmbeddingError> {
        if !self
            .invalidated
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            ownstate_storage::knowledge::set_version_status(
                &self.pool,
                self.version_id,
                "ACTIVE",
                self.status,
            )
            .await
            .map_err(|_| {
                ownstate_embeddings::EmbeddingError::Embed(
                    "test lifecycle transition failed".into(),
                )
            })?;
        }
        ownstate_embeddings::DeterministicProvider::new()
            .embed(texts)
            .await
    }
}

async fn promote_text(
    services: &ownstate_services::policy::PrincipalServices,
    project: &Project,
    subject: &str,
    content: &str,
) -> (KnowledgeItem, KnowledgeVersion) {
    let c = services
        .propose_knowledge(proposal(project, subject, content, vec![]))
        .await
        .unwrap();
    let promoted = services.promote_candidate(c.id).await.unwrap();
    // Generate the embedding for the new version immediately.
    services.drain_jobs(10).await.unwrap();
    promoted
}

fn search(project: &Project, query: &str, limit: usize) -> SearchParams {
    SearchParams {
        project_id: project.id,
        query: query.into(),
        kinds: None,
        limit,
        max_classification: SecurityClassification::Confidential,
    }
}

#[tokio::test]
async fn search_finds_promoted_knowledge() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Search").await;
    promote_text(
        &services,
        &project,
        "authentication-architecture",
        "Authentication requests flow through middleware into the policy service.",
    )
    .await;
    promote_text(
        &services,
        &project,
        "deployment",
        "Deployment uses a single container image with migrations at startup.",
    )
    .await;

    let hits = services
        .search_knowledge(search(&project, "authentication middleware", 5))
        .await
        .unwrap();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].item.subject_key, "authentication-architecture");
}

#[tokio::test]
async fn search_is_project_scoped() {
    let (services, _db) = services().await;
    let project_a = make_project(&services, "IsoA").await;
    let project_b = make_project(&services, "IsoB").await;
    promote_text(
        &services,
        &project_a,
        "secret-arch",
        "the alpha architecture",
    )
    .await;

    let hits_in_b = services
        .search_knowledge(search(&project_b, "alpha architecture", 10))
        .await
        .unwrap();
    assert!(
        hits_in_b.is_empty(),
        "project B must not see project A's knowledge"
    );

    let hits_in_a = services
        .search_knowledge(search(&project_a, "alpha architecture", 10))
        .await
        .unwrap();
    assert_eq!(hits_in_a.len(), 1);
}

#[tokio::test]
async fn search_respects_classification_ceiling() {
    let (services, db) = services().await;
    let project = make_project(&services, "Classified").await;

    let mut restricted = proposal(
        &project,
        "secret-plan",
        "the restricted architecture",
        vec![],
    );
    restricted.security_classification = Some(SecurityClassification::Restricted);
    let c = services.propose_knowledge(restricted).await.unwrap();
    services.promote_candidate(c.id).await.unwrap();
    services.drain_jobs(10).await.unwrap();

    // Default server ceiling is CONFIDENTIAL: RESTRICTED must not surface.
    let hits = services
        .search_knowledge(search(&project, "restricted architecture", 10))
        .await
        .unwrap();
    assert!(
        hits.is_empty(),
        "RESTRICTED must be filtered at the default ceiling"
    );

    // A caller REQUESTING a higher ceiling is clamped to the server maximum:
    // the model/agent does not decide what it is authorized to receive.
    let hits = services
        .search_knowledge(SearchParams {
            max_classification: SecurityClassification::Secret,
            ..search(&project, "restricted architecture", 10)
        })
        .await
        .unwrap();
    assert!(
        hits.is_empty(),
        "requested ceilings above the server maximum must be clamped"
    );

    // Only a deployment configured with a higher ceiling exposes it.
    let elevated = TestServices::from_app(std::sync::Arc::new(
        ownstate_services::AppServices::new(
            db.pool.clone(),
            std::sync::Arc::new(ownstate_embeddings::DeterministicProvider::new()),
            services.tenant_id(),
        )
        .with_max_classification(SecurityClassification::Restricted),
    ))
    .await;
    let hits = elevated
        .search_knowledge(SearchParams {
            max_classification: SecurityClassification::Restricted,
            ..search(&project, "restricted architecture", 10)
        })
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
}

#[tokio::test]
async fn get_knowledge_applies_ceiling_and_withholds_curated_content() {
    let (services, db) = services().await;
    let project = make_project(&services, "HistoryLeak").await;

    // v1 RESTRICTED, later superseded by v2 INTERNAL on the same subject.
    let mut v1_proposal = proposal(&project, "auth", "the RESTRICTED v1 design", vec![]);
    v1_proposal.security_classification = Some(SecurityClassification::Restricted);
    let c1 = services.propose_knowledge(v1_proposal).await.unwrap();
    let (item, _v1) = services.promote_candidate(c1.id).await.unwrap();
    let c2 = services
        .propose_knowledge(proposal(&project, "auth", "the INTERNAL v2 design", vec![]))
        .await
        .unwrap();
    let (_, v2) = services.promote_candidate(c2.id).await.unwrap();

    // At the default CONFIDENTIAL ceiling the RESTRICTED v1 must not be
    // served through version history either.
    let detail = services.get_knowledge(item.id).await.unwrap();
    assert_eq!(detail.versions.len(), 1, "restricted v1 must be omitted");
    assert_eq!(detail.versions[0].id, v2.id);

    // Revoked content is withheld (identity/status remain visible).
    ownstate_storage::knowledge::set_version_status(&db.pool, v2.id, "ACTIVE", "REVOKED")
        .await
        .unwrap();
    let detail = services.get_knowledge(item.id).await.unwrap();
    assert_eq!(detail.versions.len(), 1);
    assert_eq!(detail.versions[0].status, KnowledgeStatus::Revoked);
    assert_eq!(detail.versions[0].content, "[content withheld: REVOKED]");
}

#[tokio::test]
async fn event_content_above_ceiling_is_withheld_on_read() {
    let (services, _db) = services().await;
    let project = make_project(&services, "EvidenceCeiling").await;
    let session = make_session(&services, &project).await;

    let mut secret = message_event("the launch codes");
    secret.security_classification = Some(SecurityClassification::Secret);
    services
        .append_events(session.id, vec![secret, message_event("ordinary note")])
        .await
        .unwrap();

    let events = services.list_events(session.id).await.unwrap();
    assert_eq!(events.len(), 2, "rows stay visible for audit continuity");
    assert_eq!(
        events[0].content.as_deref(),
        Some("[content withheld: SECRET]"),
        "above-ceiling content must be withheld"
    );
    // The hash still proves what was stored, without revealing it.
    assert!(
        events[0]
            .content_hash
            .as_deref()
            .unwrap()
            .starts_with("blake3:")
    );
    assert_eq!(events[1].content.as_deref(), Some("ordinary note"));
}

#[tokio::test]
async fn event_sequences_cannot_poison_a_session() {
    let (services, _db) = services().await;
    let project = make_project(&services, "SeqGuard").await;
    let session = make_session(&services, &project).await;

    // A sequence at the edge of the space is rejected outright.
    let mut poisoned = message_event("poison");
    poisoned.sequence = Some(i64::MAX);
    let result = services.append_events(session.id, vec![poisoned]).await;
    assert!(result.is_err(), "i64::MAX sequence must be rejected");

    // Jumps beyond the allowed gap are rejected.
    let mut too_far = message_event("too far");
    too_far.sequence = Some(ownstate_domain::limits::MAX_SEQUENCE_GAP + 10);
    let result = services.append_events(session.id, vec![too_far]).await;
    assert!(result.is_err(), "excessive sequence gap must be rejected");

    // Bounded explicit jumps and subsequent auto-assignment still work.
    let mut ahead = message_event("ahead");
    ahead.sequence = Some(50);
    services
        .append_events(session.id, vec![ahead, message_event("auto")])
        .await
        .unwrap();
    let events = services.list_events(session.id).await.unwrap();
    let sequences: Vec<i64> = events.iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, vec![50, 51]);
}

#[tokio::test]
async fn superseded_and_revoked_versions_are_not_retrievable() {
    let (services, db) = services().await;
    let project = make_project(&services, "Lifecycle").await;

    let (_, v1) = promote_text(&services, &project, "auth", "the OLD auth design").await;
    promote_text(&services, &project, "auth", "the NEW auth design").await;

    let hits = services
        .search_knowledge(search(&project, "auth design", 10))
        .await
        .unwrap();
    assert_eq!(hits.len(), 1, "only the ACTIVE version is retrievable");
    assert_eq!(hits[0].version.content, "the NEW auth design");
    assert_ne!(hits[0].version.id, v1.id);

    // Revoke the active version → nothing retrievable.
    let active_id = hits[0].version.id;
    ownstate_storage::knowledge::set_version_status(&db.pool, active_id, "ACTIVE", "REVOKED")
        .await
        .unwrap();
    let hits = services
        .search_knowledge(search(&project, "auth design", 10))
        .await
        .unwrap();
    assert!(hits.is_empty(), "REVOKED knowledge must not be retrievable");
}

#[tokio::test]
async fn context_compiler_respects_budgets_and_records_packet() {
    let (services, db) = services().await;
    let project = make_project(&services, "Budget").await;
    for i in 0..6 {
        promote_text(
            &services,
            &project,
            &format!("auth-part-{i}"),
            &format!("authentication component {i} does part {i} of the auth flow"),
        )
        .await;
    }

    let compiled = services
        .compile_context(CompileRequest {
            project_id: project.id,
            task: "Explain the authentication architecture".into(),
            session_id: None,
            provider: None,
            model: None,
            agent: Some("test-agent".into()),
            max_items: Some(3),
            token_budget: None,
            max_classification: None,
        })
        .await
        .unwrap();

    let total_items: usize = compiled.sections.iter().map(|s| s.items.len()).sum();
    assert!(
        total_items <= 3,
        "max_items must be respected, got {total_items}"
    );
    assert!(compiled.estimated_tokens > 0);

    // The packet audit records exactly the returned versions.
    let packet_id: ContextPacketId = compiled.context_packet_id.parse().unwrap();
    let packet = ownstate_storage::context_packets::get(
        &db.pool,
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
        packet_id,
    )
    .await
    .unwrap();
    let returned: Vec<String> = compiled
        .sections
        .iter()
        .flat_map(|s| s.items.iter().map(|i| i.knowledge_version_id.clone()))
        .collect();
    let recorded: Vec<String> = packet
        .knowledge_version_ids
        .iter()
        .map(|id| id.to_string())
        .collect();
    assert_eq!(returned.len(), recorded.len());
    for id in &returned {
        assert!(recorded.contains(id));
    }
}

#[tokio::test]
async fn context_compiler_respects_token_budget() {
    let (services, _db) = services().await;
    let project = make_project(&services, "TokenBudget").await;
    // ~50 tokens each (200 chars).
    for i in 0..4 {
        promote_text(
            &services,
            &project,
            &format!("auth-{i}"),
            &format!("authentication {} {}", i, "x".repeat(200)),
        )
        .await;
    }

    let compiled = services
        .compile_context(CompileRequest {
            project_id: project.id,
            task: "authentication".into(),
            session_id: None,
            provider: None,
            model: None,
            agent: None,
            max_items: Some(10),
            token_budget: Some(120),
            max_classification: None,
        })
        .await
        .unwrap();

    assert!(
        compiled.estimated_tokens <= 120,
        "token budget exceeded: {}",
        compiled.estimated_tokens
    );
    let total_items: usize = compiled.sections.iter().map(|s| s.items.len()).sum();
    assert!(
        total_items >= 1,
        "budget allows at least two items; got none"
    );
}

#[tokio::test]
async fn context_includes_provenance_sources() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Provenance").await;
    let session = make_session(&services, &project).await;
    let events = services
        .append_events(session.id, vec![message_event("auth evidence text")])
        .await
        .unwrap();
    let c = services
        .propose_knowledge(proposal(
            &project,
            "auth",
            "authentication uses middleware",
            events.iter().map(|e| e.id).collect(),
        ))
        .await
        .unwrap();
    services.promote_candidate(c.id).await.unwrap();
    services.drain_jobs(10).await.unwrap();

    let compiled = services
        .compile_context(CompileRequest {
            project_id: project.id,
            task: "authentication middleware".into(),
            session_id: None,
            provider: None,
            model: None,
            agent: None,
            max_items: Some(5),
            token_budget: None,
            max_classification: None,
        })
        .await
        .unwrap();

    let item = &compiled.sections[0].items[0];
    assert_eq!(item.sources.len(), 1);
    assert_eq!(
        item.sources[0].event_id.as_deref(),
        Some(events[0].id.to_string().as_str())
    );
    assert!(
        item.sources[0]
            .content_hash
            .as_deref()
            .unwrap()
            .starts_with("blake3:")
    );
}

#[tokio::test]
async fn bootstrap_returns_recent_knowledge_and_records_packet() {
    let (services, db) = services().await;
    let project = make_project(&services, "Bootstrap").await;
    promote_text(&services, &project, "auth", "authentication design").await;
    promote_text(&services, &project, "deploy", "deployment design").await;

    // One un-promoted proposal must be visible as a pending count, so an
    // un-curated backlog is never mistaken for an empty knowledge base.
    services
        .propose_knowledge(proposal(&project, "unreviewed", "not yet promoted", vec![]))
        .await
        .unwrap();

    let bootstrap = services
        .bootstrap_project(project.id, 10, Some("agent-x".into()))
        .await
        .unwrap();
    assert_eq!(bootstrap.active_knowledge_count, 2);
    assert_eq!(bootstrap.knowledge.len(), 2);
    assert_eq!(bootstrap.pending_candidate_count, 1);

    let packet_id: ContextPacketId = bootstrap.context_packet_id.parse().unwrap();
    let packet = ownstate_storage::context_packets::get(
        &db.pool,
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
        packet_id,
    )
    .await
    .unwrap();
    assert_eq!(packet.task, "PROJECT_BOOTSTRAP");
    assert_eq!(packet.agent.as_deref(), Some("agent-x"));
    assert_eq!(packet.knowledge_version_ids.len(), 2);
}

#[tokio::test]
async fn retrieval_rechecks_versions_invalidated_during_query_embedding() {
    for status in ["STALE", "SUPERSEDED", "QUARANTINED", "REVOKED"] {
        for compile in [false, true] {
            let (services, db) = services().await;
            let project = make_project(&services, "ConcurrentLifecycle").await;
            let (_, version) =
                promote_text(&services, &project, "auth", "authentication architecture").await;
            let raced =
                TestServices::from_app(std::sync::Arc::new(ownstate_services::AppServices::new(
                    db.pool.clone(),
                    std::sync::Arc::new(InvalidateDuringQuery {
                        pool: db.pool.clone(),
                        version_id: version.id,
                        status,
                        invalidated: std::sync::atomic::AtomicBool::new(false),
                    }),
                    services.tenant_id(),
                )))
                .await;
            if compile {
                let context = raced
                    .compile_context(CompileRequest {
                        project_id: project.id,
                        task: "authentication architecture".into(),
                        session_id: None,
                        provider: None,
                        model: None,
                        agent: None,
                        max_items: Some(10),
                        token_budget: None,
                        max_classification: None,
                    })
                    .await
                    .unwrap();
                assert!(
                    context.sections.is_empty(),
                    "{status} must not enter context"
                );
                let packet = ownstate_storage::context_packets::get(
                    &db.pool,
                    services.tenant_id(),
                    context.context_packet_id.parse().unwrap(),
                )
                .await
                .unwrap();
                assert!(
                    packet.knowledge_version_ids.is_empty(),
                    "audit must match delivered context"
                );
            } else {
                let hits = raced
                    .search_knowledge(search(&project, "authentication architecture", 10))
                    .await
                    .unwrap();
                assert!(
                    hits.is_empty(),
                    "{status} must not escape through lexical IDs"
                );
            }
            let bootstrap = raced.bootstrap_project(project.id, 10, None).await.unwrap();
            assert!(bootstrap.knowledge.is_empty());
            let stored = ownstate_storage::knowledge::get_version_unscoped(&db.pool, version.id)
                .await
                .unwrap();
            assert_eq!(stored.status.as_str(), status);
            assert_eq!(stored.content, "authentication architecture");
        }
    }
}

#[tokio::test]
async fn derived_classification_is_filtered_by_search_bootstrap_and_context() {
    let (services, db) = services().await;
    let project = make_project(&services, "DerivedRetrieval").await;
    let session = make_session(&services, &project).await;
    let mut secret = message_event("fictional secret architecture source");
    secret.security_classification = Some(SecurityClassification::Secret);
    let events = services
        .append_events(session.id, vec![secret])
        .await
        .unwrap();
    let mut req = proposal(
        &project,
        "auth",
        "authentication architecture",
        vec![events[0].id],
    );
    req.security_classification = Some(SecurityClassification::Public);
    let candidate = services.propose_knowledge(req).await.unwrap();
    let (_, version) = services.promote_candidate(candidate.id).await.unwrap();
    services.drain_jobs(10).await.unwrap();
    assert_eq!(
        version.security_classification,
        SecurityClassification::Secret
    );
    assert!(
        services
            .search_knowledge(search(&project, "authentication architecture", 10))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        services
            .bootstrap_project(project.id, 10, None)
            .await
            .unwrap()
            .knowledge
            .is_empty()
    );
    let context = services
        .compile_context(CompileRequest {
            project_id: project.id,
            task: "authentication architecture".into(),
            session_id: None,
            provider: None,
            model: None,
            agent: None,
            max_items: None,
            token_budget: None,
            max_classification: None,
        })
        .await
        .unwrap();
    assert!(context.sections.is_empty());
    let elevated = TestServices::from_app(std::sync::Arc::new(
        ownstate_services::AppServices::new(
            db.pool.clone(),
            std::sync::Arc::new(ownstate_embeddings::DeterministicProvider::new()),
            services.tenant_id(),
        )
        .with_max_classification(SecurityClassification::Secret),
    ))
    .await;
    let hits = elevated
        .search_knowledge(SearchParams {
            max_classification: SecurityClassification::Secret,
            ..search(&project, "authentication architecture", 10)
        })
        .await
        .unwrap();
    assert_eq!(
        hits.len(),
        1,
        "configured authority can retrieve the preserved version"
    );
    assert_eq!(hits[0].evidence[0].event_id, Some(events[0].id));
}
