use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub active: bool,
    pub score: i32,
    pub name: String,
}


// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL,
            score INTEGER NOT NULL,
            name TEXT NOT NULL
        )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing basic builder without custom conditions");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            active BOOLEAN NOT NULL,
            score INTEGER NOT NULL,
            name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query(
        "INSERT INTO users (email, active, score, name) VALUES (?, ?, ?, ?)"
    )
    .bind("user1@example.com")
    .bind(true)
    .bind(15)
    .bind("User 1")
    .execute(&pool)
    .await?;
    
    // Test basic builder
    println!("Testing basic builder:");
    let users = User::builder_select()
        .email("user1@example.com")?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    for user in users {
        println!("  - {}: {} (active: {}, score: {})", user.id, user.name, user.active, user.score);
    }
    
    Ok(())
}
