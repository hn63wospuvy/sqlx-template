
use chrono::{DateTime, Utc};
use futures::StreamExt;
use sqlx_template::{insert, multi_query, query, select, update, Columns, DeleteTemplate, SelectTemplate, SqliteTemplate, TableName, UpdateTemplate, UpsertTemplate};
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
    insert_new_user("user3@abc.com", "password", org_1.id, &db).await.unwrap();


    // Query user
    let users = query_all_user_info("user", 0, &db).await.unwrap();
    println!("Query Users: {users:#?}");

    // Stream all users order by id

    let mut users = User::stream_order_by_id_desc(&db);
    while let Some(Ok(u)) = users.next().await {
        println!("Stream User: {u:#?}");
    }

    // Stream org
    let mut org_list = query_user_org("user", 0, &db);
    while let Some(Ok(o)) = org_list.next().await {
        println!("Steam Org: {o:#?}");
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

    // Test upsert functionality
    let upsert_user = User {
        email: "user3@abc.com".to_string(),
        password: "upsert_password".to_string(),
        org: Some(org.id),
        active: true,
        updated_at: Some(Utc::now()),
        ..Default::default()
    };

    // This will insert since user3@abc.com doesn't exist
    User::upsert_by_email(&upsert_user, &mut *tx).await.unwrap();

    // This will update since user3@abc.com now exists
    let mut updated_upsert_user = upsert_user.clone();
    updated_upsert_user.password = "updated_upsert_password".to_string();
    User::upsert_by_email(&updated_upsert_user, &mut *tx).await.unwrap();

    tx.commit().await.unwrap();

    let user = User::find_one_by_email(&"user2@abc.com".to_string(), &db).await.unwrap().unwrap();
    println!("User after update: {user:#?}");

    let upserted_user = User::find_one_by_email(&"user3@abc.com".to_string(), &db).await.unwrap().unwrap();
    println!("Upserted user: {upserted_user:#?}");


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

#[derive(SqliteTemplate, FromRow, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table("users")]
#[tp_delete(by = "id")]
#[tp_delete(by = "id, email")]
#[tp_select_all(by = "id, email", order = "id desc")]
#[tp_select_one(by = "id", order = "id desc", fn_name = "get_last_inserted")]
#[tp_select_one(by = "email")]
#[tp_select_page(by = "org", order = "id desc, org desc")]
#[tp_select_count(by = "id, email")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
#[tp_select_stream(order = "id desc")]
#[tp_upsert(by = "email", update = "password, updated_at")]
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



#[derive(InsertTemplate, UpdateTemplate, SelectTemplate, DeleteTemplate, FromRow, TableName, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table("chats")]
#[db("sqlite")]
#[tp_delete(by = "id")]
#[tp_select_one(by = "id, sender", order = "id desc")]
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


#[derive(SqliteTemplate, FromRow, Default, Clone, Debug, Columns)]
#[table("organizations")]
#[tp_delete(by = "id")]
#[tp_select_one(by = "code")]
#[tp_select_all(order = "id desc")]
pub struct Organization {
    #[auto]
    pub id: i32,
    #[group = "a"]
    #[group = "b"]
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
#[db("sqlite")]
async fn migrate() {}


#[insert("INSERT INTO users(email, password, org, active, created_by, updated_by, updated_at) VALUES (:email, :password, :org, true, NULL, NULL, NULL)")]
#[db("sqlite")]
async fn insert_new_user(email: &str, password: &str, org: i32) {}

#[select(
    sql = "
    SELECT *
    FROM users
    WHERE (email = :name and org = :org) OR email LIKE '%' || :name || '%'
",
    debug = 100
)]
#[db("sqlite")]
pub async fn query_all_user_info(name: &str, org: i32) -> Vec<User> {}

#[select("
    SELECT organizations.id, organizations.name
    FROM organizations
    JOIN users ON users.org = organizations.id
    WHERE users.email LIKE '%' || :name || '%'
    GROUP BY organizations.id
")]
#[db("sqlite")]
pub fn query_user_org(name: &str, org: i32) -> Stream<(i32, String)> {} // Stream does not need async because it return a future. `:org` does not need to appear in the query

