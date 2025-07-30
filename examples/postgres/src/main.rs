
use chrono::{DateTime, Utc};
use futures::StreamExt;
use sqlx_template::{insert, multi_query, postgres_delete, postgres_select, query, select, update, Columns, DeleteTemplate, PostgresTemplate, SelectTemplate, SqlxTemplate, TableName, UpdateTemplate, UpsertTemplate};
use sqlx::{migrate::MigrateDatabase, prelude::FromRow, types::{chrono, Json}, Sqlite, SqlitePool};
use sqlx_template::InsertTemplate;

mod test_null_handling;

const DB_URL: &str = "sqlite://sqlite.db";

use testcontainers_modules::{postgres, testcontainers::runners::AsyncRunner};


#[tokio::main]
async fn main() {

    const USERNAME: &str = "postgres";
    const PASSWORD: &str = "postgres";
    const DATABASE: &str = "sqlx";
    
    let container = postgres::Postgres::default()
    .with_user(USERNAME)
    .with_password(PASSWORD)
    .with_db_name(DATABASE)
    .start().await.unwrap();

    let host = container.get_host().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();

    let dsn = format!("postgresql://{USERNAME}:{PASSWORD}@{host}:{port}/{DATABASE}");

    let db = sqlx::PgPool::connect(&dsn).await.unwrap();
    println!("Setup db done");
    migrate(&db).await.unwrap();

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

    let user_4 = User { 
        email: format!("user4@abc.com"), 
        password: "password".into(), 
        org: None, 
        active: true, 
        ..Default::default()
    };

    // Insert user
    User::insert(&user_1, &db).await.unwrap();
    User::insert(&user_2, &db).await.unwrap();
    User::insert(&user_4, &db).await.unwrap();
    insert_new_user("user3@abc.com", "password", org_1.id, &db).await.unwrap();

    let org_page = User::find_page_by_org_order_by_id_desc_and_org_desc(&None, PageRequest::default(),  &db)
        .await.unwrap()
        .into_page(PageRequest::default())
        ;
    println!("Page Users with no org: {org_page:#?}");

    // Query user
    let users = query_all_user_info("user", 0, &db).await.unwrap();
    println!("Query Users: {users:#?}");

    let users = User::get_last_inserted(&1, &db).await.unwrap();
    println!("Last inserted user: {users:#?}");

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

    let user = User::find_one_by_group(&None, &db).await.unwrap();


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


    User::upsert_by_email(&user,  &mut *tx).await.unwrap();
    let mut user_updated_stream = User::update_user_returning(&user.id, &user, &mut *tx).await.unwrap();
    let mut user_updated_stream = User::update_user_returning_id(&user.id, &user, &mut *tx).await.unwrap();
    let mut user_updated_stream = User::update_user_returning_id_email(&user.id, &user, &mut *tx).await.unwrap();

    tx.commit().await.unwrap();

    
    let mut user_updated_stream = User::update_user_returning_stream(&user.id, &user, &db).await;
    while let Some(Ok(o)) = user_updated_stream.next().await {
        println!("Updated user: {o:#?}");
    }

    let user = User::find_one_by_email(&"user2@abc.com".to_string(), &db).await.unwrap().unwrap();
    println!("User after update: {user:#?}");

    // ========== BUILDER PATTERN EXAMPLES ==========
    println!("\n=== BUILDER PATTERN EXAMPLES ===");

    // Example 1: Simple find_all with single condition
    println!("\n1. Find all active users:");
    let active_users = User::builder_select()
        .active(&true).unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} active users", active_users.len());

    // Example 2: Find_all with multiple conditions
    println!("\n2. Find users with multiple conditions (active=true AND org=1):");
    let filtered_users = User::builder_select()
        .active(&true).unwrap()
        .org(&Some(1)).unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} users with org=1 and active=true", filtered_users.len());

    // Example 3: Find_all with string conditions
    println!("\n3. Find users by email pattern:");
    let email_users = User::builder_select()
        .email_like("%@abc.com").unwrap()
        .active(&true).unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} users with @abc.com email", email_users.len());

    // Example 4: Find_all with ordering
    println!("\n4. Find users ordered by ID descending:");
    let ordered_users = User::builder_select()
        .active(&true).unwrap()
        .order_by_id_desc().unwrap()
        .find_all(&db).await.unwrap();
    println!("Users ordered by ID desc:");
    for user in &ordered_users {
        println!("  - ID: {}, Email: {}", user.id, user.email);
    }

    // Example 5: Find_one with conditions
    println!("\n5. Find one user by email:");
    let single_user = User::builder_select()
        .email("user2@abc.com").unwrap()
        .active(&true).unwrap()
        .find_one(&db).await.unwrap();
    if let Some(user) = single_user {
        println!("Found user: {} (ID: {})", user.email, user.id);
    }

    // Example 6: Find_page with pagination
    println!("\n6. Paginated results (page 1, limit 2):");
    let page_result = User::builder_select()
        .active(&true).unwrap()
        .order_by_id_asc().unwrap()
        .find_page((0, 2, true), &db).await.unwrap(); // offset=0, limit=2, count=true
    println!("Page info: offset={}, limit=2, total={:?}", 0, page_result.1);
    println!("Users on page 1:");
    for user in &page_result.0 {
        println!("  - ID: {}, Email: {}", user.id, user.email);
    }

    // Example 7: Find_page second page
    println!("\n7. Paginated results (page 2, limit 2):");
    let page2_result = User::builder_select()
        .active(&true).unwrap()
        .order_by_id_asc().unwrap()
        .find_page((2, 2, true), &db).await.unwrap(); // offset=2, limit=2, count=true
    println!("Page info: offset=2, limit=2, total={:?}", page2_result.1);
    println!("Users on page 2:");
    for user in &page2_result.0 {
        println!("  - ID: {}, Email: {}", user.id, user.email);
    }

    // Example 8: Stream with conditions
    println!("\n8. Stream users with conditions:");
    let mut builder = User::builder_select()
        .active(&true).unwrap()
        .org(&Some(1)).unwrap()
        .order_by_email_asc().unwrap();
    let mut user_stream = builder.stream(&db).await;

    println!("Streaming users (active=true, org=1, ordered by email):");
    let mut count = 0;
    while let Some(user_result) = user_stream.next().await {
        match user_result {
            Ok(user) => {
                count += 1;
                println!("  Stream #{}: {} (ID: {})", count, user.email, user.id);
            }
            Err(e) => println!("Stream error: {}", e),
        }
    }

    // Example 9: Complex conditions with numeric comparisons
    println!("\n9. Find users with ID greater than 1:");
    let high_id_users = User::builder_select()
        .id_gt(&1).unwrap()
        .active(&true).unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} users with ID > 1", high_id_users.len());

    // Example 10: Count with conditions
    println!("\n10. Count users with conditions:");
    let user_count = User::builder_select()
        .active(&true).unwrap()
        .org(&Some(1)).unwrap()
        .count(&db).await.unwrap();
    println!("Total count of active users in org 1: {}", user_count);

    // Example 11: Multiple string conditions
    println!("\n11. Find users with email starting with 'user':");
    let prefix_users = User::builder_select()
        .email_start_with("user").unwrap()
        .active(&true).unwrap()
        .order_by_id_asc().unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} users with email starting with 'user'", prefix_users.len());

    // Example 12: Build SQL without executing
    println!("\n12. Build SQL query without executing:");
    let query_builder = User::builder_select()
        .active(&true).unwrap()
        .org(&Some(1)).unwrap()
        .email_like("%abc%").unwrap()
        .order_by_id_desc().unwrap();
    let sql = query_builder.build_sql();
    println!("Generated SQL: {}", sql);

    // Example 13: Update builder with conditions
    println!("\n13. Update users with builder pattern:");
    let update_result = User::builder_update()
        .on_version(&1).unwrap()  // SET version = 1
        .on_updated_by("admin").unwrap()  // SET updated_by = 'admin'
        .by_org(&Some(1)).unwrap()  // WHERE org = 1
        .by_active(&true).unwrap()  // WHERE active = true
        .execute(&db).await.unwrap();
    println!("Updated {} users", update_result);

    // Example 14: Delete builder with conditions
    println!("\n14. Delete inactive users (if any exist):");
    // First, let's create an inactive user for demo
    let inactive_user = User {
        email: "inactive@test.com".to_string(),
        password: "password".to_string(),
        org: Some(1),
        active: false,  // inactive
        ..Default::default()
    };
    let _ = User::insert(&inactive_user, &db).await;

    let delete_result = User::builder_delete()
        .active(&false).unwrap()  // WHERE active = false
        .email_like("%test.com").unwrap()  // WHERE email LIKE '%test.com'
        .execute(&db).await.unwrap();
    println!("Deleted {} inactive users", delete_result);

    // Example 15: Complex query with multiple conditions and ordering
    println!("\n15. Complex query - active users in org 1, ordered by email, limit 5:");
    let complex_users = User::builder_select()
        .active(&true).unwrap()
        .org(&Some(1)).unwrap()
        .id_gte(&1).unwrap()  // ID >= 1
        .email_end_with(".com").unwrap()  // email ends with .com
        .order_by_email_asc().unwrap()
        .order_by_id_desc().unwrap()  // secondary sort
        .find_page((0, 5, false), &db).await.unwrap();  // limit 5, no count
    println!("Complex query results:");
    for user in &complex_users.0 {
        println!("  - ID: {}, Email: {}, Org: {:?}", user.id, user.email, user.org);
    }

    // Example 16: Custom conditions - email domain filtering
    println!("\n16. Custom condition - email domain filtering:");
    let domain_users = User::builder_select()
        .active(&true).unwrap()
        .with_email_domain("@abc.com").unwrap()
        .order_by_id_asc().unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} users with @abc.com domain", domain_users.len());

    // Example 17: Custom condition - version range filtering
    println!("\n17. Custom condition - version range filtering:");
    let version_users = User::builder_select()
        .with_score_range(0, 2).unwrap()  // version BETWEEN 0 AND 2
        .active(&true).unwrap()
        .find_all(&db).await.unwrap();
    println!("Found {} users with version 0-2", version_users.len());

    // Example 18: Custom condition - active users in specific org
    println!("\n18. Custom condition - active users in specific org:");
    let org_users = User::builder_select()
        .with_active_org(1).unwrap()  // active = true AND org = 1
        .count(&db).await.unwrap();
    println!("Found {} active users in org 1", org_users);

    // Example 19: UPDATE with custom condition
    println!("\n19. UPDATE with custom condition:");
    let high_version_update = User::builder_update()
        .on_updated_by("system").unwrap()
        .with_high_version(0).unwrap()  // WHERE version > 0
        .execute(&db).await.unwrap();
    println!("Updated {} users with high version", high_version_update);

    // Example 20: DELETE with custom condition
    println!("\n20. DELETE with custom condition:");
    let max_version = 5;
    let deleted_old = User::builder_delete()
        .with_old_inactive(max_version).unwrap()  // active = false AND version < 5
        .execute(&db).await.unwrap();
    println!("Deleted {} old inactive users with version < {}", deleted_old, max_version);

    println!("\n=== PostgreSQL Builder Pattern Examples Completed! ===");
    println!("Note: PostgreSQL uses $1, $2, $3... placeholders (not ? like SQLite/MySQL)");

    // Test NULL value handling
    println!("\n{}", "=".repeat(50));
    println!("Running PostgreSQL NULL value handling tests...");
    println!("{}", "=".repeat(50));

    if let Err(e) = test_null_handling::test_null_value_handling().await {
        eprintln!("PostgreSQL NULL value handling test failed: {}", e);
    } else {
        println!("PostgreSQL NULL value handling tests completed successfully!");
    }
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

