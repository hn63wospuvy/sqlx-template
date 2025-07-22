use sqlx_template::{SqliteTemplate, sqlite_query};
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
    #[auto]
    pub id: i32,
    pub name: String,
    pub score: i32,
    pub active: bool,
}

// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT NOT NULL,
        score INTEGER NOT NULL,
        active BOOLEAN NOT NULL
    )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing complex SQL expressions");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table using generated function
    create_users_table(&pool).await?;

    // Insert test data using generated insert method
    let test_users = vec![
        User {
            id: 0, // Will be auto-generated
            name: "Alice".to_string(),
            score: 15,
            active: true,
        },
        User {
            id: 0, // Will be auto-generated
            name: "Bob".to_string(),
            score: 8,
            active: false,
        },
        User {
            id: 0, // Will be auto-generated
            name: "Charlie".to_string(),
            score: 12,
            active: true,
        },
    ];

    for user in test_users {
        User::insert(&user, &pool).await?;
    }
    
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
