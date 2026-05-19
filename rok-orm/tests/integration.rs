use rok_orm::Model;

// ── basic derive ──────────────────────────────────────────────────────────────

#[derive(Model)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub email: String,
}

#[derive(Model)]
pub struct BlogPost {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub published: bool,
}

// snake_case + "s"
#[derive(Model)]
pub struct OrderItem {
    pub id: i64,
    pub quantity: i32,
}

// custom table name
#[derive(Model)]
#[rok_orm(table = "articles")]
pub struct Article {
    pub id: i64,
    pub title: String,
}

// custom primary key via struct attribute
#[derive(Model)]
#[rok_orm(primary_key = "user_id")]
pub struct Profile {
    pub user_id: i64,
    pub bio: String,
}

// field-level: skip + column rename + primary_key
#[derive(Model)]
pub struct Tag {
    #[rok_orm(primary_key)]
    pub tag_id: i64,
    #[rok_orm(column = "tag_name")]
    pub name: String,
    #[rok_orm(skip)]
    pub cached_count: i32,
}

// ── table names ───────────────────────────────────────────────────────────────

#[test]
fn table_name_simple() {
    assert_eq!(User::table_name(), "users");
}

#[test]
fn table_name_multi_word() {
    assert_eq!(BlogPost::table_name(), "blog_posts");
    assert_eq!(OrderItem::table_name(), "order_items");
}

// ── columns ───────────────────────────────────────────────────────────────────

#[test]
fn columns_list() {
    assert_eq!(User::columns(), &["id", "name", "email"]);
    assert_eq!(BlogPost::columns(), &["id", "title", "body", "published"]);
}

// ── query builder through Model trait ────────────────────────────────────────

#[test]
fn query_select_all() {
    let (sql, params) = User::query().to_sql();
    assert_eq!(sql, "SELECT * FROM users");
    assert!(params.is_empty());
}

#[test]
fn query_where_eq() {
    let (sql, params) = User::query().where_eq("id", 1i64).to_sql();
    assert!(sql.contains("WHERE id = $1"));
    assert_eq!(params.len(), 1);
}

#[test]
fn query_find() {
    let (sql, params) = User::find(42i64).to_sql();
    assert!(sql.contains("WHERE id = $1"));
    assert_eq!(params[0], rok_orm::SqlValue::Integer(42));
}

#[test]
fn query_chaining() {
    let (sql, params) = BlogPost::query()
        .where_eq("published", true)
        .where_like("title", "%rust%")
        .order_by_desc("id")
        .limit(5)
        .offset(10)
        .to_sql();

    assert!(sql.contains("FROM blog_posts"));
    assert!(sql.contains("WHERE published = $1 AND title LIKE $2"));
    assert!(sql.contains("ORDER BY id DESC"));
    assert!(sql.contains("LIMIT 5"));
    assert!(sql.contains("OFFSET 10"));
    assert_eq!(params.len(), 2);
}

#[test]
fn count_sql() {
    let (sql, _) = User::query().where_not_null("email").to_count_sql();
    assert!(sql.starts_with("SELECT COUNT(*) FROM users"));
    assert!(sql.contains("email IS NOT NULL"));
}

#[test]
fn insert_sql() {
    use rok_orm::SqlValue;
    let (sql, params) = rok_orm::QueryBuilder::<User>::insert_sql(
        "users",
        &[
            ("name", "Alice".into()),
            ("email", "alice@example.com".into()),
        ],
    );
    assert!(sql.contains("INSERT INTO users (name, email) VALUES ($1, $2)"));
    assert_eq!(
        params,
        vec![
            SqlValue::Text("Alice".into()),
            SqlValue::Text("alice@example.com".into()),
        ]
    );
}

// ── attribute: custom table name ──────────────────────────────────────────────

#[test]
fn custom_table_name() {
    assert_eq!(Article::table_name(), "articles");
    assert_eq!(Article::columns(), &["id", "title"]);
}

// ── attribute: custom primary key ─────────────────────────────────────────────

#[test]
fn struct_level_primary_key() {
    assert_eq!(Profile::primary_key(), "user_id");
}

