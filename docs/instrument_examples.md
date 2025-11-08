# Ví dụ sử dụng instrument attribute với sqlx-template

## Tổng quan

Khi feature `tracing` được bật, tất cả các hàm được sinh ra bởi macros sẽ tự động có `#[instrument(name = "{fn_name}", skip_all)]`.

Bạn có thể override hành vi mặc định này bằng cách thêm attribute `instrument`.

---

## 1. Derive Macros (tp_*)

### Crate thường (KHÔNG có tracing feature)

```rust
// Cargo.toml
[dependencies]
sqlx-template = "0.1"  // Không bật feature tracing
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }
tokio = { version = "1", features = ["full"] }

// main.rs
use sqlx::{FromRow, SqlitePool};
use sqlx_template::SqliteTemplate;

/// Không có tracing - các hàm được sinh ra KHÔNG có #[instrument]
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_all(by = "active")]
#[tp_select_one(by = "id")]
#[tp_update(by = "id", on = "email")]
#[tp_delete(by = "id")]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub active: bool,
}

#[tokio::main]
async fn main() {
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    
    // Các hàm này hoạt động bình thường nhưng KHÔNG có tracing
    let users = User::find_all_by_active(&true, &db).await.unwrap();
    let user = User::find_one_by_id(&1, &db).await.unwrap();
}
```

### Crate có tracing feature

```rust
// Cargo.toml
[dependencies]
sqlx-template = { version = "0.1", features = ["tracing"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio"] }
tokio = { version = "1", features = ["full"] }
tracing = "0.1"  // PHẢI có dependency này
tracing-subscriber = "0.3"

// main.rs
use sqlx::{FromRow, SqlitePool};
use sqlx_template::SqliteTemplate;

/// Example 1: Tracing mặc định - TẤT CẢ hàm có #[instrument(name = "{fn_name}", skip_all)]
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_all(by = "active")]           // -> #[instrument(name = "find_all_by_active", skip_all)]
#[tp_select_one(by = "id")]               // -> #[instrument(name = "find_one_by_id", skip_all)]
#[tp_update(by = "id", on = "email")]     // -> #[instrument(name = "update_by_id_on_email", skip_all)]
#[tp_delete(by = "id")]                   // -> #[instrument(name = "delete_by_id", skip_all)]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub active: bool,
}

/// Example 2: Override instrument attribute - tùy chỉnh tracing behavior
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("products")]
#[tp_select_all(by = "category", instrument = "skip(conn)")]  
// -> #[instrument(name = "find_all_by_category", skip(conn))]

#[tp_select_one(by = "id", instrument = true)]
// -> #[instrument(name = "find_one_by_id")]

#[tp_update(by = "id", on = "name", instrument = "skip(category, conn)")]
// -> #[instrument(name = "update_by_id_on_name", skip(category, conn))]
pub struct Product {
    #[auto]
    pub id: i32,
    pub name: String,
    pub category: String,
}

#[tokio::main]
async fn main() {
    // Khởi tạo tracing subscriber
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();
    
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    
    // TRACE find_all_by_active
    let users = User::find_all_by_active(&true, &db).await.unwrap();
    
    // TRACE find_one_by_id
    let user = User::find_one_by_id(&1, &db).await.unwrap();
    
    // TRACE find_all_by_category category="electronics"
    let products = Product::find_all_by_category("electronics", &db).await.unwrap();
}
```

---

## 2. Raw Macros (query, select, insert, update, delete, multi_query)

### Crate thường (KHÔNG tracing)

```rust
// Cargo.toml
[dependencies]
sqlx-template = "0.1"  // Không bật feature tracing

// main.rs
use sqlx::{FromRow, SqlitePool};
use sqlx_template::{query, select, insert, update, delete, multi_query};

#[derive(FromRow, Debug)]
struct User {
    id: i32,
    email: String,
    active: bool,
}

// Example 1: query macro - không có tracing
#[query("SELECT * FROM users WHERE email = :email")]
#[db("sqlite")]
async fn find_by_email(email: &str) -> Vec<User> {}

// Example 2: insert macro - không có tracing
#[insert("INSERT INTO users(email, active) VALUES (:email, :active)")]
#[db("sqlite")]
async fn create_user(email: &str, active: &bool) {}

#[tokio::main]
async fn main() {
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    
    // Tất cả hàm này hoạt động bình thường nhưng KHÔNG có tracing
    let users = find_by_email("test@example.com", &db).await.unwrap();
    create_user("new@example.com", &true, &db).await.unwrap();
}
```

