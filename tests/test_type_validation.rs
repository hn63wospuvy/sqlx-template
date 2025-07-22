use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

// Test case 1: Missing type should cause compile error
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_name = "name = :name"  // This should cause compile error
)]
pub struct UserMissingType {
    pub id: i32,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("This should not compile due to missing type");
    Ok(())
}
