use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

// Test column mapping - placeholder should map to column type automatically
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email = "email = :email",  // Should map to String field -> &str parameter
    with_score = "score = :score",  // Should map to i32 field -> &i32 parameter
    with_active = "active = :active",  // Should map to bool field -> &bool parameter
    with_explicit_type = "name = :name$String"  // Explicit type should still work
)]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub score: i32,
    pub active: bool,
    pub name: String,
}


// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            score INTEGER NOT NULL,
            active BOOLEAN NOT NULL,
            name TEXT NOT NULL
        )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing column mapping");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            score INTEGER NOT NULL,
            active BOOLEAN NOT NULL,
            name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (email, score, active, name) VALUES (?, ?, ?, ?)")
        .bind("alice@example.com")
        .bind(95)
        .bind(true)
        .bind("Alice")
        .execute(&pool)
        .await?;
    
    // Test column mapping
    println!("Testing with_email (mapped to String -> &str):");
    let users = User::builder_select()
        .with_email("alice@example.com")?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    println!("Testing with_score (mapped to i32):");
    let users = User::builder_select()
        .with_score(95)?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());

    println!("Testing with_active (mapped to bool):");
    let users = User::builder_select()
        .with_active(true)?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    println!("Testing with_explicit_type (explicit String type):");
    let users = User::builder_select()
        .with_explicit_type("Alice")?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    Ok(())
}
