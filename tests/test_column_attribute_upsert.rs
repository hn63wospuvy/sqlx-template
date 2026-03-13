/// Test #[column("name")] attribute with UpsertTemplate
/// Verifies that UPSERT (INSERT ON CONFLICT) uses custom column names.

use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

// Struct with #[column] and upsert
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("settings")]
#[tp_upsert(by = "key")]
pub struct Setting {
    #[column("setting_key")]
    pub key: String,
    #[column("setting_value")]
    pub value: String,
    #[column("last_updated")]
    pub updated_at: String,
}

// Struct with mixed #[column] and #[auto] for upsert
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("user_scores")]
#[tp_upsert(by = "user_id")]
pub struct UserScore {
    #[auto]
    pub id: i32,
    #[column("uid")]
    pub user_id: i32,
    #[column("display_name")]
    pub name: String,
    #[column("total_score")]
    pub score: i32,
}

// Struct with upsert and partial update (on)
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("inventory")]
#[tp_upsert(by = "sku", on = "quantity, unit_price")]
pub struct InventoryItem {
    #[column("item_sku")]
    pub sku: String,
    #[column("item_name")]
    pub name: String,
    #[column("item_qty")]
    pub quantity: i32,
    #[column("item_price")]
    pub unit_price: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing #[column] with UpsertTemplate ===\n");

    let pool = SqlitePool::connect(":memory:").await?;

    // Create tables with actual DB column names
    sqlx::query(
        r#"
        CREATE TABLE settings (
            setting_key TEXT PRIMARY KEY,
            setting_value TEXT NOT NULL,
            last_updated TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE user_scores (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            uid INTEGER UNIQUE NOT NULL,
            display_name TEXT NOT NULL,
            total_score INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE inventory (
            item_sku TEXT PRIMARY KEY,
            item_name TEXT NOT NULL,
            item_qty INTEGER NOT NULL,
            item_price INTEGER NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // ========= Test UPSERT with Setting =========
    println!("--- Testing UPSERT with Setting ---");

    let setting = Setting {
        key: "theme".to_string(),
        value: "dark".to_string(),
        updated_at: "2024-01-01".to_string(),
    };
    Setting::upsert_by_key(&setting, &pool).await?;
    println!("✅ Initial insert via upsert succeeded");

    // Verify
    let row: (String, String, String) = sqlx::query_as(
        "SELECT setting_key, setting_value, last_updated FROM settings WHERE setting_key = 'theme'"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "theme");
    assert_eq!(row.1, "dark");
    assert_eq!(row.2, "2024-01-01");
    println!("✅ Initial data correct in DB with custom column names");

    // Update via upsert
    let updated_setting = Setting {
        key: "theme".to_string(),
        value: "light".to_string(),
        updated_at: "2024-06-01".to_string(),
    };
    Setting::upsert_by_key(&updated_setting, &pool).await?;

    let row: (String, String, String) = sqlx::query_as(
        "SELECT setting_key, setting_value, last_updated FROM settings WHERE setting_key = 'theme'"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "theme");
    assert_eq!(row.1, "light");
    assert_eq!(row.2, "2024-06-01");
    println!("✅ Upsert update correctly uses setting_key, setting_value, last_updated columns");

    // Multiple settings
    let settings = vec![
        Setting { key: "lang".to_string(), value: "en".to_string(), updated_at: "2024-01-01".to_string() },
        Setting { key: "timezone".to_string(), value: "UTC".to_string(), updated_at: "2024-01-01".to_string() },
    ];
    for s in &settings {
        Setting::upsert_by_key(s, &pool).await?;
    }
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM settings")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count.0, 3);
    println!("✅ Multiple upserts work correctly");

    // ========= Test UPSERT with UserScore (#[auto] + #[column]) =========
    println!("\n--- Testing UPSERT with UserScore (#[auto] + #[column]) ---");

    let score = UserScore {
        id: 0, // auto
        user_id: 42,
        name: "Alice".to_string(),
        score: 100,
    };
    UserScore::upsert_by_user_id(&score, &pool).await?;

    let row: (i32, String, i32) = sqlx::query_as(
        "SELECT uid, display_name, total_score FROM user_scores WHERE uid = 42"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, 42);
    assert_eq!(row.1, "Alice");
    assert_eq!(row.2, 100);
    println!("✅ Upsert with #[auto] and #[column] correctly uses uid, display_name, total_score");

    // Update score via upsert
    let updated_score = UserScore {
        id: 0,
        user_id: 42,
        name: "Alice V2".to_string(),
        score: 200,
    };
    UserScore::upsert_by_user_id(&updated_score, &pool).await?;

    let row: (i32, String, i32) = sqlx::query_as(
        "SELECT uid, display_name, total_score FROM user_scores WHERE uid = 42"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, 42);
    assert_eq!(row.1, "Alice V2");
    assert_eq!(row.2, 200);
    println!("✅ Upsert update with #[auto] and #[column] works correctly");

    // ========= Test UPSERT with partial update (on) =========
    println!("\n--- Testing UPSERT with partial update (on) ---");

    let item = InventoryItem {
        sku: "WIDGET-001".to_string(),
        name: "Blue Widget".to_string(),
        quantity: 10,
        unit_price: 999,
    };
    InventoryItem::upsert_by_sku(&item, &pool).await?;

    let row: (String, String, i32, i32) = sqlx::query_as(
        "SELECT item_sku, item_name, item_qty, item_price FROM inventory WHERE item_sku = 'WIDGET-001'"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "WIDGET-001");
    assert_eq!(row.1, "Blue Widget");
    assert_eq!(row.2, 10);
    assert_eq!(row.3, 999);
    println!("✅ Initial insert via partial upsert works");

    // Update only quantity and unit_price (on = "quantity, unit_price")
    let updated_item = InventoryItem {
        sku: "WIDGET-001".to_string(),
        name: "Red Widget".to_string(), // This should NOT be updated
        quantity: 25,                    // This SHOULD be updated
        unit_price: 899,                // This SHOULD be updated
    };
    InventoryItem::upsert_by_sku(&updated_item, &pool).await?;

    let row: (String, String, i32, i32) = sqlx::query_as(
        "SELECT item_sku, item_name, item_qty, item_price FROM inventory WHERE item_sku = 'WIDGET-001'"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0, "WIDGET-001");
    assert_eq!(row.1, "Blue Widget"); // Unchanged!
    assert_eq!(row.2, 25); // Updated
    assert_eq!(row.3, 899); // Updated
    println!("✅ Partial upsert correctly updates only qty and price columns (using #[column] names)");

    // ========= Verify COLUMNS constants =========
    println!("\n--- Verifying COLUMNS constants ---");
    assert_eq!(Setting::COLUMNS, ["setting_key", "setting_value", "last_updated"]);
    assert_eq!(UserScore::COLUMNS, ["id", "uid", "display_name", "total_score"]);
    assert_eq!(InventoryItem::COLUMNS, ["item_sku", "item_name", "item_qty", "item_price"]);
    println!("✅ All COLUMNS constants correct for upsert structs");

    println!("\n🎉 All UpsertTemplate #[column] tests passed!");

    Ok(())
}
