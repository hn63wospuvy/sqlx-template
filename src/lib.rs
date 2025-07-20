#![allow(warnings)]
use proc_macro::TokenStream;
use quote::quote;
use sqlx_template::raw;
use syn::{
    parse_macro_input, Attribute, AttributeArgs, Data, DeriveInput, Field, Fields, Ident, ItemStruct, Lit, Meta, MetaNameValue, NestedMeta
};

use crate::sqlx_template::Database;

mod sqlx_template;
mod columns;
mod parser;



/// `InsertTemplate` is a derive macro designed to automatically generate record insert functions
/// based on `sqlx`. This macro creates `insert` methods for the struct it is applied to, returning
/// the number of new records added. It assumes that the columns in the database correspond to the fields in the struct.
///
/// # Attributes
///
/// `InsertTemplate` accepts the following attributes:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `auto`: Applied to fields that should be excluded from the insert statement, typically for auto-incrementing primary keys.
/// - `db`: Specifies the target database type (e.g., `#[db("postgres")]`, `#[db("mysql")]`, `#[db("sqlite")]`).
///
/// Additionally, when using PostgreSQL (`#[db("postgres")]`), the library will generate an `insert_return` function that returns the newly inserted record.
///
/// # Example
///
/// ```rust
/// use sqlx_template::InsertTemplate;
///
/// #[derive(InsertTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[debug_slow = 1000]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // Insert a new user record
/// let user = User { id: 0, email: "john.doe@example.com".to_string(), password: "password123".to_string() };
/// let rows_affected = User::insert(&user, &pool).await?;
/// println!("Rows affected: {}", rows_affected);
///
/// // With PostgreSQL database
/// #[derive(InsertTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[db("postgres")]
/// pub struct UserPg {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// let new_user = UserPg::insert_return(&user, &pool).await?;
/// println!("New user: {:?}", new_user);
/// ```
///
/// In the example above:
/// - `table` is set to "users", specifying the table to insert into. (mandatory).
/// - `debug_slow` is set to 1000 milliseconds, meaning only queries taking longer than 1 second will be logged for debugging.
/// - The `id` field is marked with `#[auto]`, indicating that it should be excluded from the insert statement, typically for auto-incrementing primary keys.
///

/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated insert methods.
///


