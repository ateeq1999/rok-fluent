//! Eager loading — batch-load `has_many` relations in a single `IN` query,
//! avoiding N+1, via both the low-level helpers and [`CrudService::all_with`].
//!
//! Requires a live PostgreSQL database:
//!
//! ```sh
//! DATABASE_URL=postgres://user:pass@localhost/rok_fluent_db \
//!   cargo run --example 03_relations_eager_loading --features postgres,active
//! ```
//!
//! # Deviation from the plan
//!
//! The plan suggested `features = "sqlite,active"`. Eager loading
//! (`src/orm/eager.rs`: `with_has_many`, `group_has_many`, `EagerLoadable`,
//! `EagerModelQuery`) and [`CrudService`] (`src/services/crud.rs`) are both
//! gated `#[cfg(all(feature = "active", feature = "postgres"))]` — they only
//! exist when PostgreSQL is enabled, since they're built on `PgModel` and
//! `ModelQuery<M>`, which are PostgreSQL-only today. So this example
//! requires a live PostgreSQL database; it cannot run against SQLite.

use rok_fluent::core::model::Model;
use rok_fluent::orm::eager::{group_has_many, with_has_many};
use rok_fluent::orm::postgres::model::PgModel;
use rok_fluent::services::CrudService;
use rok_fluent::ModelDerive;

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, ModelDerive)]
#[model(table = "authors")]
pub struct Author {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow, ModelDerive)]
#[model(table = "articles")]
pub struct Article {
    pub id: i64,
    pub author_id: i64,
    pub title: String,
}

// `EagerLoadable` must be implemented per-model to use the string-keyed
// `CrudService::all_with()` / `.paginate_with()` API; this crate ships no
// default implementation, so a minimal one is provided here for `Author`.
impl rok_fluent::orm::eager::EagerLoadable for Author {
    async fn load_eager(relation: &str, parents: Vec<Self>) -> Result<Vec<Self>, sqlx::Error> {
        match relation {
            "articles" => {
                // Eager loading only groups here; the loaded data itself is
                // discarded since `Author` has no field to hold it. A real
                // model would carry `#[serde(skip)] pub articles: Vec<Article>`.
                let _ = with_has_many(
                    parents.clone(),
                    "author_id",
                    |a: &Article| a.author_id,
                    |au: &Author| au.id,
                )
                .await?;
                Ok(parents)
            }
            _ => Ok(parents),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/rok_fluent_db".into());
    let pool = sqlx::PgPool::connect(&database_url).await?;

    sqlx::query("DROP TABLE IF EXISTS articles, authors")
        .execute(&pool)
        .await?;
    sqlx::query("CREATE TABLE authors (id BIGSERIAL PRIMARY KEY, name TEXT NOT NULL)")
        .execute(&pool)
        .await?;
    sqlx::query(
        "CREATE TABLE articles (
            id        BIGSERIAL PRIMARY KEY,
            author_id BIGINT NOT NULL REFERENCES authors(id),
            title     TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    let ada: Author = Author::create_returning(&pool, &[("name", "Ada Lovelace".into())]).await?;
    let alan: Author = Author::create_returning(&pool, &[("name", "Alan Turing".into())]).await?;

    for (author_id, title) in [
        (ada.id, "Notes on the Analytical Engine"),
        (ada.id, "On the Bernoulli Numbers"),
        (alan.id, "On Computable Numbers"),
    ] {
        Article::create(
            &pool,
            &[("author_id", author_id.into()), ("title", title.into())],
        )
        .await?;
    }

    // ── Low-level helper: with_has_many + group_has_many ───────────────────
    let authors: Vec<Author> = Author::all(&pool).await?;
    let articles: Vec<Article> = Article::all(&pool).await?;
    let grouped = group_has_many(
        authors,
        articles,
        |a: &Article| a.author_id,
        |au: &Author| au.id,
    );
    println!("group_has_many:");
    for entry in &grouped {
        println!(
            "  {} — {} article(s)",
            entry.model.name,
            entry.related.len()
        );
    }

    // `with_has_many` fetches the children itself, batched in one `IN (...)` query.
    let authors_again: Vec<Author> = Author::all(&pool).await?;
    let with_many = with_has_many(
        authors_again,
        "author_id",
        |a: &Article| a.author_id,
        |au: &Author| au.id,
    )
    .await?;
    println!("\nwith_has_many:");
    for entry in &with_many {
        for article in &entry.related {
            println!("  {}: {}", entry.model.name, article.title);
        }
    }

    // ── CrudService::all_with / paginate_with ───────────────────────────────
    let crud = CrudService::<Author>::new(pool.clone());
    let all_with_articles = crud.all_with(&["articles"]).await?;
    println!(
        "\nCrudService::all_with(\"articles\"): {} authors",
        all_with_articles.len()
    );

    let page = crud.paginate_with(1, 10, &["articles"]).await?;
    println!(
        "CrudService::paginate_with: page {} of {}, {} total",
        page.meta.current_page, page.meta.last_page, page.meta.total
    );

    Ok(())
}
