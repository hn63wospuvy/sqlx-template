use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

// Test with a very simple custom condition
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_name = "name = :name$String",
    with_id = "id = :id$i32"
)]
pub struct User {
    pub id: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing debug custom condition");

    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;

    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // Insert test data
    sqlx::query("INSERT INTO users (name) VALUES (?)")
        .bind("Alice")
        .execute(&pool)
        .await?;

    sqlx::query("INSERT INTO users (name) VALUES (?)")
        .bind("Bob")
        .execute(&pool)
        .await?;

    // Test custom condition with_name
    println!("Testing with_name condition:");
    let users = User::builder_select()
        .with_name("Alice")?
        .find_all(&pool)
        .await?;
    println!("Found {} users with name Alice", users.len());
    for user in users {
        println!("  - {}: {}", user.id, user.name);
    }

    // Test custom condition with_id
    println!("\nTesting with_id condition:");
    let users = User::builder_select()
        .with_id(1)?
        .find_all(&pool)
        .await?;
    println!("Found {} users with id 1", users.len());
    for user in users {
        println!("  - {}: {}", user.id, user.name);
    }


    Ok(())
}