#[test]
fn field_level_primary_key() {
    assert_eq!(Tag::primary_key(), "tag_id");
}

// ── attribute: skip and column rename ────────────────────────────────────────

#[test]
fn skip_excludes_field() {
    // cached_count is skipped
    assert_eq!(Tag::columns(), &["tag_id", "tag_name"]);
}

// ── OR conditions ─────────────────────────────────────────────────────────────

#[test]
fn or_where_conditions() {
    let (sql, params) = User::query()
        .where_eq("role", "admin")
        .or_where_eq("role", "moderator")
        .to_sql();
    assert!(sql.contains("role = $1 OR role = $2"));
    assert_eq!(params.len(), 2);
}

// ── between ───────────────────────────────────────────────────────────────────

#[test]
fn where_between_query() {
    let (sql, params) = User::query().where_between("id", 1i64, 100i64).to_sql();
    assert!(sql.contains("id BETWEEN $1 AND $2"));
    assert_eq!(params.len(), 2);
}

// ── not_in ────────────────────────────────────────────────────────────────────

#[test]
fn where_not_in_query() {
    let (sql, params) = User::query().where_not_in("id", vec![1i64, 2, 3]).to_sql();
    assert!(sql.contains("id NOT IN ($1, $2, $3)"));
    assert_eq!(params.len(), 3);
}

// ── distinct ──────────────────────────────────────────────────────────────────

#[test]
fn distinct_query() {
    let (sql, _) = User::query().distinct().select(&["email"]).to_sql();
    assert!(sql.starts_with("SELECT DISTINCT email FROM users"));
}

// ── to_update_sql ─────────────────────────────────────────────────────────────

#[test]
fn update_sql_via_builder() {
    let (sql, params) =
        User::find(1i64).to_update_sql(&[("name", "Bob".into()), ("email", "b@b.com".into())]);
    assert!(sql.starts_with("UPDATE users SET name = $1, email = $2"));
    assert!(sql.contains("WHERE id = $3"));
    assert_eq!(params.len(), 3);
}

// ── join ──────────────────────────────────────────────────────────────────────

#[test]
fn inner_join_query() {
    let (sql, params) = User::query()
        .inner_join("posts", "posts.user_id = users.id")
        .where_eq("users.active", true)
        .to_sql();
    assert!(sql.contains("INNER JOIN posts ON posts.user_id = users.id"));
    assert!(sql.contains("WHERE users.active = $1"));
    assert_eq!(params.len(), 1);
}

#[test]
fn left_join_query() {
    let (sql, _) = User::query()
        .left_join("profiles", "profiles.user_id = users.id")
        .to_sql();
    assert!(sql.contains("LEFT JOIN profiles ON profiles.user_id = users.id"));
}

// ── group by / having ─────────────────────────────────────────────────────────

#[test]
fn group_by_having_query() {
    use rok_orm::QueryBuilder;
    let (sql, _) = QueryBuilder::<User>::new("users")
        .select(&["role", "COUNT(*) as n"])
        .group_by(&["role"])
        .having("COUNT(*) > 1")
        .to_sql();
    assert!(sql.contains("GROUP BY role"));
    assert!(sql.contains("HAVING COUNT(*) > 1"));
}

// ── bulk insert ───────────────────────────────────────────────────────────────

#[test]
fn bulk_insert_sql() {
    use rok_orm::{QueryBuilder, SqlValue};

    let rows: Vec<Vec<(&str, SqlValue)>> = vec![
        vec![("name", "Alice".into()), ("email", "a@a.com".into())],
        vec![("name", "Bob".into()), ("email", "b@b.com".into())],
        vec![("name", "Carol".into()), ("email", "c@c.com".into())],
    ];
    let (sql, params) = QueryBuilder::<User>::bulk_insert_sql("users", &rows);
    assert!(sql.starts_with("INSERT INTO users (name, email) VALUES"));
    assert!(sql.contains("($1, $2), ($3, $4), ($5, $6)"));
    assert_eq!(params.len(), 6);
}
