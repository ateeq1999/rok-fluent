//! Postgres integration tests for [`rok_orm::PgModel`].
//!
//! Requires a live Postgres instance.  Set `DATABASE_URL` to a connection
//! string (e.g. `postgres://postgres:password@localhost/rok_test`) and run:
//!
//! ```bash
//! cargo test -p rok-orm --features postgres --test pg_integration
//! ```
//!
//! Tests are skipped automatically when `DATABASE_URL` is not set.

#[cfg(feature = "postgres")]
mod tests {
    use rok_orm::{Model, PgModel, SqlValue};

    // ── test model ────────────────────────────────────────────────────────────

    #[derive(Debug, Model, sqlx::FromRow, PartialEq)]
    pub struct TestUser {
        pub id: i64,
        pub name: String,
        pub email: String,
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    async fn connect() -> Option<sqlx::PgPool> {
        let url = std::env::var("DATABASE_URL").ok()?;
        sqlx::PgPool::connect(&url).await.ok()
    }

    async fn setup(pool: &sqlx::PgPool) {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS test_users (
                id    BIGSERIAL PRIMARY KEY,
                name  TEXT NOT NULL,
                email TEXT NOT NULL
            )",
        )
        .execute(pool)
        .await
        .expect("create test_users");

        // Clear any leftover rows from a previous run.
        sqlx::query("DELETE FROM test_users")
            .execute(pool)
            .await
            .expect("truncate test_users");
    }

    async fn teardown(pool: &sqlx::PgPool) {
        sqlx::query("DROP TABLE IF EXISTS test_users")
            .execute(pool)
            .await
            .expect("drop test_users");
    }

    // ── tests ─────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn create_and_all() {
        let Some(pool) = connect().await else {
            eprintln!("skipped: DATABASE_URL not set");
            return;
        };
        setup(&pool).await;

        TestUser::create(
            &pool,
            &[
                ("name", SqlValue::Text("Alice".into())),
                ("email", SqlValue::Text("alice@example.com".into())),
            ],
        )
        .await
        .expect("insert Alice");

        TestUser::create(
            &pool,
            &[
                ("name", SqlValue::Text("Bob".into())),
                ("email", SqlValue::Text("bob@example.com".into())),
            ],
        )
        .await
        .expect("insert Bob");

        let users = TestUser::all(&pool).await.expect("all");
        assert_eq!(users.len(), 2);

        teardown(&pool).await;
    }

    #[tokio::test]
    async fn find_by_pk() {
        let Some(pool) = connect().await else {
            eprintln!("skipped: DATABASE_URL not set");
            return;
        };
        setup(&pool).await;

        // Insert and get the generated id back.
        let row: (i64,) =
            sqlx::query_as("INSERT INTO test_users (name, email) VALUES ($1, $2) RETURNING id")
                .bind("Carol")
                .bind("carol@example.com")
                .fetch_one(&pool)
                .await
                .expect("insert Carol");

        let found = TestUser::find_by_pk(&pool, row.0)
            .await
            .expect("find_by_pk");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "Carol");

        let missing = TestUser::find_by_pk(&pool, 999_999i64)
            .await
            .expect("find_by_pk missing");
        assert!(missing.is_none());

        teardown(&pool).await;
    }

    #[tokio::test]
    async fn delete_by_pk() {
        let Some(pool) = connect().await else {
            eprintln!("skipped: DATABASE_URL not set");
            return;
        };
        setup(&pool).await;

        let row: (i64,) =
            sqlx::query_as("INSERT INTO test_users (name, email) VALUES ($1, $2) RETURNING id")
                .bind("Dave")
                .bind("dave@example.com")
                .fetch_one(&pool)
                .await
                .expect("insert Dave");

        let affected = TestUser::delete_by_pk(&pool, row.0)
            .await
            .expect("delete_by_pk");
        assert_eq!(affected, 1);

        let after = TestUser::all(&pool).await.expect("all after delete");
        assert!(after.is_empty());

        teardown(&pool).await;
    }

    #[tokio::test]
    async fn count_via_executor() {
        let Some(pool) = connect().await else {
            eprintln!("skipped: DATABASE_URL not set");
            return;
        };
        setup(&pool).await;

        sqlx::query("INSERT INTO test_users (name, email) VALUES ($1, $2)")
            .bind("Eve")
            .bind("eve@example.com")
            .execute(&pool)
            .await
            .expect("insert Eve");

        let n = rok_orm::executor::count(&pool, TestUser::query())
            .await
            .expect("count");
        assert_eq!(n, 1);

        teardown(&pool).await;
    }
}
