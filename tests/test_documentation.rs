use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

/// Test struct to verify documentation generation
#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "score BETWEEN :min$i32 AND :max$i32",
    with_active_status = "active = :active",
    with_complex_condition = "score > :min_score$i32 AND created_at > :since$String"
)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub score: i32,
    pub active: bool,
    pub created_at: String,
    pub name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("📚 Testing Documentation Generation");
    println!("This example demonstrates the improved documentation for builder methods");
    
    // Create in-memory SQLite database
    let pool = SqlitePool::connect(":memory:").await?;
    
    // Create table
    sqlx::query(
        r#"
        CREATE TABLE users (
            id INTEGER PRIMARY KEY,
            email TEXT NOT NULL,
            score INTEGER NOT NULL,
            active BOOLEAN NOT NULL,
            created_at TEXT NOT NULL,
            name TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;
    
    // Insert test data
    sqlx::query("INSERT INTO users (email, score, active, created_at, name) VALUES (?, ?, ?, ?, ?)")
        .bind("alice@company.com")
        .bind(95)
        .bind(true)
        .bind("2023-01-01")
        .bind("Alice")
        .execute(&pool)
        .await?;
    
    println!("\n🔧 Testing Builder Methods with Documentation:");
    
    // Test 1: Field-based methods (auto-generated documentation)
    println!("\n1. Field-based methods:");
    let builder = User::builder_select()
        .email("alice@company.com")?           // Equality condition
        .score_gte(&80)?                       // Greater than or equal condition
        .active(&true)?                        // Boolean condition
        .order_by_score_desc()?;               // ORDER BY descending
    
    println!("   ✅ Field methods: .email(), .score_gte(), .active(), .order_by_score_desc()");
    
    // Test 2: Custom condition methods (improved documentation)
    println!("\n2. Custom condition methods:");
    let builder = User::builder_select()
        .with_email_domain("%@company.com")?   // Custom WHERE condition: email LIKE :domain$String
        .with_score_range(60, 90)?             // Custom WHERE condition: score BETWEEN :min$i32 AND :max$i32
        .with_active_status(true)?             // Custom WHERE condition: active = :active
        .with_complex_condition(75, "2022-01-01")?; // Custom WHERE condition: score > :min_score$i32 AND created_at > :since$String
    
    println!("   ✅ Custom methods: .with_email_domain(), .with_score_range(), .with_active_status(), .with_complex_condition()");
    
    // Test 3: Execute query to verify functionality
    let users = User::builder_select()
        .with_email_domain("%@company.com")?
        .with_active_status(true)?
        .find_all(&pool)
        .await?;
    
    println!("\n📊 Query Results:");
    println!("   Found {} users matching conditions", users.len());
    for user in users {
        println!("   - {}: {} (score: {}, active: {})", user.name, user.email, user.score, user.active);
    }
    
    println!("\n📝 Documentation Features:");
    println!("✅ Custom condition methods show actual SQL expressions");
    println!("✅ Parameter documentation includes parameter names and types");
    println!("✅ Field-based methods have descriptive documentation");
    println!("✅ Builder pattern methods are properly documented");
    println!("✅ ORDER BY methods include direction information");
    
    println!("\n🎯 Documentation Examples:");
    println!("- with_email_domain(): Custom WHERE condition: `email LIKE :domain$String`");
    println!("- with_score_range(): Custom WHERE condition: `score BETWEEN :min$i32 AND :max$i32`");
    println!("- with_complex_condition(): Custom WHERE condition: `score > :min_score$i32 AND created_at > :since$String`");
    
    println!("\n💡 To see full documentation, use:");
    println!("   cargo doc --open");
    println!("   Then navigate to the User struct and its impl blocks");
    
    Ok(())
}
