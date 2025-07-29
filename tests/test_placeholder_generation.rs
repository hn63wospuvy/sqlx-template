use sqlx_template::{SqliteTemplate, PostgresTemplate};
use sqlx::FromRow;

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder]
pub struct UserSqlite {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[derive(PostgresTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder]
pub struct UserPostgres {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[test]
fn test_placeholder_generation() {
    println!("=== Testing Placeholder Generation ===");

    // Test SQLite (should use ? placeholders)
    let sqlite_builder = UserSqlite::builder_select()
        .name("John").unwrap()
        .email("john@example.com").unwrap();

    let sqlite_sql = sqlite_builder.build_sql();
    println!("SQLite SQL: {}", sqlite_sql);

    // Test PostgreSQL (should use $1, $2 placeholders)
    let postgres_builder = UserPostgres::builder_select()
        .name("John").unwrap()
        .email("john@example.com").unwrap();

    let postgres_sql = postgres_builder.build_sql();
    println!("PostgreSQL SQL: {}", postgres_sql);

    // Verify placeholders
    assert!(sqlite_sql.contains("?"), "SQLite should use ? placeholders");
    assert!(postgres_sql.contains("$1") && postgres_sql.contains("$2"),
            "PostgreSQL should use $1, $2 placeholders. Actual: {}", postgres_sql);
}
