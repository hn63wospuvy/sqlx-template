use sqlx_template::{UpdateTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

#[derive(UpdateTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_update_builder(
    with_active_status = "active = :status"
)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub active: bool,
    pub score: i32,
}


// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL,
            score INTEGER NOT NULL
        )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing UPDATE builder with custom conditions");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL,
            score INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (email, active, score) VALUES (?, ?, ?)")
        .bind("alice@example.com")
        .bind(true)
        .bind(85)
        .execute(&pool)
        .await?;
    
    sqlx::query("INSERT INTO users (email, active, score) VALUES (?, ?, ?)")
        .bind("bob@example.com")
        .bind(false)
        .bind(65)
        .execute(&pool)
        .await?;
    
    // Test UPDATE builder with custom conditions
    println!("Testing with_active_status custom condition:");
    let affected = User::builder_update()
        .on_score(&95)?
        .with_active_status(true)?
        .execute(&pool)
        .await?;
    println!("Updated {} active users' scores", affected);
    
    Ok(())
}
