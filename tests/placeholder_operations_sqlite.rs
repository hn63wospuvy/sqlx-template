use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
// SELECT examples with different WHERE patterns
#[tp_select_one(by = "id", where = "name = :name and age > :min_age$i32")]
#[tp_select_all(where = "status = :status and score > :min_score$f64", fn_name = "find_by_status_and_score")]
#[tp_select_all(by = "active", where = "age BETWEEN :min_age$i32 AND :max_age$i32", fn_name = "find_active_by_age_range")]
#[tp_select_one(where = "email = :email and active = :active", fn_name = "find_by_email_and_status")]
#[tp_select_count(where = "score >= :threshold$f64 and status = :status", fn_name = "count_high_scorers")]

// UPDATE examples with different WHERE patterns
#[tp_update(by = "id", on = "name", where = "active = :active")]
#[tp_update(by = "email", on = "score, status", where = "updated_at < :cutoff$String", fn_name = "update_stale_users")]
#[tp_update(where = "age < :min_age$i32", on = "status", fn_name = "update_young_users_status")]

// DELETE examples with different WHERE patterns
#[tp_delete(where = "name = :name and created_at < :cutoff_date$String", fn_name = "delete")]
#[tp_delete(by = "id", where = "active = :active and score < :min_score$f64", fn_name = "delete_inactive_low_scorers")]
#[tp_delete(where = "status = :status and updated_at < :stale_date$String", fn_name = "cleanup_stale_records")]

// UPSERT examples with WHERE patterns
#[tp_upsert(by = "email", where = "updated_at > :min_update_time$String")]
#[tp_upsert(by = "id", where = "score < :max_score$f64", fn_name = "upsert_if_score_below")]
pub struct User {
    #[auto]
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: i32,
    pub status: String,
    pub score: f64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        email TEXT UNIQUE NOT NULL,
        age INTEGER NOT NULL,
        status TEXT NOT NULL,
        score REAL NOT NULL,
        active BOOLEAN NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table using query macro
    create_users_table(&pool).await?;
    
    // Insert test data using generated insert method
    let test_users = vec![
        User {
            id: 0, // Will be auto-generated
            name: "Alice".to_string(),
            email: "alice@example.com".to_string(),
            age: 25,
            status: "active".to_string(),
            score: 85.5,
            active: true,
            created_at: "2023-01-01".to_string(),
            updated_at: "2023-06-01".to_string(),
        },
        User {
            id: 0, // Will be auto-generated
            name: "Bob".to_string(),
            email: "bob@example.com".to_string(),
            age: 30,
            status: "inactive".to_string(),
            score: 92.0,
            active: false,
            created_at: "2023-02-01".to_string(),
            updated_at: "2023-05-01".to_string(),
        },
        User {
            id: 0, // Will be auto-generated
            name: "Charlie".to_string(),
            email: "charlie@example.com".to_string(),
            age: 35,
            status: "active".to_string(),
            score: 78.0,
            active: true,
            created_at: "2023-03-01".to_string(),
            updated_at: "2023-07-01".to_string(),
        },
    ];

    for user in test_users {
        User::insert(&user, &pool).await?;
    }
    
    println!("=== Testing Placeholder Operations with SQLite ===\n");

    // Test 1: SELECT with mixed placeholders
    println!("1. SELECT ONE with mixed placeholders:");
    println!("   - :name mapped to name column (String type from struct)");
    println!("   - :min_age$i32 custom type (not mapped to column)");

    let user = User::find_one_by_id(&1, "Alice", &20, &pool).await?;
    println!("   Found user: {:?}\n", user);

    // Test 2: SELECT ALL with custom types
    println!("2. SELECT ALL with custom placeholders:");
    println!("   - :status mapped to status column");
    println!("   - :min_score$f64 custom type");

    let users = User::find_by_status_and_score("active", &80.0, &pool).await?;
    println!("   Found {} active users with score > 80.0", users.len());
    for user in &users {
        println!("   - {}: {} (score: {})", user.name, user.status, user.score);
    }
    println!();

    // Test 3: SELECT with age range (by + where combination)
    println!("3. SELECT with BY + WHERE combination:");
    println!("   - by: active column");
    println!("   - where: age BETWEEN :min_age$i32 AND :max_age$i32");

    let users = User::find_active_by_age_range(&true, &20, &40, &pool).await?;
    println!("   Found {} active users aged 20-40", users.len());
    for user in &users {
        println!("   - {}: age {} (active: {})", user.name, user.age, user.active);
    }
    println!();

    // Test 4: SELECT with email and status
    println!("4. SELECT with email and status placeholders:");
    println!("   - :email mapped to email column");
    println!("   - :active mapped to active column");

    let user = User::find_by_email_and_status("alice@example.com", &true, &pool).await?;
    println!("   Found user: {:?}\n", user);

