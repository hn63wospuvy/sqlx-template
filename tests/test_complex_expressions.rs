use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

// Test complex SQL expressions
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_name_and_score = "name = :name$String AND score > :min_score$i32",
    with_complex_condition = "score * score > :threshold$i32 AND active = :active$bool",
    with_like_pattern = "name LIKE :pattern$String"
)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub score: i32,
    pub active: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing complex SQL expressions");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            score INTEGER NOT NULL,
            active BOOLEAN NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (name, score, active) VALUES (?, ?, ?)")
        .bind("Alice")
        .bind(15)
        .bind(true)
        .execute(&pool)
        .await?;
    
    sqlx::query("INSERT INTO users (name, score, active) VALUES (?, ?, ?)")
        .bind("Bob")
        .bind(8)
        .bind(false)
        .execute(&pool)
        .await?;
    
    sqlx::query("INSERT INTO users (name, score, active) VALUES (?, ?, ?)")
        .bind("Charlie")
        .bind(12)
        .bind(true)
        .execute(&pool)
        .await?;
    
    // Test complex conditions
    println!("Testing with_name_and_score:");
    let users = User::builder_select()
        .with_name_and_score("Alice", 10)?
        .find_all(&pool)
        .await?;
    println!("Found {} users with name Alice and score > 10", users.len());
    
    println!("Testing with_complex_condition:");
    let users = User::builder_select()
        .with_complex_condition(100, true)?
        .find_all(&pool)
        .await?;
    println!("Found {} users with score^2 > 100 and active = true", users.len());
    
    println!("Testing with_like_pattern:");
    let users = User::builder_select()
        .with_like_pattern("A%")?
        .find_all(&pool)
        .await?;
    println!("Found {} users with name starting with 'A'", users.len());
    
    Ok(())
}
