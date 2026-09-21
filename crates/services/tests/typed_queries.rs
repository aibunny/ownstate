//! Public fictional institutional scenarios against migrated PostgreSQL.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use chrono::{Duration, Utc};
use ownstate_domain::*;
use ownstate_embeddings::DeterministicProvider;
use ownstate_services::{
    AppServices, ServiceError, institutional::ProposeInstitutional, policy::PrincipalServices,
    projects::CreateProject, sessions::CreateSession,
};
use ownstate_storage::test_support::{TestDb, fresh_db};
use std::{collections::BTreeMap, sync::Arc};

async fn fixture() -> (
    Arc<AppServices>,
    PrincipalServices,
    TestDb,
    Project,
    EventId,
) {
    let db = fresh_db().await.unwrap();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::generate(),
    ));
    let owner = app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = app.for_principal(owner).unwrap();
    let p = s
        .create_project(CreateProject {
            name: "Fictional Atlas".into(),
            description: None,
            ownership_domain: None,
            metadata: None,
        })
        .await
        .unwrap();
    let session = s
        .create_session(CreateSession {
            project_id: p.id,
            source: "fictional-record".into(),
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
            vec![NewInteractionEvent {
                event_type: InteractionEventType::DocumentRead,
                actor_type: ActorType::User,
                actor_id: None,
                sequence: None,
                occurred_at: None,
                source_event_id: None,
                model_provider: None,
                model_name: None,
                content: Some("Fictional legal register and board minutes".into()),
                tool_name: None,
                tool_call_id: None,
                repository: None,
                branch: None,
                commit_sha: None,
                file_path: None,
                security_classification: Some(SecurityClassification::Internal),
                metadata: None,
            }],
        )
        .await
        .unwrap()
        .remove(0);
    (app, s, db, p, event.id)
}
fn entity(name: &str, kind: EntityKind, identifier: &str, aliases: &[&str]) -> GraphProposal {
    GraphProposal::Entity {
        kind,
        name: name.into(),
        external_ids: if identifier.is_empty() {
            BTreeMap::new()
        } else {
            BTreeMap::from([("register".into(), identifier.into())])
        },
        aliases: aliases.iter().map(|a| (*a).into()).collect(),
    }
}
fn request(
    p: &Project,
    event: EventId,
    proposal: GraphProposal,
    by: ProposerKind,
) -> ProposeInstitutional {
    ProposeInstitutional {
        project_id: p.id,
        proposal,
        evidence_event_ids: vec![event],
        security_classification: Some(SecurityClassification::Public),
        observed_at: Some(Utc::now() - Duration::days(2)),
        proposed_by: by,
        confidence: Some(0.9),
    }
}
async fn promote(
    s: &PrincipalServices,
    p: &Project,
    e: EventId,
    proposal: GraphProposal,
) -> InstitutionalVersion {
    let c = s
        .propose_institutional(request(p, e, proposal, ProposerKind::Human))
        .await
        .unwrap();
    s.promote_institutional(c.id).await.unwrap()
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
        limit: 10,
    }
}
async fn semantic(
    s: &PrincipalServices,
    p: &Project,
    event: EventId,
    key: &str,
    text: &str,
    ids: &[EntityId],
) -> KnowledgeVersion {
    let c = s
        .propose_knowledge(ownstate_services::knowledge::ProposeKnowledge {
            project_id: p.id,
            session_id: None,
            kind: KnowledgeKind::Requirement,
            subject_key: key.into(),
            content: text.into(),
            structured_content: Some(serde_json::json!({"entity_ids":ids})),
            confidence: Some(0.9),
            proposed_by: ProposerKind::Human,
            source: Some("fictional".into()),
            evidence_event_ids: vec![event],
            security_classification: Some(SecurityClassification::Public),
        })
        .await
        .unwrap();
    s.promote_candidate(c.id).await.unwrap().1
}

