//! Integration tests for database-enforced invariants. These exercise real
//! PostgreSQL semantics (triggers, partial unique indexes, row locks) and
//! deliberately do not mock the database.

// Tests assert with unwrap/expect by design.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::Utc;
use ownstate_domain::*;
use ownstate_storage::test_support::fresh_db;
use ownstate_storage::{events, knowledge, projects, retrieval, sessions};
use serde_json::json;

fn tenant() -> TenantId {
    TenantId::from_uuid(uuid::Uuid::from_u128(1))
}

fn project(tenant_id: TenantId) -> Project {
    Project {
        id: ProjectId::generate(),
        tenant_id,
        ownership_domain: OwnershipDomain::Personal,
        name: "Test Project".into(),
        description: None,
        metadata: json!({}),
        created_at: Utc::now(),
    }
}

fn session(p: &Project) -> Session {
    Session {
        id: SessionId::generate(),
        tenant_id: p.tenant_id,
        project_id: p.id,
        source: "test".into(),
        source_session_id: None,
        agent: None,
        provider: None,
        model: None,
        status: SessionStatus::Active,
        started_at: Utc::now(),
        ended_at: None,
        repository: None,
        initial_commit: None,
        final_commit: None,
        metadata: json!({}),
        created_at: Utc::now(),
    }
}

fn event(s: &Session, sequence: i64, content: &str) -> InteractionEvent {
    InteractionEvent {
        id: EventId::generate(),
        tenant_id: s.tenant_id,
        project_id: s.project_id,
        session_id: s.id,
        source: s.source.clone(),
        source_event_id: None,
        event_type: InteractionEventType::AssistantMessage,
        actor_type: ActorType::Assistant,
        actor_id: None,
        sequence,
        occurred_at: Utc::now(),
        model_provider: None,
        model_name: None,
        content: Some(content.to_string()),
        content_hash: Some(content_hash(content.as_bytes())),
        tool_name: None,
        tool_call_id: None,
        repository: None,
        branch: None,
        commit_sha: None,
        file_path: None,
        security_classification: SecurityClassification::Internal,
        metadata: json!({}),
        created_at: Utc::now(),
    }
}

fn item(p: &Project) -> KnowledgeItem {
    KnowledgeItem {
        id: KnowledgeItemId::generate(),
        tenant_id: p.tenant_id,
        project_id: p.id,
        ownership_domain: OwnershipDomain::Personal,
        kind: KnowledgeKind::Architecture,
        subject_key: "auth".into(),
        created_at: Utc::now(),
        created_by: "test".into(),
    }
}

fn version(i: &KnowledgeItem, number: i32, status: KnowledgeStatus) -> KnowledgeVersion {
    KnowledgeVersion {
        id: KnowledgeVersionId::generate(),
        knowledge_item_id: i.id,
        version_number: number,
        content: format!("architecture v{number}"),
        structured_content: None,
        status,
        trust_level: TrustLevel::AgentDerived,
        confidence: None,
        security_classification: SecurityClassification::Internal,
        // Ordinary fixtures represent established current knowledge, not a
        // boundary between the application clock and the PostgreSQL clock.
        valid_from: Utc::now() - chrono::Duration::days(1),
        valid_until: None,
        supersedes_version_id: None,
        candidate_id: None,
        created_at: Utc::now(),
        created_by: "test".into(),
    }
}

fn evidence(v: &KnowledgeVersion, e: &InteractionEvent) -> KnowledgeEvidence {
    KnowledgeEvidence {
        id: EvidenceId::generate(),
        knowledge_version_id: v.id,
        event_id: Some(e.id),
        source_type: "INTERACTION_EVENT".into(),
        content_hash: e.content_hash.clone(),
        repository: None,
        commit_sha: None,
        file_path: None,
        line_start: None,
        line_end: None,
        created_at: Utc::now(),
    }
}

/// Model a real upgrade: run the original migration metadata/checksums, insert
/// legacy history, and apply the additive migration without bypassing triggers.
async fn fresh_legacy_db() -> ownstate_storage::test_support::TestDb {
    let base = std::env::var("OWNSTATE_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate".into());
    let admin = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&base)
        .await
        .unwrap();
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    let name = format!("ownstate_test_{}_{}", Utc::now().timestamp(), &suffix[..16]);
    // Only an internally generated timestamp/UUID is interpolated as an SQL
    // identifier, matching the disposable-database harness's naming contract.
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let (prefix, _) = base.rsplit_once('/').unwrap();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&format!("{prefix}/{name}"))
        .await
        .unwrap();
    let first_six = sqlx::migrate::Migrator::with_migrations(
        ownstate_storage::MIGRATOR
            .iter()
            .filter(|migration| migration.version <= 6)
            .cloned()
            .collect(),
    );
    first_six.run(&pool).await.unwrap();
    ownstate_storage::test_support::TestDb { pool, name }
}

