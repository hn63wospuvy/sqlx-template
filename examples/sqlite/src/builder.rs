use sqlx_template::SqliteTemplate;


#[derive(sqlx::FromRow, Clone, Debug, SqliteTemplate)]
#[table("users")]
#[tp_select_stream(order = "id desc")]
#[tp_select_builder(
    with_email = "active = :active", 
    with_score = "active = :active",
)]
pub struct Userrrrr {
    pub id: i32,
    pub email: String,
    pub score: f64,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}