//! Public fictional institutional scenarios against migrated PostgreSQL.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use chrono::{Duration, Utc};
use ownstate_domain::*;
use ownstate_embeddings::DeterministicProvider;
use ownstate_services::{
    AppServices, ServiceError,
    institutional::{InstitutionalQuery, ProposeInstitutional},
    policy::PrincipalServices,
    projects::CreateProject,
    sessions::CreateSession,
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
fn query(p: &Project, as_of: Option<chrono::DateTime<Utc>>) -> InstitutionalQuery {
    InstitutionalQuery {
        project_id: p.id,
        as_of,
        limit: Some(100),
    }
}

#[tokio::test]
async fn fictional_legal_entities_jurisdictions_and_claims_have_temporal_provenance() {
    let (_app, s, _db, p, e) = fixture().await;
    let legal = promote(
        &s,
        &p,
        e,
        entity(
            "Atlas Research Ltd",
            EntityKind::LegalEntity,
            "ATLAS-001",
            &["Atlas"],
        ),
    )
    .await;
    let jurisdiction = promote(
        &s,
        &p,
        e,
        entity("Fictional North", EntityKind::Jurisdiction, "NORTH", &[]),
    )
    .await;
    let relation = promote(
        &s,
        &p,
        e,
        GraphProposal::Relationship {
            source_entity_id: legal.entity_id.unwrap(),
            target_entity_id: jurisdiction.entity_id.unwrap(),
            relationship_type: "REGISTERED_IN".into(),
        },
    )
    .await;
    let claim = promote(
        &s,
        &p,
        e,
        GraphProposal::Claim {
            subject_entity_id: legal.entity_id.unwrap(),
            predicate: "registration_status".into(),
            object: serde_json::json!("active"),
        },
    )
    .await;
    assert_eq!(
        claim.security_classification,
        SecurityClassification::Internal
    );
    assert_eq!(claim.evidence_event_ids, vec![e]);
    assert_eq!(claim.confidence, Some(0.9));
    assert!(claim.observed_at < claim.recorded_at);
    assert_eq!(claim.valid_from, claim.recorded_at);
    assert_eq!(
        s.resolve_entity(p.id, "atlas", None, None)
            .await
            .unwrap()
            .matches[0]
            .id,
        legal.entity_id.unwrap()
    );
    assert_eq!(
        s.resolve_entity(p.id, "ATLAS-001", Some("register"), None)
            .await
            .unwrap()
            .matches
            .len(),
        1
    );
    let relations = s
        .entity_relationships(p.id, legal.entity_id.unwrap(), 1, None)
        .await
        .unwrap();
    assert_eq!(relations[0].id, relation.id);
    assert_eq!(
        s.institutional_snapshot(query(&p, None), None, Some("ENTITY"))
            .await
            .unwrap()
            .versions
            .len(),
        2
    );
}

#[tokio::test]
async fn candidates_are_invisible_and_cannot_be_replayed() {
    let (_app, s, _db, p, e) = fixture().await;
    let c = s
        .propose_institutional(request(
            &p,
            e,
            entity("Atlas", EntityKind::LegalEntity, "A", &[]),
            ProposerKind::Agent,
        ))
        .await
        .unwrap();
    assert!(
        s.institutional_snapshot(query(&p, None), None, None)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    let v = s.promote_institutional(c.id).await.unwrap();
    assert_eq!(v.trust_level, TrustLevel::AgentDerived);
    assert!(matches!(
        s.promote_institutional(c.id).await,
        Err(ServiceError::Conflict(_))
    ));
}

#[tokio::test]
async fn exact_identity_updates_preserve_versions_and_as_of_intervals() {
    let (_app, s, _db, p, e) = fixture().await;
    let old = promote(
        &s,
        &p,
        e,
        entity("Atlas Ltd", EntityKind::LegalEntity, "A", &["Atlas"]),
    )
    .await;
    let new = promote(
        &s,
        &p,
        e,
        entity(
            "Atlas Research Ltd",
            EntityKind::LegalEntity,
            "A",
            &["Atlas", "Atlas Ltd"],
        ),
    )
    .await;
    assert_eq!(old.entity_id, new.entity_id);
    assert_eq!(new.supersedes_version_id, Some(old.id));
    let history = s
        .institutional_history(p.id, old.entity_id.unwrap().as_uuid())
        .await
        .unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].status, KnowledgeStatus::Superseded);
    assert_eq!(history[0].valid_until, Some(new.valid_from));
    let past = s
        .institutional_snapshot(
            query(&p, Some(old.valid_from)),
            Some(old.entity_id.unwrap().as_uuid()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(past.versions[0].id, old.id);
    let present = s
        .institutional_snapshot(
            query(&p, Some(new.valid_from)),
            Some(old.entity_id.unwrap().as_uuid()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(present.versions[0].id, new.id);
}

#[tokio::test]
async fn shared_alias_is_ambiguous_and_incompatible_external_ids_never_merge() {
    let (_app, s, db, p, e) = fixture().await;
    let a = promote(
        &s,
        &p,
        e,
        entity("Mercury One", EntityKind::LegalEntity, "ONE", &["Mercury"]),
    )
    .await;
    let b = promote(
        &s,
        &p,
        e,
        entity("Mercury Two", EntityKind::LegalEntity, "TWO", &["Mercury"]),
    )
    .await;
    assert_ne!(a.entity_id, b.entity_id);
    let r = s.resolve_entity(p.id, "Mercury", None, None).await.unwrap();
    assert!(r.ambiguous);
    assert_eq!(r.matches.len(), 2);
    // A contradictory second identifier must not overwrite the first identity.
    let mut proposal = entity("Mercury renamed", EntityKind::LegalEntity, "ONE", &[]);
    if let GraphProposal::Entity { external_ids, .. } = &mut proposal {
        external_ids.insert("other-register".into(), "XYZ".into());
    }
    let v = promote(&s, &p, e, proposal).await;
    let mut bad = entity("Mercury contradiction", EntityKind::LegalEntity, "ONE", &[]);
    if let GraphProposal::Entity { external_ids, .. } = &mut bad {
        external_ids.insert("other-register".into(), "DIFFERENT".into());
    }
    let c = s
        .propose_institutional(request(&p, e, bad, ProposerKind::Human))
        .await
        .unwrap();
    assert!(matches!(
        s.promote_institutional(c.id).await,
        Err(ServiceError::Conflict(_))
    ));
    let (status,): (String,) =
        sqlx::query_as("SELECT status FROM institutional_candidates WHERE id=$1")
            .bind(c.id.as_uuid())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(status, "PENDING");
    assert_eq!(
        s.resolve_entity(p.id, "ONE", Some("register"), None)
            .await
            .unwrap()
            .matches[0]
            .version
            .id,
        v.id
    );
}

#[tokio::test]
async fn evidence_and_assertion_endpoints_cannot_cross_project_or_tenant() {
    let (_app, s, db, p, e) = fixture().await;
    let other = s
        .create_project(CreateProject {
            name: "Other".into(),
            description: None,
            ownership_domain: None,
            metadata: None,
        })
        .await
        .unwrap();
    let mut req = request(
        &other,
        e,
        entity("foreign", EntityKind::LegalEntity, "X", &[]),
        ProposerKind::Human,
    );
    assert!(s.propose_institutional(req).await.is_err());
    let a = promote(
        &s,
        &p,
        e,
        entity("Atlas", EntityKind::LegalEntity, "A", &[]),
    )
    .await;
    req = request(
        &p,
        e,
        GraphProposal::Relationship {
            source_entity_id: a.entity_id.unwrap(),
            target_entity_id: EntityId::generate(),
            relationship_type: "PARTNER".into(),
        },
        ProposerKind::Human,
    );
    assert!(s.propose_institutional(req).await.is_err());
    let foreign_app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::generate(),
    ));
    let foreign_owner = foreign_app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let foreign = foreign_app.for_principal(foreign_owner).unwrap();
    assert!(matches!(
        foreign
            .institutional_snapshot(query(&p, None), None, None)
            .await,
        Err(ServiceError::NotFound(_))
    ));
    let limited_app = Arc::new(
        AppServices::new(
            db.pool.clone(),
            Arc::new(DeterministicProvider::new()),
            _app.tenant_id(),
        )
        .with_max_classification(SecurityClassification::Public),
    );
    let limited_owner = limited_app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let limited = limited_app.for_principal(limited_owner).unwrap();
    assert!(
        limited
            .institutional_snapshot(query(&p, None), None, None)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    assert!(
        limited
            .resolve_entity(p.id, "A", Some("register"), None)
            .await
            .unwrap()
            .matches
            .is_empty()
    );
    assert!(
        limited
            .institutional_history(p.id, a.entity_id.unwrap().as_uuid())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn postgres_rejects_missing_provenance_and_freezes_content_evidence_and_recording_time() {
    let (_app, s, db, p, e) = fixture().await;
    let mut c = s
        .propose_institutional(request(
            &p,
            e,
            entity("Atlas", EntityKind::LegalEntity, "A", &[]),
            ProposerKind::Human,
        ))
        .await
        .unwrap();
    c.id = GraphCandidateId::generate();
    c.security_classification = SecurityClassification::Public;
    assert!(
        ownstate_storage::institutional::insert_candidate(&db.pool, &c)
            .await
            .is_err()
    );
    c.security_classification = SecurityClassification::Internal;
    c.evidence_event_ids.clear();
    assert!(
        ownstate_storage::institutional::insert_candidate(&db.pool, &c)
            .await
            .is_err()
    );
    let v = promote(
        &s,
        &p,
        e,
        entity("Atlas", EntityKind::LegalEntity, "A", &[]),
    )
    .await;
    assert!(
        sqlx::query("UPDATE institutional_versions SET proposal='{}' WHERE id=$1")
            .bind(v.id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE institutional_versions SET evidence_event_ids='{}' WHERE id=$1")
            .bind(v.id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE institutional_versions SET recorded_at=clock_timestamp() WHERE id=$1")
            .bind(v.id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM institutional_versions WHERE id=$1")
            .bind(v.id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
    sqlx::query("UPDATE institutional_versions SET status='STALE' WHERE id=$1")
        .bind(v.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(
        s.institutional_snapshot(query(&p, None), None, None)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    assert_eq!(
        s.institutional_history(p.id, v.entity_id.unwrap().as_uuid())
            .await
            .unwrap()[0]
            .status,
        KnowledgeStatus::Stale
    );
}

#[tokio::test]
async fn lower_authority_contradiction_preserves_current_claim_and_visible_conflict() {
    let (_app, s, _db, p, e) = fixture().await;
    let a = promote(
        &s,
        &p,
        e,
        entity("Atlas", EntityKind::LegalEntity, "A", &[]),
    )
    .await;
    let claim = GraphProposal::Claim {
        subject_entity_id: a.entity_id.unwrap(),
        predicate: "registered_in".into(),
        object: serde_json::json!("North"),
    };
    let old = promote(&s, &p, e, claim).await;
    let contradictory = GraphProposal::Claim {
        subject_entity_id: a.entity_id.unwrap(),
        predicate: "registered_in".into(),
        object: serde_json::json!("South"),
    };
    let c = s
        .propose_institutional(request(&p, e, contradictory, ProposerKind::Agent))
        .await
        .unwrap();
    let conflict = s.promote_institutional(c.id).await.unwrap();
    assert_eq!(conflict.status, KnowledgeStatus::Conflict);
    let current = s
        .institutional_snapshot(
            query(&p, None),
            Some(old.assertion_id.unwrap().as_uuid()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(current.versions[0].id, old.id);
    assert_eq!(
        s.institutional_history(p.id, old.assertion_id.unwrap().as_uuid())
            .await
            .unwrap()[1]
            .status,
        KnowledgeStatus::Conflict
    );
}

#[tokio::test]
async fn relationship_filter_applies_before_limit_and_hidden_endpoint_withholds_edge() {
    let (_app, s, db, p, e) = fixture().await;
    let a = promote(&s, &p, e, entity("A", EntityKind::Customer, "A", &[]))
        .await
        .entity_id
        .unwrap();
    let b = promote(&s, &p, e, entity("B", EntityKind::Customer, "B", &[]))
        .await
        .entity_id
        .unwrap();
    let c = promote(&s, &p, e, entity("C", EntityKind::Customer, "C", &[])).await;
    promote(
        &s,
        &p,
        e,
        GraphProposal::Relationship {
            source_entity_id: a,
            target_entity_id: b,
            relationship_type: "PARTNER".into(),
        },
    )
    .await;
    let selected = promote(
        &s,
        &p,
        e,
        GraphProposal::Relationship {
            source_entity_id: b,
            target_entity_id: c.entity_id.unwrap(),
            relationship_type: "PARTNER".into(),
        },
    )
    .await;
    assert_eq!(
        s.entity_relationships(p.id, c.entity_id.unwrap(), 1, None)
            .await
            .unwrap()[0]
            .id,
        selected.id
    );
    sqlx::query("UPDATE institutional_versions SET status='REVOKED' WHERE id=$1")
        .bind(c.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(
        s.entity_relationships(p.id, c.entity_id.unwrap(), 1, None)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn duplicate_confirmation_preserves_human_trust_and_audits_candidate_result() {
    let (_app, s, db, p, e) = fixture().await;
    let proposal = entity("Atlas", EntityKind::LegalEntity, "A", &[]);
    let old = promote(&s, &p, e, proposal.clone()).await;
    let c = s
        .propose_institutional(request(&p, e, proposal, ProposerKind::Agent))
        .await
        .unwrap();
    let result = s.promote_institutional(c.id).await.unwrap();
    assert_eq!(result.id, old.id);
    assert_eq!(result.trust_level, TrustLevel::HumanExplicit);
    assert_eq!(
        s.institutional_history(p.id, old.entity_id.unwrap().as_uuid())
            .await
            .unwrap()
            .len(),
        1
    );
    let (status, result_id): (String, uuid::Uuid) =
        sqlx::query_as("SELECT status,result_version_id FROM institutional_candidates WHERE id=$1")
            .bind(c.id.as_uuid())
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(status, "PROMOTED");
    assert_eq!(result_id, old.id.as_uuid());
    assert!(
        sqlx::query("UPDATE institutional_candidates SET status='PENDING' WHERE id=$1")
            .bind(c.id.as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn lower_classification_cannot_change_restricted_identity_or_its_trust() {
    let (_app, s, db, p, e) = fixture().await;
    let elevated_app = Arc::new(
        AppServices::new(
            db.pool.clone(),
            Arc::new(DeterministicProvider::new()),
            _app.tenant_id(),
        )
        .with_max_classification(SecurityClassification::Secret),
    );
    let elevated_owner = elevated_app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let elevated = elevated_app.for_principal(elevated_owner).unwrap();
    let mut req = request(
        &p,
        e,
        entity("Secret Atlas", EntityKind::LegalEntity, "A", &[]),
        ProposerKind::Human,
    );
    req.security_classification = Some(SecurityClassification::Secret);
    let c = elevated.propose_institutional(req).await.unwrap();
    let old = elevated.promote_institutional(c.id).await.unwrap();
    let c = s
        .propose_institutional(request(
            &p,
            e,
            entity("Atlas", EntityKind::LegalEntity, "A", &[]),
            ProposerKind::Human,
        ))
        .await
        .unwrap();
    assert!(matches!(
        s.promote_institutional(c.id).await,
        Err(ServiceError::Conflict(_))
    ));
    assert!(
        s.resolve_entity(p.id, "A", Some("register"), None)
            .await
            .unwrap()
            .matches
            .is_empty()
    );
    assert_eq!(
        elevated
            .resolve_entity(p.id, "A", Some("register"), None)
            .await
            .unwrap()
            .matches[0]
            .version
            .id,
        old.id
    );
}

#[tokio::test]
async fn assertion_history_withholds_hidden_endpoints_but_keeps_database_history() {
    let (_app, s, db, p, e) = fixture().await;
    let a = promote(&s, &p, e, entity("A", EntityKind::Customer, "A", &[])).await;
    let b = promote(&s, &p, e, entity("B", EntityKind::Investor, "B", &[])).await;
    let edge = promote(
        &s,
        &p,
        e,
        GraphProposal::Relationship {
            source_entity_id: a.entity_id.unwrap(),
            target_entity_id: b.entity_id.unwrap(),
            relationship_type: "SENT_TO".into(),
        },
    )
    .await;
    let claim = promote(
        &s,
        &p,
        e,
        GraphProposal::Claim {
            subject_entity_id: a.entity_id.unwrap(),
            predicate: "commitment".into(),
            object: serde_json::json!("fictional update"),
        },
    )
    .await;
    for status in ["STALE", "QUARANTINED", "REVOKED"] {
        sqlx::query("UPDATE institutional_versions SET status=$2 WHERE id=$1")
            .bind(a.id.as_uuid())
            .bind(status)
            .execute(&db.pool)
            .await
            .unwrap();
        assert!(
            s.institutional_history(p.id, edge.assertion_id.unwrap().as_uuid())
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            s.institutional_history(p.id, claim.assertion_id.unwrap().as_uuid())
                .await
                .unwrap()
                .is_empty()
        );
        sqlx::query("UPDATE institutional_versions SET status='ACTIVE' WHERE id=$1")
            .bind(a.id.as_uuid())
            .execute(&db.pool)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE institutional_versions SET status='REVOKED' WHERE id=$1")
        .bind(b.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    assert!(
        s.institutional_history(p.id, edge.assertion_id.unwrap().as_uuid())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        s.institutional_history(p.id, claim.assertion_id.unwrap().as_uuid())
            .await
            .unwrap()
            .len(),
        1
    );
    let (count,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM institutional_versions WHERE id=ANY($1)")
            .bind(vec![edge.id.as_uuid(), claim.id.as_uuid()])
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn inactive_restricted_identity_retains_identifiers_and_cannot_be_recreated_below_ceiling() {
    let (_app, s, db, p, e) = fixture().await;
    let elevated_app = Arc::new(
        AppServices::new(
            db.pool.clone(),
            Arc::new(DeterministicProvider::new()),
            _app.tenant_id(),
        )
        .with_max_classification(SecurityClassification::Secret),
    );
    let elevated_owner = elevated_app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let elevated = elevated_app.for_principal(elevated_owner).unwrap();
    let mut req = request(
        &p,
        e,
        entity("Secret Atlas", EntityKind::LegalEntity, "A", &[]),
        ProposerKind::Human,
    );
    req.security_classification = Some(SecurityClassification::Secret);
    let c = elevated.propose_institutional(req).await.unwrap();
    let v = elevated.promote_institutional(c.id).await.unwrap();
    for status in ["STALE", "REVOKED", "QUARANTINED"] {
        sqlx::query("UPDATE institutional_versions SET status=$2 WHERE id=$1")
            .bind(v.id.as_uuid())
            .bind(status)
            .execute(&db.pool)
            .await
            .unwrap();
        let c = s
            .propose_institutional(request(
                &p,
                e,
                entity("Replacement Atlas", EntityKind::LegalEntity, "A", &[]),
                ProposerKind::Human,
            ))
            .await
            .unwrap();
        assert!(matches!(
            s.promote_institutional(c.id).await,
            Err(ServiceError::Conflict(_))
        ));
        let(count,):(i64,)=sqlx::query_as("SELECT count(*) FROM institutional_records WHERE tenant_id=$1 AND project_id=$2 AND record_type='ENTITY'")
            .bind(p.tenant_id.as_uuid()).bind(p.id.as_uuid()).fetch_one(&db.pool).await.unwrap();
        assert_eq!(count, 1);
    }
    assert!(
        sqlx::query("DELETE FROM institutional_identifiers WHERE entity_id=$1")
            .bind(v.entity_id.unwrap().as_uuid())
            .execute(&db.pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE institutional_identifiers SET external_id='replacement' WHERE entity_id=$1"
        )
        .bind(v.entity_id.unwrap().as_uuid())
        .execute(&db.pool)
        .await
        .is_err()
    );
}

#[tokio::test]
async fn stale_identity_confirmation_does_not_reactivate_or_downgrade_human_state() {
    let (_app, s, db, p, e) = fixture().await;
    let original = entity("Atlas", EntityKind::LegalEntity, "A", &[]);
    let old = promote(&s, &p, e, original.clone()).await;
    sqlx::query("UPDATE institutional_versions SET status='STALE' WHERE id=$1")
        .bind(old.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    let c = s
        .propose_institutional(request(&p, e, original, ProposerKind::Agent))
        .await
        .unwrap();
    let confirmation = s.promote_institutional(c.id).await.unwrap();
    assert_eq!(confirmation.id, old.id);
    assert_eq!(confirmation.status, KnowledgeStatus::Stale);
    assert_eq!(confirmation.trust_level, TrustLevel::HumanExplicit);
    let c = s
        .propose_institutional(request(
            &p,
            e,
            entity("Changed Atlas", EntityKind::LegalEntity, "A", &[]),
            ProposerKind::Agent,
        ))
        .await
        .unwrap();
    let conflicting = s.promote_institutional(c.id).await.unwrap();
    assert_eq!(conflicting.status, KnowledgeStatus::Conflict);
    assert!(
        s.institutional_snapshot(query(&p, None), None, None)
            .await
            .unwrap()
            .versions
            .is_empty()
    );
    let history = s
        .institutional_history(p.id, old.entity_id.unwrap().as_uuid())
        .await
        .unwrap();
    assert_eq!(history[0].status, KnowledgeStatus::Stale);
    assert_eq!(history[0].trust_level, TrustLevel::HumanExplicit);
}

#[tokio::test]
async fn supersession_shortens_future_end_and_preserves_an_already_closed_stale_interval() {
    let (_app, s, db, p, e) = fixture().await;
    let old = promote(
        &s,
        &p,
        e,
        entity("Atlas", EntityKind::LegalEntity, "A", &[]),
    )
    .await;
    sqlx::query("UPDATE institutional_versions SET valid_until=$2 WHERE id=$1")
        .bind(old.id.as_uuid())
        .bind(old.valid_from + Duration::days(1))
        .execute(&db.pool)
        .await
        .unwrap();
    let new = promote(
        &s,
        &p,
        e,
        entity("Atlas Updated", EntityKind::LegalEntity, "A", &[]),
    )
    .await;
    let history = s
        .institutional_history(p.id, old.entity_id.unwrap().as_uuid())
        .await
        .unwrap();
    assert_eq!(history[0].valid_until, Some(new.valid_from));
    let snapshot = s
        .institutional_snapshot(
            query(&p, Some(new.valid_from)),
            Some(old.entity_id.unwrap().as_uuid()),
            None,
        )
        .await
        .unwrap();
    assert_eq!(snapshot.versions.len(), 1);
    assert_eq!(snapshot.versions[0].id, new.id);
    sqlx::query(
        "UPDATE institutional_versions SET status='STALE',valid_until=valid_from WHERE id=$1",
    )
    .bind(new.id.as_uuid())
    .execute(&db.pool)
    .await
    .unwrap();
    let restored = promote(
        &s,
        &p,
        e,
        entity("Atlas Reviewed", EntityKind::LegalEntity, "A", &[]),
    )
    .await;
    let history = s
        .institutional_history(p.id, old.entity_id.unwrap().as_uuid())
        .await
        .unwrap();
    assert_eq!(history[1].valid_until, Some(new.valid_from));
    assert_eq!(restored.supersedes_version_id, Some(new.id));
}

#[tokio::test]
async fn claim_and_relationship_lifecycle_cannot_be_bypassed_by_reproposing_the_same_identity() {
    let (_app, s, db, p, e) = fixture().await;
    let a = promote(&s, &p, e, entity("A", EntityKind::Customer, "A", &[]))
        .await
        .entity_id
        .unwrap();
    let b = promote(&s, &p, e, entity("B", EntityKind::Investor, "B", &[]))
        .await
        .entity_id
        .unwrap();
    for proposal in [
        GraphProposal::Claim {
            subject_entity_id: a,
            predicate: "commitment".into(),
            object: serde_json::json!("fictional update"),
        },
        GraphProposal::Relationship {
            source_entity_id: a,
            target_entity_id: b,
            relationship_type: "SENT_TO".into(),
        },
    ] {
        let old = promote(&s, &p, e, proposal.clone()).await;
        for status in ["REVOKED", "QUARANTINED"] {
            sqlx::query("UPDATE institutional_versions SET status=$2 WHERE id=$1")
                .bind(old.id.as_uuid())
                .bind(status)
                .execute(&db.pool)
                .await
                .unwrap();
            let c = s
                .propose_institutional(request(&p, e, proposal.clone(), ProposerKind::Agent))
                .await
                .unwrap();
            assert!(matches!(
                s.promote_institutional(c.id).await,
                Err(ServiceError::Conflict(_))
            ));
        }
    }
    let elevated_app = Arc::new(
        AppServices::new(
            db.pool.clone(),
            Arc::new(DeterministicProvider::new()),
            _app.tenant_id(),
        )
        .with_max_classification(SecurityClassification::Secret),
    );
    let elevated_owner = elevated_app
        .bootstrap_personal_policy_actors(None, None)
        .await
        .unwrap()
        .owner;
    let elevated = elevated_app.for_principal(elevated_owner).unwrap();
    let proposal = GraphProposal::Claim {
        subject_entity_id: a,
        predicate: "restricted_commitment".into(),
        object: serde_json::json!("fictional confidential promise"),
    };
    let mut req = request(&p, e, proposal.clone(), ProposerKind::Human);
    req.security_classification = Some(SecurityClassification::Restricted);
    let c = elevated.propose_institutional(req).await.unwrap();
    let old = elevated.promote_institutional(c.id).await.unwrap();
    sqlx::query("UPDATE institutional_versions SET status='STALE' WHERE id=$1")
        .bind(old.id.as_uuid())
        .execute(&db.pool)
        .await
        .unwrap();
    let c = s
        .propose_institutional(request(&p, e, proposal, ProposerKind::Agent))
        .await
        .unwrap();
    assert!(matches!(
        s.promote_institutional(c.id).await,
        Err(ServiceError::Conflict(_))
    ));
}
