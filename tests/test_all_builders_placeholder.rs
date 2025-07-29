use sqlx_template::{SqliteTemplate, PostgresTemplate};
use sqlx::FromRow;

#[derive(SqliteTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder]
#[tp_update_builder]
#[tp_delete_builder]
pub struct UserSqlite {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[derive(PostgresTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[tp_select_builder]
#[tp_update_builder]
#[tp_delete_builder]
pub struct UserPostgres {
    pub id: i32,
    pub name: String,
    pub email: String,
}

#[test]
fn test_select_builder_placeholders() {
    // Test SQLite SELECT
    let sqlite_builder = UserSqlite::builder_select()
        .name("John").unwrap()
        .email("john@example.com").unwrap();
    let sqlite_sql = sqlite_builder.build_sql();
    println!("SQLite SELECT: {}", sqlite_sql);
    assert!(sqlite_sql.contains("?"), "SQLite SELECT should use ? placeholders");

    // Test PostgreSQL SELECT
    let postgres_builder = UserPostgres::builder_select()
        .name("John").unwrap()
        .email("john@example.com").unwrap();
    let postgres_sql = postgres_builder.build_sql();
    println!("PostgreSQL SELECT: {}", postgres_sql);
    assert!(postgres_sql.contains("$1") && postgres_sql.contains("$2"), 
            "PostgreSQL SELECT should use $1, $2 placeholders");
}

#[test]
fn test_update_builder_placeholders() {
    // Test SQLite UPDATE
    let sqlite_builder = UserSqlite::builder_update()
        .on_name("John").unwrap()
        .on_email("john@example.com").unwrap()
        .by_id(&1).unwrap();
    let sqlite_sql = sqlite_builder.build_sql();
    println!("SQLite UPDATE: {}", sqlite_sql);
    assert!(sqlite_sql.contains("?"), "SQLite UPDATE should use ? placeholders");

    // Test PostgreSQL UPDATE
    let postgres_builder = UserPostgres::builder_update()
        .on_name("John").unwrap()
        .on_email("john@example.com").unwrap()
        .by_id(&1).unwrap();
    let postgres_sql = postgres_builder.build_sql();
    println!("PostgreSQL UPDATE: {}", postgres_sql);
    assert!(postgres_sql.contains("$1") && postgres_sql.contains("$2") && postgres_sql.contains("$3"), 
            "PostgreSQL UPDATE should use $1, $2, $3 placeholders");
}

#[test]
fn test_delete_builder_placeholders() {
    // Test SQLite DELETE
    let sqlite_builder = UserSqlite::builder_delete()
        .id(&1).unwrap()
        .name("John").unwrap();
    let sqlite_sql = sqlite_builder.build_sql();
    println!("SQLite DELETE: {}", sqlite_sql);
    assert!(sqlite_sql.contains("?"), "SQLite DELETE should use ? placeholders");

    // Test PostgreSQL DELETE
    let postgres_builder = UserPostgres::builder_delete()
        .id(&1).unwrap()
        .name("John").unwrap();
    let postgres_sql = postgres_builder.build_sql();
    println!("PostgreSQL DELETE: {}", postgres_sql);
    assert!(postgres_sql.contains("$1") && postgres_sql.contains("$2"), 
            "PostgreSQL DELETE should use $1, $2 placeholders");
}
