use sqlx_template::*;
use sqlx::FromRow;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

#[derive(PostgresTemplate, FromRow, Default, Clone, Debug)]
#[table("chats")]
// #[tp_select_one(where = "active = true")]
#[tp_delete(where = "active = :active and 2 * id > :id", returning = true)]
// #[tp_select_one(by = "id, sender", order = "id desc", where = "active = :active", fn_name = "get_by_sender_active")]
#[tp_update(by = "id", where = "groups = :sender", returning = true)]
#[tp_update(by = "id", on = "content", where = "access = :sender", returning = "id, sender")]
// #[tp_update(by = "id", on = "receiver", where = "sender = :sender")]
// #[tp_upsert(by = "id", on = "sender, content", fn_name = "test1")]
pub struct Chat {
    pub id: i32,
    pub sender: i32,
    pub receiver: i32,
    pub content: String,
    pub groups: String,
    pub access: String,
    pub active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
}


impl Chat {
    fn test() {
        // Chat::update_by_id_on_content_return(id, content, sender, conn)
    }
}