#[derive(PostgresTemplate, FromRow, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table("users")]
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "version BETWEEN :min$i32 AND :max$i32",
    with_active_org = "active = true AND org = :org_id$i32"
)]
#[tp_update_builder(
    with_high_version = "version > :threshold$i32"
)]
#[tp_delete_builder(
    with_old_inactive = "active = false AND version < :max_version$i32"
)]
#[tp_upsert(by = "id")]
#[tp_upsert(by = "email")]
#[tp_delete(by = "id")]
#[tp_delete(by = "id, email")]
#[tp_select_all(by = "id, email", order = "id desc")]
#[tp_select_one(by = "id", order = "id desc", fn_name = "get_last_inserted", where = "active = true AND id > :id")]
// #[tp_select_one(by = "id", order = "id desc", fn_name = "get_last_inserted")]
#[tp_select_one(by = "email")]
#[tp_select_one(by = "group")]
#[tp_select_page(by = "org", order = "id desc, org desc")]
#[tp_select_count(by = "id, email")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user_returning", returning = true)]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user_returning_id", returning = "id")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user_returning_id_email", returning = "id, email")]
#[tp_select_stream(order = "id desc")]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub password: String,
    pub org: Option<i32>,
    pub active: bool,
    pub group: Option<String>,
    #[auto]
    pub version: i32,
    pub created_by: Option<String>,
    #[auto]
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}





