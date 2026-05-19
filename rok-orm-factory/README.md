# rok-orm-factory

> Model factories for generating realistic test data in rok-orm applications.

Part of the [Rok Framework](https://rok.rs) — a full-stack Rust web framework built on Axum 0.8 and SQLx 0.8.

[![crates.io](https://img.shields.io/crates/v/rok-orm-factory)](https://crates.io/crates/rok-orm-factory) [![docs.rs](https://img.shields.io/docsrs/rok-orm-factory)](https://docs.rs/rok-orm-factory) [![MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

## Features

- `Factory` trait with `build()` for in-memory construction and `create()` for DB insertion
- `#[derive(Factory)]` proc-macro for zero-boilerplate factory generation
- `Faker` helpers for realistic data: names, emails, UUIDs, URLs, sentences, numbers
- Sequences for auto-incrementing unique values across test cases
- Named states via `.state("admin")` for variant factory configurations
- Relationship factories: `.for_user(&user)` sets foreign keys automatically
- `count(n).create_many()` for bulk data seeding
- Works alongside `sqlx::test` and `tokio::test` for isolated test databases

## Installation

```toml
[dev-dependencies]
rok-orm-factory = "0.2"
```

## Quick Start

```rust
use rok_orm_factory::{Factory, Faker, Sequence};

pub struct UserFactory;

impl Factory for UserFactory {
    type Model = User;

    fn build(&self) -> User {
        User {
            id: 0,
            email: Faker::unique_email(),
            name: Faker::name(),
            role: "member".into(),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }
}

#[tokio::test]
async fn test_user_listing() {
    let pool = test_pool().await;

    // Single user
    let user = UserFactory.create(&pool).await.unwrap();

    // Bulk users
    let users = UserFactory.count(20).create_many(&pool).await.unwrap();

    assert_eq!(users.len(), 20);
}
```

## Core API

### Overrides and states

```rust
// Override specific fields at call site
let admin = UserFactory
    .with(|u| {
        u.role = "admin".into();
        u.email = "admin@example.com".into();
    })
    .create(&pool)
    .await?;

// Named states defined on the factory
pub struct UserFactory;

impl Factory for UserFactory {
    type Model = User;

    fn build(&self) -> User {
        User { role: "member".into(), active: true, .. }
    }

    fn state(&self, name: &str) -> User {
        let mut user = self.build();
        match name {
            "admin"    => { user.role = "admin".into(); }
            "inactive" => { user.active = false; }
            "verified" => { user.email_verified_at = Some(chrono::Utc::now()); }
            _ => {}
        }
        user
    }
}

let inactive = UserFactory.as_state("inactive").create(&pool).await?;
```

### Sequences

```rust
use rok_orm_factory::Sequence;

static EMAIL_SEQ: Sequence = Sequence::new();

fn build_user() -> User {
    let n = EMAIL_SEQ.next();
    User {
        email: format!("user{}@example.com", n),
        ..
    }
}
```

### Relationship factories

```rust
pub struct PostFactory;

impl Factory for PostFactory {
    type Model = Post;

    fn build(&self) -> Post {
        Post {
            id: 0,
            user_id: 0,  // set via for_user()
            title: Faker::sentence(),
            body: Faker::paragraph(),
            published: Faker::bool(),
            created_at: chrono::Utc::now(),
        }
    }
}

impl PostFactory {
    pub fn for_user(self, user: &User) -> impl Factory<Model = Post> {
        self.with(move |p| p.user_id = user.id)
    }
}

let user = UserFactory.create(&pool).await?;
let posts = PostFactory.for_user(&user).count(5).create_many(&pool).await?;
```

### Faker helpers

```rust
Faker::name()             // "Eleanor Vance"
Faker::first_name()       // "Eleanor"
Faker::email()            // "eleanor42@example.com"
Faker::unique_email()     // guaranteed unique within test run
Faker::uuid()             // "550e8400-e29b-41d4-a716-446655440000"
Faker::word()             // "luminous"
Faker::sentence()         // "The quick brown fox jumps."
Faker::paragraph()        // multi-sentence paragraph
Faker::number(1..=1000)   // 347
Faker::bool()             // true
Faker::url()              // "https://example.com/path/to/page"
Faker::phone()            // "+1-555-0247"
Faker::ip()               // "192.168.1.42"
```

## Integration

`rok-orm-factory` works with any `rok-orm` model by requiring `Model + sqlx::FromRow + Default`. It pairs with `rok-orm-migrate` to set up a clean test schema via `MigrationRunner` before seeding, and with Axum's `TestClient` for full integration tests.

```rust
#[tokio::test]
async fn test_post_api() {
    let pool = test_db_with_migrations().await;
    let user = UserFactory.create(&pool).await.unwrap();
    let _posts = PostFactory.for_user(&user).count(3).create_many(&pool).await.unwrap();

    let client = TestClient::new(app_with_pool(pool));
    let res = client.get("/api/posts").await;
    assert_eq!(res.status(), 200);
}
```

## License

MIT