#[tokio::test]
async fn hybrid_query_returns_only_customer_associated_semantics_and_withholds_revoked_links() {
    use ownstate_services::query::{QueryResult, QuerySnapshot};
    let (_app, s, db, p, e) = fixture().await;
    let customer = promote(
        &s,
        &p,
        e,
        entity("Fictional Mercury", EntityKind::Customer, "M", &[]),
    )
    .await;
    let vendor = promote(
        &s,
        &p,
        e,
        entity("Fictional supplier", EntityKind::Vendor, "V", &[]),
    )
    .await;
    let linked = semantic(
        &s,
        &p,
        e,
        "mercury.feature",
        "Mercury requested recovery feature support",
        &[customer.entity_id.unwrap()],
    )
    .await;
    semantic(
        &s,
        &p,
        e,
        "vendor.feature",
        "Supplier requested recovery feature support",
        &[vendor.entity_id.unwrap()],
    )
    .await;
    semantic(
        &s,
        &p,
        e,
        "global.feature",
        "Generic recovery feature support",
        &[],
    )
    .await;
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::Customer);
    let plan = QueryPlan::Hybrid {
        constraints: c.clone(),
        query: "recovery feature".into(),
    };
    let result = s.execute_query(plan.clone()).await.unwrap();
    match result {
        QueryResult::Snapshot {
            snapshot:
                QuerySnapshot::Hybrid {
                    matched_entity_ids,
                    matches,
                    ..
                },
            ..
        } => {
            assert_eq!(matched_entity_ids, vec![customer.entity_id.unwrap()]);
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].version.id, linked.id);
            assert_eq!(
                matches[0].version.security_classification,
                SecurityClassification::Internal
            );
            assert_eq!(matches[0].evidence[0].event_id, Some(e));
        }
        _ => panic!("wrong typed response"),
    }
    sqlx::query("UPDATE institutional_versions SET status='REVOKED' WHERE id=$1")
        .bind(customer.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    match s.execute_query(plan).await.unwrap() {
        QueryResult::Snapshot {
            snapshot:
                QuerySnapshot::Hybrid {
                    matched_entity_ids,
                    matches,
                    ..
                },
            ..
        } => {
            assert!(matched_entity_ids.is_empty());
            assert!(matches.is_empty());
        }
        _ => panic!("wrong typed response"),
    }
    let legacy = s
        .search_knowledge(ownstate_services::search::SearchParams {
            project_id: p.id,
            query: "recovery feature".into(),
            kinds: None,
            limit: 10,
            max_classification: SecurityClassification::Internal,
        })
        .await
        .unwrap();
    assert!(!legacy.iter().any(|h| h.version.id == linked.id));
    assert!(semantic_request_rejected(&s, &p, e, customer.entity_id.unwrap()).await);
}
async fn semantic_request_rejected(
    s: &PrincipalServices,
    p: &Project,
    e: EventId,
    id: EntityId,
) -> bool {
    s.propose_knowledge(ownstate_services::knowledge::ProposeKnowledge {
        project_id: p.id,
        session_id: None,
        kind: KnowledgeKind::Fact,
        subject_key: "bad.association".into(),
        content: "Fictional fact".into(),
        structured_content: Some(serde_json::json!({"entity_ids":[id]})),
        confidence: None,
        proposed_by: ProposerKind::Agent,
        source: None,
        evidence_event_ids: vec![e],
        security_classification: None,
    })
    .await
    .is_err()
}

#[tokio::test]
async fn temporal_service_preserves_transient_changes_and_historical_semantic_versions() {
    use ownstate_services::query::{QueryResult, QuerySnapshot};
    let (app, s, _db, p, e) = fixture().await;
    let start = ownstate_storage::query::snapshot_timestamp(app.pool())
        .await
        .unwrap();
    let first = semantic(
        &s,
        &p,
        e,
        "recovery.design",
        "Recovery feature uses threshold signatures",
        &[],
    )
    .await;
    let midpoint = ownstate_storage::query::snapshot_timestamp(app.pool())
        .await
        .unwrap();
    let second = semantic(
        &s,
        &p,
        e,
        "recovery.design",
        "Recovery feature uses threshold signatures with reviewed rotation",
        &[],
    )
    .await;
    let end = ownstate_storage::query::snapshot_timestamp(app.pool())
        .await
        .unwrap();
    let c = constraints(&p);
    let r = s
        .execute_query(QueryPlan::Temporal {
            constraints: c.clone(),
            start,
            end,
            subject: TemporalSubject::Semantic {
                query: "recovery feature".into(),
            },
        })
        .await
        .unwrap();
    match r {
        QueryResult::Temporal {
            recorded_changes,
            recorded_change_evidence,
            before,
            after,
            ..
        } => {
            assert_eq!(recorded_changes.total, 2);
            assert_eq!(recorded_change_evidence.len(), 2);
            assert!(
                recorded_change_evidence
                    .iter()
                    .all(|source| source.event_id == Some(e))
            );
            assert!(!recorded_changes.has_more);
            assert!(
                recorded_changes
                    .knowledge
                    .iter()
                    .any(|(v, _)| v.id == first.id)
            );
            assert!(
                recorded_changes
                    .knowledge
                    .iter()
                    .any(|(v, _)| v.id == second.id)
            );
            assert!(matches!(before, QuerySnapshot::Semantic { matches } if matches.is_empty()));
            assert!(
                matches!(after, QuerySnapshot::Semantic { matches } if matches.len() == 1 && matches[0].version.id == second.id)
            );
        }
        _ => panic!("wrong temporal response"),
    }
    let mut c = c;
    c.as_of = Some(midpoint);
    match s
        .execute_query(QueryPlan::Semantic {
            constraints: c,
            query: "recovery feature".into(),
        })
        .await
        .unwrap()
    {
        QueryResult::Snapshot {
            snapshot: QuerySnapshot::Semantic { matches },
            ..
        } => {
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].version.id, first.id);
            assert_eq!(matches[0].version.status, KnowledgeStatus::Superseded);
        }
        _ => panic!("wrong snapshot"),
    }
}

