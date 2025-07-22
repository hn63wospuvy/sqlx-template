use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email = "email = :email$String",
    with_active = "active = :active",
    with_score = "score * score > :min_score$i32"
)]
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
    .bind(15) // score^2 = 225 > 100
    .bind("User 1")
    .execute(&pool)
    .await?;
    
    sqlx::query(
        "INSERT INTO users (email, active, score, name) VALUES (?, ?, ?, ?)"
    )
    .bind("user2@example.com")
    .bind(false)
    .bind(8) // score^2 = 64 < 100
    .bind("User 2")
    .execute(&pool)
    .await?;
    
    sqlx::query(
        "INSERT INTO users (email, active, score, name) VALUES (?, ?, ?, ?)"
    )
    .bind("user3@example.com")
    .bind(true)
    .bind(12) // score^2 = 144 > 100
    .bind("User 3")
    .execute(&pool)
    .await?;
    
    // Test with_email condition
    println!("Testing with_email condition:");
    let users = User::builder_select()
        .with_email("user1@example.com")?
        .find_all(&pool)
        .await?;
    println!("Found {} users with email user1@example.com", users.len());
    for user in users {
        println!("  - {}: {} (active: {}, score: {})", user.id, user.name, user.active, user.score);
    }
    
    // Test with_active condition
    println!("\nTesting with_active condition:");
    let users = User::builder_select()
        .with_active(true)?
        .find_all(&pool)
        .await?;
    println!("Found {} active users", users.len());
    for user in users {
        println!("  - {}: {} (email: {}, score: {})", user.id, user.name, user.email, user.score);
    }
    
    // Test with_score condition
    println!("\nTesting with_score condition:");
    let users = User::builder_select()
        .with_score(100)?
        .find_all(&pool)
        .await?;
    println!("Found {} users with score^2 > 100", users.len());
    for user in users {
        println!("  - {}: {} (email: {}, score: {}, score^2: {})", 
                 user.id, user.name, user.email, user.score, user.score * user.score);
    }
    
    // Test combining conditions
    println!("\nTesting combined conditions:");
    let users = User::builder_select()
        .with_active(true)?
        .with_score(100)?
        .find_all(&pool)
        .await?;
    println!("Found {} active users with score^2 > 100", users.len());
    for user in users {
        println!("  - {}: {} (email: {}, score: {})", user.id, user.name, user.email, user.score);
    }
    
    Ok(())
}
