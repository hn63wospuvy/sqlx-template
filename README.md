# sqlx-template

`sqlx-template` is a Rust library designed to generate database query functions using macros, based on the `sqlx` framework. It aims to provide a flexible, simple way to interact with databases such as MySQL, Postgres, and SQLite.

## Features

- Generate functions for select, insert, update, delete, and order by queries based on fields.
- Various return types such as counting, paging, streaming, returning (Postgres only), fetch_one, fetch_all, and rows_affected.
- All generated query functions include the corresponding code in the documentation.
- Supports transactions and optimistic locking templates.
- Customizable queries with named parameters and the ability to run multiple queries.
- Import queries from files.
- Customizable logging and debugging for queries and execution time.
- Compile-time query syntax validation.

## Requirements

- The generated functions depend on the `sqlx` crate, so you need to add it to your dependencies before using this library.
- Columns in the database must match the names and data types of the fields in the struct.
- Structs need to derive `sqlx::FromRow` and `TableName`.

## Example Code

```rust
use sqlx_template::{multi_query, query, select, update, DeleteTemplate, SelectTemplate, TableName, UpdateTemplate};

#[derive(sqlx::FromRow, InsertTemplate, UpdateTemplate, SelectTemplate, DeleteTemplate, TableName)]
#[debug_slow = 1000]
#[table_name = "users"]
#[tp_delete(by = "id")]
#[tp_delete(by = "id, email")]
#[tp_select_all(by = "id, email", order = "id desc")]
#[tp_select_one(by = "id", order = "id desc", fn_name = "get_last_inserted")]
#[tp_select_one(by = "email")]
#[tp_select_page(by = "org", order = "id desc, org desc")]
#[tp_select_count(by = "id, email")]
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
#[tp_select_stream(order = "id desc")]
#[tp_select_stream(by = "email", order = "id desc")]
pub struct User {
    #[auto]
    id: i32,
    email: String,
    password: String,
    org: Option<i32>,
    active: bool,
    #[auto]
    version: i32,
    created_by: Option<String>,
    #[auto]
    created_at: DateTime<Utc>,
    updated_by: Option<String>,
    updated_at: Option<DateTime<Utc>>,
}

#[select("
    SELECT name, age
    FROM users
    WHERE name = :name and age = :age
")]
pub async fn query_user_info(name: String, age: i32) -> Stream<(String, i16)> {}

#[multi_query(file = "sql/init.sql", 0)]
async fn migrate() {}
```

For more details, please see the examples in the repository.

## Features

- `postgres`: Target PostgreSQL databases.
- `mysql`: Target MySQL databases.
- `sqlite`: Target SQLite databases.
- `tracing`: Use the `tracing::debug!` macro for logging (requires adding the `tracing` crate to `Cargo.toml`).
- `log`: Use the `log::debug!` macro for logging (requires adding the `log` crate to `Cargo.toml`).

## Notes

- If you encounter errors caused by macros in the library, try regenerating the function by copying the generated code from the function's documentation. If documentation is not available, most errors are due to syntax issues, incorrect variable names, column names, or file paths.
- `debug_slow` applies to all attributes using derived macros of the struct. It can be overridden by declaring the `debug_slow` attribute within the attribute itself. To disable it, set `debug_slow = -1` explicity.
- By default, if neither `tracing` nor `log` features are declared, information will be printed to the screen using the `println!` macro.

## TODO

- Add more tests and example code.
- Support more type of parameter
- More grammar check
- Integrate with `sqlx::query!` marco
- Better interface for `multi_query` marco

## License

This project is licensed under the Apache 2.0 License.

## Contributions

All PRs are welcome!