#[tokio::test]
async fn events_are_append_only_at_the_database_level() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let s = session(&p);
    sessions::insert(&db.pool, &s).await.unwrap();
    let e = event(&s, 0, "hello");
    events::insert(&db.pool, &e).await.unwrap();

    // UPDATE is refused by trigger.
    let update = sqlx::query("UPDATE interaction_events SET content = 'rewritten' WHERE id = $1")
        .bind(e.id.as_uuid())
        .execute(&db.pool)
        .await;
    let err = update.expect_err("UPDATE must be rejected").to_string();
    assert!(err.contains("append-only"), "unexpected error: {err}");

    // DELETE is refused by trigger.
    let delete = sqlx::query("DELETE FROM interaction_events WHERE id = $1")
        .bind(e.id.as_uuid())
        .execute(&db.pool)
        .await;
    let err = delete.expect_err("DELETE must be rejected").to_string();
    assert!(err.contains("append-only"), "unexpected error: {err}");

    // The row is untouched.
    let stored = events::list_for_session(&db.pool, s.id).await.unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].content.as_deref(), Some("hello"));
}

#[tokio::test]
async fn duplicate_sequences_conflict() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let s = session(&p);
    sessions::insert(&db.pool, &s).await.unwrap();

    events::insert(&db.pool, &event(&s, 7, "first"))
        .await
        .unwrap();
    let dup = events::insert(&db.pool, &event(&s, 7, "second")).await;
    assert!(matches!(
        dup,
        Err(ownstate_storage::StorageError::Conflict(_))
    ));
}

#[tokio::test]
async fn events_are_ordered_by_sequence() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let s = session(&p);
    sessions::insert(&db.pool, &s).await.unwrap();

    for seq in [2i64, 0, 1] {
        events::insert(&db.pool, &event(&s, seq, &format!("m{seq}")))
            .await
            .unwrap();
    }
    let stored = events::list_for_session(&db.pool, s.id).await.unwrap();
    let sequences: Vec<i64> = stored.iter().map(|e| e.sequence).collect();
    assert_eq!(sequences, vec![0, 1, 2]);
}

#[tokio::test]
async fn knowledge_version_content_is_immutable_but_status_may_change() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let v = version(&i, 1, KnowledgeStatus::Active);
    knowledge::insert_version(&db.pool, &v).await.unwrap();

    // Content rewrite is refused by trigger.
    let rewrite = sqlx::query("UPDATE knowledge_versions SET content = 'tampered' WHERE id = $1")
        .bind(v.id.as_uuid())
        .execute(&db.pool)
        .await;
    let err = rewrite
        .expect_err("content UPDATE must be rejected")
        .to_string();
    assert!(err.contains("immutable"), "unexpected error: {err}");

    // Trust rewrite is refused too.
    let trust =
        sqlx::query("UPDATE knowledge_versions SET trust_level = 'HUMAN_EXPLICIT' WHERE id = $1")
            .bind(v.id.as_uuid())
            .execute(&db.pool)
            .await;
    assert!(trust.is_err(), "trust UPDATE must be rejected");

    // DELETE is refused.
    let delete = sqlx::query("DELETE FROM knowledge_versions WHERE id = $1")
        .bind(v.id.as_uuid())
        .execute(&db.pool)
        .await;
    assert!(delete.is_err(), "DELETE must be rejected");

    // Status transition through the sanctioned path works.
    let mut tx = db.pool.begin().await.unwrap();
    knowledge::supersede_version(&mut tx, v.id).await.unwrap();
    tx.commit().await.unwrap();

    let versions = knowledge::list_versions_for_item(&db.pool, i.id)
        .await
        .unwrap();
    assert_eq!(versions[0].status, KnowledgeStatus::Superseded);
    assert!(versions[0].valid_until.is_some());
    assert_eq!(
        versions[0].content, v.content,
        "content survived transition"
    );
}

