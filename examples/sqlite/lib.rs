use sqlx::{FromRow, SqlitePool};
use sqlx_template::{tp_select_builder, tp_update_builder, tp_delete_builder, SqlxTemplate};
use chrono::{DateTime, Utc};

#[derive(SqlxTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_select_builder]
#[tp_update_builder]
#[tp_delete_builder]
pub struct User {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub age: i32,
    pub score: f64,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(SqlxTemplate, FromRow, Debug, Clone)]
#[table("posts")]
#[db("sqlite")]
#[tp_select_builder]
#[tp_update_builder]
#[tp_delete_builder]
pub struct Post {
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub content: String,
    pub published: bool,
    pub view_count: i32,
    pub created_at: DateTime<Utc>,
}

pub async fn setup_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Create users table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            score REAL NOT NULL DEFAULT 0.0,
            active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create posts table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            published BOOLEAN NOT NULL DEFAULT 0,
            view_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn example_user_operations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("=== User Operations Examples ===");

    // 1. SELECT Examples
    println!("\n1. SELECT Examples:");

    // Find users by email
    let user = User::builder_select()
        .email("john@example.com")
        .find_one(pool)
        .await?;
    println!("User by email: {:?}", user);

    // Find active users older than 18
    let adult_users = User::builder_select()
        .active(true)
        .age_gt(18)
        .order_by_age_desc()
        .find_all(pool)
        .await?;
    println!("Adult active users: {} found", adult_users.len());

    // Find users with high scores
    let high_score_users = User::builder_select()
        .score_gte(85.0)
        .active(true)
        .order_by_score_desc()
        .order_by_name_asc()
        .find_all(pool)
        .await?;
    println!("High score users: {} found", high_score_users.len());

    // Find users by name pattern
    let users_with_john = User::builder_select()
        .name_like("%John%")
        .find_all(pool)
        .await?;
    println!("Users with 'John' in name: {} found", users_with_john.len());

    // Find users by email domain
    let gmail_users = User::builder_select()
        .email_end_with("@gmail.com")
        .active(true)
        .find_all(pool)
        .await?;
    println!("Gmail users: {} found", gmail_users.len());

    // 2. UPDATE Examples
    println!("\n2. UPDATE Examples:");

    // Update user score
    let updated_rows = User::builder_update()
        .on_score(95.5)
        .on_active(true)
        .by_email("john@example.com")
        .execute(pool)
        .await?;
    println!("Updated {} user(s) score", updated_rows);

    // Deactivate old users
    let deactivated_rows = User::builder_update()
        .on_active(false)
        .by_age_gt(65)
        .execute(pool)
        .await?;
    println!("Deactivated {} old user(s)", deactivated_rows);

    // Update users by score range
    let score_updated = User::builder_update()
        .on_score(50.0)
        .by_score_lt(30.0)
        .by_active(true)
        .execute(pool)
        .await?;
    println!("Updated {} user(s) with low scores", score_updated);

    // 3. DELETE Examples
    println!("\n3. DELETE Examples:");

    // Delete inactive users
    let deleted_inactive = User::builder_delete()
        .active(false)
        .execute(pool)
        .await?;
    println!("Deleted {} inactive user(s)", deleted_inactive);

    // Delete users with very low scores
    let deleted_low_score = User::builder_delete()
        .score_lt(10.0)
        .execute(pool)
        .await?;
    println!("Deleted {} user(s) with very low scores", deleted_low_score);

    Ok(())
}

