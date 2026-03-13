/// Test #[column("name")] with $StructName template in raw queries.
/// Verifies that `SELECT $Struct FROM table` expands to correct column names.

use sqlx_template::{Columns, sqlite_query};
use sqlx::{FromRow, SqlitePool};

// Struct with #[column] attributes to test $StructName expansion
#[derive(Columns, FromRow, Debug, Clone)]
pub struct User {
    pub id: i32,
    pub email: String,
    #[column("q_name")]
    pub name: String,
    #[column("q_group")]
    pub group_name: String,
}

// Struct without #[column] for comparison
#[derive(Columns, FromRow, Debug, Clone)]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub price: i32,
}

// Raw query using $User template - should expand to: id, email, q_name, q_group
#[sqlite_query("SELECT $User FROM users WHERE id = :id")]
async fn find_user_by_id(id: i32) -> Vec<User> {}

// Raw query using $User template - select all
#[sqlite_query("SELECT $User FROM users")]
async fn find_all_users() -> Vec<User> {}

// Raw query using $Product template - should expand to: id, name, price
#[sqlite_query("SELECT $Product FROM products WHERE price > :min_price")]
async fn find_expensive_products(min_price: i32) -> Vec<Product> {}

// Raw query using $Product - select all
#[sqlite_query("SELECT $Product FROM products")]
async fn find_all_products() -> Vec<Product> {}

// Raw void queries for table setup
#[sqlite_query("CREATE TABLE IF NOT EXISTS users (id INTEGER PRIMARY KEY AUTOINCREMENT, email TEXT NOT NULL, q_name TEXT NOT NULL, q_group TEXT NOT NULL)")]
async fn create_users_table() {}

#[sqlite_query("CREATE TABLE IF NOT EXISTS products (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, price INTEGER NOT NULL)")]
async fn create_products_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing $StructName Template in Raw Queries ===\n");

    let pool = SqlitePool::connect(":memory:").await?;

    // Create tables
    create_users_table(&pool).await?;
    create_products_table(&pool).await?;

    // Insert test data using raw sqlx (to keep test focused on $Struct template)
    sqlx::query("INSERT INTO users (email, q_name, q_group) VALUES (?, ?, ?)")
        .bind("alice@example.com").bind("Alice").bind("admin")
        .execute(&pool).await?;
    sqlx::query("INSERT INTO users (email, q_name, q_group) VALUES (?, ?, ?)")
        .bind("bob@example.com").bind("Bob").bind("user")
        .execute(&pool).await?;
    sqlx::query("INSERT INTO users (email, q_name, q_group) VALUES (?, ?, ?)")
        .bind("charlie@example.com").bind("Charlie").bind("admin")
        .execute(&pool).await?;

    sqlx::query("INSERT INTO products (name, price) VALUES (?, ?)")
        .bind("Widget").bind(100)
        .execute(&pool).await?;
    sqlx::query("INSERT INTO products (name, price) VALUES (?, ?)")
        .bind("Gadget").bind(250)
        .execute(&pool).await?;
    sqlx::query("INSERT INTO products (name, price) VALUES (?, ?)")
        .bind("Doohickey").bind(50)
        .execute(&pool).await?;

    // ========= Test $User Template =========
    println!("--- Testing $User template ---");

    // Verify COLUMNS_STR which drives the expansion
    assert_eq!(User::COLUMNS_STR, "id, email, q_name, q_group");
    println!("✅ User::COLUMNS_STR = \"{}\"", User::COLUMNS_STR);

    // Test find_user_by_id - uses $User which should expand to: id, email, q_name, q_group
    let users = find_user_by_id(1, &pool).await?;
    assert_eq!(users.len(), 1);
    assert_eq!(users[0].id, 1);
    assert_eq!(users[0].email, "alice@example.com");
    assert_eq!(users[0].name, "Alice"); // maps from q_name
    assert_eq!(users[0].group_name, "admin"); // maps from q_group
    println!("✅ $User template correctly expands in SELECT ... WHERE query");

    // Test find_all_users
    let all_users = find_all_users(&pool).await?;
    assert_eq!(all_users.len(), 3);
    // Verify all fields are correctly mapped
    let alice = all_users.iter().find(|u| u.id == 1).unwrap();
    assert_eq!(alice.name, "Alice");
    assert_eq!(alice.group_name, "admin");
    let bob = all_users.iter().find(|u| u.id == 2).unwrap();
    assert_eq!(bob.name, "Bob");
    assert_eq!(bob.group_name, "user");
    println!("✅ $User template correctly expands in SELECT all query");

    // ========= Test $Product Template (no #[column]) =========
    println!("\n--- Testing $Product template (no #[column]) ---");

    assert_eq!(Product::COLUMNS_STR, "id, name, price");
    println!("✅ Product::COLUMNS_STR = \"{}\"", Product::COLUMNS_STR);

    let expensive = find_expensive_products(100, &pool).await?;
    assert_eq!(expensive.len(), 1);
    assert_eq!(expensive[0].name, "Gadget");
    assert_eq!(expensive[0].price, 250);
    println!("✅ $Product template correctly expands (no #[column])");

    let all_products = find_all_products(&pool).await?;
    assert_eq!(all_products.len(), 3);
    println!("✅ $Product template select all works");

    // ========= Verify cross-query data integrity =========
    println!("\n--- Verifying data integrity ---");

    // Make sure $Struct expansion doesn't interfere with regular queries
    let user_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await?;
    assert_eq!(user_count.0, 3);

    let product_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM products")
        .fetch_one(&pool)
        .await?;
    assert_eq!(product_count.0, 3);
    println!("✅ Data integrity verified");

    println!("\n🎉 All $StructName template tests passed!");

    Ok(())
}
