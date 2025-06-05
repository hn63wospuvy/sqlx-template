use sqlx_template::*;
use sqlx::FromRow;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

#[derive(PostgresTemplate, FromRow, Default, Clone, Debug)]
#[table("chats")]
#[tp_delete(by = "id, active", where = "active = :active and 2 * id > :id")]
// #[tp_select_one(by = "id, sender", order = "id desc", where = "active = :active", fn_name = "get_by_sender_active")]
// #[tp_upsert(by = "id")]
// #[tp_upsert(by = "id", on = "sender, content", fn_name = "test1")]
pub struct Chat {
    pub id: i32,
    pub sender: i32,
    pub receiver: i32,
    pub content: String,
    pub active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}


fn test() {
}