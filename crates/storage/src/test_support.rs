//! Disposable-database harness for integration tests.
//!
//! Each call to [`fresh_db`] creates `ownstate_test_<unix>_<suffix>`, runs the
//! workspace migrations and returns a pool connected to it. Databases from
//! runs older than one hour are dropped opportunistically so the local server
//! stays tidy without racing tests running in parallel right now.

use sqlx::postgres::PgPoolOptions;
use sqlx::{AssertSqlSafe, PgPool};
use uuid::Uuid;

use crate::{Result, run_migrations};

/// Admin/base connection string. Override with OWNSTATE_TEST_DATABASE_URL.
fn base_url() -> String {
    std::env::var("OWNSTATE_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://ownstate:ownstate_dev@127.0.0.1:5432/ownstate".to_string())
}

fn replace_database(url: &str, db: &str) -> String {
    // postgres://user:pass@host:port/dbname — swap the final path segment.
    match url.rsplit_once('/') {
        Some((prefix, _)) => format!("{prefix}/{db}"),
        None => url.to_string(),
    }
}

pub struct TestDb {
    pub pool: PgPool,
    pub name: String,
}

pub async fn fresh_db() -> Result<TestDb> {
    let base = base_url();
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&base)
        .await?;

    cleanup_stale(&admin).await;

    let now = chrono::Utc::now().timestamp();
    // Random v4 suffix: v7's leading hex is a millisecond timestamp, which
    // collides for tests that start in the same instant.
    let suffix = Uuid::new_v4().simple().to_string();
    let name = format!("ownstate_test_{now}_{}", &suffix[..16]);

    // Identifiers cannot be bound as parameters; the name is generated above
    // from a timestamp and a UUID — never from external input — so this is a
    // justified AssertSqlSafe (test-support only, never compiled into
    // production binaries without the `test-support` feature).
    sqlx::query(AssertSqlSafe(format!("CREATE DATABASE {name}")))
        .execute(&admin)
        .await?;
    admin.close().await;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&replace_database(&base, &name))
        .await?;
    run_migrations(&pool).await?;

    Ok(TestDb { pool, name })
}

/// Drop leftover test databases from previous runs (older than one hour).
async fn cleanup_stale(admin: &PgPool) {
    let cutoff = chrono::Utc::now().timestamp() - 3600;
    let stale: Vec<(String,)> =
        sqlx::query_as("SELECT datname FROM pg_database WHERE datname LIKE 'ownstate_test_%'")
            .fetch_all(admin)
            .await
            .unwrap_or_default();

    for (db,) in stale {
        let age_ok = db
            .strip_prefix("ownstate_test_")
            .and_then(|rest| rest.split('_').next())
            .and_then(|ts| ts.parse::<i64>().ok())
            .is_some_and(|ts| ts < cutoff);
        if age_ok {
            // Same justification as above: internally generated identifier.
            let _ = sqlx::query(AssertSqlSafe(format!(
                "DROP DATABASE IF EXISTS {db} WITH (FORCE)"
            )))
            .execute(admin)
            .await;
        }
    }
}