pub async fn example_post_operations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("\n=== Post Operations Examples ===");

    // 1. SELECT Examples
    println!("\n1. SELECT Examples:");

    // Find published posts
    let published_posts = Post::builder_select()
        .published(true)
        .order_by_created_at_desc()
        .find_all(pool)
        .await?;
    println!("Published posts: {} found", published_posts.len());

    // Find popular posts
    let popular_posts = Post::builder_select()
        .view_count_gt(100)
        .published(true)
        .order_by_view_count_desc()
        .find_all(pool)
        .await?;
    println!("Popular posts: {} found", popular_posts.len());

    // Find posts by title pattern
    let tech_posts = Post::builder_select()
        .title_like("%Tech%")
        .published(true)
        .find_all(pool)
        .await?;
    println!("Tech posts: {} found", tech_posts.len());

    // Find posts by user
    let user_posts = Post::builder_select()
        .user_id(1)
        .order_by_created_at_desc()
        .find_all(pool)
        .await?;
    println!("Posts by user 1: {} found", user_posts.len());

    // 2. UPDATE Examples
    println!("\n2. UPDATE Examples:");

    // Publish draft posts
    let published_count = Post::builder_update()
        .on_published(true)
        .by_published(false)
        .by_title_like("%Ready%")
        .execute(pool)
        .await?;
    println!("Published {} draft post(s)", published_count);

    // Increment view count
    let view_updated = Post::builder_update()
        .on_view_count(150)
        .by_id(1)
        .execute(pool)
        .await?;
    println!("Updated view count for {} post(s)", view_updated);

    // 3. DELETE Examples
    println!("\n3. DELETE Examples:");

    // Delete unpublished old posts
    let old_date = DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let deleted_old = Post::builder_delete()
        .published(false)
        .created_at_lt(old_date)
        .execute(pool)
        .await?;
    println!("Deleted {} old unpublished post(s)", deleted_old);

    Ok(())
}

pub async fn example_complex_queries(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("\n=== Complex Query Examples ===");

    // Complex user filtering
    let complex_users = User::builder_select()
        .active(true)
        .age_gte(21)
        .age_lte(65)
        .score_gt(50.0)
        .email_not("admin@example.com")
        .name_start_with("J")
        .order_by_score_desc()
        .order_by_age_asc()
        .find_all(pool)
        .await?;
    println!("Complex filtered users: {} found", complex_users.len());

    // Complex post filtering
    let complex_posts = Post::builder_select()
        .published(true)
        .view_count_gte(50)
        .view_count_lte(1000)
        .title_not("Draft")
        .content_like("%important%")
        .order_by_view_count_desc()
        .find_all(pool)
        .await?;
    println!("Complex filtered posts: {} found", complex_posts.len());

    // Bulk operations
    let bulk_update = User::builder_update()
        .on_score(75.0)
        .by_score_gte(70.0)
        .by_score_lt(80.0)
        .by_active(true)
        .execute(pool)
        .await?;
    println!("Bulk updated {} user(s) scores", bulk_update);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // Setup database connection
    let database_url = "sqlite::memory:";
    let pool = SqlitePool::connect(database_url).await?;

    // Setup tables
    setup_database(&pool).await?;

    // Insert sample data
    insert_sample_data(&pool).await?;

    // Run examples
    example_user_operations(&pool).await?;
    example_post_operations(&pool).await?;
    example_complex_queries(&pool).await?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

async fn insert_sample_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Insert sample users
    let users = vec![
        ("john@example.com", "John Doe", 25, 85.5),
        ("jane@gmail.com", "Jane Smith", 30, 92.0),
        ("bob@yahoo.com", "Bob Johnson", 22, 78.0),
        ("alice@gmail.com", "Alice Brown", 28, 88.5),
        ("charlie@example.com", "Charlie Wilson", 35, 95.0),
    ];

    for (email, name, age, score) in users {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO users (email, name, age, score, active, created_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(email)
        .bind(name)
        .bind(age)
        .bind(score)
        .bind(true)
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    }

    // Insert sample posts
    let posts = vec![
        (1, "Tech Trends 2024", "Content about tech trends", true, 150),
        (1, "Draft Post", "This is a draft", false, 0),
        (2, "Cooking Tips", "How to cook better", true, 75),
        (3, "Travel Guide", "Best places to visit", true, 200),
        (2, "Ready to Publish", "This post is ready", false, 5),
    ];

    for (user_id, title, content, published, view_count) in posts {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO posts (user_id, title, content, published, view_count, created_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(published)
        .bind(view_count)
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    }

    Ok(())
}
