use sqlx_template::UpdateTemplate;
use sqlx::FromRow;

#[derive(UpdateTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("postgres")]
#[tp_update_builder]
pub struct User {
    pub id: i32,
    pub email: String,
    pub score: i32,
}

#[test]
fn test_postgres_placeholder_order() {
    // Test 1: SET then WHERE - should be $1, $2
    let result = User::builder_update()
        .on_score(&95);
    assert!(result.is_ok());
    let builder = result.unwrap();
    
    let result = builder.by_id(&1);
    assert!(result.is_ok());
    let builder = result.unwrap();
    
    let sql = builder.build_sql();
    println!("Test 1 SQL: {}", sql);
    assert_eq!(sql, "UPDATE users SET score = $1 WHERE id = $2");
    
    // Test 2: Multiple SET, then multiple WHERE
    let result = User::builder_update()
        .on_score(&95);
    assert!(result.is_ok());
    let builder = result.unwrap();
    
    let result = builder.on_email("test@example.com");
    assert!(result.is_ok());
    let builder = result.unwrap();
    
    let result = builder.by_id(&1);
    assert!(result.is_ok());
    let builder = result.unwrap();
    
    let result = builder.by_score_gte(&80);
    assert!(result.is_ok());
    let builder = result.unwrap();
    
    let sql = builder.build_sql();
    println!("Test 2 SQL: {}", sql);
    assert_eq!(sql, "UPDATE users SET score = $1, email = $2 WHERE id = $3 AND score >= $4");
}