#[tokio::test]
async fn only_one_active_version_per_item() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();

    knowledge::insert_version(&db.pool, &version(&i, 1, KnowledgeStatus::Active))
        .await
        .unwrap();
    let second_active =
        knowledge::insert_version(&db.pool, &version(&i, 2, KnowledgeStatus::Active)).await;
    assert!(
        second_active.is_err(),
        "partial unique index must forbid two ACTIVE versions"
    );

    // A SUPERSEDED second version is fine.
    knowledge::insert_version(&db.pool, &version(&i, 2, KnowledgeStatus::Superseded))
        .await
        .unwrap();
}

#[tokio::test]
async fn evidence_rows_are_append_only() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let s = session(&p);
    sessions::insert(&db.pool, &s).await.unwrap();
    let e = event(&s, 0, "the auth uses middleware");
    events::insert(&db.pool, &e).await.unwrap();
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let v = version(&i, 1, KnowledgeStatus::Active);
    knowledge::insert_version(&db.pool, &v).await.unwrap();

    let ev = KnowledgeEvidence {
        id: EvidenceId::generate(),
        knowledge_version_id: v.id,
        event_id: Some(e.id),
        source_type: "INTERACTION_EVENT".into(),
        content_hash: e.content_hash.clone(),
        repository: None,
        commit_sha: None,
        file_path: None,
        line_start: None,
        line_end: None,
        created_at: Utc::now(),
    };
    knowledge::insert_evidence(&db.pool, &ev).await.unwrap();

    let tamper = sqlx::query("UPDATE knowledge_evidence SET event_id = NULL WHERE id = $1")
        .bind(ev.id.as_uuid())
        .execute(&db.pool)
        .await;
    assert!(tamper.is_err(), "evidence UPDATE must be rejected");

    let fetched = knowledge::list_evidence_for_versions(&db.pool, &[v.id])
        .await
        .unwrap();
    assert_eq!(fetched.len(), 1);
    assert_eq!(fetched[0].event_id, Some(e.id));
}

#[tokio::test]
async fn database_classification_order_matches_rust_and_unknowns_fail_closed() {
    let db = fresh_db().await.unwrap();
    for (rank, classification) in SecurityClassification::ALL.iter().enumerate() {
        let (stored_rank,): (i32,) = sqlx::query_as("SELECT ownstate_classification_rank($1)")
            .bind(classification.as_str())
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(stored_rank, rank as i32, "{classification}");
    }
    let (unknown_rank,): (i32,) = sqlx::query_as("SELECT ownstate_classification_rank('UNKNOWN')")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(
        unknown_rank > 4,
        "unknown classifications must not lower protection"
    );
}

#[tokio::test]
async fn event_evidence_requires_matching_scope_and_sufficient_classification() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    let mut other_project = project(tenant());
    other_project.name = "Other Project".into();
    let other_tenant = project(TenantId::generate());
    for project in [&p, &other_project, &other_tenant] {
        projects::insert(&db.pool, project).await.unwrap();
    }
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let v = version(&i, 1, KnowledgeStatus::Active);
    knowledge::insert_version(&db.pool, &v).await.unwrap();
    let s = session(&p);
    sessions::insert(&db.pool, &s).await.unwrap();
    let ordinary = event(&s, 0, "ordinary evidence");
    let mut secret = event(&s, 1, "sensitive evidence");
    secret.security_classification = SecurityClassification::Secret;
    let foreign_project_session = session(&other_project);
    let foreign_tenant_session = session(&other_tenant);
    for session in [&foreign_project_session, &foreign_tenant_session] {
        sessions::insert(&db.pool, session).await.unwrap();
    }
    let foreign_project_event = event(&foreign_project_session, 0, "other project");
    let mut foreign_tenant_event = event(&foreign_tenant_session, 0, "other tenant");
    // Existing event foreign keys are independent. Keep the project equal to
    // isolate the evidence trigger's tenant check from its project check.
    foreign_tenant_event.project_id = p.id;
    for source in [
        &ordinary,
        &secret,
        &foreign_project_event,
        &foreign_tenant_event,
    ] {
        events::insert(&db.pool, source).await.unwrap();
    }
    for invalid in [&secret, &foreign_project_event, &foreign_tenant_event] {
        let error = knowledge::insert_evidence(&db.pool, &evidence(&v, invalid))
            .await
            .expect_err("invalid provenance must be rejected by PostgreSQL")
            .to_string();
        assert!(error.contains("scope or classification"), "{error}");
    }
    knowledge::insert_evidence(&db.pool, &evidence(&v, &ordinary))
        .await
        .unwrap();
    let mut classified_item = item(&p);
    classified_item.subject_key = "classified".into();
    knowledge::insert_item(&db.pool, &classified_item)
        .await
        .unwrap();
    let mut classified_version = version(&classified_item, 1, KnowledgeStatus::Active);
    classified_version.security_classification = SecurityClassification::Secret;
    knowledge::insert_version(&db.pool, &classified_version)
        .await
        .unwrap();
    knowledge::insert_evidence(&db.pool, &evidence(&classified_version, &secret))
        .await
        .unwrap();
    let stored = knowledge::list_evidence_for_versions(&db.pool, &[v.id])
        .await
        .unwrap();
    assert_eq!(
        stored.len(),
        1,
        "rejected inserts leave no provenance behind"
    );
    assert_eq!(stored[0].event_id, Some(ordinary.id));
}

