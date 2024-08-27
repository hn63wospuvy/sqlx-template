#![allow(warnings)]
use proc_macro::TokenStream;
use quote::quote;
use sqlx_template::raw;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, Meta,
    MetaNameValue, NestedMeta,
};

mod sqlx_template;
mod columns;
mod util;



/// `InsertTemplate` is a derive macro designed to automatically generate record insert functions
/// based on `sqlx`. This macro creates `insert` methods for the struct it is applied to, returning
/// the number of new records added. It assumes that the columns in the database correspond to the fields in the struct.
///
/// # Attributes
///
/// `InsertTemplate` accepts the following attributes:
/// - `table_name`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `auto`: Applied to fields that should be excluded from the insert statement, typically for auto-incrementing primary keys.
///
/// Additionally, if the feature `postgres` is enabled, the library will generate an `insert_return` function that returns the newly inserted record.
///
/// # Example
///
/// ```rust
/// use sqlx_template::InsertTemplate;
///
/// #[derive(InsertTemplate, sqlx::FromRow)]
/// #[table_name = "users"]
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
/// // If the `postgres` feature is enabled
/// #[cfg(feature = "postgres")]
/// let new_user = User.insert_return(&user, &pool).await?;
/// println!("New user: {:?}", new_user);
/// ```
///
/// In the example above:
/// - `table_name` is set to "users", specifying the table to insert into. (mandatory).
/// - `debug_slow` is set to 1000 milliseconds, meaning only queries taking longer than 1 second will be logged for debugging.
/// - The `id` field is marked with `#[auto]`, indicating that it should be excluded from the insert statement, typically for auto-incrementing primary keys.
///

/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated insert methods.
///