#[proc_macro_derive(InsertTemplate, attributes(table, auto, debug_slow, db))]
pub fn insert_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::insert::derive_insert(&input, None, sqlx_template::Scope::Struct, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// `UpdateTemplate` is a derive macro designed to automatically generate record update functions
/// based on `sqlx`. This macro creates `update` methods for the struct it is applied to, reducing
/// repetitive code and improving the readability and maintainability of your code.
/// It assumes that the columns in the database correspond to the fields in the struct.
///
/// # Attributes
///
/// `UpdateTemplate` accepts the following attributes:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `tp_update`: The main configuration for generating the update function, with the following sub-attributes:
///   - `by`: List of columns that will be the update condition, will be the function's input (mandatory and non-empty).
///   - `on`: List of columns that will be updated. If empty, all columns will be updated.
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `op_lock`: The name of the column to apply optimistic locking (optional).
///   - `returning`: Can be set to `true` for returning the full record, or specify specific columns (e.g., `returning = "id, email"`).
///   - `debug_slow`: Configures debug logs for the executed query:
///     - If `0`: Only logs the executed query.
///     - If `> 0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///     - If not configured, no debug logs will be generated.
/// - `debug_slow`: Configures debug logs for the executed query, with priority given to the value in `tp_update`.
/// - `db`: Specifies the target database type (e.g., `#[db("postgres")]`).
///
/// # Example
///
/// ```rust
/// use sqlx_template::UpdateTemplate;
///
/// #[derive(UpdateTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
/// #[tp_update(by = "id", on = "email, password", fn_name = "update_user_password")]
/// #[tp_update(by = "id", fn_name = "update_user_returning", returning = true)]
/// #[tp_update(by = "id", fn_name = "update_user_returning_id", returning = "id")]
/// #[debug_slow = 1000]
/// #[db("postgres")]
/// pub struct User {
///     pub id: i32,
///     pub email: String,
///     pub password: String,
///     pub version: i32
/// }
/// ```
///
/// In the example above:
/// - `table` is set to "users", specifying the table to update.
/// - The first `tp_update` generates a function named `update_user` to update record, using `id` as the condition and applying optimistic locking on the `version` column.
/// - The second `tp_update` generates a function named `update_user_password` to update both `email` and `password` columns, using `id` as the condition.
/// - The third `tp_update` generates a function that returns the full updated record.
/// - The fourth `tp_update` generates a function that returns only the `id` column of the updated record.
/// - `debug_slow` is set to 1000 milliseconds, meaning only queries taking longer than 1 second will be logged for debugging.
///

/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated update methods.

#[proc_macro_derive(UpdateTemplate, attributes(table, tp_update, tp_update_builder, debug_slow, db, tp_update_builder))]
pub fn update_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::update::derive_update(&input, None, sqlx_template::Scope::Struct, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// `DeleteTemplate` is a derive macro designed to automatically generate record deletion functions
/// based on `sqlx`. This macro creates `delete` methods for the struct it is applied to, returning
/// the number of records deleted.
/// It assumes that the columns in the database correspond to the fields in the struct.
///
/// # Attributes
///
/// `DeleteTemplate` accepts the following attributes:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `tp_delete`: The main configuration for generating the delete function, with the following sub-attributes:
///   - `by`: List of columns that will be the delete condition, will be the function's input (mandatory and non-empty).
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `returning`: Can be set to `true` for returning the full record, or specify specific columns (e.g., `returning = "id, email"`).
///   - `debug_slow`: Configures debug logs for the executed query:
///     - If set to `0`: Only logs the executed query.
///     - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///     - If not configured, no debug logs will be generated.
/// - `db`: Specifies the target database type (e.g., `#[db("postgres")]`).
///
/// The `debug_slow` attribute at the struct level has priority over the value in `tp_delete`.
///
/// # Example
///
/// ```rust
/// use sqlx_template::DeleteTemplate;
///
/// #[derive(DeleteTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_delete(by = "id", fn_name = "delete_user", returning = true)]
/// #[tp_delete(by = "id")]
/// #[tp_delete(by = "id, email")]
/// #[db("postgres")]
/// pub struct User {
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // Delete a user record by id
/// let user = User { id: 1, email: "john.doe@example.com".to_string(), password: "password123".to_string() };
/// let rows_affected = User::delete_by_id(&user.id, &pool).await?;
/// println!("Rows affected: {}", rows_affected);
///
/// // With returning enabled (PostgreSQL)
/// let deleted_user = User::delete_user(&user.id, &pool).await?;
/// println!("Deleted user: {:?}", deleted_user);
///
/// // Delete by multiple columns
/// let rows_affected = User::delete_by_id_and_email(&user.id, &user.email, &pool).await?;
/// ```
///
/// In the example above:
/// - `table` is set to "users", specifying the table to delete from.
/// - The first `tp_delete` generates a function named `delete_user` to delete a record based on the `id` column and return the deleted record.
/// - The second `tp_delete` generates a function named `delete_by_id` to delete a record based on the `id` column.
/// - The third `tp_delete` generates a function named `delete_by_id_and_email` to delete a record based on both `id` and `email` columns.
///
/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated delete methods.
///

#[proc_macro_derive(DeleteTemplate, attributes(table, tp_delete, tp_delete_builder, debug_slow, db, tp_delete_builder))]
pub fn delete_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::delete::derive_delete(&input, None, sqlx_template::Scope::Struct, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// `SelectTemplate` is a derive macro designed to automatically generate record retrieval functions
/// based on `sqlx`. This macro creates various `query` methods for the struct it is applied to,
/// returning records from the database, assuming that the columns in the database correspond to the fields in the struct.
///
/// # Attributes
///
/// `SelectTemplate` accepts the following attributes:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `tp_select_all`: Generates a function that returns all records as a `Vec<T>`. It has the following sub-attributes:
///   - `by`: List of columns for the `WHERE` condition, used as function input (can be empty).
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `order`: Adds an `ORDER BY` clause based on the specified columns and order (supports `asc|desc`, default is `asc`).
///   - `debug_slow`: Configures debug logs for the executed query.
/// - `tp_select_one`: Similar to `tp_select_all`, but returns a single record as `Option<T>`.
/// - `tp_select_stream`: Similar to `tp_select_all`, but returns an `impl Stream<Item = T>`.
/// - `tp_select_count`: Similar to `tp_select_all`, but returns the count of records as `i64`.
/// - `tp_select_page`: Similar to `tp_select_all`, but accepts pagination parameters and returns a tuple of all records and the total count.
/// - `db`: Specifies the target database type (e.g., `#[db("postgres")]`).
///
/// The `debug_slow` attribute at the struct level has priority over the value in `tp_select_*`.
///
/// Additionally, the library will automatically generate the following default functions when `SelectTemplate` is derived:
/// - `find_all`: Returns all records in the table.
/// - `count_all`: Counts all records in the table.
/// - `find_page_all`: Returns all records and the total count in the table based on pagination parameters.
///
/// # Example
///
/// ```rust
/// use sqlx_template::SelectTemplate;
/// use sqlx::FromRow;
///
/// #[derive(SelectTemplate, FromRow)]
/// #[table("users")]
/// #[tp_select_one(by = "id", fn_name = "find_user_by_id")]
/// #[tp_select_one(by = "email")]
/// #[tp_select_all(by = "id, email", order = "id desc")]
/// #[tp_select_count(by = "id, email")]
/// #[tp_select_page(by = "org", order = "id desc, org desc")]
/// #[tp_select_stream(order = "id desc")]
/// #[debug_slow = 1000]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
///     pub org: Option<i32>,
/// }
///
/// // Example usage:
/// // Find user by id
/// let user = User::find_user_by_id(&1, &pool).await?;
/// println!("Found user: {:?}", user);
///
/// // Find user by email
/// let user = User::find_one_by_email(&"user@example.com".to_string(), &pool).await?;
///
/// // Find all users with conditions
/// let users = User::find_all_by_id_and_email(&1, &"user@example.com".to_string(), &pool).await?;
///
/// // Count users
/// let user_count = User::count_by_id_and_email(&1, &"user@example.com".to_string(), &pool).await?;
///
/// // Find users with pagination
/// let page_request = (0i64, 10i32, true); // (offset, limit, count)
/// let (users, total_count) = User::find_page_by_org_order_by_id_desc_and_org_desc(&Some(1), page_request, &pool).await?;
///
/// // Stream users
/// let mut user_stream = User::stream_order_by_id_desc(&pool);
/// while let Some(Ok(user)) = user_stream.next().await {
///     println!("Streamed user: {:?}", user);
/// }
/// ```
///
/// In the example above:
/// - `table` is set to "users", specifying the table to query from.
/// - Various `tp_select_*` configurations generate different types of query functions.
/// - Function names are automatically generated based on the `by` and `order` parameters.
///

///
/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated query methods.
///

#[proc_macro_derive(SelectTemplate, attributes(table, debug_slow, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count, tp_select_builder, db, tp_select_builder))]
pub fn select_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::select::derive_select(&input, None, sqlx_template::Scope::Struct, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}



/// `Columns` is a derive macro that generates column name constants and utility functions
/// for database operations. This macro creates static string constants for each field
/// in the struct, making it easier to reference column names in queries.
///
/// # Attributes
///
/// `Columns` accepts the following attributes:
/// - `group`: Groups fields together for specific operations (optional).
///
/// # Generated Functions
///
/// The macro generates the following for each field:
/// - A constant with the column name (e.g., `COLUMN_ID` for field `id`)
/// - Utility functions for accessing column names programmatically
///
/// # Example
///
/// ```rust
/// use sqlx_template::Columns;
///
/// #[derive(Columns)]
/// pub struct User {
///     pub id: i32,
///     pub email: String,
///     #[group = "personal"]
///     pub name: String,
///     #[group = "personal"]
///     pub age: i32,
/// }
///
/// // Usage:
/// // User::COLUMN_ID returns "id"
/// // User::COLUMN_EMAIL returns "email"
/// // User::COLUMN_NAME returns "name"
/// // User::COLUMN_AGE returns "age"
/// ```
///
/// # Note
///
/// This macro is useful for maintaining consistency between struct field names
/// and database column names, and provides compile-time safety when referencing columns.
///
#[proc_macro_derive(Columns, attributes(group))]
pub fn columns_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match columns::derive(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}



/// `DDLTemplate` is a derive macro that generates Data Definition Language (DDL) statements
/// for creating database tables based on the struct definition. This macro analyzes the struct
/// fields and their types to generate appropriate CREATE TABLE statements.
///
/// # Attributes
///
/// `DDLTemplate` accepts the following attributes:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `column`: Applied to individual fields to customize column properties such as:
///   - Column type overrides
///   - Constraints (PRIMARY KEY, NOT NULL, UNIQUE, etc.)
///   - Default values
/// - `db`: Specifies the target database type for database-specific DDL generation.
///
/// # Generated Functions
///
/// The macro generates the following functions:
/// - `create_table_sql()`: Returns the CREATE TABLE statement as a string
/// - `drop_table_sql()`: Returns the DROP TABLE statement as a string
///
/// # Example
///
/// ```rust
/// use sqlx_template::DDLTemplate;
///
/// #[derive(DDLTemplate)]
/// #[table = "users"]
/// pub struct User {
///     #[column(primary_key, auto_increment)]
///     pub id: i32,
///     #[column(unique, not_null)]
///     pub email: String,
///     #[column(not_null)]
///     pub password: String,
///     #[column(default = "true")]
///     pub active: bool,
///     pub created_at: Option<chrono::DateTime<chrono::Utc>>,
/// }
///
/// // Usage:
/// let create_sql = User::create_table_sql();
/// let drop_sql = User::drop_table_sql();
/// ```
///
/// # Note
///
/// This macro is useful for database migrations and schema management.
/// The generated DDL statements are database-specific and should be tested
/// with your target database system.
///
#[proc_macro_derive(DDLTemplate, attributes(column, table, db))]
pub fn ddl_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::ddl::derive(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// `UpsertTemplate` is a derive macro designed to automatically generate upsert (INSERT ... ON CONFLICT)
/// functions based on `sqlx`. This macro creates `upsert` methods for the struct it is applied to,
/// which can either insert a new record or update an existing one based on conflict resolution.
/// It assumes that the columns in the database correspond to the fields in the struct.
///
/// # Attributes
///
/// `UpsertTemplate` accepts the following attributes:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `tp_upsert`: The main configuration for generating the upsert function, with the following sub-attributes:
///   - `conflict`: Specifies the columns that define the conflict condition (mandatory).
///   - `update`: List of columns that will be updated on conflict. If empty, all non-conflict columns will be updated.
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `returning`: If set to true, the generated function will return the upserted record (PostgreSQL only).
///   - `debug_slow`: Configures debug logs for the executed query (overrides struct-level setting).
///
/// # Database Support
///
/// Upsert functionality is supported in:
/// - **PostgreSQL**: Uses `INSERT ... ON CONFLICT ... DO UPDATE` syntax with full RETURNING support
/// - **SQLite**: Uses `INSERT ... ON CONFLICT ... DO UPDATE` syntax with RETURNING support (SQLite 3.35.0+)
/// - **MySQL**: Uses `INSERT ... ON DUPLICATE KEY UPDATE` syntax (RETURNING not supported)
///
/// # Example
///
/// ```rust
/// use sqlx_template::UpsertTemplate;
///
/// // PostgreSQL example
/// #[derive(UpsertTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_upsert(by = "email", update = "password, updated_at", fn_name = "upsert_user")]
/// #[tp_upsert(by = "id", fn_name = "upsert_by_id", returning = true)]
/// #[debug_slow = 1000]
/// #[db("postgres")]
/// pub struct UserPg {
///     pub id: i32,
///     pub email: String,
///     pub password: String,
///     pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
/// }
///
/// // SQLite example
/// #[derive(UpsertTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_upsert(by = "email", update = "password")]
/// #[db("sqlite")]
/// pub struct UserSqlite {
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // MySQL example
/// #[derive(UpsertTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_upsert(by = "email", update = "password")]
/// #[db("mysql")]
/// pub struct UserMysql {
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // Usage:
/// let user = UserPg { id: 1, email: "john@example.com".to_string(), password: "newpass".to_string(), updated_at: None };
/// let rows_affected = UserPg::upsert_user(&user, &pool).await?;
///
/// // With returning (PostgreSQL and SQLite)
/// let upserted_user = UserPg::upsert_by_id(&user, &pool).await?;
/// ```
///
/// # Note
///
/// This macro relies on `sqlx` and database-specific upsert syntax. Make sure your target
/// database supports the generated upsert statements.
///
#[proc_macro_derive(UpsertTemplate, attributes(table, tp_upsert, debug_slow, db))]
pub fn upsert_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::upsert::derive_upsert(&input, None, sqlx_template::Scope::Struct, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// `SqlxTemplate` is a comprehensive derive macro that combines all database operation templates
/// into a single macro. This macro generates functions for insert, update, select, delete, and upsert
/// operations based on `sqlx`. It's a convenience macro that applies `InsertTemplate`, `UpdateTemplate`,
/// `SelectTemplate`, `DeleteTemplate`, `UpsertTemplate`, and `TableName` all at once.
///
/// # Attributes
///
/// `SqlxTemplate` accepts all attributes from the individual template macros:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Global debug configuration for all generated functions.
/// - `auto`: Applied to fields that should be excluded from insert statements.
/// - `tp_select_all`, `tp_select_one`, `tp_select_page`, `tp_select_stream`, `tp_select_count`: Select operation configurations.
/// - `tp_update`: Update operation configurations.
/// - `tp_delete`: Delete operation configurations.
/// - `tp_upsert`: Upsert operation configurations.
/// - `db`: Specifies the target database type.
///
/// # Generated Functions
///
/// This macro generates all functions from the individual templates:
/// - Insert operations: `insert()`, `insert_return()` (PostgreSQL only)
/// - Update operations: Based on `tp_update` configurations
/// - Select operations: Based on `tp_select_*` configurations, plus default `find_all()`, `count_all()`, `find_page_all()`
/// - Delete operations: Based on `tp_delete` configurations
/// - Upsert operations: Based on `tp_upsert` configurations
/// - Table name function: `table_name()`
///
/// # Example
///
/// ```rust
/// use sqlx_template::SqlxTemplate;
///
/// #[derive(SqlxTemplate, sqlx::FromRow)]
/// #[table = "users"]
/// #[debug_slow = 1000]
/// #[tp_select_one(by = "id", fn_name = "find_by_id")]
/// #[tp_select_all(by = "email", order = "id desc")]
/// #[tp_update(by = "id", op_lock = "version")]
/// #[tp_delete(by = "id")]
/// #[tp_upsert(conflict = "email", update = "password")]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
///     pub version: i32,
/// }
///
/// // All operations are now available:
/// // User::insert(&user, &pool).await?;
/// // User::find_by_id(&pool, 1).await?;
/// // User::update_by_id(&user, &pool).await?;
/// // User::delete_by_id(&1, &pool).await?;
/// // User::upsert_on_conflict_email(&user, &pool).await?;
/// ```
///
/// # Note
///
/// This is the most convenient macro to use when you need comprehensive database operations
/// for a struct. It combines all individual template macros into one.
///
#[proc_macro_derive(SqlxTemplate, attributes(table, tp_upsert, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count, tp_update, tp_delete, tp_update_builder, tp_select_builder, tp_delete_builder, auto, debug_slow, db))]
pub fn sqlx_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::derive_all(&input, None, sqlx_template::Scope::Struct, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// `PostgresTemplate` is a database-specific version of `SqlxTemplate` optimized for PostgreSQL.
/// This macro generates all database operation functions specifically targeting PostgreSQL features
/// and syntax. It combines insert, update, select, delete, and upsert operations with PostgreSQL-specific
/// optimizations and features like RETURNING clauses.
///
/// # Attributes
///
/// `PostgresTemplate` accepts the same attributes as `SqlxTemplate`:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Global debug configuration for all generated functions.
/// - `auto`: Applied to fields that should be excluded from insert statements.
/// - `tp_select_all`, `tp_select_one`, `tp_select_page`, `tp_select_stream`, `tp_select_count`: Select operation configurations.
/// - `tp_update`: Update operation configurations.
/// - `tp_delete`: Delete operation configurations.
/// - `tp_upsert`: Upsert operation configurations.
///
/// # PostgreSQL-Specific Features
///
/// - Enhanced RETURNING clause support for insert, update, delete, and upsert operations
/// - Optimized upsert using PostgreSQL's ON CONFLICT syntax
/// - Better support for PostgreSQL-specific data types
/// - Optimized query generation for PostgreSQL
///
/// # Example
///
/// ```rust
/// use sqlx_template::PostgresTemplate;
///
/// #[derive(PostgresTemplate, sqlx::FromRow)]
/// #[table = "users"]
/// #[tp_update(by = "id", returning = true)]
/// #[tp_delete(by = "id", returning = true)]
/// #[tp_upsert(conflict = "email", returning = true)]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // PostgreSQL-specific features:
/// // let new_user = User::insert_return(&user, &pool).await?;
/// // let updated_user = User::update_by_id_return(&user, &pool).await?;
/// // let deleted_user = User::delete_by_id_return(&1, &pool).await?;
/// // let upserted_user = User::upsert_on_conflict_email_return(&user, &pool).await?;
/// ```
///
/// # Note
///
/// This macro is specifically designed for PostgreSQL and may not work with other databases.
/// Use `SqlxTemplate` for database-agnostic code or other database-specific templates for other databases.
///
#[proc_macro_derive(PostgresTemplate, attributes(table, tp_upsert, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count, tp_update, tp_delete, auto, debug_slow, tp_select_builder, tp_update_builder, tp_delete_builder))]
pub fn postgres_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::derive_all(&input, None, sqlx_template::Scope::Struct, Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// `MysqlTemplate` is a database-specific version of `SqlxTemplate` optimized for MySQL.
/// This macro generates all database operation functions specifically targeting MySQL features
/// and syntax. It combines insert, update, select, delete, and upsert operations with MySQL-specific
/// optimizations and syntax compatibility.
///
/// # Attributes
///
/// `MysqlTemplate` accepts the same attributes as `SqlxTemplate`:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Global debug configuration for all generated functions.
/// - `auto`: Applied to fields that should be excluded from insert statements.
/// - `tp_select_all`, `tp_select_one`, `tp_select_page`, `tp_select_stream`, `tp_select_count`: Select operation configurations.
/// - `tp_update`: Update operation configurations.
/// - `tp_delete`: Delete operation configurations.
/// - `tp_upsert`: Upsert operation configurations.
///
/// # MySQL-Specific Features
///
/// - Optimized query generation for MySQL syntax
/// - Support for MySQL-specific data types
/// - Upsert operations using MySQL's ON DUPLICATE KEY UPDATE syntax
/// - Proper handling of MySQL's auto-increment columns
/// - MySQL-compatible LIMIT and OFFSET syntax for pagination
///
/// # Example
///
/// ```rust
/// use sqlx_template::MysqlTemplate;
///
/// #[derive(MysqlTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_update(by = "id")]
/// #[tp_delete(by = "id")]
/// #[tp_upsert(by = "email", update = "password")]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // MySQL-optimized operations:
/// // User::insert(&user, &pool).await?;
/// // User::update_by_id(&user, &pool).await?;
/// // User::delete_by_id(&1, &pool).await?;
/// // User::upsert_by_email(&user, &pool).await?;
/// ```
///
/// # Note
///
/// This macro is specifically designed for MySQL and generates MySQL-compatible SQL syntax.
/// Use `SqlxTemplate` for database-agnostic code or other database-specific templates for other databases.
///
#[proc_macro_derive(MysqlTemplate, attributes(table, tp_upsert, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count, tp_update, tp_delete, auto, debug_slow, tp_select_builder, tp_update_builder, tp_delete_builder))]
pub fn mysql_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::derive_all(&input, None, sqlx_template::Scope::Struct, Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// `SqliteTemplate` is a database-specific version of `SqlxTemplate` optimized for SQLite.
/// This macro generates all database operation functions specifically targeting SQLite features
/// and syntax. It combines insert, update, select, delete, and upsert operations with SQLite-specific
/// optimizations and syntax compatibility.
///
/// # Attributes
///
/// `SqliteTemplate` accepts the same attributes as `SqlxTemplate`:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Global debug configuration for all generated functions.
/// - `auto`: Applied to fields that should be excluded from insert statements.
/// - `tp_select_all`, `tp_select_one`, `tp_select_page`, `tp_select_stream`, `tp_select_count`: Select operation configurations.
/// - `tp_update`: Update operation configurations.
/// - `tp_delete`: Delete operation configurations.
/// - `tp_upsert`: Upsert operation configurations.
///
/// # SQLite-Specific Features
///
/// - Optimized query generation for SQLite syntax
/// - Support for SQLite-specific data types and functions
/// - Upsert operations using SQLite's INSERT ... ON CONFLICT syntax
/// - Proper handling of SQLite's ROWID and auto-increment columns
/// - SQLite-compatible LIMIT and OFFSET syntax for pagination
///
/// # Example
///
/// ```rust
/// use sqlx_template::SqliteTemplate;
///
/// #[derive(SqliteTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_update(by = "id")]
/// #[tp_delete(by = "id")]
/// #[tp_upsert(by = "email", update = "password")]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // SQLite-optimized operations:
/// // User::insert(&user, &pool).await?;
/// // User::update_by_id(&user, &pool).await?;
/// // User::delete_by_id(&1, &pool).await?;
/// // User::upsert_by_email(&user, &pool).await?;
/// ```
///
/// # Note
///
/// This macro is specifically designed for SQLite and generates SQLite-compatible SQL syntax.
/// Use `SqlxTemplate` for database-agnostic code or other database-specific templates for other databases.
///
#[proc_macro_derive(SqliteTemplate, attributes(table, tp_upsert, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count, tp_update, tp_delete, auto, debug_slow, tp_select_builder, tp_update_builder, tp_delete_builder))]
pub fn sqlite_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::derive_all(&input, None, sqlx_template::Scope::Struct, Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// `AnyTemplate` is a database-agnostic version of `SqlxTemplate` that generates
/// database operations compatible with multiple database types. This macro generates
/// functions that work across different databases by using the most common SQL syntax.
///
/// # Attributes
///
/// `AnyTemplate` accepts the same attributes as `SqlxTemplate`:
/// - `table`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Global debug configuration for all generated functions.
/// - `auto`: Applied to fields that should be excluded from insert statements.
/// - `tp_select_all`, `tp_select_one`, `tp_select_page`, `tp_select_stream`, `tp_select_count`: Select operation configurations.
/// - `tp_update`: Update operation configurations.
/// - `tp_delete`: Delete operation configurations.
/// - `tp_upsert`: Upsert operation configurations.
///
/// # Database Compatibility Features
///
/// - Generates SQL syntax compatible with multiple database types
/// - Uses standard SQL features that work across databases
/// - Avoids database-specific syntax and functions
/// - Compatible with sqlx::Any database driver
///
/// # Example
///
/// ```rust
/// use sqlx_template::AnyTemplate;
///
/// #[derive(AnyTemplate, sqlx::FromRow)]
/// #[table("users")]
/// #[tp_update(by = "id")]
/// #[tp_delete(by = "id")]
/// #[tp_select_one(by = "email")]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // Database-agnostic operations:
/// // User::insert(&user, &pool).await?;
/// // User::update_by_id(&user, &pool).await?;
/// // User::delete_by_id(&1, &pool).await?;
/// // User::find_one_by_email(&"user@example.com".to_string(), &pool).await?;
/// ```
///
/// # Note
///
/// This macro is designed for maximum database compatibility but may not take advantage
/// of database-specific optimizations. Use database-specific templates for better performance
/// when targeting a single database type.
///
#[proc_macro_derive(AnyTemplate, attributes(table, tp_upsert, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count, tp_update, tp_delete, auto, debug_slow))]
pub fn any_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::derive_all(&input, None, sqlx_template::Scope::Struct, Some(Database::Any)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// `tp_gen` is a procedural macro attribute that provides advanced code generation capabilities
/// for database operations. This macro allows for more complex and customizable generation
/// of database-related functions beyond what the derive macros provide.
///
/// # Syntax
///
/// ```rust
/// #[tp_gen(<configuration>)]
/// pub struct <StructName> { ... }
/// ```
///
/// # Attributes
///
/// The specific attributes and configuration options for `tp_gen` depend on the implementation
/// and can include various database operation configurations.
///
/// # Example Usage
///
/// ```rust
/// use sqlx_template::tp_gen;
///
/// #[tp_gen(/* configuration parameters */)]
/// pub struct User {
///     pub id: i32,
///     pub name: String,
///     pub email: String,
/// }
/// ```
///
/// # Note
///
/// This is an advanced macro for specialized use cases. For most common database operations,
/// consider using the individual derive macros like `InsertTemplate`, `UpdateTemplate`, etc.,
/// or the comprehensive `SqlxTemplate` macro.
///
#[proc_macro_attribute]
pub fn tp_gen(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let args = parse_macro_input!(args as AttributeArgs);
    match sqlx_template::proc::proc_gen(input, args) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// The `TableName` derive macro automatically generates a `table_name` function
/// for a struct, returning the value specified in the `table` attribute.
///
/// # Syntax
///
/// ```rust
/// #[derive(TableName)]
/// #[table("<table_name>")]
/// pub struct <StructName> { ... }
/// ```
///
/// # Attributes
///
/// - `table`: Specifies the name of the table as a string (e.g., `#[table("users")]`).
///
/// # Function Signature
///
/// The macro generates a const function named `table_name()` which returns a `&'static str` containing the table name.
///
/// # Example Usage
///
/// ```rust
/// use sqlx_template::TableName;
///
/// #[derive(TableName)]
/// #[table("users")]
/// pub struct User {
///     pub id: i32,
///     pub name: String,
///     pub age: i32,
/// }
///
/// fn main() {
///     assert_eq!(User::table_name(), "users");
/// }
/// ```
///
/// # Note
///
/// This macro is often used in combination with other sqlx-template macros to provide
/// a consistent way to reference table names throughout your application.
#[proc_macro_derive(TableName, attributes(table))]
pub fn table_name_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::table_name_derive(&input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// The `multi_query` procedural macro transforms a series of SQL queries with named parameters into
/// an asynchronous function that interacts with the database. It provides various
/// features, including debugging options, and is designed to handle multiple SQL statements with no return value (`void`).
///
/// # Syntax
///
/// ```rust
/// #[multi_query(
///     sql = "<query>" | file = "<path to file query>" | "<query>",
///     debug = <integer>
/// )]
/// pub async fn <function_name>(<parameters>) {}
/// ```
///
/// # Attributes
///
/// - `sql`: Specifies the SQL queries to be executed. This can be:
///   - A raw SQL query as a string (e.g., `sql = "BEGIN; UPDATE user SET age = :age WHERE name = :name; COMMIT;"`).
///   - A path to a file containing the SQL queries (e.g., `file = "path/to/queries.sql"`).
///   - The queries directly as a string without the `sql` or `file` keyword.
///
///   **Constraints:**
///   - The queries can contain multiple SQL statements.
///   - Named parameters (if exist) must be in the format `:<param_name>` and must correspond to the function's parameters.
///
/// - `debug`: Controls the debug behavior of the macro. It can be:
///   - An integer value. If not provided, the default is no debugging.
///   - `0`: Prints the queries before execution.
///   - Greater than `0`: Prints the queries and execution time if it exceeds the specified number of milliseconds.
///
/// # Function Signature
///
/// The macro generates an asynchronous function with the following characteristics:
/// - The function signature remains unchanged (e.g., `pub async fn <function_name>`).
/// - The function parameters are preserved in their original order.
/// - An additional parameter for the database connection is required.
///
/// # Return Types
///
/// The macro only supports the void return type:
///
/// - **Void:**
///   - : Returns nothing.
///
/// # Example Usage
///
/// ```rust
/// #[multi_query(
///     sql = "BEGIN; UPDATE user SET age = :age WHERE name = :name; DELETE FROM session WHERE user_name = :name; COMMIT;",
///     debug = 100
/// )]
/// pub async fn update_user_and_clear_sessions(name: &str, age: i32) {}
///
/// #[multi_query(
///     sql = "INSERT INTO user (name, age) VALUES (:name, :age); INSERT INTO log (user_name, action) VALUES (:name, 'created')",
///     debug = 0
/// )]
/// pub async fn insert_user_with_log(name: &str, age: i32) {}
/// ```
///


#[proc_macro_attribute]
pub fn multi_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::multi_query_derive(input, args, None, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::multi_query] proc macro, but specified for Postgres database
#[proc_macro_attribute]
pub fn postgres_multi_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::multi_query_derive(input, args, None, Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::multi_query] proc macro, but specified for MySQL database
#[proc_macro_attribute]
pub fn mysql_multi_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::multi_query_derive(input, args, None, Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::multi_query] proc macro, but specified for SQLite database
#[proc_macro_attribute]
pub fn sqlite_multi_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::multi_query_derive(input, args, None, Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}



/// The `query` procedural macro transforms an SQL query with named parameters into
/// an asynchronous function that interacts with the database. It provides various
/// features, including debugging options and support for multiple return types.
///
/// # Syntax
///
/// ```rust
/// #[query(
///     sql = "<query>" | file = "<path to file query>" | "<query>",
///     debug = <integer>
/// )]
/// pub async fn <function_name>(<parameters>) -> <return_type> {}
/// ```
///
/// # Attributes
///
/// - `sql`: Specifies the SQL query to be executed. This can be:
///   - A raw SQL query as a string (e.g., `sql = "SELECT * FROM user WHERE (name = :name and age = :age) OR name LIKE '%:name%'"`).
///   - A path to a file containing the SQL query (e.g., `file = "path/to/query.sql"`).
///   - The query directly as a string without the `sql` or `file` keyword.
///
///   **Constraints:**
///   - The query must contain a single SQL statement.
///   - Named parameters (if exist) must be in the format `:<param_name>` and must correspond to the function's parameters.
///
/// - `debug`: Controls the debug behavior of the macro. It can be:
///   - An integer value. If not provided, the default is no debugging.
///   - `0`: Prints the query before execution.
///   - Greater than `0`: Prints the query and execution time if it exceeds the specified number of milliseconds.
///
/// # Function Signature
///
/// The macro generates an asynchronous function with the following characteristics:
/// - The function signature remains unchanged (e.g., `pub async fn <function_name>`).
/// - The function parameters are preserved in their original order.
/// - An additional parameter for the database connection is required.
///
/// # Return Types
///
/// The macro supports various return types based on the SQL query:
///
/// - **Single Record:**
///   - `T`: Returns a single record, which must be present. If no record is found, an error is returned.
///   - `Option<T>`: Returns a single record if present, or `None` if no record is found.
///
/// - **Multiple Records:**
///   - `Vec<T>`: Returns all matching records as a vector.
///
/// - **Asynchronous Stream:**
///   - `Stream<T>`: Returns an asynchronous stream of records.
///
/// - **Paged Records:**
///   - `Page<T>`: Returns paginated results. Requires an additional parameter for pagination (e.g., `impl Into<(i64, i32, bool)>`). The function returns a tuple `(Vec<T>, Option<i64>)`, where the vector contains the paginated records, and the optional value represents the total number of records if requested.
///
/// - **Scalar Value:**
///   - `Scalar<T>`: Returns a single scalar value from the query.
///
/// - **Affected Rows:**
///   - `RowAffected`: Returns the number of affected rows.
///
/// - **Void:**
///   - : Returns nothing.
///
/// # Example Usage
///
/// ```rust
/// #[query(
///     sql = "SELECT * FROM user WHERE (name = :name and age = :age) OR name LIKE '%:name%'",
///     debug = 100
/// )]
/// pub async fn query_user_info(name: &str, age: i32) -> Vec<UserInfo> {}
///
/// #[query(
///     sql = "INSERT INTO user (name, age) VALUES (:name, :age)",
///     debug = 0
/// )]
/// pub async fn insert_user(name: &str, age: i32) -> RowAffected {}
///
/// #[query("DELETE FROM user WHERE name = :name")]
/// pub async fn delete_user(name: &str) {}
/// ```
///

#[proc_macro_attribute]
pub fn query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, None, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// Same as [crate::query] proc macro, but specified for Postgres database
#[proc_macro_attribute]
pub fn postgres_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, None, Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::query] proc macro, but specified for MySQL database
#[proc_macro_attribute]
pub fn mysql_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, None, Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::query] proc macro, but specified for SQLite database  
#[proc_macro_attribute]
pub fn sqlite_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, None, Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// The `select` procedural macro transforms a SQL query with named parameters into
/// an asynchronous function that interacts with the database. It provides various
/// features, including debugging options and support for multiple return types.
///
/// # Syntax
///
/// ```rust
/// #[select(
///     sql = "<query>" | file = "<path to file query>" | "<query>",
///     debug = <integer>
/// )]
/// pub async fn <function_name>(<parameters>) -> <return_type> {}
/// ```
///
/// # Attributes
///
/// - `sql`: Specifies the SQL query to be executed. This can be:
///   - A raw SQL query as a string (e.g., `sql = "SELECT * FROM user WHERE (name = :name and age = :age) OR name LIKE '%:name%'`).
///   - A path to a file containing the SQL query (e.g., `file = "path/to/query.sql"`).
///   - The query directly as a string without the `sql` or `file` keyword.
///
///   **Constraints:**
///   - The query must contain a single SQL SELECT statement.
///   - Named parameters (if exist) must be in the format `:<param_name>` and must correspond to the function's parameters.
///
/// - `debug`: Controls the debug behavior of the macro. It can be:
///   - An integer value. If not provided, the default is no debugging.
///   - `0`: Prints the query before execution.
///   - Greater than `0`: Prints the query and execution time if it exceeds the specified number of milliseconds.
///
/// # Function Signature
///
/// The macro generates an asynchronous function with the following characteristics:
/// - The function signature remains unchanged (e.g., `pub async fn <function_name>`).
/// - The function parameters are preserved in their original order.
/// - An additional parameter for the database connection is required.
///
/// # Return Types
///
/// The macro supports various return types based on the SQL query:
///
/// - **Single Record:**
///   - `T`: Returns a single record, which must be present. If no record is found, an error is returned.
///   - `Option<T>`: Returns a single record if present, or `None` if no record is found.
///
/// - **Multiple Records:**
///   - `Vec<T>`: Returns all matching records as a vector.
///
/// - **Asynchronous Stream:**
///   - `Stream<T>`: Returns an asynchronous stream of records.
///
/// - **Paged Records:**
///   - `Page<T>`: Returns paginated results. Requires an additional parameter for pagination (e.g., `impl Into<(i64, i32, bool)>`). The function returns a tuple `(Vec<T>, Option<i64>)`, where the vector contains the paginated records, and the optional value represents the total number of records if requested.
///
/// - **Scalar Value:**
///   - `Scalar<T>`: Returns a single scalar value from the query.
///
/// - **Affected Rows:**
///   - `RowAffected`: Returns the number of affected rows.
///
/// - **Void:**
///   - : Returns nothing.
/// # Example Usage
///
/// ```rust
/// #[select(
///     sql = "
///     SELECT *
///     FROM user
///     WHERE (name = :name and age = :age) OR name LIKE '%:name%'
/// ",
///     debug = 100
/// )]
/// pub async fn query_user_info(name: &str, age: i32) -> Vec<UserInfo> {}

#[proc_macro_attribute]
pub fn select(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Select), None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::select] proc macro, but specified for Postgres database
#[proc_macro_attribute]
pub fn postgres_select(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Select), Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}
/// Same as [crate::select] proc macro, but specified for MySQL database
#[proc_macro_attribute]
pub fn mysql_select(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Select), Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::select] proc macro, but specified for SQLite database
#[proc_macro_attribute]
pub fn sqlite_select(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Select), Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// The `update` procedural macro transforms an SQL `UPDATE` query with named parameters into
/// an asynchronous function that interacts with the database. It provides various
/// features, including debugging options and support for returning the number of affected rows.
///
/// # Syntax
///
/// ```rust
/// #[update(
///     sql = "<query>" | file = "<path to file query>" | "<query>",
///     debug = <integer>
/// )]
/// pub async fn <function_name>(<parameters>) -> <return_type> {}
/// ```
///
/// # Attributes
///
/// - `sql`: Specifies the SQL `UPDATE` query to be executed. This can be:
///   - A raw SQL query as a string (e.g., `sql = "UPDATE user SET age = :age WHERE name = :name"`).
///   - A path to a file containing the SQL query (e.g., `file = "path/to/query.sql"`).
///   - The query directly as a string without the `sql` or `file` keyword.
///
///   **Constraints:**
///   - The query must be a single SQL `UPDATE` statement.
///   - Named parameters (if exist) must be in the format `:<param_name>` and must correspond to the function's parameters.
///
/// - `debug`: Controls the debug behavior of the macro. It can be:
///   - An integer value. If not provided, the default is no debugging.
///   - `0`: Prints the query before execution.
///   - Greater than `0`: Prints the query and execution time if it exceeds the specified number of milliseconds.
///
/// # Function Signature
///
/// The macro generates an asynchronous function with the following characteristics:
/// - The function signature remains unchanged (e.g., `pub async fn <function_name>`).
/// - The function parameters are preserved in their original order.
/// - An additional parameter for the database connection is required.
///
/// # Return Types
///
/// The macro supports the following return type based on the SQL query:
///
/// - **Single Record:**
///   - `T`: Returns a single record, which must be present. If no record is found, an error is returned.
///   - `Option<T>`: Returns a single record if present, or `None` if no record is found.
///
/// - **Multiple Records:**
///   - `Vec<T>`: Returns all matching records as a vector.
///
/// - **Asynchronous Stream:**
///   - `Stream<T>`: Returns an asynchronous stream of records.
///
/// - **Paged Records:**
///   - `Page<T>`: Returns paginated results. Requires an additional parameter for pagination (e.g., `impl Into<(i64, i32, bool)>`). The function returns a tuple `(Vec<T>, Option<i64>)`, where the vector contains the paginated records, and the optional value represents the total number of records if requested.
///
/// - **Scalar Value:**
///   - `Scalar<T>`: Returns a single scalar value from the query.
///
/// - **Affected Rows:**
///   - `RowAffected`: Returns the number of affected rows.
/// 
/// - **Void:**
///   - : Returns nothing.
///
/// # Example Usage
///
/// ```rust
/// #[update(
///     sql = "UPDATE user SET age = :age WHERE name = :name",
///     debug = 100
/// )]
/// pub async fn update_user_age(name: &str, age: i32) -> RowAffected {}
/// ```
///


#[proc_macro_attribute]
pub fn update(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Update), None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::update] proc macro, but specified for Postgres database
#[proc_macro_attribute]
pub fn postgres_update(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Update), Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::update] proc macro, but specified for MySQL database
#[proc_macro_attribute]
pub fn mysql_update(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Update), Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::update] proc macro, but specified for SQLite database
#[proc_macro_attribute]
pub fn sqlite_update(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Update), Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


/// The `insert` procedural macro transforms an SQL `INSERT` query with named parameters into
/// an asynchronous function that interacts with the database. It provides various
/// features, including debugging options and support for returning the number of affected rows
/// or no return value (`void`).
///
/// # Syntax
///
/// ```rust
/// #[insert(
///     sql = "<query>" | file = "<path to file query>" | "<query>",
///     debug = <integer>
/// )]
/// pub async fn <function_name>(<parameters>) -> <return_type> {}
/// ```
///
/// # Attributes
///
/// - `sql`: Specifies the SQL `INSERT` query to be executed. This can be:
///   - A raw SQL query as a string (e.g., `sql = "INSERT INTO user (name, age) VALUES (:name, :age)"`).
///   - A path to a file containing the SQL query (e.g., `file = "path/to/query.sql"`).
///   - The query directly as a string without the `sql` or `file` keyword.
///
///   **Constraints:**
///   - The query must be a single SQL `INSERT` statement.
///   - Named parameters (if exist) must be in the format `:<param_name>` and must correspond to the function's parameters.
///
/// - `debug`: Controls the debug behavior of the macro. It can be:
///   - An integer value. If not provided, the default is no debugging.
///   - `0`: Prints the query before execution.
///   - Greater than `0`: Prints the query and execution time if it exceeds the specified number of milliseconds.
///
/// # Function Signature
///
/// The macro generates an asynchronous function with the following characteristics:
/// - The function signature remains unchanged (e.g., `pub async fn <function_name>`).
/// - The function parameters are preserved in their original order.
/// - An additional parameter for the database connection is required.
///
/// # Return Types
///
/// The macro supports the following return type based on the SQL query:
///
/// - **Single Record:**
///   - `T`: Returns a single record, which must be present. If no record is found, an error is returned.
///   - `Option<T>`: Returns a single record if present, or `None` if no record is found.
///
/// - **Multiple Records:**
///   - `Vec<T>`: Returns all matching records as a vector.
///
/// - **Asynchronous Stream:**
///   - `Stream<T>`: Returns an asynchronous stream of records.
///
/// - **Paged Records:**
///   - `Page<T>`: Returns paginated results. Requires an additional parameter for pagination (e.g., `impl Into<(i64, i32, bool)>`). The function returns a tuple `(Vec<T>, Option<i64>)`, where the vector contains the paginated records, and the optional value represents the total number of records if requested.
///
/// - **Scalar Value:**
///   - `Scalar<T>`: Returns a single scalar value from the query.
///
/// - **Affected Rows:**
///   - `RowAffected`: Returns the number of affected rows.
/// 
/// - **Void:**
///   - : Returns nothing.
///
/// # Example Usage
///
/// ```rust
/// #[insert(
///     sql = "INSERT INTO user (name, age) VALUES (:name, :age)",
///     debug = 100
/// )]
/// pub async fn insert_user(name: &str, age: i32) -> RowAffected {}
///
/// #[insert("INSERT INTO user (name, age) VALUES (:name, :age)")]
/// pub async fn insert_user_no_return(name: &str, age: i32) {}
/// ```
///


#[proc_macro_attribute]
pub fn insert(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Insert), None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::insert] proc macro, but specified for Postgres database
#[proc_macro_attribute]
pub fn postgres_insert(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Insert), Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::insert] proc macro, but specified for MySQL database
#[proc_macro_attribute]
pub fn mysql_insert(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Insert), Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::insert] proc macro, but specified for SQLite database
#[proc_macro_attribute]
pub fn sqlite_insert(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Insert), Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// The `delete` procedural macro transforms an SQL `DELETE` query with named parameters into
/// an asynchronous function that interacts with the database. It provides various
/// features, including debugging options and support for returning the number of affected rows
/// or no return value (`void`).
///
/// # Syntax
///
/// ```rust
/// #[delete(
///     sql = "<query>" | file = "<path to file query>" | "<query>",
///     debug = <integer>
/// )]
/// pub async fn <function_name>(<parameters>) -> <return_type> {}
/// ```
///
/// # Attributes
///
/// - `sql`: Specifies the SQL `DELETE` query to be executed. This can be:
///   - A raw SQL query as a string (e.g., `sql = "DELETE FROM user WHERE name = :name"`).
///   - A path to a file containing the SQL query (e.g., `file = "path/to/query.sql"`).
///   - The query directly as a string without the `sql` or `file` keyword.
///
///   **Constraints:**
///   - The query must be a single SQL `DELETE` statement.
///   - Named parameters (if exist) must be in the format `:<param_name>` and must correspond to the function's parameters.
///
/// - `debug`: Controls the debug behavior of the macro. It can be:
///   - An integer value. If not provided, the default is no debugging.
///   - `0`: Prints the query before execution.
///   - Greater than `0`: Prints the query and execution time if it exceeds the specified number of milliseconds.
///
/// # Function Signature
///
/// The macro generates an asynchronous function with the following characteristics:
/// - The function signature remains unchanged (e.g., `pub async fn <function_name>`).
/// - The function parameters are preserved in their original order.
/// - An additional parameter for the database connection is required.
///
/// # Return Types
///
/// The macro supports the following return type based on the SQL query:
///
/// - **Single Record:**
///   - `T`: Returns a single record, which must be present. If no record is found, an error is returned.
///   - `Option<T>`: Returns a single record if present, or `None` if no record is found.
///
/// - **Multiple Records:**
///   - `Vec<T>`: Returns all matching records as a vector.
///
/// - **Asynchronous Stream:**
///   - `Stream<T>`: Returns an asynchronous stream of records.
///
/// - **Paged Records:**
///   - `Page<T>`: Returns paginated results. Requires an additional parameter for pagination (e.g., `impl Into<(i64, i32, bool)>`). The function returns a tuple `(Vec<T>, Option<i64>)`, where the vector contains the paginated records, and the optional value represents the total number of records if requested.
///
/// - **Scalar Value:**
///   - `Scalar<T>`: Returns a single scalar value from the query.
///
/// - **Affected Rows:**
///   - `RowAffected`: Returns the number of affected rows.
/// 
/// - **Void:**
///   - : Returns nothing.
///
/// # Example Usage
///
/// ```rust
/// #[delete(
///     sql = "DELETE FROM user WHERE name = :name",
///     debug = 100
/// )]
/// pub async fn delete_user(name: &str) -> RowAffected {}
///
/// #[delete(
///     sql = "DELETE FROM user WHERE name = :name",
///     debug = 0
/// )]
/// pub async fn delete_user_no_return(name: &str) {}
/// ```
///

#[proc_macro_attribute]
pub fn delete(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Delete), None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

/// Same as [crate::delete] proc macro, but specified for Postgres database
#[proc_macro_attribute]
pub fn postgres_delete(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Delete), Some(Database::Postgres)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(), 
    }
    .into()
}

/// Same as [crate::delete] proc macro, but specified for MySQL database
#[proc_macro_attribute]
pub fn mysql_delete(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Delete), Some(Database::Mysql)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into() 
}

/// Same as [crate::delete] proc macro, but specified for SQLite database
#[proc_macro_attribute]
pub fn sqlite_delete(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Delete), Some(Database::Sqlite)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}





