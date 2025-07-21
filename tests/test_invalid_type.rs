// use sqlx_template::SqliteTemplate;
// use sqlx::{FromRow, SqlitePool};

// // Test case: Invalid type should cause compiler error
// #[derive(SqliteTemplate, FromRow, Debug, Clone)]
// #[table("users")]
// #[tp_select_builder(
//     with_score = "score = :score$NonExistentType"  // This type doesn't exist
// )]
// pub struct UserInvalidType {
//     pub id: i32,
//     pub score: i32,
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     println!("This should cause compiler error for invalid type");
//     Ok(())
// }