#[tokio::test]
async fn typed_queries_reject_wrong_project_and_malformed_entity_associations() {
    let (_app, s, _db, p, e) = fixture().await;
    let mut c = constraints(&p);
    c.project_id = ProjectId::generate();
    assert!(matches!(
        s.execute_query(QueryPlan::Semantic {
            constraints: c,
            query: "fictional".into()
        })
        .await,
        Err(ServiceError::NotFound(_))
    ));
    assert!(semantic_request_rejected(&s, &p, e, EntityId::generate()).await);
    let result = s
        .propose_knowledge(ownstate_services::knowledge::ProposeKnowledge {
            project_id: p.id,
            session_id: None,
            kind: KnowledgeKind::Fact,
            subject_key: "bad.uuid".into(),
            content: "Fictional fact".into(),
            structured_content: Some(serde_json::json!({"entity_ids":["not-a-uuid"]})),
            confidence: None,
            proposed_by: ProposerKind::Agent,
            source: None,
            evidence_event_ids: vec![e],
            security_classification: None,
        })
        .await;
    assert!(matches!(result, Err(ServiceError::Validation(_))));
}

struct CountingEmbedder(std::sync::atomic::AtomicUsize);
#[async_trait::async_trait]
impl ownstate_embeddings::EmbeddingProvider for CountingEmbedder {
    fn model_name(&self) -> &str {
        "fictional-counting"
    }
    fn model_version(&self) -> &str {
        "1"
    }
    fn dimensions(&self) -> usize {
        384
    }
    async fn embed(
        &self,
        _texts: &[String],
    ) -> Result<Vec<Vec<f32>>, ownstate_embeddings::EmbeddingError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Err(ownstate_embeddings::EmbeddingError::Embed(
            "fictional unavailable embedder".into(),
        ))
    }
}

#[tokio::test]
async fn exact_structured_queries_need_no_model_and_wrong_scope_is_denied_before_embedding() {
    use ownstate_services::query::{QueryResult, QuerySnapshot};
    let (_app, original, db, p, e) = fixture().await;
    promote(
        &original,
        &p,
        e,
        entity("Fictional legal", EntityKind::LegalEntity, "L", &[]),
    )
    .await;
    let embedder = Arc::new(CountingEmbedder(std::sync::atomic::AtomicUsize::new(0)));
    let app2 = Arc::new(AppServices::new(
        db.pool.clone(),
        embedder.clone(),
        _app.tenant_id(),
    ));
    let owner2 = app2
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let s = app2.for_principal(owner2).unwrap();
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::LegalEntity);
    c.limit = 1;
    match s
        .execute_query(QueryPlan::Structured {
            constraints: c.clone(),
            operation: StructuredOperation::Count,
        })
        .await
        .unwrap()
    {
        QueryResult::Snapshot {
            bounded_by_limit: false,
            semantic_candidate_limit: None,
            snapshot:
                QuerySnapshot::Structured {
                    result:
                        StructuredResult::Count {
                            count,
                            evidence_event_ids,
                        },
                },
            ..
        } => {
            assert_eq!(count, 1);
            assert_eq!(evidence_event_ids, vec![e]);
        }
        _ => panic!("wrong exact count response"),
    }
    c.project_id = ProjectId::generate();
    assert!(matches!(
        s.execute_query(QueryPlan::Semantic {
            constraints: c,
            query: "fictional".into()
        })
        .await,
        Err(ServiceError::NotFound(_))
    ));
    assert_eq!(embedder.0.load(std::sync::atomic::Ordering::SeqCst), 0);
}

#[tokio::test]
async fn hybrid_response_entity_limit_does_not_limit_sql_association_candidates() {
    use ownstate_services::query::{QueryResult, QuerySnapshot};
    let (_app, s, _db, p, e) = fixture().await;
    let mut customers = Vec::new();
    for id in ["fictional-A", "fictional-B", "fictional-C"] {
        customers.push(
            promote(&s, &p, e, entity(id, EntityKind::Customer, id, &[]))
                .await
                .entity_id
                .unwrap(),
        );
    }
    let linked = semantic(
        &s,
        &p,
        e,
        "last.customer.promise",
        "Unique recovery commitment requested feature",
        &[customers[2]],
    )
    .await;
    let mut c = constraints(&p);
    c.entity_kind = Some(EntityKind::Customer);
    c.limit = 1;
    match s
        .execute_query(QueryPlan::Hybrid {
            constraints: c,
            query: "unique recovery commitment".into(),
        })
        .await
        .unwrap()
    {
        QueryResult::Snapshot {
            snapshot:
                QuerySnapshot::Hybrid {
                    matched_entity_ids,
                    matched_entity_count,
                    has_more_entities,
                    matches,
                },
            ..
        } => {
            assert_eq!(matched_entity_ids.len(), 1);
            assert_eq!(matched_entity_count, 3);
            assert!(has_more_entities);
            assert_eq!(matches.len(), 1);
            assert_eq!(matches[0].version.id, linked.id);
            assert_eq!(
                matches[0].version.structured_content.as_ref().unwrap()["entity_ids"],
                serde_json::json!([customers[2]])
            );
        }
        _ => panic!("wrong hybrid response"),
    }
}
