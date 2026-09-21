//! End-to-end lifecycle tests over real PostgreSQL: propose → promote →
//! version supersession → provenance, plus the promotion guard rails.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use ownstate_domain::*;
use ownstate_services::ServiceError;
use ownstate_services::jobs::JobOutcome;
use ownstate_services::projects::EnsureProject;

fn ensure(name: &str, origin: Option<&str>, path: Option<&str>) -> EnsureProject {
    EnsureProject {
        name: name.into(),
        git_origin: origin.map(String::from),
        root_path: path.map(String::from),
    }
}

#[tokio::test]
async fn ensure_project_is_get_or_create_and_records_workspace_identity() {
    let (services, _db) = services().await;

    // First call creates the project with origin + location in metadata.
    let created = services
        .ensure_project(ensure(
            "my-workspace",
            Some("git@github.com:example/my-workspace.git"),
            Some("/home/u/my-workspace"),
        ))
        .await
        .unwrap();
    assert_eq!(created.name, "my-workspace");
    assert_eq!(
        created.metadata["git_origin"],
        "git@github.com:example/my-workspace.git"
    );
    assert_eq!(created.metadata["root_path"], "/home/u/my-workspace");

    // Second call resolves the same identity; origin is retained.
    let resolved = services
        .ensure_project(ensure("my-workspace", None, None))
        .await
        .unwrap();
    assert_eq!(resolved.id, created.id);
    assert_eq!(
        resolved.metadata["git_origin"],
        "git@github.com:example/my-workspace.git"
    );

    // A project created without an origin gets it backfilled on detection.
    let bare = services
        .ensure_project(ensure("bare-dir", None, None))
        .await
        .unwrap();
    assert!(bare.metadata.get("git_origin").is_none());
    let backfilled = services
        .ensure_project(ensure(
            "bare-dir",
            Some("https://github.com/example/bare-dir.git"),
            Some("/home/u/bare-dir"),
        ))
        .await
        .unwrap();
    assert_eq!(backfilled.id, bare.id);
    assert_eq!(
        backfilled.metadata["git_origin"],
        "https://github.com/example/bare-dir.git"
    );
    assert_eq!(backfilled.metadata["root_path"], "/home/u/bare-dir");
}

#[tokio::test]
async fn same_repository_resolves_to_one_project_across_clones_and_clients() {
    let (services, _db) = services().await;

    // The dlala scenario: Codex creates the project from one location…
    let from_codex = services
        .ensure_project(ensure(
            "dlala",
            Some("git@github.com:0xsereel/dlala.git"),
            Some("/Users/u/work/dlala"),
        ))
        .await
        .unwrap();

    // …Claude later opens a clone of the SAME repo under a different
    // directory name and location: same project, no duplicate.
    let from_claude = services
        .ensure_project(ensure(
            "dlala-clone",
            Some("git@github.com:0xsereel/dlala.git"),
            Some("/Users/u/tmp/dlala-clone"),
        ))
        .await
        .unwrap();
    assert_eq!(
        from_claude.id, from_codex.id,
        "same origin must not duplicate"
    );
    // Location is updated to where the workspace was last seen.
    assert_eq!(
        from_claude.metadata["root_path"],
        "/Users/u/tmp/dlala-clone"
    );

    // Case-insensitive names also resolve to the same identity.
    let case_variant = services
        .ensure_project(ensure("DLALA", None, None))
        .await
        .unwrap();
    assert_eq!(case_variant.id, from_codex.id);

    // Same recorded location without an origin (remote added later) resolves too.
    let by_path = services
        .ensure_project(ensure(
            "renamed-dir",
            None,
            Some("/Users/u/tmp/dlala-clone"),
        ))
        .await
        .unwrap();
    assert_eq!(by_path.id, from_codex.id);
}

#[tokio::test]
async fn same_name_different_repository_stays_separate() {
    let (services, _db) = services().await;

    let repo_a = services
        .ensure_project(ensure(
            "api",
            Some("git@github.com:acme/api.git"),
            Some("/home/u/acme/api"),
        ))
        .await
        .unwrap();

    // A different repository whose directory is also named "api" must NOT
    // merge into repo_a's knowledge.
    let repo_b = services
        .ensure_project(ensure(
            "api",
            Some("git@github.com:other/api.git"),
            Some("/home/u/other/api"),
        ))
        .await
        .unwrap();
    assert_ne!(repo_b.id, repo_a.id);
    assert!(repo_b.name.starts_with("api-"), "got name {}", repo_b.name);
    assert_eq!(
        repo_b.metadata["git_origin"],
        "git@github.com:other/api.git"
    );

    // Resolving repo B again is stable (same suffixed identity).
    let repo_b_again = services
        .ensure_project(ensure(
            "api",
            Some("git@github.com:other/api.git"),
            Some("/home/u/other/api"),
        ))
        .await
        .unwrap();
    assert_eq!(repo_b_again.id, repo_b.id);
}

