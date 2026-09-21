//! Real PostgreSQL acceptance for exact, temporal, scoped, associated query plans.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use chrono::{Duration, Utc};
use ownstate_domain::*;
use ownstate_storage::test_support::{TestDb, fresh_db};
use ownstate_storage::{
    embeddings, events, institutional, knowledge, projects, query, retrieval, sessions,
};
use pgvector::Vector;
use serde_json::json;
use std::collections::BTreeMap;
fn tenant() -> TenantId {
    TenantId::generate()
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

async fn fixture() -> (TestDb, Project, InteractionEvent) {
    let db = fresh_db().await.unwrap();
    let p = project(tenant());
    projects::insert(&db.pool, &p).await.unwrap();
    let s = session(&p);
    sessions::insert(&db.pool, &s).await.unwrap();
    let e = event(&s, 0, "Fictional authoritative records");
    events::insert(&db.pool, &e).await.unwrap();
    (db, p, e)
}
async fn promote(
    db: &TestDb,
    p: &Project,
    e: &InteractionEvent,
    proposal: GraphProposal,
    class: SecurityClassification,
) -> InstitutionalVersion {
    let c = GraphCandidate {
        id: GraphCandidateId::generate(),
        tenant_id: p.tenant_id,
        project_id: p.id,
        proposal,
        evidence_event_ids: vec![e.id],
        security_classification: class,
        proposed_by: ProposerKind::Human,
        confidence: None,
        status: CandidateStatus::Pending,
        observed_at: Utc::now() - Duration::days(1),
        created_at: Utc::now(),
    };
    institutional::insert_candidate(&db.pool, &c).await.unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    let v = institutional::promote_candidate(&mut tx, &c, TrustLevel::HumanExplicit, class)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    v
}
async fn make_entity(
    db: &TestDb,
    p: &Project,
    e: &InteractionEvent,
    name: &str,
    kind: EntityKind,
    class: SecurityClassification,
) -> InstitutionalVersion {
    promote(
        db,
        p,
        e,
        GraphProposal::Entity {
            kind,
            name: name.into(),
            external_ids: BTreeMap::from([("register".into(), name.into())]),
            aliases: vec![],
        },
        class,
    )
    .await
}
fn constraints(p: &Project) -> QueryConstraints {
    QueryConstraints {
        project_id: p.id,
        as_of: None,
        max_classification: SecurityClassification::Internal,
        entity_kind: None,
        knowledge_kinds: vec![],
        entity_ids: vec![],
        relationship: None,
        trust_levels: vec![],
        source_types: vec![],
        limit: 1,
    }
}
fn read<'a>(
    p: &Project,
    c: &'a QueryConstraints,
    time: chrono::DateTime<Utc>,
) -> query::QueryRead<'a> {
    query::QueryRead {
        tenant: p.tenant_id,
        constraints: c,
        allowed: &["PUBLIC", "INTERNAL"],
        as_of: time,
    }
}
async fn semantic(
    db: &TestDb,
    p: &Project,
    e: &InteractionEvent,
    entity: Option<EntityId>,
    text: &str,
) -> KnowledgeVersion {
    let mut i = item(p);
    i.subject_key = KnowledgeItemId::generate().to_string();
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let mut v = version(&i, 1, KnowledgeStatus::Active);
    v.content = text.into();
    v.structured_content = entity.map(|id| json!({"entity_ids":[id]}));
    knowledge::insert_version(&db.pool, &v).await.unwrap();
    knowledge::insert_evidence(&db.pool, &evidence(&v, e))
        .await
        .unwrap();
    if let Some(id) = entity {
        let mut tx = db.pool.begin().await.unwrap();
        query::insert_knowledge_links(&mut tx, p.tenant_id, p.id, v.id, &[id])
            .await
            .unwrap();
        tx.commit().await.unwrap();
    }
    embeddings::upsert_knowledge_version_embedding(
        &db.pool,
        embeddings::NewEmbedding {
            entity_id: v.id,
            model: "fictional",
            model_version: "1",
            dimensions: 384,
            vector: Vector::from(vec![1.0; 384]),
        },
    )
    .await
    .unwrap();
    v
}
#[tokio::test]
async fn exact_legal_counts_and_jurisdiction_groups_exceed_response_limit() {
    let (db, p, e) = fixture().await;
    let j = make_entity(
        &db,
        &p,
        &e,
        "North",
        EntityKind::Jurisdiction,
        SecurityClassification::Internal,
    )
    .await;
    for n in 0..4 {
        let legal = make_entity(
            &db,
            &p,
            &e,
            &format!("Legal{n}"),
            EntityKind::LegalEntity,
            SecurityClassification::Internal,
        )
        .await;
        promote(
            &db,
            &p,
            &e,
            GraphProposal::Relationship {
                source_entity_id: legal.entity_id.unwrap(),
                target_entity_id: j.entity_id.unwrap(),
                relationship_type: "REGISTERED_IN".into(),
            },
            SecurityClassification::Internal,
        )
        .await;
    }
    make_entity(
        &db,
        &p,
        &e,
        "Hidden",
        EntityKind::LegalEntity,
        SecurityClassification::Confidential,
    )
    .await;
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::LegalEntity);
    let q = read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap());
    match query::structured(&db.pool, q, StructuredOperation::Count)
        .await
        .unwrap()
    {
        StructuredResult::Count {
            count,
            evidence_event_ids,
        } => {
            assert_eq!(count, 4);
            assert_eq!(evidence_event_ids, vec![e.id]);
        }
        _ => panic!("wrong result"),
    }
    match query::structured(&db.pool, q, StructuredOperation::GroupByJurisdiction)
        .await
        .unwrap()
    {
        StructuredResult::GroupByJurisdiction { groups } => {
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].count, 4);
            assert_eq!(
                groups[0].jurisdiction.as_ref().unwrap().id,
                j.entity_id.unwrap()
            );
            assert_eq!(groups[0].evidence_event_ids, vec![e.id]);
        }
        _ => panic!("wrong result"),
    }
    let foreign = project(tenant());
    projects::insert(&db.pool, &foreign).await.unwrap();
    let mut wrong = c.clone();
    wrong.project_id = foreign.id;
    assert!(matches!(
        query::structured(
            &db.pool,
            read(
                &p,
                &wrong,
                query::snapshot_timestamp(&db.pool).await.unwrap()
            ),
            StructuredOperation::Count
        )
        .await
        .unwrap(),
        StructuredResult::Count { count: 0, .. }
    ));
}
#[tokio::test]
async fn hybrid_semantics_require_matching_customer_association_and_final_revalidation() {
    let (db, p, e) = fixture().await;
    let north = make_entity(
        &db,
        &p,
        &e,
        "North",
        EntityKind::Jurisdiction,
        SecurityClassification::Internal,
    )
    .await;
    let a = make_entity(
        &db,
        &p,
        &e,
        "CustomerA",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let b = make_entity(
        &db,
        &p,
        &e,
        "CustomerB",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    promote(
        &db,
        &p,
        &e,
        GraphProposal::Relationship {
            source_entity_id: a.entity_id.unwrap(),
            target_entity_id: north.entity_id.unwrap(),
            relationship_type: "LOCATED_IN".into(),
        },
        SecurityClassification::Internal,
    )
    .await;
    let va = semantic(
        &db,
        &p,
        &e,
        a.entity_id,
        "Requested settlement automation with daily audit",
    )
    .await;
    let vb = semantic(
        &db,
        &p,
        &e,
        b.entity_id,
        "Requested settlement automation with weekly audit",
    )
    .await;
    let global = semantic(&db, &p, &e, None, "Global settlement automation overview").await;
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::Customer);
    c.relationship = Some(RelationshipConstraint {
        relationship_type: "LOCATED_IN".into(),
        target_entity_ids: vec![north.entity_id.unwrap()],
    });
    let q = read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap());
    let ids = query::hybrid_entities(&db.pool, q).await.unwrap();
    assert_eq!(ids, vec![a.entity_id.unwrap()]);
    assert_eq!(
        query::semantic_fts(&db.pool, q, "settlement automation", Some(&ids))
            .await
            .unwrap(),
        vec![va.id]
    );
    assert_eq!(
        query::semantic_vector(
            &db.pool,
            q,
            Vector::from(vec![1.0; 384]),
            "fictional",
            Some(&ids)
        )
        .await
        .unwrap(),
        vec![va.id]
    );
    assert_eq!(
        query::load_ranked(&db.pool, q, &[vb.id, global.id, va.id])
            .await
            .unwrap()[0]
            .0
            .id,
        va.id
    );
    sqlx::query("UPDATE institutional_versions SET status='REVOKED',valid_until=clock_timestamp() WHERE id=$1").bind(a.id.as_uuid()).execute(&db.pool).await.unwrap();
    assert!(
        query::load_ranked(&db.pool, q, &[va.id])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        !retrieval::fts_search(
            &db.pool,
            p.tenant_id,
            p.id,
            "settlement",
            &["PUBLIC", "INTERNAL"],
            None,
            100
        )
        .await
        .unwrap()
        .contains(&va.id)
    );
    assert!(
        knowledge::list_versions_for_item(&db.pool, va.knowledge_item_id)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn associations_are_immutable_scoped_typed_and_classified() {
    let (db, p, e) = fixture().await;
    let a = make_entity(
        &db,
        &p,
        &e,
        "Customer",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let v = semantic(&db, &p, &e, a.entity_id, "Settlement automation").await;
    assert!(
        sqlx::query("DELETE FROM knowledge_entity_links WHERE knowledge_version_id=$1")
            .bind(v.id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("UPDATE knowledge_entity_links SET evidence_event_ids='{}' WHERE knowledge_version_id=$1").bind(v.id.as_uuid()).execute(&db.pool).await.is_err());
    let unrelated = semantic(&db, &p, &e, None, "No typed association").await;
    let mut tx = db.pool.begin().await.unwrap();
    assert!(
        query::insert_knowledge_links(
            &mut tx,
            p.tenant_id,
            p.id,
            unrelated.id,
            &[a.entity_id.unwrap()]
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    assert!(
        query::insert_knowledge_links(
            &mut tx,
            TenantId::generate(),
            p.id,
            v.id,
            &[a.entity_id.unwrap()]
        )
        .await
        .is_err()
    );
    tx.rollback().await.unwrap();
    // A later class floor hides all linked old semantic versions in existing
    // retrieval paths without rewriting immutable history.
    let mut proposal = a.proposal.clone();
    if let GraphProposal::Entity { name, .. } = &mut proposal {
        *name = "Customer confidential".into();
    }
    promote(&db, &p, &e, proposal, SecurityClassification::Confidential).await;
    assert!(
        knowledge::list_versions_for_item(&db.pool, v.knowledge_item_id)
            .await
            .unwrap()
            .is_empty()
    );
}
#[tokio::test]
async fn temporal_semantic_snapshots_and_recorded_changelog_preserve_transient_versions() {
    let (db, p, e) = fixture().await;
    let first = semantic(&db, &p, &e, None, "Recovery architecture initial").await;
    let start = first.created_at - Duration::minutes(1);
    let boundary = Utc::now();
    let mut tx = db.pool.begin().await.unwrap();
    knowledge::supersede_version_at(&mut tx, first.id, boundary)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let mut second = first.clone();
    second.id = KnowledgeVersionId::generate();
    second.version_number = 2;
    second.content = "Recovery architecture revised".into();
    second.valid_from = boundary;
    second.created_at = boundary;
    second.supersedes_version_id = Some(first.id);
    knowledge::insert_version(&db.pool, &second).await.unwrap();
    knowledge::insert_evidence(&db.pool, &evidence(&second, &e))
        .await
        .unwrap();
    let mut c = constraints(&p);
    c.limit = 10;
    let before = read(&p, &c, boundary - Duration::microseconds(1));
    assert_eq!(
        query::semantic_fts(&db.pool, before, "recovery", None)
            .await
            .unwrap(),
        vec![first.id]
    );
    assert_eq!(
        query::load_ranked(&db.pool, before, &[first.id, second.id])
            .await
            .unwrap()[0]
            .0
            .id,
        first.id
    );
    let after = read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap());
    assert_eq!(
        query::semantic_fts(&db.pool, after, "recovery", None)
            .await
            .unwrap(),
        vec![second.id]
    );
    let changes = query::temporal_changes(
        &db.pool,
        after,
        start,
        Utc::now(),
        &TemporalSubject::Semantic {
            query: "recovery".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(changes.total, 2);
    assert!(!changes.has_more);
    assert_eq!(changes.knowledge.len(), 2);
    c.limit = 1;
    let changes = query::temporal_changes(
        &db.pool,
        read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap()),
        start,
        Utc::now(),
        &TemporalSubject::Semantic {
            query: "recovery".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(changes.total, 2);
    assert!(changes.has_more);
    assert_eq!(changes.knowledge.len(), 1);
}
#[tokio::test]
async fn relationship_filters_apply_before_limits_and_structured_history_is_nonoverlapping() {
    let (db, p, e) = fixture().await;
    let a = make_entity(
        &db,
        &p,
        &e,
        "A",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let target = make_entity(
        &db,
        &p,
        &e,
        "Target",
        EntityKind::Jurisdiction,
        SecurityClassification::Internal,
    )
    .await;
    let edge = promote(
        &db,
        &p,
        &e,
        GraphProposal::Relationship {
            source_entity_id: a.entity_id.unwrap(),
            target_entity_id: target.entity_id.unwrap(),
            relationship_type: "LOCATED_IN".into(),
        },
        SecurityClassification::Internal,
    )
    .await;
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::Jurisdiction);
    assert_eq!(
        query::relationships(
            &db.pool,
            read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap()),
            a.entity_id.unwrap(),
            RelationshipDirection::Outgoing,
            Some("LOCATED_IN")
        )
        .await
        .unwrap()[0]
            .id,
        edge.id
    );
    let mut proposal = a.proposal.clone();
    if let GraphProposal::Entity { name, .. } = &mut proposal {
        *name = "A revised".into();
    }
    let revised = promote(&db, &p, &e, proposal, SecurityClassification::Internal).await;
    c.entity_kind = Some(EntityKind::Customer);
    c.entity_ids = vec![a.entity_id.unwrap()];
    let old = read(&p, &c, revised.valid_from - Duration::microseconds(1));
    match query::structured(&db.pool, old, StructuredOperation::Entities)
        .await
        .unwrap()
    {
        StructuredResult::Entities { entities } => assert_eq!(entities[0].version.id, a.id),
        _ => panic!("wrong result"),
    };
    let current = read(&p, &c, revised.valid_from);
    match query::structured(&db.pool, current, StructuredOperation::Entities)
        .await
        .unwrap()
    {
        StructuredResult::Entities { entities } => assert_eq!(entities[0].version.id, revised.id),
        _ => panic!("wrong result"),
    };
    let changes = query::temporal_changes(
        &db.pool,
        current,
        a.recorded_at - Duration::minutes(1),
        query::snapshot_timestamp(&db.pool).await.unwrap(),
        &TemporalSubject::Structured {
            operation: StructuredOperation::Entities,
        },
    )
    .await
    .unwrap();
    assert!(changes.total >= 3);
    assert!(changes.has_more);
    assert!(changes.institutional[0].observed_at < changes.institutional[0].recorded_at);
}
#[tokio::test]
async fn semantic_bound_data_and_sources_cannot_escape_scope() {
    let (db, p, e) = fixture().await;
    let v = semantic(&db, &p, &e, None, "Settlement automation").await;
    let mut c = constraints(&p);
    c.source_types = vec!["INTERACTION_EVENT".into()];
    let q = read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap());
    assert_eq!(
        query::semantic_fts(&db.pool, q, "settlement", None)
            .await
            .unwrap(),
        vec![v.id]
    );
    assert!(
        query::semantic_fts(&db.pool, q, "'; SELECT * FROM interaction_events; --", None)
            .await
            .unwrap()
            .is_empty()
    );
    c.source_types = vec!["REPOSITORY".into()];
    assert!(
        query::load_ranked(
            &db.pool,
            read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap()),
            &[v.id]
        )
        .await
        .unwrap()
        .is_empty()
    );
}

#[tokio::test]
async fn dense_only_transient_semantic_versions_are_retrieved_between_empty_snapshots() {
    let (db, p, e) = fixture().await;
    let start = query::snapshot_timestamp(&db.pool).await.unwrap();
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let introduced = query::snapshot_timestamp(&db.pool).await.unwrap();
    let mut v = version(&i, 1, KnowledgeStatus::Active);
    v.content = "Batch ledger latency".into();
    v.valid_from = introduced;
    v.created_at = introduced;
    knowledge::insert_version(&db.pool, &v).await.unwrap();
    knowledge::insert_evidence(&db.pool, &evidence(&v, &e))
        .await
        .unwrap();
    embeddings::upsert_knowledge_version_embedding(
        &db.pool,
        embeddings::NewEmbedding {
            entity_id: v.id,
            model: "fictional",
            model_version: "1",
            dimensions: 384,
            vector: Vector::from(vec![1.0; 384]),
        },
    )
    .await
    .unwrap();
    let ended = query::snapshot_timestamp(&db.pool).await.unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    knowledge::supersede_version_at(&mut tx, v.id, ended)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let end = query::snapshot_timestamp(&db.pool).await.unwrap();
    let c = constraints(&p);
    for time in [start, end] {
        assert!(
            query::semantic_vector(
                &db.pool,
                read(&p, &c, time),
                Vector::from(vec![1.0; 384]),
                "fictional",
                None
            )
            .await
            .unwrap()
            .is_empty()
        );
    }
    let subject = TemporalSubject::Semantic {
        query: "inversion resilience".into(),
    };
    assert_eq!(
        query::temporal_changes(&db.pool, read(&p, &c, end), start, end, &subject)
            .await
            .unwrap()
            .total,
        0
    );
    let changes = query::temporal_changes_with_semantic(
        &db.pool,
        read(&p, &c, end),
        start,
        end,
        &subject,
        Some(query::TemporalSemanticInput {
            embedding: Vector::from(vec![1.0; 384]),
            model: "fictional",
        }),
    )
    .await
    .unwrap();
    assert_eq!(changes.total, 1);
    assert_eq!(changes.knowledge[0].0.id, v.id);
    assert_eq!(changes.candidate_limit, Some(4));
    assert!(!changes.has_more);
}
#[tokio::test]
async fn bounded_hybrid_entity_output_does_not_limit_the_sql_association_population() {
    let (db, p, e) = fixture().await;
    let a = make_entity(
        &db,
        &p,
        &e,
        "First",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let b = make_entity(
        &db,
        &p,
        &e,
        "Second",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let selected = semantic(
        &db,
        &p,
        &e,
        b.entity_id,
        "Settlement precision requirements",
    )
    .await;
    let global = semantic(&db, &p, &e, None, "Settlement precision generic advice").await;
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::Customer);
    let q = read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap());
    assert_eq!(
        query::hybrid_entities(&db.pool, q).await.unwrap(),
        vec![a.entity_id.unwrap()]
    );
    assert_eq!(
        query::hybrid_fts(&db.pool, q, "precision").await.unwrap(),
        vec![selected.id]
    );
    assert_eq!(
        query::hybrid_vector(&db.pool, q, Vector::from(vec![1.0; 384]), "fictional")
            .await
            .unwrap(),
        vec![selected.id]
    );
    assert_eq!(
        query::load_hybrid_ranked(&db.pool, q, &[global.id, selected.id])
            .await
            .unwrap()[0]
            .0
            .id,
        selected.id
    );
    // An unconstrained hybrid still requires an entity association.
    let at = q.as_of;
    c.entity_kind = None;
    assert_eq!(
        query::hybrid_fts(&db.pool, read(&p, &c, at), "precision")
            .await
            .unwrap(),
        vec![selected.id]
    );
}
#[tokio::test]
async fn relationship_trust_and_sources_filter_the_returned_assertion_before_limit() {
    let (db, p, e) = fixture().await;
    let start = query::snapshot_timestamp(&db.pool).await.unwrap();
    let a = make_entity(
        &db,
        &p,
        &e,
        "Subject",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let b = make_entity(
        &db,
        &p,
        &e,
        "Target",
        EntityKind::Jurisdiction,
        SecurityClassification::Internal,
    )
    .await;
    let human = promote(
        &db,
        &p,
        &e,
        GraphProposal::Relationship {
            source_entity_id: a.entity_id.unwrap(),
            target_entity_id: b.entity_id.unwrap(),
            relationship_type: "HUMAN_EDGE".into(),
        },
        SecurityClassification::Internal,
    )
    .await;
    let mut board = e.clone();
    board.id = EventId::generate();
    board.sequence = 1;
    board.source = "board-record".into();
    events::insert(&db.pool, &board).await.unwrap();
    let c = GraphCandidate {
        id: GraphCandidateId::generate(),
        tenant_id: p.tenant_id,
        project_id: p.id,
        proposal: GraphProposal::Relationship {
            source_entity_id: a.entity_id.unwrap(),
            target_entity_id: b.entity_id.unwrap(),
            relationship_type: "DERIVED_EDGE".into(),
        },
        evidence_event_ids: vec![board.id],
        security_classification: SecurityClassification::Internal,
        proposed_by: ProposerKind::Agent,
        confidence: None,
        status: CandidateStatus::Pending,
        observed_at: Utc::now() - Duration::days(1),
        created_at: Utc::now(),
    };
    institutional::insert_candidate(&db.pool, &c).await.unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    let derived = institutional::promote_candidate(
        &mut tx,
        &c,
        TrustLevel::AgentDerived,
        SecurityClassification::Internal,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut filters = constraints(&p);
    filters.trust_levels = vec![TrustLevel::AgentDerived];
    filters.source_types = vec!["board-record".into()];
    let result = query::relationships(
        &db.pool,
        read(
            &p,
            &filters,
            query::snapshot_timestamp(&db.pool).await.unwrap(),
        ),
        a.entity_id.unwrap(),
        RelationshipDirection::Outgoing,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, derived.id);
    let temporal = query::temporal_changes(
        &db.pool,
        read(
            &p,
            &filters,
            query::snapshot_timestamp(&db.pool).await.unwrap(),
        ),
        start,
        query::snapshot_timestamp(&db.pool).await.unwrap(),
        &TemporalSubject::Structured {
            operation: StructuredOperation::Entities,
        },
    )
    .await
    .unwrap();
    assert_eq!(temporal.total, 1);
    assert_eq!(temporal.institutional[0].id, derived.id);
    filters.trust_levels = vec![TrustLevel::HumanExplicit];
    filters.source_types = vec![e.source.clone()];
    let result = query::relationships(
        &db.pool,
        read(
            &p,
            &filters,
            query::snapshot_timestamp(&db.pool).await.unwrap(),
        ),
        a.entity_id.unwrap(),
        RelationshipDirection::Outgoing,
        None,
    )
    .await
    .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, human.id);
    filters.source_types = vec!["board-record".into()];
    assert!(
        query::relationships(
            &db.pool,
            read(
                &p,
                &filters,
                query::snapshot_timestamp(&db.pool).await.unwrap()
            ),
            a.entity_id.unwrap(),
            RelationshipDirection::Outgoing,
            None
        )
        .await
        .unwrap()
        .is_empty()
    );
}

#[tokio::test]
async fn temporal_dense_fusion_applies_domain_trust_weights_before_limiting() {
    let (db, p, e) = fixture().await;
    let start = query::snapshot_timestamp(&db.pool).await.unwrap();
    let mut versions = Vec::new();
    for trust in [TrustLevel::ExternalUntrusted, TrustLevel::HumanExplicit] {
        let mut i = item(&p);
        i.subject_key = KnowledgeItemId::generate().to_string();
        knowledge::insert_item(&db.pool, &i).await.unwrap();
        let time = query::snapshot_timestamp(&db.pool).await.unwrap();
        let mut v = version(&i, 1, KnowledgeStatus::Active);
        v.content = "Batch ledger latency".into();
        v.trust_level = trust;
        v.valid_from = time;
        v.created_at = time;
        knowledge::insert_version(&db.pool, &v).await.unwrap();
        knowledge::insert_evidence(&db.pool, &evidence(&v, &e))
            .await
            .unwrap();
        let mut vector = vec![1.0; 384];
        if trust == TrustLevel::HumanExplicit {
            vector[0] = 2.0;
        }
        embeddings::upsert_knowledge_version_embedding(
            &db.pool,
            embeddings::NewEmbedding {
                entity_id: v.id,
                model: "fictional",
                model_version: "1",
                dimensions: 384,
                vector: Vector::from(vector),
            },
        )
        .await
        .unwrap();
        versions.push(v);
    }
    let end = query::snapshot_timestamp(&db.pool).await.unwrap();
    let c = constraints(&p);
    let q = read(&p, &c, end);
    assert_eq!(
        query::semantic_vector(&db.pool, q, Vector::from(vec![1.0; 384]), "fictional", None)
            .await
            .unwrap()[0],
        versions[0].id
    );
    let changes = query::temporal_changes_with_semantic(
        &db.pool,
        q,
        start,
        end,
        &TemporalSubject::Semantic {
            query: "inversion resilience".into(),
        },
        Some(query::TemporalSemanticInput {
            embedding: Vector::from(vec![1.0; 384]),
            model: "fictional",
        }),
    )
    .await
    .unwrap();
    assert_eq!(changes.total, 2);
    assert!(changes.has_more);
    assert_eq!(changes.knowledge[0].0.id, versions[1].id);
}

#[tokio::test]
async fn typed_queries_deny_legacy_zero_evidence_versions_and_preserve_disk_history() {
    let (db, p, _e) = fixture().await;
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let at = query::snapshot_timestamp(&db.pool).await.unwrap();
    let mut v = version(&i, 1, KnowledgeStatus::Active);
    v.content = "Recovery feature historical legacy".into();
    v.valid_from = at;
    v.created_at = at;
    knowledge::insert_version(&db.pool, &v).await.unwrap();
    embeddings::upsert_knowledge_version_embedding(
        &db.pool,
        embeddings::NewEmbedding {
            entity_id: v.id,
            model: "fictional",
            model_version: "1",
            dimensions: 384,
            vector: Vector::from(vec![1.0; 384]),
        },
    )
    .await
    .unwrap();
    let end = query::snapshot_timestamp(&db.pool).await.unwrap();
    let c = constraints(&p);
    let q = read(&p, &c, end);
    assert!(
        query::semantic_fts(&db.pool, q, "recovery feature", None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        query::semantic_vector(&db.pool, q, Vector::from(vec![1.0; 384]), "fictional", None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        query::load_ranked(&db.pool, q, &[v.id])
            .await
            .unwrap()
            .is_empty()
    );
    let changes = query::temporal_changes_with_semantic(
        &db.pool,
        q,
        at - Duration::seconds(1),
        end,
        &TemporalSubject::Semantic {
            query: "recovery feature".into(),
        },
        Some(query::TemporalSemanticInput {
            embedding: Vector::from(vec![1.0; 384]),
            model: "fictional",
        }),
    )
    .await
    .unwrap();
    assert_eq!(changes.total, 0);
    assert_eq!(
        knowledge::get_version_unscoped(&db.pool, v.id)
            .await
            .unwrap()
            .content,
        v.content
    );
    // Legacy history has a separate, tracked provenance policy gap. This
    // bounded admission change neither rewrites it nor claims global repair.
    assert_eq!(
        knowledge::list_versions_for_item(&db.pool, i.id)
            .await
            .unwrap()
            .len(),
        1
    );
}
#[tokio::test]
async fn malformed_legacy_entity_metadata_cannot_bypass_current_link_visibility() {
    let (db, p, e) = fixture().await;
    let entity = make_entity(
        &db,
        &p,
        &e,
        "LegacyCustomer",
        EntityKind::Customer,
        SecurityClassification::Internal,
    )
    .await;
    let i = item(&p);
    knowledge::insert_item(&db.pool, &i).await.unwrap();
    let mut v = version(&i, 1, KnowledgeStatus::Active);
    v.content = "Recovery feature malformed association".into();
    v.structured_content = Some(json!({"entity_ids":entity.entity_id.unwrap().to_string()}));
    knowledge::insert_version(&db.pool, &v).await.unwrap();
    knowledge::insert_evidence(&db.pool, &evidence(&v, &e))
        .await
        .unwrap();
    let c = constraints(&p);
    let q = read(&p, &c, query::snapshot_timestamp(&db.pool).await.unwrap());
    assert!(
        query::semantic_fts(&db.pool, q, "recovery feature", None)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        query::load_ranked(&db.pool, q, &[v.id])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        knowledge::list_versions_for_item(&db.pool, i.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        knowledge::get_version_unscoped(&db.pool, v.id)
            .await
            .unwrap()
            .structured_content,
        v.structured_content
    );
}
