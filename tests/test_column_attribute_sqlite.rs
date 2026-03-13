/// Test #[column("name")] attribute with SqliteTemplate
/// Verifies that SELECT, INSERT, UPDATE, DELETE all use custom column names.

use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

// Struct with #[column] attribute - maps Rust field names to DB column names
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_all(by = "id")]
#[tp_select_all(by = "email")]
#[tp_select_one(by = "id")]
#[tp_delete(by = "id")]
#[tp_update(by = "id")]
#[tp_update(by = "id", on = "name, group_name")]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    #[column("q_name")]
    pub name: String,
    #[column("q_group")]
    pub group_name: String,
    pub score: i32,
}

// Struct without #[column] for comparison
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("products")]
#[tp_select_all(by = "id")]
#[tp_select_one(by = "id")]
#[tp_delete(by = "id")]
#[tp_update(by = "id")]
pub struct Product {
    #[auto]
    pub id: i32,
    pub name: String,
    pub price: i32,
}

// Struct with all fields having #[column] attribute
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("metrics")]
#[tp_select_all(by = "id")]
#[tp_update(by = "id")]
pub struct Metric {
    #[auto]
    #[column("metric_id")]
    pub id: i32,
    #[column("metric_name")]
    pub name: String,
    #[column("metric_value")]
    pub value: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing #[column] attribute with SqliteTemplate ===\n");

    let pool = SqlitePool::connect(":memory:").await?;