#[tokio::test]
async fn final_retrieval_rechecks_ranked_ids_after_lifecycle_changes() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let allowed = SecurityClassification::allowed_up_to(SecurityClassification::Internal);
    for status in [
        KnowledgeStatus::Stale,
        KnowledgeStatus::Revoked,
        KnowledgeStatus::Quarantined,
    ] {
        let mut i = item(&p);
        i.subject_key = status.as_str().into();
        knowledge::insert_item(&db.pool, &i).await.unwrap();
        let (database_clock,): (chrono::DateTime<Utc>,) =
            sqlx::query_as("SELECT statement_timestamp()")
                .fetch_one(&db.pool)
                .await
                .unwrap();
        let mut v = version(&i, 1, KnowledgeStatus::Active);
        v.valid_from = database_clock - chrono::Duration::minutes(1);
        knowledge::insert_version(&db.pool, &v).await.unwrap();
        let (currently_valid,): (bool,) = sqlx::query_as(
            "SELECT valid_from <= statement_timestamp() AND (valid_until IS NULL OR valid_until > statement_timestamp()) FROM knowledge_versions WHERE id = $1",
        )
        .bind(v.id.as_uuid())
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert!(
            currently_valid,
            "lifecycle fixture is valid by the database clock"
        );
        let ranked =
            retrieval::fts_search(&db.pool, tenant(), p.id, "architecture", &allowed, None, 20)
                .await
                .unwrap();
        assert_eq!(ranked, vec![v.id], "source was eligible during ranking");
        assert_eq!(
            knowledge::load_retrievable_versions_with_items(
                &db.pool,
                tenant(),
                p.id,
                &ranked,
                &allowed
            )
            .await
            .unwrap()
            .len(),
            1
        );
        sqlx::query("UPDATE knowledge_versions SET status = $2 WHERE id = $1")
            .bind(v.id.as_uuid())
            .bind(status.as_str())
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            knowledge::load_retrievable_versions_with_items(
                &db.pool,
                tenant(),
                p.id,
                &ranked,
                &allowed
            )
            .await
            .unwrap()
            .is_empty(),
            "{status} content must not leave final loading"
        );
    }
}

#[tokio::test]
async fn final_retrieval_rejects_supplied_ids_outside_scope_classification_or_validity() {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    let mut foreign_project = project(tenant());
    foreign_project.name = "Foreign Project".into();
    let foreign_tenant = project(TenantId::generate());
    for project in [&p, &foreign_project, &foreign_tenant] {
        projects::insert(&db.pool, project).await.unwrap();
    }
    let mut ids = Vec::new();
    let mut current_id = None;
    for (label, owner, status) in [
        ("current", &p, KnowledgeStatus::Active),
        ("foreign-project", &foreign_project, KnowledgeStatus::Active),
        ("foreign-tenant", &foreign_tenant, KnowledgeStatus::Active),
        ("secret", &p, KnowledgeStatus::Active),
        ("future", &p, KnowledgeStatus::Active),
        ("expired", &p, KnowledgeStatus::Active),
        ("superseded", &p, KnowledgeStatus::Superseded),
        ("conflict", &p, KnowledgeStatus::Conflict),
    ] {
        let mut i = item(owner);
        i.subject_key = label.into();
        knowledge::insert_item(&db.pool, &i).await.unwrap();
        let mut v = version(&i, 1, status);
        match label {
            "current" => current_id = Some(v.id),
            "secret" => v.security_classification = SecurityClassification::Secret,
            "future" => v.valid_from = Utc::now() + chrono::Duration::days(1),
            "expired" => {
                v.valid_from = Utc::now() - chrono::Duration::days(2);
                v.valid_until = Some(Utc::now() - chrono::Duration::days(1));
            }
            _ => {}
        }
        knowledge::insert_version(&db.pool, &v).await.unwrap();
        ids.push(v.id);
    }
    let allowed = SecurityClassification::allowed_up_to(SecurityClassification::Internal);
    let loaded =
        knowledge::load_retrievable_versions_with_items(&db.pool, tenant(), p.id, &ids, &allowed)
            .await
            .unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(Some(loaded[0].0.id), current_id);
    assert_eq!(loaded[0].1.project_id, p.id);
}

