// use sqlx_template::SqliteTemplate;
// use sqlx::{FromRow, SqlitePool};

// // Test case: Complex type should work
// #[derive(SqliteTemplate, FromRow, Debug, Clone)]
// #[table("users")]
// #[tp_select_builder(
//     with_score = "score = :score$u64"  // Simple type not in original list
// )]
// pub struct UserComplexType {
//     pub id: i32,
//     pub score: i32,
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     println!("Testing complex type Option<i32>");
    
//     // Create in-memory SQLite database
//     let pool = SqlitePool::connect(":memory:").await?;
    
//     // Create table
//     sqlx::query(
//         r#"
//         CREATE TABLE users (
//             id INTEGER PRIMARY KEY,
//             score INTEGER NOT NULL
//         )
//         "#,
//     )
//     .execute(&pool)
//     .await?;
    
//     // Insert test data
//     sqlx::query("INSERT INTO users (score) VALUES (?)")
//         .bind(100)
//         .execute(&pool)
//         .await?;
    
//     // Test u64 type
//     println!("Testing with_score with u64:");
//     let users = UserComplexType::builder_select()
//         .with_score(100u64)?
//         .find_all(&pool)
//         .await?;
//     println!("Found {} users", users.len());
    
//     Ok(())
// }