#[tokio::test]
async fn duplicate_project_names_conflict() {
    let (services, _db) = services().await;
    make_project(&services, "TakenName").await;
    let second = services
        .create_project(ownstate_services::projects::CreateProject {
            name: "TakenName".into(),
            description: None,
            ownership_domain: None,
            metadata: None,
        })
        .await;
    assert!(matches!(second, Err(ServiceError::Conflict(_))));
}

#[tokio::test]
async fn promotion_creates_canonical_knowledge_with_provenance() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Alpha").await;
    let session = make_session(&services, &project).await;

    let events = services
        .append_events(
            session.id,
            vec![
                message_event("The authentication flow goes through middleware."),
                message_event("Then a policy service authorizes the request."),
            ],
        )
        .await
        .unwrap();
    assert_eq!(events.len(), 2);
    assert!(
        events[0]
            .content_hash
            .as_deref()
            .unwrap()
            .starts_with("blake3:")
    );

    let candidate = services
        .propose_knowledge(proposal(
            &project,
            "authentication-architecture",
            "Authentication: middleware -> policy service -> handler.",
            events.iter().map(|e| e.id).collect(),
        ))
        .await
        .unwrap();
    assert_eq!(candidate.status, CandidateStatus::Pending);

    let (before,): (chrono::DateTime<chrono::Utc>,) = sqlx::query_as("SELECT clock_timestamp()")
        .fetch_one(services.pool())
        .await
        .unwrap();
    let (item, version) = services.promote_candidate(candidate.id).await.unwrap();
    let (after,): (chrono::DateTime<chrono::Utc>,) = sqlx::query_as("SELECT clock_timestamp()")
        .fetch_one(services.pool())
        .await
        .unwrap();
    assert!(
        version.valid_from >= before && version.valid_from <= after,
        "promotion validity uses PostgreSQL's clock"
    );
    assert_eq!(version.created_at, version.valid_from);
    assert_eq!(item.kind, KnowledgeKind::Architecture);
    assert_eq!(version.version_number, 1);
    assert_eq!(version.status, KnowledgeStatus::Active);
    // Agent proposal with evidence → AGENT_DERIVED, decided by domain rules.
    assert_eq!(version.trust_level, TrustLevel::AgentDerived);

    // Provenance links back to the exact source events.
    let detail = services.get_knowledge(item.id).await.unwrap();
    assert_eq!(detail.versions.len(), 1);
    let evidence_events: Vec<_> = detail.evidence.iter().filter_map(|e| e.event_id).collect();
    assert_eq!(evidence_events.len(), 2);
    for e in &events {
        assert!(evidence_events.contains(&e.id));
    }

    // Candidate is marked promoted and points at the version.
    let promoted = services.get_candidate(candidate.id).await.unwrap();
    assert_eq!(promoted.status, CandidateStatus::Promoted);
    assert_eq!(promoted.promoted_version_id, Some(version.id));
}

#[tokio::test]
async fn updating_knowledge_supersedes_rather_than_overwrites() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Beta").await;

    let c1 = services
        .propose_knowledge(proposal(
            &project,
            "auth",
            "v1: middleware -> handler",
            vec![],
        ))
        .await
        .unwrap();
    let (item1, v1) = services.promote_candidate(c1.id).await.unwrap();

    let c2 = services
        .propose_knowledge(proposal(
            &project,
            "auth",
            "v2: middleware -> policy service -> handler",
            vec![],
        ))
        .await
        .unwrap();
    let (item2, v2) = services.promote_candidate(c2.id).await.unwrap();

    // Same conceptual identity, new version.
    assert_eq!(item1.id, item2.id);
    assert_eq!(v2.version_number, 2);
    assert_eq!(v2.supersedes_version_id, Some(v1.id));

    let detail = services.get_knowledge(item1.id).await.unwrap();
    assert_eq!(detail.versions.len(), 2);
    let v1_stored = detail.versions.iter().find(|v| v.id == v1.id).unwrap();
    let v2_stored = detail.versions.iter().find(|v| v.id == v2.id).unwrap();
    assert_eq!(v1_stored.status, KnowledgeStatus::Superseded);
    assert!(v1_stored.valid_until.is_some());
    assert_eq!(
        v1_stored.valid_until,
        Some(v2_stored.valid_from),
        "supersession shares one temporal boundary"
    );
    assert!(v1_stored.valid_until.unwrap() >= v1_stored.valid_from);
    // History preserved verbatim.
    assert_eq!(v1_stored.content, "v1: middleware -> handler");
    assert_eq!(v2_stored.status, KnowledgeStatus::Active);
}

