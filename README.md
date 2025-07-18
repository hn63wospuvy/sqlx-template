# sqlx-template

`sqlx-template` is a Rust library designed to generate database query functions using macros, based on the `sqlx` framework. It aims to provide a flexible, simple way to interact with databases such as MySQL, Postgres, and SQLite.

<div align="left">
  <h4>
    <a href="https://crates.io/crates/sqlx-template">
      Crates.io
    </a>
    <span> | </span>
    <a href="https://docs.rs/sqlx-template">
      Docs.rs
    </a>
  </h4>
</div>

## Features

- Generate functions for select, insert, update, delete, upsert, and order by queries based on fields.
- Various return types such as counting, paging, streaming, returning (with specific columns), fetch_one, fetch_all, and rows_affected.
- All generated query functions include the corresponding code in the documentation.
- Supports transactions and optimistic locking templates.
- Customizable queries with named parameters and the ability to run multiple queries.
- Import queries from files.
- Customizable logging and debugging for queries and execution time.
- Compile-time query syntax validation.
- Database-specific optimizations with `#[db("database_type")]` attribute.
- Enhanced RETURNING clause support with specific column selection.
- Support for placeholder parameters in WHERE conditions.
- Improved function name generation based on query parameters.

## Requirements

- The generated functions depend on the `sqlx` crate, so you need to add it to your dependencies before using this library.
- Columns in the database must match the names and data types of the fields in the struct.
- Structs need to derive `sqlx::FromRow` and `TableName`.

## Example Code

```rust
use sqlx_template::{multi_query, query, select, update, PostgresTemplate, SqlxTemplate, TableName};
use chrono::{DateTime, Utc};

// Using PostgresTemplate for PostgreSQL-specific features
#[derive(PostgresTemplate, sqlx::FromRow, Default, Clone, Debug)]
#[debug_slow = 1000]
#[table("users")]
#[tp_upsert(by = "email")]
#[tp_delete(by = "id")]
#[tp_select_all(by = "id, email", order = "id desc")]
#[tp_select_one(by = "id", order = "id desc", fn_name = "get_last_inserted")]
#[tp_select_one(by = "email")]
#[tp_select_page(by = "org", order = "id desc, org desc")]
#[tp_select_count(by = "id, email")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
#[tp_update(by = "id", fn_name = "update_user_returning", returning = true)]
#[tp_update(by = "id", fn_name = "update_user_returning_id", returning = "id")]
#[tp_select_stream(order = "id desc")]
pub struct User {
    #[auto]
    pub id: i32,
    pub email: String,
    pub password: String,
    pub org: Option<i32>,
    pub active: bool,
    #[auto]
    pub version: i32,
    pub created_by: Option<String>,
    #[auto]
    pub created_at: DateTime<Utc>,
    pub updated_by: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
}

// Using individual derive macros for more control
#[derive(SqlxTemplate, sqlx::FromRow, Default, Clone, Debug)]
#[table("organizations")]
#[db("postgres")]
#[tp_delete(by = "id")]
#[tp_select_one(by = "code")]
#[tp_select_all(order = "id desc")]
pub struct Organization {
    #[auto]
    pub id: i32,
    pub name: String,
    pub code: String,
    pub active: bool,
    pub created_by: Option<String>,
    #[auto]
    pub created_at: DateTime<Utc>,
}

#[select("
    SELECT *
    FROM users
    WHERE (email = :name and org = :org) OR email LIKE '%' || :name || '%'
")]
#[db("postgres")]
pub async fn query_user_info(name: &str, org: i32) -> Vec<User> {}

#[multi_query(file = "sql/init.sql", 0)]
#[db("postgres")]
async fn migrate() {}

#[tokio::main]
async fn main() {
  let db = sqlx::PgPool::connect(&dsn).await.unwrap();
  migrate(&db).await.unwrap();

  let user = User {
      email: "foo@bar.com".into(),
      password: "123456".into(),
      active: true,
      ..Default::default()
  };

  // Insert and get returned record (PostgreSQL)
  let new_user = User::insert_return(&user, &db).await.unwrap();

  // Update with returning specific columns
  let updated_id = User::update_user_returning_id(&new_user.id, &user, &db).await.unwrap();

  // Upsert operation
  User::upsert_by_email(&user, &db).await.unwrap();

  // Stream results
  let mut stream = User::stream_order_by_id_desc(&db);
  while let Some(Ok(u)) = stream.next().await {
      println!("User: {u:?}");
  }
}
```

