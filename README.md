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
// Enable builder patterns for flexible queries with custom conditions
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "version BETWEEN :min$i32 AND :max$i32",
    with_active_org = "active = true AND org = :org_id$i32"
)]
#[tp_update_builder(
    with_high_version = "version > :threshold$i32"
)]
#[tp_delete_builder(
    with_old_inactive = "active = false AND created_at < :cutoff$String"
)]
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
#[tp_select_builder] // Enable builder pattern
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

// Builder Pattern Examples - Flexible and Dynamic Queries
async fn builder_examples(db: &sqlx::PgPool) -> Result<(), sqlx::Error> {
    // SELECT Builder Pattern - Build complex queries dynamically

    // 1. Simple query with single condition
    let active_users = User::builder_select()
        .active(&true)?
        .find_all(db).await?;
    println!("Found {} active users", active_users.len());

    // 2. Multiple conditions with AND logic
    let filtered_users = User::builder_select()
        .active(&true)?
        .org(&Some(1))?
        .find_all(db).await?;
    println!("Found {} users with org=1 and active=true", filtered_users.len());

    // 3. String field conditions (like, starts_with, ends_with)
    let email_users = User::builder_select()
        .email_like("%@abc.com")?
        .active(&true)?
        .find_all(db).await?;
    println!("Found {} users with @abc.com email", email_users.len());

    // 4. Ordering and pagination
    let ordered_users = User::builder_select()
        .active(&true)?
        .order_by_id_desc()?
        .find_all(db).await?;
    println!("Users ordered by ID desc:");
    for user in &ordered_users {
        println!("  - ID: {}, Email: {}", user.id, user.email);
    }

    // 5. Find one record
    let single_user = User::builder_select()
        .email("user2@abc.com")?
        .active(&true)?
        .find_one(db).await?;
    if let Some(user) = single_user {
        println!("Found user: {} (ID: {})", user.email, user.id);
    }

    // 6. Paginated results
    let page_result = User::builder_select()
        .active(&true)?
        .order_by_id_asc()?
        .find_page((0, 2, true), db).await?; // offset=0, limit=2, count=true
    println!("Page info: offset=0, limit=2, total={:?}", page_result.1);
    println!("Users on page 1:");
    for user in &page_result.0 {
        println!("  - ID: {}, Email: {}", user.id, user.email);
    }

    // 7. Stream results for large datasets
    let org_ref = Some(1);
    let mut builder = User::builder_select()
        .active(&true)?
        .org(&org_ref)?
        .order_by_email_asc()?;
    let mut user_stream = builder.stream(db).await;

    println!("Streaming users (active=true, org=1, ordered by email):");
    let mut count = 0;
    while let Some(user_result) = user_stream.next().await {
        match user_result {
            Ok(user) => {
                count += 1;
                println!("  Stream #{}: {} (ID: {})", count, user.email, user.id);
            }
            Err(e) => println!("Stream error: {}", e),
        }
    }

    // 8. Numeric comparisons
    let high_id_users = User::builder_select()
        .id_gt(&1)?
        .active(&true)?
        .find_all(db).await?;
    println!("Found {} users with ID > 1", high_id_users.len());

    // 9. Count records
    let user_count = User::builder_select()
        .active(&true)?
        .org(&Some(1))?
        .count(db).await?;
    println!("Total count of active users in org 1: {}", user_count);

    // 10. String prefix/suffix matching
    let prefix_users = User::builder_select()
        .email_start_with("user")?
        .active(&true)?
        .order_by_id_asc()?
        .find_all(db).await?;
    println!("Found {} users with email starting with 'user'", prefix_users.len());

    // 11. Build SQL without executing (for debugging)
    let org_ref2 = Some(1);
    let query_builder = User::builder_select()
        .active(&true)?
        .org(&org_ref2)?
        .email_like("%abc%")?
        .order_by_id_desc()?;
    let sql = query_builder.build_sql();
    println!("Generated SQL: {}", sql);

    // UPDATE Builder Pattern - Flexible updates

    // 1. Update specific fields with conditions
    let update_result = User::builder_update()
        .on_version(&1).unwrap()  // SET version = 1
        .on_updated_by("admin").unwrap()  // SET updated_by = 'admin'
        .by_org(&Some(1)).unwrap()  // WHERE org = 1
        .by_active(&true).unwrap()  // WHERE active = true
        .execute(db).await.unwrap();
    println!("Updated {} users", update_result);

    // DELETE Builder Pattern - Safe deletions

    // 1. Delete with multiple conditions
    let delete_result = User::builder_delete()
        .active(&false).unwrap()  // WHERE active = false
        .email_like("%test.com").unwrap()  // WHERE email LIKE '%test.com'
        .execute(db).await.unwrap();
    println!("Deleted {} inactive users", delete_result);

    // 2. Complex query with multiple conditions and ordering
    let complex_users = User::builder_select()
        .active(&true).unwrap()
        .org(&Some(1)).unwrap()
        .id_gte(&1).unwrap()  // ID >= 1
        .email_end_with(".com").unwrap()  // email ends with .com
        .order_by_email_asc().unwrap()
        .order_by_id_desc().unwrap()  // secondary sort
        .find_page((0, 5, false), db).await.unwrap();  // limit 5, no count
    println!("Complex query results:");
    for user in &complex_users.0 {
        println!("  - ID: {}, Email: {}, Org: {:?}", user.id, user.email, user.org);
    }

    // Custom Conditions Examples

    // 1. Email domain filtering
    let domain_users = User::builder_select()
        .active(&true).unwrap()
        .with_email_domain("@company.com").unwrap()
        .order_by_id_asc().unwrap()
        .find_all(db).await.unwrap();
    println!("Found {} users with @company.com domain", domain_users.len());

    // 2. Version range filtering
    let version_users = User::builder_select()
        .with_score_range(0, 10).unwrap()  // version BETWEEN 0 AND 10
        .active(&true).unwrap()
        .find_all(db).await.unwrap();
    println!("Found {} users with version 0-10", version_users.len());

    // 3. Active users in specific org
    let org_users = User::builder_select()
        .with_active_org(1).unwrap()  // active = true AND org = 1
        .count(db).await.unwrap();
    println!("Found {} active users in org 1", org_users);

    // 4. UPDATE with custom condition
    let high_version_update = User::builder_update()
        .on_updated_by("system").unwrap()
        .with_high_version(5).unwrap()  // WHERE version > 5
        .execute(db).await.unwrap();
    println!("Updated {} users with high version", high_version_update);

    // 5. DELETE with custom condition
    let old_cutoff = "2023-01-01 00:00:00";
    let deleted_old = User::builder_delete()
        .with_old_inactive(old_cutoff).unwrap()
        .execute(db).await.unwrap();
    println!("Deleted {} old inactive users", deleted_old);

    // Organization builder examples
    let orgs = Organization::builder_select()
        .active(&true).unwrap()
        .name_like("%Corp%").unwrap()
        .find_all(db).await.unwrap();

    Ok(())
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

### Builder Pattern Attributes
- `#[tp_select_builder]`: Generate flexible SELECT query builder
- `#[tp_update_builder]`: Generate flexible UPDATE query builder
- `#[tp_delete_builder]`: Generate flexible DELETE query builder

The builder pattern provides:
- **Dynamic query construction**: Build queries with optional conditions
- **Type-safe field access**: All field names are validated at compile time
- **Flexible ordering**: Multiple ordering options with asc/desc
- **Pagination support**: Built-in offset/limit with optional counting
- **Streaming support**: Handle large result sets efficiently
- **SQL generation**: Build SQL without executing for debugging

### Procedural Macros
- `query`: Transform SQL query into async function
- `select`: Transform SELECT query into async function
- `insert`: Transform INSERT query into async function
- `update`: Transform UPDATE query into async function
- `delete`: Transform DELETE query into async function
- `multi_query`: Transform multiple SQL queries into async function
- Database-specific versions: `postgres_query`, `mysql_query`, `sqlite_query`, etc.

## Builder Pattern Features

The builder pattern in sqlx-template provides a fluent, type-safe way to construct dynamic SQL queries:

### SELECT Builder Methods
- **Field conditions**: `.field_name(&value).unwrap()` - Exact match
- **String operations**: `.field_name_like("pattern").unwrap()`, `.field_name_start_with("prefix").unwrap()`, `.field_name_end_with("suffix").unwrap()`
- **Numeric comparisons**: `.field_name_gt(&value).unwrap()`, `.field_name_gte(&value).unwrap()`, `.field_name_lt(&value).unwrap()`, `.field_name_lte(&value).unwrap()`
- **Negation**: `.field_name_not(&value).unwrap()` - Not equal
- **Custom conditions**: `.with_method_name(params).unwrap()` - User-defined SQL expressions
- **Ordering**: `.order_by_field_name_asc().unwrap()`, `.order_by_field_name_desc().unwrap()`
- **Execution**: `.find_all()`, `.find_one()`, `.find_page((offset, limit, count))`, `.stream()`, `.count()`
- **SQL generation**: `.build_sql()` - Returns SQL string for debugging

### UPDATE Builder Methods
- **Set fields**: `.on_field_name(&value).unwrap()` - Set field to value
- **Where conditions**: `.by_field_name(&value).unwrap()` - Update condition
- **Custom conditions**: `.with_method_name(params).unwrap()` - User-defined WHERE expressions
- **Execution**: `.execute()` - Returns number of affected rows

### DELETE Builder Methods
- **Where conditions**: `.field_name(&value).unwrap()` - Delete condition
- **Custom conditions**: `.with_method_name(params).unwrap()` - User-defined WHERE expressions
- **Execution**: `.execute()` - Returns number of deleted rows

### Custom Conditions

Custom conditions allow you to define complex SQL expressions that go beyond simple field comparisons:

```rust
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "score BETWEEN :min$i32 AND :max$i32",
    with_active_org = "active = true AND org = :org_id$i32",
    with_complex_calc = "score * multiplier > :threshold$f64"
)]
```

**Syntax Rules:**
- Method name: `with_method_name` becomes `.with_method_name(params)`
- Placeholders: `:param_name$Type` for typed parameters
- Auto-mapping: `:param_name` maps to struct fields automatically
- Multiple params: Each unique placeholder becomes a method parameter

**Examples:**
- `.with_email_domain("@company.com")` → `email LIKE '@company.com'`
- `.with_score_range(10, 100)` → `score BETWEEN 10 AND 100`
- `.with_active_org(1)` → `active = true AND org = 1`

### Advantages
- **Type safety**: All field names validated at compile time
- **Dynamic queries**: Add conditions based on runtime logic
- **Clean syntax**: Fluent interface for readable code
- **Performance**: Compiled to efficient SQL with proper placeholders
- **Database agnostic**: Works with PostgreSQL, MySQL, SQLite
- **Custom logic**: Complex SQL expressions with custom conditions

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