#[tokio::test]
async fn promoting_twice_is_a_conflict() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Gamma").await;
    let c = services
        .propose_knowledge(proposal(&project, "auth", "claim", vec![]))
        .await
        .unwrap();
    services.promote_candidate(c.id).await.unwrap();
    let second = services.promote_candidate(c.id).await;
    assert!(matches!(second, Err(ServiceError::Conflict(_))));
}

#[tokio::test]
async fn rejected_candidates_cannot_be_promoted() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Delta").await;
    let c = services
        .propose_knowledge(proposal(&project, "auth", "claim", vec![]))
        .await
        .unwrap();
    services.reject_candidate(c.id, "wrong").await.unwrap();
    let promoted = services.promote_candidate(c.id).await;
    assert!(matches!(promoted, Err(ServiceError::Conflict(_))));
}

#[tokio::test]
async fn evidence_from_another_project_is_refused() {
    let (services, _db) = services().await;
    let project_a = make_project(&services, "A").await;
    let project_b = make_project(&services, "B").await;
    let session_b = make_session(&services, &project_b).await;
    let foreign_events = services
        .append_events(session_b.id, vec![message_event("b's secret evidence")])
        .await
        .unwrap();

    // A proposal for project A cannot cite project B's events as evidence.
    let result = services
        .propose_knowledge(proposal(
            &project_a,
            "auth",
            "claim",
            foreign_events.iter().map(|e| e.id).collect(),
        ))
        .await;
    assert!(matches!(result, Err(ServiceError::Validation(_))));
}

#[tokio::test]
async fn extractor_without_evidence_gets_untrusted_level() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Eps").await;
    let mut p = proposal(&project, "auth", "unsupported claim", vec![]);
    p.proposed_by = ProposerKind::Extractor;
    let c = services.propose_knowledge(p).await.unwrap();
    let (_, version) = services.promote_candidate(c.id).await.unwrap();
    assert_eq!(version.trust_level, TrustLevel::ExternalUntrusted);
}

