use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email = "email = :email"
)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub active: bool,
    pub score: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing one custom condition");
    Ok(())
}
