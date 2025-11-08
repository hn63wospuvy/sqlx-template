use sqlx_template::SqliteTemplate;


#[derive(sqlx::FromRow, Clone, Debug, SqliteTemplate)]
#[table("users")]
#[tp_select_stream(order = "id desc")]
#[tp_select_builder(
    with_email = "email = :email", 
    with_active = "active = true",
    with_score = "score * score > 100"
)]
#[tp_update_builder(
    with_high_score = "score > :threshold$i32"
    with_ca = "created_at > :c$chrono::DateTime<chrono::Utc>"
)]
pub struct Userrrrr {
    pub id: i32,
    pub email: String,
    pub score: f64,
    pub active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

async fn tst() {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    
}