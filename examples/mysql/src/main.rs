use chrono::{DateTime, Utc};
use futures::StreamExt;
use sqlx_template::{insert, multi_query, mysql_delete, mysql_select, query, select, update, Columns, DeleteTemplate, MysqlTemplate, SelectTemplate, SqlxTemplate, TableName, UpdateTemplate, UpsertTemplate};
use sqlx::{prelude::FromRow, types::{chrono, Json}, MySql, MySqlPool};
use sqlx_template::InsertTemplate;
use testcontainers_modules::{mysql, testcontainers::{runners::AsyncRunner, ImageExt}};


#[tokio::main]
async fn main() {

    const USERNAME: &str = "root";
    const PASSWORD: &str = "password";
    const DATABASE: &str = "testdb";

    // Start MySQL container
    let mysql_container = mysql::Mysql::default()
        .with_env_var("MYSQL_ROOT_PASSWORD", PASSWORD)
        .with_env_var("MYSQL_DATABASE", DATABASE)
        .start()
        .await
        .expect("Failed to start MySQL container");

    let host = mysql_container.get_host().await.expect("Failed to get host");
    let port = mysql_container.get_host_port_ipv4(3306).await.expect("Failed to get port");

    let connection_string = format!(
        "mysql://{}:{}@{}:{}/{}",
        USERNAME, PASSWORD, host, port, DATABASE
    );

    println!("Connecting to MySQL: {}", connection_string);

    // Connect to MySQL
    let db = MySqlPool::connect(&connection_string).await.unwrap();
    
    // Run migration script
    migrate(&db).await.unwrap();
    println!("Setup db done");

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

    // Find one user by group (should be None since no group is set)
    let user = User::find_one_by_group(&None, &db).await.unwrap();

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


    User::upsert_by_email(&user,  &mut *tx).await.unwrap();
    let mut user_updated_stream = User::update_user_returning(&user.id, &user, &mut *tx).await.unwrap();
    let mut user_updated_stream = User::update_user_returning_id(&user.id, &user, &mut *tx).await.unwrap();
    let mut user_updated_stream = User::update_user_returning_id_email(&user.id, &user, &mut *tx).await.unwrap();

    tx.commit().await.unwrap();

    
    // Note: update_user_returning_stream doesn't exist, using update_user_returning instead
    let updated_user = User::update_user_returning(&user.id, &user, &db).await.unwrap();
    println!("Updated user: {updated_user:#?}");

    let user = User::find_one_by_email(&"user2@abc.com".to_string(), &db).await.unwrap().unwrap();
    println!("User after update: {user:#?}");

    // ========== BUILDER PATTERN EXAMPLES ==========
    println!("\n=== MYSQL BUILDER PATTERN EXAMPLES ===");

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
    let org_ref = Some(1);
    let mut builder = User::builder_select()
        .active(&true).unwrap()
        .org(&org_ref).unwrap()
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
    let org_ref2 = Some(1);
    let query_builder = User::builder_select()
        .active(&true).unwrap()
        .org(&org_ref2).unwrap()
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

    // Example 18: Custom condition - recent activity
    println!("\n18. Custom condition - recent activity:");
    let recent_cutoff = "2025-01-01 00:00:00";
    let recent_users = User::builder_select()
        .with_recent_activity(recent_cutoff, recent_cutoff).unwrap()  // updated_since, created_since
        .active(&true).unwrap()
        .count(&db).await.unwrap();
    println!("Found {} users with recent activity since {}", recent_users, recent_cutoff);

    // Example 19: UPDATE with custom condition
    println!("\n19. UPDATE with custom condition:");
    let high_version_update = User::builder_update()
        .on_updated_by("system").unwrap()
        .with_high_version(0).unwrap()  // WHERE version > 0
        .execute(&db).await.unwrap();
    println!("Updated {} users with high version", high_version_update);

    // Example 20: DELETE with custom condition
    println!("\n20. DELETE with custom condition:");
    // First create an old inactive user for demo
    let old_user = User {
        email: "old@inactive.com".to_string(),
        password: "password".to_string(),
        org: Some(1),
        active: false,
        created_at: chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc),
        ..Default::default()
    };
    let _ = User::insert(&old_user, &db).await;

    let old_cutoff = "2021-01-01 00:00:00";
    let deleted_old = User::builder_delete()
        .with_old_inactive(old_cutoff).unwrap()
        .execute(&db).await.unwrap();
    println!("Deleted {} old inactive users before {}", deleted_old, old_cutoff);

    println!("\n=== MySQL Builder Pattern Examples Completed! ===");
    println!("Note: MySQL uses ? placeholders (like SQLite, not $1, $2 like PostgreSQL)");

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

#[derive(SqlxTemplate, FromRow, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table("users")]
#[db("mysql")]
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "version BETWEEN :min$i32 AND :max$i32",
    with_recent_activity = "updated_at > :updated_since$String OR created_at > :created_since$String"
)]
#[tp_update_builder(
    with_high_version = "version > :threshold$i32"
)]
#[tp_delete_builder(
    with_old_inactive = "active = false AND created_at < :cutoff$String"
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

#[derive(SqlxTemplate, FromRow, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table("chats")]
#[db("mysql")]
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

impl Chat {
    fn test() {

    }
}

#[derive(SqlxTemplate, FromRow, Default, Clone, Debug, Columns)]
#[table("organizations")]
#[db("mysql")]
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
#[db("mysql")]
async fn migrate() {}


#[insert("INSERT INTO users(email, password, org, active, created_by, updated_by, updated_at) VALUES (:email, :password, :org, true, NULL, NULL, NULL)")]
#[db("mysql")]
async fn insert_new_user(email: &str, password: &str, org: i32) {}

#[select(
    sql = "
    SELECT *
    FROM users
    WHERE (email = :name and org = :org) OR email LIKE CONCAT('%', :name, '%')
",
    debug = 100
)]
#[db("mysql")]
pub async fn query_all_user_info(name: &str, org: i32) -> Vec<User> {}

#[select("
    SELECT organizations.id, organizations.name
    FROM organizations
    JOIN users ON users.org = organizations.id
    WHERE users.email LIKE CONCAT('%', :name, '%')
    GROUP BY organizations.id
")]
#[db("mysql")]
pub fn query_user_org(name: &str, org: i32) -> Stream<(i32, String)> {} // Stream does not need async because it return a future. `:org` does not need to appear in the query

#[select("
    SELECT organizations.id, organizations.name
    FROM organizations
    JOIN users ON users.org = organizations.id
    WHERE users.email LIKE CONCAT('%', :name, '%')
    GROUP BY organizations.id
")]
#[db("mysql")]
pub fn query_user_org1(name: &str, org: i32) -> Stream<(i32, String)> {} // Stream does not need async because it return a future. `:org` does not need to appear in the query