#[tokio::test]
async fn promotion_enqueues_embedding_job_and_worker_processes_it() {
    let (services, db) = services().await;
    let project = make_project(&services, "Zeta").await;
    let c = services
        .propose_knowledge(proposal(&project, "auth", "the auth claim", vec![]))
        .await
        .unwrap();
    let (_, version) = services.promote_candidate(c.id).await.unwrap();

    // One pending job from promotion.
    let pending = ownstate_storage::jobs::count_pending(&db.pool, JobKind::GenerateEmbedding)
        .await
        .unwrap();
    assert_eq!(pending, 1);

    // Process it exactly once; queue then empty.
    assert!(matches!(
        services.process_next_job().await.unwrap(),
        JobOutcome::Processed(_)
    ));
    assert_eq!(
        services.process_next_job().await.unwrap(),
        JobOutcome::QueueEmpty
    );

    // Embedding row exists for the deterministic model.
    let count =
        ownstate_storage::embeddings::count_for_version(&db.pool, version.id, "deterministic-hash")
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn evidence_survives_supersession() {
    let (services, _db) = services().await;
    let project = make_project(&services, "Eta").await;
    let session = make_session(&services, &project).await;
    let events = services
        .append_events(session.id, vec![message_event("original evidence")])
        .await
        .unwrap();

    let c1 = services
        .propose_knowledge(proposal(
            &project,
            "auth",
            "v1",
            events.iter().map(|e| e.id).collect(),
        ))
        .await
        .unwrap();
    let (item, v1) = services.promote_candidate(c1.id).await.unwrap();

    let c2 = services
        .propose_knowledge(proposal(&project, "auth", "v2", vec![]))
        .await
        .unwrap();
    services.promote_candidate(c2.id).await.unwrap();

    // Superseded v1 keeps its provenance.
    let detail = services.get_knowledge(item.id).await.unwrap();
    let v1_evidence: Vec<_> = detail
        .evidence
        .iter()
        .filter(|e| e.knowledge_version_id == v1.id)
        .collect();
    assert_eq!(v1_evidence.len(), 1);
    assert_eq!(v1_evidence[0].event_id, Some(events[0].id));
}

#[tokio::test]
async fn classification_inherits_the_most_restricted_cited_event() {
    let (services, _db) = services().await;
    let project = make_project(&services, "ClassificationFloor").await;
    let session = make_session(&services, &project).await;
    let mut secret = message_event("fictional confidential decision record");
    secret.security_classification = Some(SecurityClassification::Secret);
    let events = services
        .append_events(
            session.id,
            vec![message_event("publicly available note"), secret],
        )
        .await
        .unwrap();
    let mut req = proposal(
        &project,
        "decision",
        "derived decision",
        events.iter().map(|e| e.id).collect(),
    );
    req.security_classification = Some(SecurityClassification::Public);
    let candidate = services.propose_knowledge(req).await.unwrap();
    assert_eq!(
        candidate.security_classification,
        SecurityClassification::Secret
    );
    let (item, version) = services.promote_candidate(candidate.id).await.unwrap();
    assert_eq!(
        version.security_classification,
        SecurityClassification::Secret
    );
    assert!(
        services
            .get_knowledge(item.id)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    let stored = ownstate_storage::events::list_for_session(services.pool(), session.id)
        .await
        .unwrap();
    assert_eq!(
        stored[1].content, events[1].content,
        "source history is untouched"
    );
    assert_eq!(
        stored[1].security_classification,
        SecurityClassification::Secret
    );
}

#[tokio::test]
async fn promotion_rederives_classification_for_legacy_pending_candidates() {
    let (services, db) = services().await;
    let project = make_project(&services, "LegacyPending").await;
    let session = make_session(&services, &project).await;
    let mut restricted = message_event("fictional restricted source");
    restricted.security_classification = Some(SecurityClassification::Restricted);
    let events = services
        .append_events(session.id, vec![restricted])
        .await
        .unwrap();
    let candidate = services
        .propose_knowledge(proposal(
            &project,
            "design",
            "derived design",
            vec![events[0].id],
        ))
        .await
        .unwrap();
    // Candidates are untrusted/mutable records. Reproduce the lower label an
    // older server stored; do not bypass canonical/evidence immutability.
    sqlx::query("UPDATE candidate_knowledge SET security_classification = 'PUBLIC' WHERE id = $1")
        .bind(candidate.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    let (item, version) = services.promote_candidate(candidate.id).await.unwrap();
    assert_eq!(
        version.security_classification,
        SecurityClassification::Restricted
    );
    assert!(
        services
            .get_knowledge(item.id)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    assert_eq!(
        ownstate_storage::knowledge::list_evidence_for_versions(&db.pool, &[version.id])
            .await
            .unwrap()[0]
            .event_id,
        Some(events[0].id)
    );
}

#[tokio::test]
async fn promotion_rejects_foreign_evidence_without_superseding_current_state() {
    let (services, db) = services().await;
    let project = make_project(&services, "ValidProject").await;
    let other = make_project(&services, "ForeignProject").await;
    let session = make_session(&services, &other).await;
    let events = services
        .append_events(session.id, vec![message_event("foreign source")])
        .await
        .unwrap();
    let original = services
        .propose_knowledge(proposal(&project, "design", "original", vec![]))
        .await
        .unwrap();
    let (item, version) = services.promote_candidate(original.id).await.unwrap();
    let candidate = services
        .propose_knowledge(proposal(&project, "design", "invalid update", vec![]))
        .await
        .unwrap();
    sqlx::query("UPDATE candidate_knowledge SET evidence_event_ids = $2 WHERE id = $1")
        .bind(candidate.id.as_uuid())
        .bind(vec![events[0].id.as_uuid()])
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(matches!(
        services.promote_candidate(candidate.id).await,
        Err(ServiceError::Validation(_))
    ));
    let history = services.get_knowledge(item.id).await.unwrap();
    assert_eq!(history.versions.len(), 1);
    assert_eq!(history.versions[0].id, version.id);
    assert_eq!(history.versions[0].status, KnowledgeStatus::Active);
    assert_eq!(history.versions[0].content, "original");
    assert_eq!(
        services.get_candidate(candidate.id).await.unwrap().status,
        CandidateStatus::Pending
    );
    assert_eq!(
        ownstate_storage::jobs::count_pending(&db.pool, JobKind::GenerateEmbedding)
            .await
            .unwrap(),
        1
    );
}
