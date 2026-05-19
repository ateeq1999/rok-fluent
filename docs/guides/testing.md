# Guide: Testing with Factories

## Setup

```toml
[dev-dependencies]
rok-fluent = { version = "0.4", features = ["factory-postgres"] }
tokio = { version = "1", features = ["full"] }
sqlx = { version = "0.8", features = ["postgres", "runtime-tokio"] }
```

## Implement `Factory` on Your Models

```rust,no_run
#[cfg(test)]
mod factories {
    use rok_fluent::factory::{Factory, Faker};
    use crate::models::{Post, User};

    impl Factory for User {
        fn definition() -> Self {
            User {
                id: 0,
                name: Faker::name(),
                email: Faker::email(),
                password_hash: Faker::hex(64),
                active: true,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                deleted_at: None,
            }
        }
    }

    impl Factory for Post {
        fn definition() -> Self {
            Post {
                id: 0,
                user_id: 0,   // override in test: .with(|p| p.user_id = user.id)
                title: Faker::sentence(),
                slug: Faker::slug(),
                body: Faker::paragraph(),
                published: false,
            }
        }
    }
}
```

Place `Factory` impls in a `factories.rs` or inline in test modules. They are
`#[cfg(test)]`-gated and never compiled into your release binary.

## In-Memory Factories (no database)

```rust,no_run
use rok_fluent::factory::Factory;

let user = User::factory().make();
let users: Vec<User> = User::factory().count(10).make_many();

// Override fields
let admin = User::factory()
    .with(|u| {
        u.email = "admin@example.com".to_string();
        u.active = false;
    })
    .make();
```

Useful for pure unit tests that don't need database access.

## Database Factories

```rust,no_run
use rok_fluent::factory::Factory;

#[sqlx::test]
async fn test_active_users_query(pool: sqlx::PgPool) {
    rok_fluent::orm::postgres::pool::set(pool.clone());

    // Create test data
    let active = User::factory()
        .count(3)
        .with(|u| u.active = true)
        .create_many(&pool)
        .await
        .unwrap();

    User::factory()
        .count(2)
        .with(|u| u.active = false)
        .create_many(&pool)
        .await
        .unwrap();

    // Run the code under test
    let result = User::query()
        .where_eq("active", true)
        .all()
        .await
        .unwrap();

    assert_eq!(result.len(), 3);
}
```

`#[sqlx::test]` creates an isolated test database per test and tears it down after.
Each test is fully independent.

## Factory Relationships

```rust,no_run
#[sqlx::test]
async fn test_posts_for_user(pool: sqlx::PgPool) {
    rok_fluent::orm::postgres::pool::set(pool.clone());

    let user = User::factory().create(&pool).await.unwrap();
    let _posts = Post::factory()
        .count(5)
        .with(|p| { p.user_id = user.id; p.published = true; })
        .create_many(&pool)
        .await
        .unwrap();

    let posts = Post::query()
        .where_eq("user_id", user.id)
        .where_eq("published", true)
        .all()
        .await
        .unwrap();

    assert_eq!(posts.len(), 5);
    assert!(posts.iter().all(|p| p.user_id == user.id));
}
```

## `Faker` Reference

```rust,no_run
use rok_fluent::factory::Faker;

// Names
Faker::name()          // full name:  "Alice Johnson"
Faker::first_name()    // "Alice"
Faker::last_name()     // "Johnson"

// Contact
Faker::email()         // "alice.johnson.4821@example.com"
Faker::phone()         // "+1-555-867-5309"
Faker::url()           // "https://example.com/path"
Faker::ip()            // "192.168.1.42"

// Identity
Faker::uuid()          // "550e8400-e29b-41d4-a716-446655440000"
Faker::hex(16)         // "a3f9c2d1e8b07645"
Faker::slug()          // "the-quick-brown-fox"

// Text
Faker::word()          // "ephemeral"
Faker::sentence()      // "The quick brown fox jumps over the lazy dog."
Faker::paragraph()     // multi-sentence text

// Numbers
Faker::number()                  // "42" (as String)
Faker::number_in(1_i64, 100)     // i64 in range

// Boolean
Faker::bool()          // true or false

// Collections
Faker::choose(&["admin", "editor", "viewer"])   // random element
```

## Seeding Development Data

```rust,no_run
// src/seeds/users.rs
use rok_fluent::factory::{Factory, Faker};
use crate::models::User;

pub async fn run(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    // Create a known admin account
    User::factory()
        .with(|u| {
            u.email = "admin@example.com".to_string();
            u.name = "Admin User".to_string();
        })
        .create(pool)
        .await?;

    // Bulk random users
    User::factory().count(50).create_many(pool).await?;

    println!("Seeded 51 users");
    Ok(())
}
```

## Best Practices

- Keep `Factory` impls in a dedicated `tests/factories/` module or alongside their models
  inside `#[cfg(test)]`.
- Always use `#[sqlx::test]` (not `#[tokio::test]` with manual pool setup) — it handles
  isolation and cleanup automatically.
- Override only the fields your test cares about. `make()` / `create()` fill everything
  else with realistic values.
- Never hard-code IDs in factory definitions — let the database assign them.
- For performance, prefer `.create_many()` over looping `.create()` calls.