    // Create tables with DB column names (q_name, q_group for users)
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL,
            q_name TEXT NOT NULL,
            q_group TEXT NOT NULL,
            score INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE products (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            price INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE metrics (
            metric_id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_name TEXT NOT NULL,
            metric_value INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // ========= Test COLUMNS constants =========
    println!("--- Testing COLUMNS constants ---");
    
    assert_eq!(User::COLUMNS, ["id", "email", "q_name", "q_group", "score"]);
    assert_eq!(User::COLUMNS_STR, "id, email, q_name, q_group, score");
    println!("✅ User::COLUMNS correct");

    assert_eq!(Product::COLUMNS, ["id", "name", "price"]);
    assert_eq!(Product::COLUMNS_STR, "id, name, price");
    println!("✅ Product::COLUMNS correct (no #[column])");

    assert_eq!(Metric::COLUMNS, ["metric_id", "metric_name", "metric_value"]);
    assert_eq!(Metric::COLUMNS_STR, "metric_id, metric_name, metric_value");
    println!("✅ Metric::COLUMNS correct (all #[column])");

    // ========= Test INSERT =========
    println!("\n--- Testing INSERT with #[column] ---");
    
    let user = User {
        id: 0,
        email: "alice@example.com".to_string(),
        name: "Alice".to_string(),
        group_name: "admin".to_string(),
        score: 95,
    };
    User::insert(&user, &pool).await?;
    println!("✅ INSERT succeeded with #[column] mapping");

    // Verify data in DB using raw SQL with actual column names
    let row: (String, String, String, i32) = sqlx::query_as(
        "SELECT email, q_name, q_group, score FROM users WHERE id = 1"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "alice@example.com");
    assert_eq!(row.1, "Alice"); // q_name column
    assert_eq!(row.2, "admin"); // q_group column
    assert_eq!(row.3, 95);
    println!("✅ INSERT correctly used q_name and q_group columns");

    // Insert more test data
    let user2 = User {
        id: 0,
        email: "bob@example.com".to_string(),
        name: "Bob".to_string(),
        group_name: "user".to_string(),
        score: 80,
    };
    User::insert(&user2, &pool).await?;

    let user3 = User {
        id: 0,
        email: "charlie@example.com".to_string(),
        name: "Charlie".to_string(),
        group_name: "admin".to_string(),
        score: 70,
    };
    User::insert(&user3, &pool).await?;

    // ========= Test SELECT =========
    println!("\n--- Testing SELECT with #[column] ---");

    // find_by_id
    let found = User::find_by_id(&1, &pool).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Alice");
    assert_eq!(found[0].group_name, "admin");
    println!("✅ find_by_id correctly maps q_name -> name, q_group -> group_name");

    // find_by_email
    let found = User::find_by_email(&"bob@example.com".to_string(), &pool).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Bob");
    assert_eq!(found[0].group_name, "user");
    println!("✅ find_by_email works with #[column] mapping");

    // find_one_by_id
    let found = User::find_one_by_id(&2, &pool).await?;
    assert!(found.is_some());
    let bob = found.unwrap();
    assert_eq!(bob.name, "Bob");
    assert_eq!(bob.group_name, "user");
    println!("✅ find_one_by_id works with #[column] mapping");

    // find_all
    let all = User::find_all(&pool).await?;
    assert_eq!(all.len(), 3);
    println!("✅ find_all returns 3 users with #[column] mapping");

    // ========= Test UPDATE =========
    println!("\n--- Testing UPDATE with #[column] ---");
    
    // update_by_id - updates all non-key fields
    let updated_user = User {
        id: 1,
        email: "alice_updated@example.com".to_string(),
        name: "Alice Updated".to_string(),
        group_name: "superadmin".to_string(),
        score: 100,
    };
    User::update_by_id(&1, &updated_user, &pool).await?;
    
    // Verify update using raw SQL
    let row: (String, String, String, i32) = sqlx::query_as(
        "SELECT email, q_name, q_group, score FROM users WHERE id = 1"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "alice_updated@example.com");
    assert_eq!(row.1, "Alice Updated"); // q_name updated
    assert_eq!(row.2, "superadmin"); // q_group updated
    assert_eq!(row.3, 100);
    println!("✅ update_by_id correctly uses #[column] mapping for SET clause");

    // update_by_id_on_group_name_and_name - partial update (fields sorted alphabetically)
    User::update_by_id_on_group_name_and_name(&2, &"Bob Renamed".to_string(), &"moderator".to_string(), &pool).await?;
    
    let row: (String, String, String, i32) = sqlx::query_as(
        "SELECT email, q_name, q_group, score FROM users WHERE id = 2"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "bob@example.com"); // unchanged
    assert_eq!(row.1, "Bob Renamed"); // q_name updated
    assert_eq!(row.2, "moderator"); // q_group updated
    assert_eq!(row.3, 80); // unchanged
    println!("✅ update_by_id_on_group_name_and_name correctly uses #[column] mapping");

    // ========= Test DELETE =========
    println!("\n--- Testing DELETE with #[column] ---");
    
    let count_before = User::find_all(&pool).await?.len();
    User::delete_by_id(&3, &pool).await?;
    let count_after = User::find_all(&pool).await?.len();
    assert_eq!(count_before - 1, count_after);
    println!("✅ delete_by_id works with #[column] mapping");

    // Verify remaining data still accessible
    let remaining = User::find_all(&pool).await?;
    assert_eq!(remaining.len(), 2);
    for user in &remaining {
        assert!(user.id == 1 || user.id == 2);
    }
    println!("✅ Remaining users correctly fetched with #[column] mapping");

    // ========= Test Product (without #[column]) =========
    println!("\n--- Testing Product without #[column] ---");
    
    let product = Product {
        id: 0,
        name: "Widget".to_string(),
        price: 999,
    };
    Product::insert(&product, &pool).await?;
    
    let found = Product::find_by_id(&1, &pool).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "Widget");
    assert_eq!(found[0].price, 999);
    println!("✅ Product without #[column] still works normally");

    // ========= Test Metric (all #[column]) =========
    println!("\n--- Testing Metric with all #[column] ---");
    
    let metric = Metric {
        id: 0,
        name: "cpu_usage".to_string(),
        value: 85,
    };
    Metric::insert(&metric, &pool).await?;
    
    let found = Metric::find_by_id(&1, &pool).await?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].name, "cpu_usage");
    assert_eq!(found[0].value, 85);
    println!("✅ Metric with all #[column] attributes works correctly");

    // Update metric
    let updated_metric = Metric {
        id: 1,
        name: "cpu_usage_peak".to_string(),
        value: 99,
    };
    Metric::update_by_id(&1, &updated_metric, &pool).await?;
    
    let row: (String, i32) = sqlx::query_as(
        "SELECT metric_name, metric_value FROM metrics WHERE metric_id = 1"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "cpu_usage_peak");
    assert_eq!(row.1, 99);
    println!("✅ Metric update correctly uses metric_name, metric_value columns");

    println!("\n🎉 All SqliteTemplate #[column] tests passed!");

    Ok(())
}