### Crate có tracing

```rust
// Cargo.toml
[dependencies]
sqlx-template = { version = "0.1", features = ["tracing"] }
tracing = "0.1"
tracing-subscriber = "0.3"

// main.rs
use sqlx::{FromRow, SqlitePool};
use sqlx_template::{query, select, insert, update, delete, multi_query};

#[derive(FromRow, Debug)]
struct User {
    id: i32,
    email: String,
    active: bool,
}

// Example 1: Mặc định - auto skip_all
#[query("SELECT * FROM users WHERE email = :email")]
#[db("sqlite")]
async fn find_by_email(email: &str) -> Vec<User> {}
// -> #[tracing::instrument(name = "find_by_email", skip_all)]

// Example 2: Override - skip chỉ conn
#[select("SELECT * FROM users WHERE active = :active", instrument = "skip(conn)")]
#[db("sqlite")]
async fn find_active_users(active: &bool) -> Vec<User> {}
// -> #[tracing::instrument(name = "find_active_users", skip(conn))]

// Example 3: Override - log tất cả parameters
#[insert("INSERT INTO users(email, active) VALUES (:email, :active)", instrument = true)]
#[db("sqlite")]
async fn create_user(email: &str, active: &bool) {}
// -> #[tracing::instrument(name = "create_user")]

// Example 4: Custom skip parameters
#[update("UPDATE users SET active = :active WHERE email = :email", instrument = "skip(email, conn)")]
#[db("sqlite")]
async fn update_user_status(email: &str, active: &bool) {}
// -> #[tracing::instrument(name = "update_user_status", skip(email, conn))]

// Example 5: multi_query với instrument
#[multi_query(file = "sql/init.sql", 0, instrument = "skip(conn)")]
#[db("sqlite")]
async fn migrate() {}
// -> #[tracing::instrument(name = "migrate", skip(conn))]

// Example 6: Page return type
#[query("SELECT * FROM users WHERE active = :active", instrument = true)]
#[db("sqlite")]
async fn find_users_page(active: &bool) -> Page<User> {}
// -> #[tracing::instrument(name = "find_users_page")]

// Example 7: Scalar return type
#[query("SELECT COUNT(*) FROM users WHERE active = :active", instrument = "skip(conn)")]
#[db("sqlite")]
async fn count_active(active: &bool) -> Scalar<i64> {}
// -> #[tracing::instrument(name = "count_active", skip(conn)")]

// Example 8: Stream return type
#[select("SELECT * FROM users ORDER BY id", instrument = true)]
#[db("sqlite")]
fn stream_all_users() -> Stream<User> {}
// -> #[tracing::instrument(name = "stream_all_users")]

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .init();
    
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    
    // TRACE find_by_email
    let users = find_by_email("test@example.com", &db).await.unwrap();
    
    // TRACE find_active_users active=true
    let active = find_active_users(&true, &db).await.unwrap();
    
    // TRACE create_user email="new@example.com" active=true
    create_user("new@example.com", &true, &db).await.unwrap();
    
    // TRACE update_user_status active=false
    update_user_status("test@example.com", &false, &db).await.unwrap();
}
```

---

## 3. Builder Pattern với Tracing

```rust
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("orders")]
#[tp_select_builder(
    with_status = "status = :status$String",
    with_high_total = "total > :min_total$f64"
)]
#[tp_update_builder(
    with_old_orders = "created_at < :cutoff$String"
)]
#[tp_delete_builder(
    with_cancelled = "status = 'cancelled'"
)]
pub struct Order {
    #[auto]
    pub id: i32,
    pub status: String,
    pub total: f64,
}

// Tất cả builder methods tự động có tracing:
// - find_all() -> #[instrument(name = "find_all", skip_all)]
// - find_one() -> #[instrument(name = "find_one", skip_all)]
// - find_page() -> #[instrument(name = "find_page", skip_all)]
// - count() -> #[instrument(name = "count", skip_all)]
// - stream() -> #[instrument(name = "stream", skip_all)]
// - execute() -> #[instrument(name = "execute", skip_all)]

#[tokio::main]
async fn main() {
    let db = SqlitePool::connect("sqlite::memory:").await.unwrap();
    
    // TRACE find_all
    let orders = Order::builder_select()
        .with_status("pending").unwrap()
        .with_high_total(100.0).unwrap()
        .find_all(&db).await.unwrap();
    
    // TRACE execute
    let updated = Order::builder_update()
        .on_status("shipped").unwrap()
        .with_old_orders("2024-01-01").unwrap()
        .execute(&db).await.unwrap();
}
```

