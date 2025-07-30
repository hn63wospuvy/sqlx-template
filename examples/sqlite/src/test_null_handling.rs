use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone, Default)]
#[table("users")]
#[tp_select_builder]
#[tp_select_page(by = "org", order = "id desc, org desc")]
#[tp_select_one(by = "id")]
#[tp_select_all(by = "email")]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub name: String,
    pub org: Option<i32>,  // This can be NULL in database
    pub department: Option<String>,  // This can be NULL in database
    pub score: Option<f64>,  // This can be NULL in database
    pub active: bool,
}

// Create table using query macro (following priority rules)
#[sqlite_query(
    r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        email TEXT NOT NULL,
        name TEXT NOT NULL,
        org INTEGER,
        department TEXT,
        score REAL,
        active BOOLEAN NOT NULL
    )
    "#
)]
async fn create_users_table() {}

pub async fn test_null_value_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing NULL Value Handling ===\n");

    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;

    // Create table using query macro instead of direct sqlx::query
    create_users_table(&pool).await?;
    
    // Insert test data with various NULL combinations using derive macro (highest priority)
    println!("📊 Inserting test data...");

    // User 1: All fields have values
    let user1 = User {
        email: "user1@example.com".to_string(),
        name: "User 1".to_string(),
        org: Some(1),
        department: Some("Engineering".to_string()),
        score: Some(95.5),
        active: true,
        ..Default::default()
    };
    User::insert(&user1, &pool).await?;

    // User 2: org is NULL
    let user2 = User {
        email: "user2@example.com".to_string(),
        name: "User 2".to_string(),
        org: None,
        department: Some("Marketing".to_string()),
        score: Some(87.0),
        active: true,
        ..Default::default()
    };
    User::insert(&user2, &pool).await?;

    // User 3: department is NULL
    let user3 = User {
        email: "user3@example.com".to_string(),
        name: "User 3".to_string(),
        org: Some(2),
        department: None,
        score: Some(92.3),
        active: false,
        ..Default::default()
    };
    User::insert(&user3, &pool).await?;

    // User 4: org and department are NULL
    let user4 = User {
        email: "user4@example.com".to_string(),
        name: "User 4".to_string(),
        org: None,
        department: None,
        score: Some(78.9),
        active: true,
        ..Default::default()
    };
    User::insert(&user4, &pool).await?;

    // User 5: score is NULL
    let user5 = User {
        email: "user5@example.com".to_string(),
        name: "User 5".to_string(),
        org: Some(3),
        department: Some("Sales".to_string()),
        score: None,
        active: false,
        ..Default::default()
    };
    User::insert(&user5, &pool).await?;
    
    println!("✅ Inserted 5 test users with various NULL combinations\n");

    // Test 1: Builder pattern with Option types - NULL equality
    println!("🔍 Test 1: Builder pattern with Option types - NULL equality");

    println!("  Testing org = None (should find users with NULL org):");
    let users_with_null_org = User::builder_select()
        .org(&None)?
        .find_all(&pool)
        .await?;
    println!("  Found {} users with NULL org", users_with_null_org.len());
    for user in &users_with_null_org {
        println!("    - {}: {} (org: {:?})", user.id, user.name, user.org);
    }
    assert_eq!(users_with_null_org.len(), 2, "Should find 2 users with NULL org");
    
    println!("  Testing department = None (should find users with NULL department):");
    let users_with_null_dept = User::builder_select()
        .department(&None)?
        .find_all(&pool)
        .await?;
    println!("  Found {} users with NULL department", users_with_null_dept.len());
    for user in &users_with_null_dept {
        println!("    - {}: {} (department: {:?})", user.id, user.name, user.department);
    }
    assert_eq!(users_with_null_dept.len(), 2, "Should find 2 users with NULL department");
    
    println!("  Testing score = None (should find users with NULL score):");
    let users_with_null_score = User::builder_select()
        .score(&None)?
        .find_all(&pool)
        .await?;
    println!("  Found {} users with NULL score", users_with_null_score.len());
    for user in &users_with_null_score {
        println!("    - {}: {} (score: {:?})", user.id, user.name, user.score);
    }
    assert_eq!(users_with_null_score.len(), 1, "Should find 1 user with NULL score");
    
    // Test 2: Builder pattern with Option types - non-NULL equality
    println!("\n🔍 Test 2: Builder pattern with Option types - non-NULL equality");
    
    println!("  Testing org = Some(1) (should find users with org = 1):");
    let users_with_org_1 = User::builder_select()
        .org(&Some(1))?
        .find_all(&pool)
        .await?;
    println!("  Found {} users with org = 1", users_with_org_1.len());
    for user in &users_with_org_1 {
        println!("    - {}: {} (org: {:?})", user.id, user.name, user.org);
    }
    assert_eq!(users_with_org_1.len(), 1, "Should find 1 user with org = 1");
    
    println!("  Testing department = Some('Engineering') (should find users with department = 'Engineering'):");
    let users_with_eng_dept = User::builder_select()
        .department(&Some("Engineering".to_string()))?
        .find_all(&pool)
        .await?;
    println!("  Found {} users with department = 'Engineering'", users_with_eng_dept.len());
    for user in &users_with_eng_dept {
        println!("    - {}: {} (department: {:?})", user.id, user.name, user.department);
    }
    assert_eq!(users_with_eng_dept.len(), 1, "Should find 1 user with department = 'Engineering'");
    
    // Test 3: Builder pattern with Option types - NOT NULL
    println!("\n🔍 Test 3: Builder pattern with Option types - NOT NULL");
    
    println!("  Testing org_not = None (should find users with org IS NOT NULL):");
    let users_with_non_null_org = User::builder_select()
        .org_not(&None)?
        .find_all(&pool)
        .await?;
    println!("  Found {} users with org IS NOT NULL", users_with_non_null_org.len());
    for user in &users_with_non_null_org {
        println!("    - {}: {} (org: {:?})", user.id, user.name, user.org);
    }
    assert_eq!(users_with_non_null_org.len(), 3, "Should find 3 users with org IS NOT NULL");
    
    // Test 4: Derive macro functions with Option types (highest priority - derive macro)
    println!("\n🔍 Test 4: Derive macro functions with Option types");

    // Note: According to the fix, tp_select_page(by = "org") should generate a function
    // that accepts &i32 (not &Option<i32>) for non-NULL cases only
    println!("  Testing find_page_by_org_order_by_id_desc_and_org_desc with org = 1:");

    // Create a simple PageRequest since we don't have access to the full one
    #[derive(Debug, Clone)]
    pub struct PageRequest {
        pub page: i64,
        pub size: i32,
        pub count: bool,
    }

    impl Default for PageRequest {
        fn default() -> Self {
            Self {
                page: 0,
                size: 10,
                count: false,
            }
        }
    }

    impl Into<(i64, i32, bool)> for PageRequest {
        fn into(self) -> (i64, i32, bool) {
            (self.page, self.size, self.count)
        }
    }

    let page_result = User::find_page_by_org_order_by_id_desc_and_org_desc(
        &1,
        PageRequest::default(),
        &pool
    ).await?;
    println!("  Found {} users in page with org = 1", page_result.0.len());
    for user in &page_result.0 {
        println!("    - {}: {} (org: {:?})", user.id, user.name, user.org);
    }
    assert_eq!(page_result.0.len(), 1, "Should find 1 user with org = 1");

    // Test 5: Using derive macro for single record lookup (highest priority)
    println!("\n🔍 Test 5: Derive macro for single record lookup");

    println!("  Testing find_one_by_id with id = 1:");
    let user_by_id = User::find_one_by_id(&1, &pool).await?;
    if let Some(user) = user_by_id {
        println!("    - Found user: {} (org: {:?})", user.name, user.org);
        assert_eq!(user.name, "User 1");
        assert_eq!(user.org, Some(1));
    } else {
        panic!("Should find user with id = 1");
    }

    // Test 6: Using derive macro for finding all by email (highest priority)
    println!("\n🔍 Test 6: Derive macro for finding all by email");

    println!("  Testing find_all_by_email with email = 'user1@example.com':");
    let users_by_email = User::find_all_by_email("user1@example.com", &pool).await?;
    println!("    - Found {} users with email 'user1@example.com'", users_by_email.len());
    for user in &users_by_email {
        println!("      - {}: {} (email: {})", user.id, user.name, user.email);
    }
    assert_eq!(users_by_email.len(), 1, "Should find 1 user with email 'user1@example.com'");
    
    println!("\n✅ All NULL value handling tests passed!");
    println!("📋 Summary of techniques used (following priority rules):");
    println!("   1. ✅ Derive macros (#[tp_insert], #[tp_select_*]) - HIGHEST PRIORITY");
    println!("   2. ✅ Builder pattern (User::builder_select()) - MEDIUM PRIORITY");
    println!("   3. ✅ Query macros (#[sqlite_query]) - LOW PRIORITY");
    println!("   4. ❌ Direct sqlx::query - NEVER USED (following rules)");

    Ok(())
}