    // Test 5: COUNT with threshold
    println!("5. COUNT with custom threshold:");
    println!("   - :threshold$f64 custom type");
    println!("   - :status mapped to status column");

    let count = User::count_high_scorers(&85.0, "active", &pool).await?;
    println!("   Found {} active users with score >= 85.0\n", count);
    
    // Test 6: UPDATE with placeholder
    println!("6. UPDATE with BY + WHERE combination:");
    println!("   - by: id column");
    println!("   - on: name column");
    println!("   - where: :active mapped to active column");

    let rows_affected = User::update_by_id_on_name(&1, "Alice Updated", &true, &pool).await?;
    println!("   Updated {} rows", rows_affected);

    // Test 7: UPDATE stale users
    println!("7. UPDATE stale users:");
    println!("   - by: email column");
    println!("   - on: score, status columns");
    println!("   - where: :cutoff$String custom type");

    let rows_affected = User::update_stale_users("alice@example.com", &90.0, "premium", &"2023-01-01".to_string(), &pool).await?;
    println!("   Updated {} stale users", rows_affected);

    // Test 8: UPDATE young users status (WHERE only, no BY)
    println!("8. UPDATE young users (WHERE only):");
    println!("   - where: :min_age$i32 custom type");
    println!("   - on: status column");

    let rows_affected = User::update_young_users_status("junior", &30,  &pool).await?;
    println!("   Updated {} young users\n", rows_affected);
    
    // Test 9: DELETE with mixed placeholders (WHERE only)
    println!("9. DELETE with WHERE only:");
    println!("   - :name mapped to name column");
    println!("   - :cutoff_date$String custom type");

    let rows_affected = User::delete("Bob", &"2023-06-01".to_string(), &pool).await?;
    println!("   Deleted {} rows", rows_affected);

    // Test 10: DELETE inactive low scorers (BY + WHERE)
    println!("10. DELETE with BY + WHERE combination:");
    println!("   - by: id column");
    println!("   - where: :active mapped to active column, :min_score$f64 custom type");

    let rows_affected = User::delete_inactive_low_scorers(&2, &false, &70.0, &pool).await?;
    println!("   Deleted {} inactive low scorers", rows_affected);

    // Test 11: Cleanup stale records (WHERE only)
    println!("11. DELETE stale records:");
    println!("   - where: :status mapped to status column, :stale_date$String custom type");

    let rows_affected = User::cleanup_stale_records("inactive", &"2023-01-01".to_string(), &pool).await?;
    println!("   Cleaned up {} stale records\n", rows_affected);
    
    // Test 12: UPSERT with WHERE placeholder
    println!("12. UPSERT with WHERE placeholder:");
    println!("   - by: email column (conflict resolution)");
    println!("   - where: :min_update_time$String custom type in WHERE clause");

    let new_user = User {
        id: 0, // Will be auto-generated
        name: "David".to_string(),
        email: "david@example.com".to_string(),
        age: 28,
        status: "active".to_string(),
        score: 88.5,
        active: true,
        created_at: "2023-08-01".to_string(),
        updated_at: "2023-08-01".to_string(),
    };

    let rows_affected = User::upsert_by_email(&new_user, &"2023-01-01".to_string(), &pool).await?;
    println!("   Upserted {} rows", rows_affected);

    // Test 13: UPSERT with score condition
    println!("13. UPSERT with score condition:");
    println!("   - by: id column (conflict resolution)");
    println!("   - where: :max_score$f64 custom type");

    let update_user = User {
        id: 1,
        name: "Alice Premium".to_string(),
        email: "alice@example.com".to_string(),
        age: 26,
        status: "premium".to_string(),
        score: 95.0,
        active: true,
        created_at: "2023-01-01".to_string(),
        updated_at: "2023-08-15".to_string(),
    };

    let rows_affected = User::upsert_if_score_below(&update_user, &100.0, &pool).await?;
    println!("   Upserted {} rows (score condition)\n", rows_affected);
    
    // Verify final state using generated find_all method
    println!("14. Final state of users table:");
    let all_users = User::find_all(&pool).await?;

    for user in all_users {
        println!("   ID: {}, Name: {}, Email: {}, Age: {}, Status: {}, Score: {}, Active: {}",
                user.id, user.name, user.email, user.age, user.status, user.score, user.active);
    }

    println!("\n=== All 13 placeholder operation tests completed successfully! ===");
    println!("✅ SELECT: Column mapping, custom types, BY+WHERE, COUNT");
    println!("✅ UPDATE: BY+WHERE, WHERE only, multiple columns");
    println!("✅ DELETE: WHERE only, BY+WHERE combinations");
    println!("✅ UPSERT: WHERE conditions with custom types");
    
    Ok(())
}