async fn setup_database(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Create users table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            age INTEGER NOT NULL,
            score REAL NOT NULL DEFAULT 0.0,
            active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create posts table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            published BOOLEAN NOT NULL DEFAULT 0,
            view_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY (user_id) REFERENCES users (id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_sample_data(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Insert sample users
    let users = vec![
        ("john@example.com", "John Doe", 25, 85.5),
        ("jane@gmail.com", "Jane Smith", 30, 92.0),
        ("bob@yahoo.com", "Bob Johnson", 22, 78.0),
        ("alice@gmail.com", "Alice Brown", 28, 88.5),
        ("charlie@example.com", "Charlie Wilson", 35, 95.0),
    ];

    for (email, name, age, score) in users {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO users (email, name, age, score, active, created_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(email)
        .bind(name)
        .bind(age)
        .bind(score)
        .bind(true)
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    }

    // Insert sample posts
    let posts = vec![
        (1, "Tech Trends 2024", "Content about tech trends", true, 150),
        (1, "Draft Post", "This is a draft", false, 0),
        (2, "Cooking Tips", "How to cook better", true, 75),
        (3, "Travel Guide", "Best places to visit", true, 200),
        (2, "Ready to Publish", "This post is ready", false, 5),
    ];

    for (user_id, title, content, published, view_count) in posts {
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO posts (user_id, title, content, published, view_count, created_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(user_id)
        .bind(title)
        .bind(content)
        .bind(published)
        .bind(view_count)
        .bind(now.to_rfc3339())
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn example_user_operations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("=== User Operations Examples ===");

    // 1. SELECT Examples
    println!("\n1. SELECT Examples:");

    // Find users by email
    let user = User::builder_select()
        .email("john@example.com")
        .find_one(pool)
        .await?;
    println!("User by email: {:?}", user);

    // Find active users older than 18
    let adult_users = User::builder_select()
        .active(true)
        .age_gt(18)
        .order_by_age_desc()
        .find_all(pool)
        .await?;
    println!("Adult active users: {} found", adult_users.len());

    // Find users with high scores
    let high_score_users = User::builder_select()
        .score_gte(85.0)
        .active(true)
        .order_by_score_desc()
        .order_by_name_asc()
        .find_all(pool)
        .await?;
    println!("High score users: {} found", high_score_users.len());

    // Find users by name pattern
    let users_with_john = User::builder_select()
        .name_like("%John%")
        .find_all(pool)
        .await?;
    println!("Users with 'John' in name: {} found", users_with_john.len());

    // Find users by email domain
    let gmail_users = User::builder_select()
        .email_end_with("@gmail.com")
        .active(true)
        .find_all(pool)
        .await?;
    println!("Gmail users: {} found", gmail_users.len());

    // 2. UPDATE Examples
    println!("\n2. UPDATE Examples:");

    // Update user score
    let updated_rows = User::builder_update()
        .on_score(95.5)
        .on_active(true)
        .by_email("john@example.com")
        .execute(pool)
        .await?;
    println!("Updated {} user(s) score", updated_rows);

    // Deactivate old users
    let deactivated_rows = User::builder_update()
        .on_active(false)
        .by_age_gt(65)
        .execute(pool)
        .await?;
    println!("Deactivated {} old user(s)", deactivated_rows);

    // 3. DELETE Examples
    println!("\n3. DELETE Examples:");

    // Delete inactive users
    let deleted_inactive = User::builder_delete()
        .active(false)
        .execute(pool)
        .await?;
    println!("Deleted {} inactive user(s)", deleted_inactive);

    Ok(())
}

async fn example_post_operations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("\n=== Post Operations Examples ===");

    // 1. SELECT Examples
    println!("\n1. SELECT Examples:");

    // Find published posts
    let published_posts = Post::builder_select()
        .published(true)
        .order_by_created_at_desc()
        .find_all(pool)
        .await?;
    println!("Published posts: {} found", published_posts.len());

    // Find popular posts
    let popular_posts = Post::builder_select()
        .view_count_gt(100)
        .published(true)
        .order_by_view_count_desc()
        .find_all(pool)
        .await?;
    println!("Popular posts: {} found", popular_posts.len());

    // Find posts by title pattern
    let tech_posts = Post::builder_select()
        .title_like("%Tech%")
        .published(true)
        .find_all(pool)
        .await?;
    println!("Tech posts: {} found", tech_posts.len());

    // Find posts by user
    let user_posts = Post::builder_select()
        .user_id(1)
        .order_by_created_at_desc()
        .find_all(pool)
        .await?;
    println!("Posts by user 1: {} found", user_posts.len());

    // 2. UPDATE Examples
    println!("\n2. UPDATE Examples:");

    // Publish draft posts
    let published_count = Post::builder_update()
        .on_published(true)
        .by_published(false)
        .by_title_like("%Ready%")
        .execute(pool)
        .await?;
    println!("Published {} draft post(s)", published_count);

    // 3. DELETE Examples
    println!("\n3. DELETE Examples:");

    // Delete unpublished old posts
    let old_date = DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let deleted_old = Post::builder_delete()
        .published(false)
        .created_at_lt(old_date)
        .execute(pool)
        .await?;
    println!("Deleted {} old unpublished post(s)", deleted_old);

    Ok(())
}

async fn example_complex_queries(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    println!("\n=== Complex Query Examples ===");

    // Complex user filtering
    let complex_users = User::builder_select()
        .active(true)
        .age_gte(21)
        .age_lte(65)
        .score_gt(50.0)
        .email_not("admin@example.com")
        .name_start_with("J")
        .order_by_score_desc()
        .order_by_age_asc()
        .find_all(pool)
        .await?;
    println!("Complex filtered users: {} found", complex_users.len());

    // Complex post filtering
    let complex_posts = Post::builder_select()
        .published(true)
        .view_count_gte(50)
        .view_count_lte(1000)
        .title_not("Draft")
        .content_like("%important%")
        .order_by_view_count_desc()
        .find_all(pool)
        .await?;
    println!("Complex filtered posts: {} found", complex_posts.len());

    // Bulk operations
    let bulk_update = User::builder_update()
        .on_score(75.0)
        .by_score_gte(70.0)
        .by_score_lt(80.0)
        .by_active(true)
        .execute(pool)
        .await?;
    println!("Bulk updated {} user(s) scores", bulk_update);

    Ok(())
}






