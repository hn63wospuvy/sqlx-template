use sqlx_template::SqliteTemplate;

pub mod builder;
pub mod builder_expanded;

#[derive(sqlx::FromRow, Clone, Debug, SqliteTemplate)]
#[table("users")]
#[tp_select_stream(order = "id desc")]
pub struct Userrrrr {
    pub id: i32,
    pub email: String,
    pub score: f64,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}