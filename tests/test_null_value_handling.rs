use sqlx_template::{SqliteTemplate, sqlite_query};
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
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

// Create table using query macro
#[sqlite_query(
    r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        email TEXT NOT NULL,
        name TEXT NOT NULL,
        org INTEGER,  -- NULL allowed
        department TEXT,  -- NULL allowed
        score REAL,  -- NULL allowed
        active BOOLEAN NOT NULL
    )
    "#
)]
async fn create_users_table() {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing NULL Value Handling ===\n");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
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
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data with various NULL combinations
    println!("📊 Inserting test data...");
    
    // User 1: All fields have values
    sqlx::query("INSERT INTO users (email, name, org, department, score, active) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("user1@example.com")
        .bind("User 1")
        .bind(1)
        .bind("Engineering")
        .bind(95.5)
        .bind(true)
        .execute(&pool)
        .await?;
    
    // User 2: org is NULL
    sqlx::query("INSERT INTO users (email, name, org, department, score, active) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("user2@example.com")
        .bind("User 2")
        .bind(None::<i32>)
        .bind("Marketing")
        .bind(87.0)
        .bind(true)
        .execute(&pool)
        .await?;
    
    // User 3: department is NULL
    sqlx::query("INSERT INTO users (email, name, org, department, score, active) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("user3@example.com")
        .bind("User 3")
        .bind(2)
        .bind(None::<String>)
        .bind(92.3)
        .bind(false)
        .execute(&pool)
        .await?;
    
    // User 4: org and department are NULL
    sqlx::query("INSERT INTO users (email, name, org, department, score, active) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("user4@example.com")
        .bind("User 4")
        .bind(None::<i32>)
        .bind(None::<String>)
        .bind(78.9)
        .bind(true)
        .execute(&pool)
        .await?;
    
    // User 5: score is NULL
    sqlx::query("INSERT INTO users (email, name, org, department, score, active) VALUES (?, ?, ?, ?, ?, ?)")
        .bind("user5@example.com")
        .bind("User 5")
        .bind(3)
        .bind("Sales")
        .bind(None::<f64>)
        .bind(false)
        .execute(&pool)
        .await?;
    
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
    
    // Test 4: Derive macro functions with Option types (should accept unwrapped types)
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
    
    println!("\n✅ All NULL value handling tests passed!");
    
    Ok(())
}
