
use chrono::{DateTime, Utc};
use futures::StreamExt;
use sqlx_template::{multi_query, query, update, DeleteTemplate, QueryTemplate, TableName, UpdateTemplate};
use sqlx::{migrate::MigrateDatabase, prelude::FromRow, types::{chrono, Json}, Sqlite, SqlitePool};
use sqlx_template::InsertTemplate;

const DB_URL: &str = "sqlite://sqlite.db";

#[tokio::main]
async fn main() {
    let fresh = if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        println!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => {
                println!("Create db success");
                true
            },
            Err(error) => panic!("error: {}", error),
        }
    } else {
        println!("Database already exists");
        false
    };

    let db = SqlitePool::connect(DB_URL).await.unwrap();
    if fresh {
        // Run migration script when db is newly created
        migrate(&db).await.unwrap();
    }

    let org_1 = Organization { 
        name: "org1".into(), 
        code: "org_1".into(), 
        active: true, 
        created_by: Some("test-user".into()), 
        ..Default::default()
    };

    // Insert new org
    let _ = Organization::insert(&org_1, &db).await.unwrap();

    // Fetch all org
    let orgs = Organization::find_all(&db).await.unwrap();
    println!("Orgs: {orgs:#?}");
    let org_1 = orgs.first().unwrap();
    let user_1 = User { 
        email: format!("user1@abc.com"), 
        password: "password".into(), 
        org: Some(org_1.id), 
        active: true, 
        ..Default::default()
    };
    let user_2 = User { 
        email: format!("user2@abc.com"), 
        password: "password".into(), 
        org: Some(org_1.id), 
        active: true, 
        ..Default::default()
    };

    // Insert user
    User::insert(&user_1, &db).await.unwrap();
    User::insert(&user_2, &db).await.unwrap();

    // Stream all users order by id
    let mut users = User::stream_order_by_id_desc(&db);
    while let Some(Ok(u)) = users.next().await {
        println!("User: {u:#?}");
    }

    // Pagination
    let page_request = PageRequest::default();
    let page = User::find_page_by_org_order_by_id_desc_and_org_desc(&Some(org_1.id), page_request, &db)
        .await
        .unwrap()
        .into_page(page_request);
    println!("Page user: {page:#?}");


    // Transaction
    let mut tx = db.begin().await.unwrap();
    let org_2 = Organization { 
        name: "org2".into(), 
        code: "org_2".into(), 
        active: true, 
        created_by: Some("test-user".into()), 
        ..Default::default()
    };
    let _ = Organization::insert(&org_2, &mut *tx).await.unwrap();
    let org = Organization::find_one_by_code(&"org_2".to_string(), &mut *tx).await.unwrap().unwrap();
    let mut user = User::find_one_by_email(&"user2@abc.com".to_string(), &mut *tx).await.unwrap().unwrap();
    user.org = Some(org.id);
    user.updated_at = Some(Utc::now());
    user.updated_by = Some("abc".into());
    User::update_user(&user.id, &user, &mut *tx).await.unwrap();
    tx.commit().await.unwrap();

    let user = User::find_one_by_email(&"user2@abc.com".to_string(), &db).await.unwrap().unwrap();
    println!("User after update: {user:#?}");


}

#[derive(Debug, Clone, Copy)]
struct PageRequest {
    offset: u64, 
    limit: u32,
    count: bool
}

impl From<PageRequest> for (i64, i32, bool) {
    fn from(value: PageRequest) -> Self {
        (value.offset as i64, value.limit as i32, value.count)
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self { 
            offset: 0, 
            limit: 10, 
            count: true 
        }
    }
}

#[derive(Debug, Clone)]
pub struct Page<T> {
    pub offset: u64,
    pub limit: u32,
    pub total: Option<u64>,
    pub data: Vec<T>
}

trait IntoPage<Item> {
    type Item;
    fn into_page(self, page_request: PageRequest) -> Page<Item>;
}

impl <T> IntoPage<T> for (Vec<T>, Option<i64>) {
    type Item = T;
    fn into_page(self, page_request: PageRequest) -> Page<T> {

        Page {
            offset: page_request.offset,
            limit: page_request.limit,
            total: self.1.map(|x| x as u64),
            data: self.0
        }
    }
}

#[derive(InsertTemplate, UpdateTemplate, QueryTemplate, DeleteTemplate, FromRow, TableName, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table_name = "users"]
#[tp_delete(by = "id")]
#[tp_delete(by = "id, email")]
#[tp_query_all(by = "id, email", order = "id desc")]
#[tp_query_one(by = "id", order = "id desc", fn_name = "get_last_inserted")]
#[tp_query_one(by = "email")]
#[tp_query_page(by = "org", order = "id desc, org desc")]
#[tp_query_count(by = "id, email")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
#[tp_query_stream(order = "id desc")]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub password: String,
    pub org: Option<i32>,
    pub active: bool,
    #[auto]
    pub version: i32,
    pub created_by: Option<String>,
    #[auto]
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}



#[derive(InsertTemplate, UpdateTemplate, QueryTemplate, DeleteTemplate, FromRow, TableName, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table_name = "chats"]
#[tp_delete(by = "id")]
#[tp_query_one(by = "id, sender", order = "id desc")]
pub struct Chat {
    #[auto]
    pub id: i32,
    pub sender: i32,
    pub receiver: i32,
    pub content: String,
    pub active: bool,
    pub created_by: Option<String>,
    #[auto]
    pub created_at: DateTime<Utc>,
}


#[derive(InsertTemplate, UpdateTemplate, QueryTemplate, DeleteTemplate, FromRow, TableName, Default, Clone, Debug)]
#[table_name = "organizations"]
#[tp_delete(by = "id")]
#[tp_query_one(by = "code")]
#[tp_query_all(order = "id desc")]
pub struct Organization {
    #[auto]
    pub id: i32,
    pub name: String,
    pub code: String,
    pub image: Option<String>,
    pub active: bool,
    pub created_by: Option<String>,
    #[auto]
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}


#[multi_query(file = "sql/init.sql", 0)]
async fn migrate() {}






