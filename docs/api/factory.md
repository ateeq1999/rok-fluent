# API: Factories (`rok_fluent::factory`) — feature: `factory`

```toml
[dev-dependencies]
rok-fluent = { version = "0.4", features = ["factory-postgres"] }
```

## `Factory` Trait

Implement on any model to get the factory DSL.

```rust,no_run
use rok_fluent::factory::{Factory, FactoryBuilder, Faker};

impl Factory for User {
    fn definition() -> Self {
        User {
            id: 0,
            name: Faker::name(),
            email: Faker::email(),
            password_hash: Faker::uuid(),
            active: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            deleted_at: None,
        }
    }
}
```

---

## `FactoryBuilder<T>`

```rust,no_run
// Build in-memory (no DB required)
let user: User = User::factory().make();
let users: Vec<User> = User::factory().count(5).make_many();

// Override specific fields
let admin: User = User::factory()
    .with(|u| { u.active = false; u.role = "admin".to_string(); })
    .make();

// Insert to database (feature: factory-postgres)
let user: User = User::factory().create(&pool).await?;
let users: Vec<User> = User::factory().count(10).create_many(&pool).await?;
```

### Methods

| Method | Description |
|---|---|
| `.count(n)` | Set quantity (default: 1) |
| `.with(fn)` | Override fields on each built instance |
| `.make()` | Build one instance in memory |
| `.make_many()` | Build `count` instances in memory |
| `.create(&pool)` | Insert one instance into DB |
| `.create_many(&pool)` | Bulk-insert `count` instances |

---

## `Faker`

Realistic fake data helpers. All methods return owned `String` values.

```rust,no_run
use rok_fluent::factory::Faker;

Faker::name()        // "Alice Johnson"
Faker::first_name()  // "Alice"
Faker::last_name()   // "Johnson"
Faker::email()       // "alice.johnson.4821@example.com"
Faker::uuid()        // "550e8400-e29b-41d4-a716-446655440000"
Faker::sentence()    // "The quick brown fox jumps over the lazy dog."
Faker::paragraph()   // multi-sentence text
Faker::word()        // "ephemeral"
Faker::number()      // "42"
Faker::number_in(1, 100) // i64 in range
Faker::bool()        // true or false
Faker::url()         // "https://example.com/path"
Faker::phone()       // "+1-555-867-5309"
Faker::ip()          // "192.168.1.42"
Faker::slug()        // "the-quick-brown-fox"
Faker::hex(16)       // "a3f9c2d1e8b07645"
Faker::choose(&["a", "b", "c"])  // random element
```

---

## Example: Full Test Setup

```rust,no_run
#[cfg(test)]
mod tests {
    use super::*;
    use rok_fluent::factory::{Factory, Faker};

    impl Factory for Post {
        fn definition() -> Self {
            Post {
                id: 0,
                user_id: 0,
                title: Faker::sentence(),
                slug: Faker::slug(),
                body: Faker::paragraph(),
                published: false,
            }
        }
    }

    #[sqlx::test]
    async fn test_published_posts(pool: sqlx::PgPool) {
        let user = User::factory().create(&pool).await.unwrap();
        let _drafts = Post::factory()
            .count(3)
            .with(|p| { p.user_id = user.id; p.published = false; })
            .create_many(&pool)
            .await
            .unwrap();
        let published = Post::factory()
            .count(2)
            .with(|p| { p.user_id = user.id; p.published = true; })
            .create_many(&pool)
            .await
            .unwrap();

        let result = Post::query()
            .where_eq("published", true)
            .where_eq("user_id", user.id)
            .all()
            .await
            .unwrap();

        assert_eq!(result.len(), 2);
    }
}
```