#[derive(SqlxTemplate, FromRow, Default, Clone, Debug, Columns)]
#[table("organizations")]
#[db("postgres")]
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
#[db("postgres")]
async fn migrate() {}


#[insert("INSERT INTO users(email, password, org, active, created_by, updated_by, updated_at) VALUES (:email, :password, :org, true, NULL, NULL, NULL)")]
#[db("postgres")]
async fn insert_new_user(email: &str, password: &str, org: i32) {}

#[select(
    sql = "
    SELECT *
    FROM users
    WHERE (email = :name and org = :org) OR email LIKE '%' || :name || '%'
",
    debug = 100
)]
#[db("postgres")]
pub async fn query_all_user_info(name: &str, org: i32) -> Vec<User> {}

#[select("
    SELECT organizations.id, organizations.name
    FROM organizations
    JOIN users ON users.org = organizations.id
    WHERE users.email LIKE '%' || :name || '%'
    GROUP BY organizations.id
")]
#[db("postgres")]
pub fn query_user_org(name: &str, org: i32) -> Stream<(i32, String)> {} // Stream does not need async because it return a future. `:org` does not need to appear in the query



#[postgres_select("
    SELECT organizations.id, organizations.name
    FROM organizations
    JOIN users ON users.org = organizations.id
    WHERE users.email LIKE '%' || :name || '%'
    GROUP BY organizations.id
")]
pub fn query_user_org1(name: &str, org: i32) -> Stream<(i32, String)> {} // Stream does not need async because it return a future. `:org` does not need to appear in the query







