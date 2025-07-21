use sqlx_template::DeleteTemplate;
use sqlx::{FromRow, SqlitePool};

#[derive(DeleteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_delete_builder(
    with_old_accounts = "created_at < :cutoff_date$String",
    with_inactive_users = "active = :active"
)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub active: bool,
    pub created_at: String,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing DELETE builder with custom conditions");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL,
            created_at TEXT NOT NULL,
            name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (email, active, created_at, name) VALUES (?, ?, ?, ?)")
        .bind("alice@example.com")
        .bind(true)
        .bind("2023-01-01")
        .bind("Alice")
        .execute(&pool)
        .await?;
    
    sqlx::query("INSERT INTO users (email, active, created_at, name) VALUES (?, ?, ?, ?)")
        .bind("bob@example.com")
        .bind(false)
        .bind("2022-01-01")
        .bind("Bob")
        .execute(&pool)
        .await?;
    
    sqlx::query("INSERT INTO users (email, active, created_at, name) VALUES (?, ?, ?, ?)")
        .bind("charlie@example.com")
        .bind(false)
        .bind("2021-01-01")
        .bind("Charlie")
        .execute(&pool)
        .await?;
    
    // Test DELETE builder with custom conditions
    println!("Testing with_old_accounts custom condition:");
    let deleted = User::builder_delete()
        .with_old_accounts("2022-06-01")?
        .execute(&pool)
        .await?;
    println!("Deleted {} old accounts", deleted);
    
    println!("Testing with_inactive_users custom condition:");
    let deleted = User::builder_delete()
        .with_inactive_users(false)?
        .execute(&pool)
        .await?;
    println!("Deleted {} inactive users", deleted);
    
    // Check remaining users
    let remaining = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await?;
    println!("Remaining users: {}", remaining);
    
    Ok(())
}
