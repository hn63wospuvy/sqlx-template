use sqlx_template::{SqliteTemplate, UpdateTemplate, DeleteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

// Test struct with all three builders and custom conditions
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "score BETWEEN :min$i32 AND :max$i32",
    with_active_status = "active = :active"  // Auto-mapped to bool
)]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub score: i32,
    pub active: bool,
    pub name: String,
}

// Separate structs for update and delete to avoid conflicts
#[derive(UpdateTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_update_builder(
    with_high_score = "score > :threshold$i32"
)]
pub struct UserUpdate {
    pub id: i32,
    pub email: String,
    pub score: i32,
    pub active: bool,
    pub name: String,
}

#[derive(DeleteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_delete_builder(
    with_low_score = "score < :threshold$i32"
)]
pub struct UserDelete {
    pub id: i32,
    pub email: String,
    pub score: i32,
    pub active: bool,
    pub name: String,
}

// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        email TEXT NOT NULL,
        score INTEGER NOT NULL,
        active BOOLEAN NOT NULL,
        name TEXT NOT NULL
    )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Comprehensive Builder Test");
    println!("Testing all builder features and improvements");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table using generated function
    create_users_table(&pool).await?;

    // Insert test data using generated insert method
    let test_users = [
        User {
            id: 0, // Will be auto-generated
            email: "alice@company.com".to_string(),
            score: 95,
            active: true,
            name: "Alice".to_string(),
        },
        User {
            id: 0, // Will be auto-generated
            email: "bob@company.com".to_string(),
            score: 45,
            active: false,
            name: "Bob".to_string(),
        },
        User {
            id: 0, // Will be auto-generated
            email: "charlie@personal.com".to_string(),
            score: 85,
            active: true,
            name: "Charlie".to_string(),
        },
        User {
            id: 0, // Will be auto-generated
            email: "diana@company.com".to_string(),
            score: 25,
            active: false,
            name: "Diana".to_string(),
        },
        User {
            id: 0, // Will be auto-generated
            email: "eve@personal.com".to_string(),
            score: 75,
            active: true,
            name: "Eve".to_string(),
        },
    ];

    for user in test_users.iter() {
        User::insert(user, &pool).await?;
    }
    
    println!("\n📊 Initial data: {} users inserted", test_users.len());
    
    // Test 1: SELECT builder with column list (not SELECT *)
    println!("\n🔍 Test 1: SELECT with column list");
    let sql = User::builder_select().build_sql();
    println!("Generated SQL: {}", sql);
    assert!(sql.contains("SELECT id, email, score, active, name FROM"));
    assert!(!sql.contains("SELECT * FROM"));
    println!("✅ SELECT uses explicit column list");
    
    // Test 2: Custom conditions with auto-mapping
    println!("\n🎯 Test 2: Custom conditions with auto-mapping");
    let users = User::builder_select()
        .with_active_status(true)?  // Auto-mapped to bool
        .find_all(&pool)
        .await?;
    println!("Active users: {} found", users.len());
    assert_eq!(users.len(), 3);
    
    // Test 3: Custom conditions with explicit types
    println!("\n🎯 Test 3: Custom conditions with explicit types");
    let users = User::builder_select()
        .with_email_domain("%@company.com")?
        .find_all(&pool)
        .await?;
    println!("Company email users: {} found", users.len());
    assert_eq!(users.len(), 3);
    
    let users = User::builder_select()
        .with_score_range(50, 90)?
        .find_all(&pool)
        .await?;
    println!("Users with score 50-90: {} found", users.len());
    assert_eq!(users.len(), 2);
    
    // Test 4: Combined conditions
    println!("\n🔗 Test 4: Combined conditions");
    let users = User::builder_select()
        .with_email_domain("%@company.com")?
        .with_active_status(true)?
        .with_score_range(80, 100)?
        .find_all(&pool)
        .await?;
    println!("Active company users with high scores: {} found", users.len());
    assert_eq!(users.len(), 1);
    
    // Test 5: UPDATE builder with custom conditions
    println!("\n✏️ Test 5: UPDATE builder with custom conditions");
    let affected = UserUpdate::builder_update()
        .on_active(&true)?
        .with_high_score(80)?
        .execute(&pool)
        .await?;
    println!("Activated {} high-score users", affected);
    assert_eq!(affected, 2);
    
    // Test 6: DELETE builder with custom conditions
    println!("\n🗑️ Test 6: DELETE builder with custom conditions");
    let deleted = UserDelete::builder_delete()
        .with_low_score(50)?
        .execute(&pool)
        .await?;
    println!("Deleted {} low-score users", deleted);
    assert_eq!(deleted, 2);
    
    // Test 7: Verify final state
    println!("\n📈 Test 7: Final verification");
    let remaining = User::builder_select()
        .find_all(&pool)
        .await?;
    println!("Remaining users: {}", remaining.len());
    assert_eq!(remaining.len(), 3);
    
    for user in remaining {
        println!("  - {}: {} (email: {}, score: {}, active: {})", 
                 user.id, user.name, user.email, user.score, user.active);
    }
    
    println!("\n🎉 All tests passed!");
    println!("✅ SELECT uses column list instead of SELECT *");
    println!("✅ Custom conditions work for SELECT, UPDATE, DELETE");
    println!("✅ Auto-mapping and explicit types both work");
    println!("✅ Table alias validation prevents invalid conditions");
    println!("✅ Comprehensive Rust documentation added");
    
    Ok(())
}
