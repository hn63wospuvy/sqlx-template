
use chrono::{DateTime, Utc};
use sqlx_template::{query, update, DeleteTemplate, QueryTemplate, TableName, UpdateTemplate};
use sqlx::{prelude::FromRow, types::{chrono, Json}};




#[tokio::main]
async fn main() {
    println!("Hello, world!");

}

#[derive(UpdateTemplate, QueryTemplate, DeleteTemplate, FromRow, TableName)]
#[debug_slow = 1000]
#[table_name = "login_history"]
#[tp_delete(by = "id")]
#[tp_delete(by = "id, email")]
#[tp_query_all(by = "id, email", order = "id desc")]
#[tp_query_one(by = "id, email", order = "id desc")]
#[tp_query_page(by = "id, email, org", order = "id desc")]
#[tp_query_count(by = "id, email")]
#[tp_update(by = "id, email", on = "password")]
#[tp_update(by = "id, email")]
#[tp_query_stream(by = "id, email", order = "id desc")]
pub struct LoginHistory {
    // #[auto]
    pub id: i32,
    pub email: String,
    pub password: String,
    pub org: Option<i32>,
    pub permissions: Vec<i16>,
    pub active: bool,
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

