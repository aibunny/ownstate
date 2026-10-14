//! Repository identity and scope-handle security acceptance against PostgreSQL.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::Utc;
use ownstate_domain::*;
use ownstate_storage::test_support::fresh_db;
use ownstate_storage::{policy, projects, repositories, scope_handles};

#[tokio::test]
async fn concurrent_repository_upserts_return_one_identity() {
    let db = fresh_db().await.unwrap();
    let tenant = TenantId::generate();
    let personal = policy::bootstrap_personal_policy(&db.pool, tenant)
        .await
        .unwrap();
    let locator = normalize_remote("git@github.com:Acme/Project.git").unwrap();

    let mut tasks = Vec::new();
    for _ in 0..20 {
        let pool = db.pool.clone();
        let locator = locator.clone();
        tasks.push(tokio::spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            let repository =
                repositories::upsert_repository(&mut tx, tenant, personal.workspace_id, &locator)
                    .await
                    .unwrap();
            tx.commit().await.unwrap();
            repository.id
        }));
    }

    let mut ids = Vec::new();
    for task in tasks {
        ids.push(task.await.unwrap());
    }
    assert!(ids.iter().all(|id| *id == ids[0]));
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM repositories WHERE tenant_id = $1 AND internal_key = $2",
    )
    .bind(tenant.as_uuid())
    .bind(&locator.internal_key)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn scope_handle_is_random_and_bound_to_its_principal() {
    let db = fresh_db().await.unwrap();
    let tenant = TenantId::generate();
    let personal = policy::bootstrap_personal_policy(&db.pool, tenant)
        .await
        .unwrap();
    let project = Project {
        id: ProjectId::generate(),
        tenant_id: tenant,
        ownership_domain: OwnershipDomain::Personal,
        name: "scope-handle-test".into(),
        description: None,
        metadata: serde_json::json!({}),
        created_at: Utc::now(),
    };
    let locator = normalize_remote("https://github.com/acme/scope-handle-test.git").unwrap();
    let mut tx = db.pool.begin().await.unwrap();
    projects::insert(&mut *tx, &project).await.unwrap();
    let repository =
        repositories::upsert_repository(&mut tx, tenant, personal.workspace_id, &locator)
            .await
            .unwrap();
    policy::bind_project(
        &mut *tx,
        tenant,
        project.id,
        personal.workspace_id,
        Some(repository.id.into()),
        OwnershipDomain::Personal,
    )
    .await
    .unwrap();
    let handle = scope_handles::insert_handle(
        &mut tx,
        tenant,
        personal.principal.id(),
        OwnershipDomain::Personal,
        Some(project.id),
        Some(repository.id.into()),
        Some(personal.workspace_id),
        ScopeKind::Repository,
        ResolutionState::ExactStable,
        Some("scope-handle-test"),
        None,
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        handle.id.as_uuid().get_version(),
        Some(uuid::Version::Random)
    );
    assert!(
        scope_handles::find_handle(
            &db.pool,
            tenant,
            personal.principal.id(),
            handle.id,
            Utc::now(),
        )
        .await
        .unwrap()
        .is_some()
    );
    assert!(
        scope_handles::find_handle(
            &db.pool,
            tenant,
            PrincipalId::generate(),
            handle.id,
            Utc::now(),
        )
        .await
        .unwrap()
        .is_none()
    );
}