#[tokio::test]
async fn upgrade_hides_unsafe_legacy_provenance_without_rewriting_history() {
    let db = fresh_legacy_db().await;
    let p = project(tenant());
    let mut foreign_project = project(tenant());
    foreign_project.name = "Legacy Foreign Project".into();
    let foreign_tenant = project(TenantId::generate());
    for project in [&p, &foreign_project, &foreign_tenant] {
        projects::insert(&db.pool, project).await.unwrap();
    }
    let s = session(&p);
    let foreign_project_session = session(&foreign_project);
    let foreign_tenant_session = session(&foreign_tenant);
    for session in [&s, &foreign_project_session, &foreign_tenant_session] {
        sessions::insert(&db.pool, session).await.unwrap();
    }
    let ordinary = event(&s, 0, "permitted source");
    let mut secret = event(&s, 1, "secret legacy source");
    secret.security_classification = SecurityClassification::Secret;
    let cross_project = event(&foreign_project_session, 0, "cross-project source");
    let mut cross_tenant = event(&foreign_tenant_session, 0, "cross-tenant source");
    cross_tenant.project_id = p.id;
    let mut legacy = Vec::new();
    for (label, source) in [
        ("safe", &ordinary),
        ("underclassified", &secret),
        ("cross-project", &cross_project),
        ("cross-tenant", &cross_tenant),
    ] {
        events::insert(&db.pool, source).await.unwrap();
        let mut i = item(&p);
        i.subject_key = label.into();
        knowledge::insert_item(&db.pool, &i).await.unwrap();
        let v = version(&i, 1, KnowledgeStatus::Active);
        knowledge::insert_version(&db.pool, &v).await.unwrap();
        let ev = evidence(&v, source);
        knowledge::insert_evidence(&db.pool, &ev).await.unwrap();
        legacy.push((i, v, ev));
    }
    ownstate_storage::run_migrations(&db.pool).await.unwrap();
    let all_ids: Vec<_> = legacy.iter().map(|(_, v, _)| v.id).collect();
    let allowed = SecurityClassification::allowed_up_to(SecurityClassification::Secret);
    let loaded = knowledge::load_retrievable_versions_with_items(
        &db.pool,
        tenant(),
        p.id,
        &all_ids,
        &allowed,
    )
    .await
    .unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0.id, legacy[0].1.id);
    let ranked =
        retrieval::fts_search(&db.pool, tenant(), p.id, "architecture", &allowed, None, 20)
            .await
            .unwrap();
    assert_eq!(ranked, vec![legacy[0].1.id]);
    let recent = retrieval::recent_active(&db.pool, tenant(), p.id, &allowed, 20)
        .await
        .unwrap();
    assert_eq!(recent, vec![legacy[0].1.id]);
    for (i, v, ev) in &legacy {
        let history = knowledge::list_versions_for_item(&db.pool, i.id)
            .await
            .unwrap();
        assert_eq!(history.len(), usize::from(v.id == legacy[0].1.id));
        let (content, status, classification): (String, String, String) = sqlx::query_as(
            "SELECT content, status, security_classification FROM knowledge_versions WHERE id = $1",
        )
        .bind(v.id.as_uuid())
        .fetch_one(&db.pool)
        .await
        .unwrap();
        assert_eq!(content, v.content);
        assert_eq!(status, "ACTIVE");
        assert_eq!(classification, "INTERNAL");
        let links = knowledge::list_evidence_for_versions(&db.pool, &[v.id])
            .await
            .unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, ev.id);
        assert_eq!(links[0].event_id, ev.event_id);
    }
    let rewrite = sqlx::query("UPDATE knowledge_versions SET content = 'rewritten' WHERE id = $1")
        .bind(legacy[1].1.id.as_uuid())
        .execute(&db.pool)
        .await;
    assert!(
        rewrite.is_err(),
        "upgrade retains immutable history enforcement"
    );
}