For more details, please see the examples in the repository.

## Available Macros

### Derive Macros
- `InsertTemplate`: Generate insert functions
- `UpdateTemplate`: Generate update functions
- `SelectTemplate`: Generate select/query functions
- `DeleteTemplate`: Generate delete functions
- `UpsertTemplate`: Generate upsert functions (INSERT ... ON CONFLICT)
- `SqlxTemplate`: Combines all above templates in one macro
- `PostgresTemplate`, `MysqlTemplate`, `SqliteTemplate`, `AnyTemplate`: Database-specific versions
- `TableName`: Generate table name function
- `Columns`: Generate column name constants
- `DDLTemplate`: Generate DDL (CREATE/DROP TABLE) statements

### Procedural Macros
- `query`: Transform SQL query into async function
- `select`: Transform SELECT query into async function
- `insert`: Transform INSERT query into async function
- `update`: Transform UPDATE query into async function
- `delete`: Transform DELETE query into async function
- `multi_query`: Transform multiple SQL queries into async function
- Database-specific versions: `postgres_query`, `mysql_query`, `sqlite_query`, etc.

## Features


- `tracing`: Use the `tracing::debug!` macro for logging (requires adding the `tracing` crate to `Cargo.toml`).
- `log`: Use the `log::debug!` macro for logging (requires adding the `log` crate to `Cargo.toml`).

## Notes

- If you encounter errors caused by macros in the library, try regenerating the function by copying the generated code from the function's documentation. If documentation is not available, most errors are due to syntax issues, incorrect variable names, column names, or file paths.
- `debug_slow` applies to all attributes using derived macros of the struct. It can be overridden by declaring the `debug_slow` attribute within the attribute itself. To disable it, set `debug_slow = -1` explicitly.
- By default, if neither `tracing` nor `log` features are declared, information will be printed to the screen using the `println!` macro.
- Use `#[db("database_type")]` to specify target database for optimized query generation.
- The `table` attribute has replaced the old `table_name` attribute.

## Changelog

### Changes since v0.1.1

#### New Features
- **Enhanced RETURNING clause support**: Now supports returning specific columns (e.g., `returning = "id, email"`) in addition to full record returning
- **Upsert operations**: Added `UpsertTemplate` macro for INSERT ... ON CONFLICT operations (PostgreSQL)
- **Database-specific macros**: Added `PostgresTemplate`, `MysqlTemplate`, `SqliteTemplate`, `AnyTemplate` for database-specific optimizations
- **Placeholder support in WHERE conditions**: Enhanced support for placeholder parameters in WHERE statements
- **Function name improvements**: Better automatic function name generation based on query parameters
- **Column utilities**: Added `Columns` derive macro for generating column name constants


#### Breaking Changes
- **Table attribute change**: `#[table_name = "..."]` is now `#[table("...")]`
- **Database specification**: Use `#[db("postgres")]` instead of feature flags for database-specific behavior

#### Improvements
- **Query validation**: Enhanced compile-time query syntax validation
- **Error handling**: Improved error messages and debugging capabilities
- **Performance**: Optimized query generation for different database types
- **Documentation**: Comprehensive documentation updates with examples for all macros

#### Bug Fixes
- Fixed page count query with JOIN and GROUP BY clauses
- Fixed update operations with string fields
- Fixed function name generation conflicts
- Fixed WHERE statement handling in tp_update with specific field selections
- Fixed returning query generation
- Fixed column name errors when matching database keywords
- Fixed TableName derive with new attribute format
- Fixed placeholder handling in WHERE conditions

## TODO

- Add more tests and example code.
- Support more parameter types
- Enhanced grammar checking
- Integration with `sqlx::query!` macro
- Better interface for `multi_query` macro

## License

This project is licensed under the Apache 2.0 License.

## Contributions

All PRs are welcome!