---

## 4. So sánh

| Feature | Derive macro (tp_*) | Raw macro (query/...) |
|---------|---------------------|----------------------|
| Syntax | `#[tp_select_one(by = "id", instrument = "skip(conn)")]` | `#[query("...", instrument = "skip(conn)")]` |
| Default | ✅ skip_all | ✅ skip_all |
| Override | ✅ Có | ✅ Có |
| Function name | Auto generated | User defined |

---

## 5. Best Practices

### ✅ DO

```rust
// 1. Mặc định cho queries thông thường (auto skip_all bảo vệ sensitive data)
#[tp_select_all(by = "active")]
#[query("SELECT * FROM sensitive_data WHERE user_id = :user_id")]

// 2. Override chỉ khi cần debug specific parameters
#[tp_update(by = "id", on = "status", instrument = "skip(conn)")]
#[query("...", instrument = "skip(conn)")]

// 3. Enable full logging cho critical operations
#[tp_delete(where = "created_at < :cutoff", fn_name = "cleanup", instrument = true)]
#[insert("...", instrument = true)]
```

### ❌ DON'T

```rust
// 1. Đừng log database connection (quá nhiều noise)
#[tp_select_all(by = "id", instrument = true)]  // Sẽ log conn parameter
#[query("...", instrument = true)]  // Sẽ log conn

// 2. Đừng override khi không cần thiết
#[query("SELECT COUNT(*) FROM logs", instrument = "skip(conn)")]
// -> Dùng default skip_all là đủ
```

---

## 6. Expanded Code Examples

### Khi feature `tracing` KHÔNG được bật:

```rust
// Input (derive macro)
#[tp_select_one(by = "id")]

// Expanded
pub async fn find_one_by_id<'c, E>(id: &i32, conn: E) -> Result<Option<User>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    // Không có #[instrument] ở đây
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await
}
```

```rust
// Input (raw macro)
#[query("SELECT * FROM users WHERE id = :id")]
async fn find_user(id: &i32) -> Option<User> {}

// Expanded
pub async fn find_user<'c, E>(id: &i32, conn: E) -> Result<Option<User>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    // Không có #[instrument] ở đây
    let sql = "SELECT * FROM users WHERE id = ?";
    sqlx::query_as::<_, User>(sql)
        .bind(id)
        .fetch_optional(conn)
        .await
}
```

### Khi feature `tracing` ĐƯỢC bật:

```rust
// Input (derive macro)
#[tp_select_one(by = "id")]

// Expanded
#[tracing::instrument(name = "find_one_by_id", skip_all)]
pub async fn find_one_by_id<'c, E>(id: &i32, conn: E) -> Result<Option<User>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await
}
```

```rust
// Input (raw macro)
#[query("SELECT * FROM users WHERE id = :id")]
async fn find_user(id: &i32) -> Option<User> {}

// Expanded
#[tracing::instrument(name = "find_user", skip_all)]
pub async fn find_user<'c, E>(id: &i32, conn: E) -> Result<Option<User>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    let sql = "SELECT * FROM users WHERE id = ?";
    sqlx::query_as::<_, User>(sql)
        .bind(id)
        .fetch_optional(conn)
        .await
}
```

### Khi có override:

```rust
// Input (derive macro)
#[tp_select_one(by = "id", instrument = "skip(conn)")]

// Expanded
#[tracing::instrument(name = "find_one_by_id", skip(conn))]
pub async fn find_one_by_id<'c, E>(id: &i32, conn: E) -> Result<Option<User>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    // id parameter sẽ được log, conn sẽ bị skip
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(conn)
        .await
}
```

```rust
// Input (raw macro)
#[query("SELECT * FROM users WHERE id = :id", instrument = "skip(conn)")]
async fn find_user(id: &i32) -> Option<User> {}

// Expanded
#[tracing::instrument(name = "find_user", skip(conn))]
pub async fn find_user<'c, E>(id: &i32, conn: E) -> Result<Option<User>, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
{
    // id parameter sẽ được log, conn sẽ bị skip
    let sql = "SELECT * FROM users WHERE id = ?";
    sqlx::query_as::<_, User>(sql)
        .bind(id)
        .fetch_optional(conn)
        .await
}
```
