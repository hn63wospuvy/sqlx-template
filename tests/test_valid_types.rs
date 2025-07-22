use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

// Test case 3: Valid types should work
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_name = "name = :name",
    with_id = "id = :id$i64",
    with_score = "score = :score$i64",
    with_active = "active = :active"
)]
pub struct UserValidTypes {
    #[auto]
    pub id: i32,
    pub name: String,
    pub score: f64,
    pub active: bool,
}


// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            score REAL NOT NULL,
            active BOOLEAN NOT NULL
        )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing valid types");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            score REAL NOT NULL,
            active BOOLEAN NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (name, score, active) VALUES (?, ?, ?)")
        .bind("Alice")
        .bind(95.5)
        .bind(true)
        .execute(&pool)
        .await?;
    
    // Test all custom conditions
    println!("Testing with_name:");
    let users = UserValidTypes::builder_select()
        .with_name("Alice")?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    println!("Testing with_id:");
    let users = UserValidTypes::builder_select()
        .with_id(1)?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    println!("Testing with_score:");
    let users = UserValidTypes::builder_select()
        .with_score(95)?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    println!("Testing with_active:");
    let users = UserValidTypes::builder_select()
        .with_active(true)?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());
    
    Ok(())
}
