use sqlx_template::{UpdateTemplate};
use sqlx::{FromRow, SqlitePool};

#[derive(UpdateTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_update_builder]
pub struct User {
    pub id: i32,
    pub email: String,
    pub score: i32,
}


#[tokio::test]
async fn test_update_builder() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing UPDATE builder - basic functionality");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            score INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (id, email, score) VALUES (?, ?, ?)")
        .bind(1)
        .bind("alice@example.com")
        .bind(85)
        .execute(&pool)
        .await?;
    
    // Test 1: Start with SET clauses, then WHERE
    println!("\nTest 1: SET then WHERE");
    let sql = User::builder_update()
        .on_score(&95)?
        .by_id(&1)?
        .build_sql();
    println!("Generated SQL: {}", sql);
    
    // Test 2: Multiple WHERE conditions (stays in WhereBuilder)
    println!("\nTest 2: Multiple WHERE conditions");
    let sql = User::builder_update()
        .on_score(&95)?
        .by_id(&1)?
        .by_score_gte(&80)?
        .build_sql();
    println!("Generated SQL: {}", sql);
    
    // Test 3: Execute update (SET then WHERE)
    println!("\nTest 3: Execute SET then WHERE");
    let affected = User::builder_update()
        .on_score(&95)?
        .on_email("newemail@example.com")?
        .by_id(&1)?
        .execute(&pool)
        .await?;
    println!("Updated {} rows", affected);
    
    // Verify update
    let user: (i32, String, i32) = sqlx::query_as("SELECT id, email, score FROM users WHERE id = ?")
        .bind(1)
        .fetch_one(&pool)
        .await?;
    println!("User after update: id={}, email={}, score={}", user.0, user.1, user.2);
    
    Ok(())
}
