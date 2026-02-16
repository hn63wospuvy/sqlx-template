// Test instrument attribute cho raw macros
use sqlx::{FromRow, SqlitePool};
use sqlx_template::{query, select, insert, multi_query};

#[derive(FromRow, Debug)]
struct User {
    id: i32,
    email: String,
    active: bool,
}

// Test 1: Default - should have skip_all when tracing enabled
#[query("SELECT * FROM users WHERE email = :email")]
#[db("sqlite")]
async fn find_by_email(email: &str) -> Vec<User> {}

// Test 2: Override with string
#[select("SELECT * FROM users WHERE active = :active", instrument = "skip(conn)")]
#[db("sqlite")]
async fn find_active(active: &bool) -> Vec<User> {}

// Test 3: Override with true
#[insert("INSERT INTO users(email, active) VALUES (:email, :active)", instrument = true)]
#[db("sqlite")]
async fn create_user(email: &str, active: &bool) {}

// Test 4: Override with false (disable tracing)
#[query("SELECT COUNT(*) FROM users", instrument = false)]
#[db("sqlite")]
async fn count_users() -> Scalar<i64> {}

// Test 5: multi_query with instrument
#[multi_query("CREATE TABLE IF NOT EXISTS users(id INTEGER PRIMARY KEY, email TEXT, active BOOLEAN)", 0, instrument = "skip(conn)")]
#[db("sqlite")]
async fn init_db() {}

#[tokio::main]
async fn main() {
    println!("Instrument attribute test compile successful!");
}
