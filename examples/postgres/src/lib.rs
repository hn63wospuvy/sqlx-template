use sqlx_template::*;
use sqlx::FromRow;
use sqlx::types::chrono::DateTime;
use sqlx::types::chrono::Utc;

// Define Page type for testing
#[derive(Debug, Clone)]
pub struct Page<T> {
    pub offset: u64,
    pub limit: u32,
    pub total: Option<u64>,
    pub data: Vec<T>
}

#[derive(PostgresTemplate, FromRow, Default, Clone, Debug)]
#[table("chats")]
// #[tp_select_one(where = "active = true")]
#[tp_delete(where = "active = :active and 2 * id > :min_id$i32", returning = true)]
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

// Test function for Page query with JOIN and GROUP BY
// This should use the new subquery approach for count
#[postgres_query(sql = "
    SELECT u.department, COUNT(c.id) as chat_count
    FROM users u
    LEFT JOIN chats c ON u.id = c.sender
    WHERE u.active = :active
    GROUP BY u.department
    ORDER BY chat_count DESC
")]
pub async fn get_department_chat_stats(active: bool) -> Page<DepartmentChatStats> {}

#[derive(FromRow, Debug)]
pub struct DepartmentChatStats {
    pub department: String,
    pub chat_count: i64,
}

// Test function for simple Page query (should use old method)
#[postgres_query(sql = "SELECT * FROM chats WHERE active = :active ORDER BY created_at DESC")]
pub async fn get_active_chats(active: bool) -> Page<Chat> {}

// Include test module
pub mod test_count_query;