#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ownstate_domain::{TenantId, content_hash};
use ownstate_embeddings::DeterministicProvider;
use ownstate_services::AppServices;
use ownstate_services::conversations::CaptureConversation;
use ownstate_services::projects::CreateProject;
use ownstate_storage::test_support::fresh_db;
use sha2::{Digest, Sha256};

#[tokio::test]
async fn account_connections_are_isolated_and_revocable() {
    let db = fresh_db().await.unwrap();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
    ));
    let first = app
        .signup("first@example.com", "long-password-for-first")
        .await
        .unwrap();
    let second = app
        .signup("second@example.com", "long-password-for-second")
        .await
        .unwrap();
    app.verify_signup(&first.verification_token).await.unwrap();
    app.verify_signup(&second.verification_token).await.unwrap();
    let first = app
        .login("first@example.com", "long-password-for-first")
        .await
        .unwrap();
    let second = app
        .login("second@example.com", "long-password-for-second")
        .await
        .unwrap();
    let first_space = app
        .account_workspaces(first.account_id)
        .await
        .unwrap()
        .remove(0)
        .workspace_id;
    let second_space = app
        .account_workspaces(second.account_id)
        .await
        .unwrap()
        .remove(0)
        .workspace_id;
    assert_ne!(first_space, second_space);
    assert!(
        app.create_mcp_connection(first.account_id, second_space, "invalid")
            .await
            .is_err()
    );
    let first_connection = app
        .create_mcp_connection(first.account_id, first_space, "Codex")
        .await
        .unwrap();
    let second_connection = app
        .create_mcp_connection(second.account_id, second_space, "Claude")
        .await
        .unwrap();
    let first_digest = content_hash(first_connection.token.as_bytes());
    let second_digest = content_hash(second_connection.token.as_bytes());
    assert!(app.is_active_mcp_connection(&first_digest).await.unwrap());
    let first_principal = app.authenticate_digest(&first_digest).await.unwrap();
    let second_principal = app.authenticate_digest(&second_digest).await.unwrap();
    assert_ne!(first_principal.tenant_id(), second_principal.tenant_id());
    let first_services = app.for_verified_principal(first_principal.clone()).unwrap();
    let sample = first_services
        .create_project(CreateProject {
            name: "Local context test".into(),
            description: None,
            ownership_domain: None,
            metadata: None,
        })
        .await
        .unwrap();
    first_services
        .bootstrap_project(sample.id, 5, None)
        .await
        .unwrap();
    let first_profile = app
        .for_verified_principal(first_principal.clone())
        .unwrap()
        .connection_profile()
        .await
        .unwrap();
    let second_profile = app
        .for_verified_principal(second_principal.clone())
        .unwrap()
        .connection_profile()
        .await
        .unwrap();
    assert_ne!(first_profile.id, second_profile.id);
    assert_eq!(first_profile.email.as_deref(), Some("first@example.com"));
    assert_eq!(second_profile.email.as_deref(), Some("second@example.com"));
    let mut threads = Vec::new();
    for principal in [first_principal.clone(), second_principal.clone()] {
        let scoped = app.for_verified_principal(principal).unwrap();
        let event = || CaptureConversation {
            source: "codex".into(),
            source_conversation_id: "same-host-id".into(),
            title: Some("Research chat".into()),
            capture_mode: "MCP_ONLY".into(),
            source_event_id: Some("message-1".into()),
            event_type: "USER_MESSAGE".into(),
            actor_type: "USER".into(),
            content: Some("Research question".into()),
            occurred_at: None,
            security_classification: None,
        };
        let stored = scoped.capture_conversation(event()).await.unwrap();
        assert!(!stored.duplicate);
        assert_eq!(
            scoped.capture_conversation(event()).await.unwrap().event_id,
            stored.event_id
        );
        assert_eq!(
            scoped
                .conversation_events(stored.thread_id)
                .await
                .unwrap()
                .len(),
            1
        );
        threads.push(stored.thread_id);
    }
    assert_ne!(threads[0], threads[1]);
    assert_eq!(
        app.account_conversations(first.account_id)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        app.account_conversation_events(first.account_id, threads[0])
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(
        app.account_conversation_events(first.account_id, threads[1])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        app.for_verified_principal(first_principal)
            .unwrap()
            .conversation_events(threads[1])
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        app.for_verified_principal(second_principal)
            .unwrap()
            .conversation_events(threads[0])
            .await
            .unwrap()
            .is_empty()
    );
    let thread_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM conversation_threads WHERE source_conversation_id='same-host-id'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(thread_count, 2);
    assert!(
        app.revoke_account_connection(first.account_id, first_connection.connection_id)
            .await
            .unwrap()
    );
    assert!(!app.is_active_mcp_connection(&first_digest).await.unwrap());
    assert!(app.is_active_mcp_connection(&second_digest).await.unwrap());
}