#[proc_macro_derive(InsertTemplate, attributes(table_name, auto, debug_slow))]
pub fn insert_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::insert::derive_insert(input) {
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
/// - `table_name`: Specifies the name of the table in the database (mandatory).
/// - `tp_update`: The main configuration for generating the update function, with the following sub-attributes:
///   - `by`: List of columns that will be the update condition, will be the function's input (mandatory and non-empty).
///   - `on`: List of columns that will be updated. If empty, all columns will be updated.
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `op_lock`: The name of the column to apply optimistic locking (optional).
///   - `returning`: if set = true, generated function will return the newly updated record (feature `postgres only`)
///   - `debug_slow`: Configures debug logs for the executed query:
///     - If `0`: Only logs the executed query.
///     - If `> 0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///     - If not configured, no debug logs will be generated.
/// - `debug_slow`: Configures debug logs for the executed query, with priority given to the value in `tp_update`.
/// # Example
///
/// ```rust
/// use sqlx_template::UpdateTemplate;
///
/// #[derive(UpdateTemplate, sqlx::FromRow)]
/// #[table_name = "users"]
/// #[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]
/// #[tp_update(by = "id", on = "email, password", fn_name = "update_user_password")]
/// #[debug_slow = 1000]
/// pub struct User {
///     pub id: i32,
///     pub email: String,
///     pub password: String,
///     pub version: i32
/// }
/// ```
///
/// In the example above:
/// - `table_name` is set to "users", specifying the table to update.
/// - The first `tp_update` generates a function named `update_user` to update record, using `id` as the condition and applying optimistic locking on the `version` column.
/// - The second `tp_update` generates a function named `update_user_password` to update both `email` and `password` columns, using `id` as the condition.
/// - `debug_slow` is set to 1000 milliseconds, meaning only queries taking longer than 1 second will be logged for debugging.
///

/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated update methods.

#[proc_macro_derive(UpdateTemplate, attributes(table_name, tp_update, debug_slow))]
pub fn update_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::update::derive_update(input) {
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
/// - `table_name`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `tp_delete`: The main configuration for generating the delete function, with the following sub-attributes:
///   - `by`: List of columns that will be the delete condition, will be the function's input (mandatory and non-empty).
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `returning`: If set to true, the generated function will return the deleted record (only enabled with the `postgres` feature).
///   - `debug_slow`: Configures debug logs for the executed query:
///     - If set to `0`: Only logs the executed query.
///     - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///     - If not configured, no debug logs will be generated.
///
/// The `debug_slow` attribute at the struct level has priority over the value in `tp_delete`.
///
/// # Example
///
/// ```rust
/// use sqlx_template::DeleteTemplate;
///
/// #[derive(DeleteTemplate, sqlx::FromRow)]
/// #[table_name = "users"]
/// #[tp_delete(by = "id", fn_name = "delete_user", returning = true)]
/// #[tp_delete(by = "id")]
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
/// // If the `postgres` feature is enabled and `returning` is set to true
/// #[cfg(feature = "postgres")]
/// let deleted_user = User::delete_user(&user.id, &pool).await?;
/// println!("Deleted user: {:?}", deleted_user);
/// ```
///
/// In the example above:
/// - `table_name` is set to "users", specifying the table to delete from.
/// - The first `tp_delete` generates a function named `delete_user` to delete a record based on the `id` column and return the deleted record (with the `postgres` feature enabled).
/// - `debug_slow` is set to 1000 milliseconds, meaning only queries taking longer than 1 second will be logged for debugging.
///
/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated delete methods.
///

#[proc_macro_derive(DeleteTemplate, attributes(table_name, tp_delete, debug_slow))]
pub fn delete_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::delete::derive_delete(input) {
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
/// - `table_name`: Specifies the name of the table in the database (mandatory).
/// - `debug_slow`: Configures debug logs for the executed query:
///   - If set to `0`: Only logs the executed query.
///   - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///   - If not configured, no debug logs will be generated.
/// - `tp_select_all`: Generates a function that returns all records as a `Vec<T>`. It has the following sub-attributes:
///   - `by`: List of columns for the `WHERE` condition, used as function input (can be empty).
///   - `fn_name`: The name of the generated function. If empty, the library will automatically generate a function name.
///   - `order`: Adds an `ORDER BY` clause based on the specified columns and order (supports `asc|desc`, default is `asc`).
///   - `debug_slow`: Configures debug logs for the executed query:
///     - If set to `0`: Only logs the executed query.
///     - If set to a value greater than `0`: Only logs the query if the execution time exceeds the configured value (in milliseconds).
///     - If not configured, no debug logs will be generated.
/// - `tp_select_one`: Similar to `tp_select_all`, but returns a single record as `Option<T>`.
/// - `tp_select_stream`: Similar to `tp_select_all`, but returns an `impl Stream<Item = T>`.
/// - `tp_select_count`: Similar to `tp_select_all`, but returns the count of records as `i64`.
/// - `tp_select_page`: Similar to `tp_select_all`, but accepts `offset` and `limit` as inputs, and returns a tuple of all records and the total count.
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
/// #[table_name = "users"]
/// #[tp_select_one(by = "id", fn_name = "find_user_by_id")]
/// #[tp_select_all(by = "id, email", order = "id desc", fn_name = "find_all_users")]
/// #[tp_select_count(by = "id", fn_name = "count_users_by_id")]
/// #[tp_select_page(by = "id", fn_name = "find_users_page")]
/// #[tp_select_stream(by = "email", order = "id desc, email", fn_name = "stream_users_by_email", debug_slow = -1)]
/// #[debug_slow = 1000]
/// pub struct User {
///     #[auto]
///     pub id: i32,
///     pub email: String,
///     pub password: String,
/// }
///
/// // Example usage:
/// // Find user by id
/// let user = User::find_user_by_id(&pool, 1).await?;
/// println!("Found user: {:?}", user);
///
/// // Find all users
/// let users = User::find_all_users(&pool, Some("example@example.com")).await?;
/// println!("All users: {:?}", users);
///
/// // Count users by id
/// let user_count = User::count_users_by_id(&pool, Some(1)).await?;
/// println!("User count: {}", user_count);
///
/// // Find users with pagination
/// let (users, total_count) = User::find_users_page(&pool, 0, 10).await?;
/// println!("Users: {:?}, Total count: {}", users, total_count);
///
/// // Stream users by email
/// let mut user_stream = User::stream_users_by_email(&pool, "example@example.com").await?;
/// while let Some(user) = user_stream.next().await {
///     println!("Streamed user: {:?}", user);
/// }
/// ```
///
/// In the example above:
/// - `table_name` is set to "users", specifying the table to query from.
/// - `tp_select_one` generates a function named `find_user_by_id` to find a user by `id`.
/// - `tp_select_all` generates a function named `find_all_users` to find all users by `id` and `email`, ordered by `id` in descending order.
/// - `tp_select_count` generates a function named `count_users_by_id` to count users by `id`.
/// - `tp_select_page` generates a function named `find_users_page` to find users with pagination by `id`.
/// - `tp_select_stream` generates a function named `stream_users_by_email` to stream users by `email`, ordered by `id` in descending order and then by `email`, with debug logging if the query execution time exceeds the configured value.
///

///
/// # Note
///
/// This macro relies on `sqlx`, so you need to add `sqlx` to your `[dependencies]` in `Cargo.toml`
/// and properly configure the database connection before using the generated query methods.
///

#[proc_macro_derive(SelectTemplate, attributes(table_name, debug_slow, tp_select_all, tp_select_one, tp_select_page, tp_select_stream, tp_select_count))]
pub fn select_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::select::derive_select(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_derive(Columns, attributes(group))]
pub fn columns_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match columns::derive(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}



#[proc_macro_derive(DDLTemplate, attributes(column, table_name, index))]
pub fn ddl_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::ddl::derive(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}



/// The `TableName` derive macro automatically generates a `table_name` function
/// for a struct, returning the value specified in the `table_name` attribute.
///
/// # Syntax
///
/// ```rust
/// #[derive(TableName)]
/// #[table_name = "<table_name>"]
/// pub struct <StructName> { ... }
/// ```
///
/// # Attributes
///
/// - `table_name`: Specifies the name of the table as a string (e.g., `#[table_name = "users"]`).
///
/// # Function Signature
///
/// The macro generates a const function named table_name() which returns a `&'static str` containing the table name
///
/// # Example Usage
///
/// ```rust
/// #[derive(TableName)]
/// #[table_name = "users"]
/// pub struct User {
///     pub id: i32,
///     pub name: String,
///     pub age: i32,
/// }
///
///
/// fn main() {
///     assert_eq!(User::table_name(), "users");
/// }
#[proc_macro_derive(TableName, attributes(table_name))]
pub fn table_name_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::table_name_derive(input) {
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
    match raw::multi_query_derive(input, args, None) {
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
    match raw::query_derive(input, args, None) {
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
    match raw::query_derive(input, args, Some(util::Mode::Select)) {
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
    match raw::query_derive(input, args, Some(util::Mode::Update)) {
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
    match raw::query_derive(input, args, Some(util::Mode::Insert)) {
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
    match raw::query_derive(input, args, Some(util::Mode::Delete)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}