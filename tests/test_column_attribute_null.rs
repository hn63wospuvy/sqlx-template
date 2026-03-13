/// Test #[column("name")] attribute with Option<T> fields (NULL handling)
/// Verifies that nullable columns work correctly with custom column names.

use sqlx_template::SqliteTemplate;
use sqlx::{FromRow, SqlitePool};

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("profiles")]
#[tp_select_one(by = "id")]
#[tp_select_all(by = "email")]
#[tp_select_all(by = "name")]
#[tp_update(by = "id")]
#[tp_select_builder]
pub struct Profile {
    #[auto]
    pub id: i32,
    pub email: String,
    #[column("display_name")]
    pub name: Option<String>,
    #[column("user_bio")]
    pub bio: Option<String>,
    #[column("avatar_url")]
    pub avatar: Option<String>,
    pub score: Option<i32>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Testing #[column] with Option<T> (NULL handling) ===\n");

    let pool = SqlitePool::connect(":memory:").await?;

    sqlx::query(
        r#"
        CREATE TABLE profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            email TEXT NOT NULL,
            display_name TEXT,
            user_bio TEXT,
            avatar_url TEXT,
            score INTEGER
        )
        "#,
    )
    .execute(&pool)
    .await?;

    // ========= Test INSERT with NULL values =========
    println!("--- Testing INSERT with NULL values ---");

    // All non-null
    let profile1 = Profile {
        id: 0,
        email: "alice@example.com".to_string(),
        name: Some("Alice".to_string()),
        bio: Some("Developer".to_string()),
        avatar: Some("https://example.com/alice.png".to_string()),
        score: Some(100),
    };
    Profile::insert(&profile1, &pool).await?;

    // Some fields NULL
    let profile2 = Profile {
        id: 0,
        email: "bob@example.com".to_string(),
        name: Some("Bob".to_string()),
        bio: None,          // NULL
        avatar: None,       // NULL
        score: Some(50),
    };
    Profile::insert(&profile2, &pool).await?;

    // All optional fields NULL
    let profile3 = Profile {
        id: 0,
        email: "charlie@example.com".to_string(),
        name: None,
        bio: None,
        avatar: None,
        score: None,
    };
    Profile::insert(&profile3, &pool).await?;

    println!("✅ INSERT with mixed NULL/non-NULL values succeeded");

    // Verify data in DB
    let row: (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT display_name, user_bio FROM profiles WHERE id = 2"
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(row.0.as_deref(), Some("Bob"));
    assert!(row.1.is_none());
    println!("✅ NULL values correctly stored with custom column names");

    // ========= Test SELECT with NULL fields =========
    println!("\n--- Testing SELECT with NULL fields ---");

    // find_one_by_id
    let alice = Profile::find_one_by_id(&1, &pool).await?.unwrap();
    assert_eq!(alice.name, Some("Alice".to_string()));
    assert_eq!(alice.bio, Some("Developer".to_string()));
    println!("✅ find_one_by_id correctly reads non-NULL #[column] fields");

    let bob = Profile::find_one_by_id(&2, &pool).await?.unwrap();
    assert_eq!(bob.name, Some("Bob".to_string()));
    assert!(bob.bio.is_none()); // user_bio is NULL
    assert!(bob.avatar.is_none()); // avatar_url is NULL
    assert_eq!(bob.score, Some(50));
    println!("✅ find_one_by_id correctly reads NULL #[column] fields");

    let charlie = Profile::find_one_by_id(&3, &pool).await?.unwrap();
    assert!(charlie.name.is_none());
    assert!(charlie.bio.is_none());
    assert!(charlie.avatar.is_none());
    assert!(charlie.score.is_none());
    println!("✅ find_one_by_id correctly reads all-NULL #[column] fields");

    // ========= Test UPDATE with NULL values =========
    println!("\n--- Testing UPDATE with NULL values ---");

    // Update bob's bio (from NULL to Some)
    let updated_bob = Profile {
        id: 2,
        email: "bob@example.com".to_string(),
        name: Some("Bob Updated".to_string()),
        bio: Some("Engineer".to_string()), // was NULL, now has value
        avatar: None,                       // stays NULL
        score: Some(75),
    };
    Profile::update_by_id(&2, &updated_bob, &pool).await?;

    let bob_after = Profile::find_one_by_id(&2, &pool).await?.unwrap();
    assert_eq!(bob_after.name, Some("Bob Updated".to_string()));
    assert_eq!(bob_after.bio, Some("Engineer".to_string()));
    assert!(bob_after.avatar.is_none());
    assert_eq!(bob_after.score, Some(75));
    println!("✅ UPDATE correctly changes NULL -> Some with custom column names");

    // Update alice's name to NULL
    let updated_alice = Profile {
        id: 1,
        email: "alice@example.com".to_string(),
        name: None, // was Some, now NULL
        bio: Some("Developer".to_string()),
        avatar: None, // was Some, now NULL
        score: None,  // was Some, now NULL
    };
    Profile::update_by_id(&1, &updated_alice, &pool).await?;

    let alice_after = Profile::find_one_by_id(&1, &pool).await?.unwrap();
    assert!(alice_after.name.is_none());
    assert_eq!(alice_after.bio, Some("Developer".to_string()));
    assert!(alice_after.avatar.is_none());
    assert!(alice_after.score.is_none());
    println!("✅ UPDATE correctly changes Some -> NULL with custom column names");

    // Verify directly in DB
    let row: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT display_name, user_bio, avatar_url FROM profiles WHERE id = 1"
    )
    .fetch_one(&pool)
    .await?;
    assert!(row.0.is_none()); // display_name
    assert_eq!(row.1.as_deref(), Some("Developer")); // user_bio
    assert!(row.2.is_none()); // avatar_url
    println!("✅ Raw SQL confirms NULL values in custom-named columns");

    // ========= Test Builder with NULL fields =========
    println!("\n--- Testing Builder with NULL #[column] fields ---");

    // Filter by non-NULL field
    let with_name = Profile::builder_select()
        .name(&Some("Bob Updated".to_string()))?
        .find_all(&pool)
        .await?;
    assert_eq!(with_name.len(), 1);
    assert_eq!(with_name[0].email, "bob@example.com");
    println!("✅ Builder filter on #[column] Option field with Some value");

    // Filter by NULL (IS NULL)
    let without_name = Profile::builder_select()
        .name(&None)?
        .find_all(&pool)
        .await?;
    assert_eq!(without_name.len(), 2); // alice and charlie
    println!("✅ Builder filter on #[column] Option field with None (IS NULL)");

    // Filter IS NOT NULL
    let with_any_name = Profile::builder_select()
        .name_not(&None)?
        .find_all(&pool)
        .await?;
    assert_eq!(with_any_name.len(), 1); // only bob
    println!("✅ Builder filter _not on #[column] Option field (IS NOT NULL)");

    // ========= Verify COLUMNS =========
    println!("\n--- Verifying COLUMNS ---");
    assert_eq!(
        Profile::COLUMNS,
        ["id", "email", "display_name", "user_bio", "avatar_url", "score"]
    );
    assert_eq!(
        Profile::COLUMNS_STR,
        "id, email, display_name, user_bio, avatar_url, score"
    );
    println!("✅ COLUMNS correct for struct with Option<T> and #[column]");

    println!("\n🎉 All NULL handling #[column] tests passed!");

    Ok(())
}