#[tokio::test]
async fn oauth_code_refresh_and_revoke_keep_one_account_owner() {
    let db = fresh_db().await.unwrap();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
    ));
    let pending = app
        .signup("oauth@example.com", "long-password-for-oauth")
        .await
        .unwrap();
    app.verify_signup(&pending.verification_token)
        .await
        .unwrap();
    let account = app
        .login("oauth@example.com", "long-password-for-oauth")
        .await
        .unwrap()
        .account_id;
    let workspace = app
        .account_workspaces(account)
        .await
        .unwrap()
        .remove(0)
        .workspace_id;
    let redirect = "https://client.example/callback";
    let client = app
        .register_oauth_client("Claude", vec![redirect.into()])
        .await
        .unwrap();
    let verifier = "correct-horse-battery-staple-verifier-0123456789";
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let code = app
        .authorize_oauth(account, workspace, &client.client_id, redirect, &challenge)
        .await
        .unwrap();
    assert!(
        app.exchange_oauth_code(
            &code,
            &client.client_id,
            redirect,
            "wrong-verifier-value-012345678901234567890"
        )
        .await
        .is_err()
    );
    let pair = app
        .exchange_oauth_code(&code, &client.client_id, redirect, verifier)
        .await
        .unwrap();
    assert!(
        app.exchange_oauth_code(&code, &client.client_id, redirect, verifier)
            .await
            .is_err()
    );
    let digest = content_hash(pair.access_token.as_bytes());
    assert!(app.is_active_mcp_connection(&digest).await.unwrap());
    let refresh = app
        .refresh_oauth(&pair.refresh_token, &client.client_id)
        .await
        .unwrap();
    assert!(
        app.refresh_oauth(&pair.refresh_token, &client.client_id)
            .await
            .is_err()
    );
    let refreshed_digest = content_hash(refresh.access_token.as_bytes());
    assert!(
        app.is_active_mcp_connection(&refreshed_digest)
            .await
            .unwrap()
    );
    let first_principal = app.authenticate_digest(&digest).await.unwrap();
    let refreshed_principal = app.authenticate_digest(&refreshed_digest).await.unwrap();
    assert_eq!(
        app.for_verified_principal(first_principal)
            .unwrap()
            .connection_profile()
            .await
            .unwrap()
            .id,
        app.for_verified_principal(refreshed_principal)
            .unwrap()
            .connection_profile()
            .await
            .unwrap()
            .id
    );
    let connection = app.account_connections(account).await.unwrap().remove(0);
    assert!(connection.oauth_grant_id.is_some());
    assert!(
        app.revoke_account_connection(account, connection.id)
            .await
            .unwrap()
    );
    assert!(!app.is_active_mcp_connection(&digest).await.unwrap());
    assert!(
        !app.is_active_mcp_connection(&refreshed_digest)
            .await
            .unwrap()
    );
    assert!(
        app.refresh_oauth(&refresh.refresh_token, &client.client_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn organization_membership_controls_existing_connection() {
    let db = fresh_db().await.unwrap();
    let app = Arc::new(AppServices::new(
        db.pool.clone(),
        Arc::new(DeterministicProvider::new()),
        TenantId::from_uuid(uuid::Uuid::from_u128(1)),
    ));
    let owner_pending = app
        .signup("owner@example.com", "long-password-for-owner")
        .await
        .unwrap();
    let member_pending = app
        .signup("member@example.com", "long-password-for-member")
        .await
        .unwrap();
    app.verify_signup(&owner_pending.verification_token)
        .await
        .unwrap();
    app.verify_signup(&member_pending.verification_token)
        .await
        .unwrap();
    let owner = app
        .login("owner@example.com", "long-password-for-owner")
        .await
        .unwrap()
        .account_id;
    let member = app
        .login("member@example.com", "long-password-for-member")
        .await
        .unwrap()
        .account_id;
    app.create_organization(owner, "Research team")
        .await
        .unwrap();
    let workspace = app
        .account_workspaces(owner)
        .await
        .unwrap()
        .into_iter()
        .find(|space| space.kind == "ORGANIZATION")
        .unwrap()
        .workspace_id;
    let invitation = app
        .invite_to_organization(owner, workspace, "member@example.com")
        .await
        .unwrap();
    app.accept_organization_invitation(member, &invitation.token)
        .await
        .unwrap();
    let connection = app
        .create_mcp_connection(member, workspace, "OpenCode")
        .await
        .unwrap();
    let digest = content_hash(connection.token.as_bytes());
    assert!(app.is_active_mcp_connection(&digest).await.unwrap());
    let tenant: uuid::Uuid = sqlx::query_scalar("SELECT tenant_id FROM workspaces WHERE id=$1")
        .bind(workspace)
        .fetch_one(&db.pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM organization_memberships WHERE tenant_id=$1 AND user_id=(SELECT user_id FROM account_tenant_memberships WHERE account_id=$2 AND workspace_id=$3)")
        .bind(tenant).bind(member).bind(workspace).execute(&db.pool).await.unwrap();
    assert!(!app.is_active_mcp_connection(&digest).await.unwrap());
    assert!(
        app.account_workspaces(member)
            .await
            .unwrap()
            .iter()
            .all(|s| s.workspace_id != workspace)
    );
}
