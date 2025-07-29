use sqlx_template::{SqliteTemplate, PostgresTemplate};
use sqlx::FromRow;

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_age_range = "age BETWEEN :min_age$i32 AND :max_age$i32"
)]
pub struct UserSqlite {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: i32,
}

#[derive(PostgresTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_age_range = "age BETWEEN :min_age$i32 AND :max_age$i32"
)]
pub struct UserPostgres {
    pub id: i32,
    pub name: String,
    pub email: String,
    pub age: i32,
}

#[test]
fn test_custom_conditions_placeholders() {
    // Test SQLite with custom conditions
    let sqlite_builder = UserSqlite::builder_select()
        .name("John").unwrap()
        .with_email_domain("@company.com").unwrap()
        .with_age_range(25, 65).unwrap();
    let sqlite_sql = sqlite_builder.build_sql();
    println!("SQLite with custom conditions: {}", sqlite_sql);
    assert!(sqlite_sql.contains("?"), "SQLite should use ? placeholders");

    // Test PostgreSQL with custom conditions
    let postgres_builder = UserPostgres::builder_select()
        .name("John").unwrap()
        .with_email_domain("@company.com").unwrap()
        .with_age_range(25, 65).unwrap();
    let postgres_sql = postgres_builder.build_sql();
    println!("PostgreSQL with custom conditions: {}", postgres_sql);
    assert!(postgres_sql.contains("$1") && postgres_sql.contains("$2") && 
            postgres_sql.contains("$3") && postgres_sql.contains("$4"), 
            "PostgreSQL should use $1, $2, $3, $4 placeholders");
}

#[test]
fn test_mixed_conditions_placeholders() {
    // Test with both field conditions and custom conditions
    let postgres_builder = UserPostgres::builder_select()
        .name("John").unwrap()
        .age(&30).unwrap()
        .with_email_domain("@company.com").unwrap();
    let postgres_sql = postgres_builder.build_sql();
    println!("PostgreSQL mixed conditions: {}", postgres_sql);
    assert!(postgres_sql.contains("$1") && postgres_sql.contains("$2") && postgres_sql.contains("$3"), 
            "PostgreSQL should use $1, $2, $3 placeholders for mixed conditions");
}
