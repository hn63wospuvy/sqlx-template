use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

// Test case 2: Custom type should work if compiler accepts it
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_score = "score = :score$u32"  // u32 should work now
)]
pub struct UserCustomType {
    #[auto]
    pub id: i32,
    pub score: i32,
}


// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            score INTEGER NOT NULL
        )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing custom type u32");

    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;

    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            score INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Insert test data
    sqlx::query("INSERT INTO users (score) VALUES (?)")
        .bind(100)
        .execute(&pool)
        .await?;

    // Test custom type
    println!("Testing with_score with u32:");
    let users = UserCustomType::builder_select()
        .with_score(100u32)?
        .find_all(&pool)
        .await?;
    println!("Found {} users", users.len());

    Ok(())
}
