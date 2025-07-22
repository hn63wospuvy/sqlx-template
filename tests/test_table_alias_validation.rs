// use sqlx_template::SqliteTemplate;
// use sqlx::{FromRow, SqlitePool};

// // This should cause compile error due to table alias
// #[derive(SqliteTemplate, FromRow, Debug, Clone)]
// #[table("users")]
// #[tp_select_builder(
//     with_user_email = "u.email = :email$String"  // Contains table alias 'u.'
// )]
// pub struct User {
//     pub id: i32,
//     pub email: String,
//     pub name: String,
// }

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     println!("This should not compile due to table alias in custom condition");
//     Ok(())
// }
