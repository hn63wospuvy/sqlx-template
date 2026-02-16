# Tracing Instrument Attribute

When the `tracing` feature is enabled, **all generated database functions automatically get `#[tracing::instrument]` attributes with `skip_all`** for observability and debugging. You can override this default behavior on a per-function basis using the `instrument` attribute on `tp_*` macros.

## Prerequisites

Enable the `tracing` feature in your `Cargo.toml`:

```toml
[dependencies]
sqlx-template = { version = "0.1", features = ["tracing"] }
tracing = "0.1"  # Also add tracing crate to your dependencies
```

## Automatic Instrumentation

### Default Behavior

When the `tracing` feature is enabled, **every generated function automatically includes**:
```rust
#[tracing::instrument(name = "{StructName}::{function_name}", skip_all)]
```

This means you don't need to do anything - just enable the feature and all functions will be traced:

```rust
use sqlx_template::SqliteTemplate;

#[derive(SqliteTemplate, FromRow)]
#[table("users")]
#[tp_select_all(by = "name")]
#[tp_insert]
#[tp_select_builder]
pub struct User {
    pub id: i32,
    pub name: String,
}

// All generated functions automatically include:
// #[tracing::instrument(name = "User::find_all_by_name", skip_all)]
// pub async fn find_all_by_name(name: &str, conn: E) -> ...
//
// #[tracing::instrument(name = "User::insert", skip_all)]
// pub async fn insert(re: &User, conn: E) -> ...
//
// #[tracing::instrument(name = "User::builder_find_all", skip_all)]
// pub async fn find_all(self, conn: E) -> ...  // Inside builder
```

### Why `skip_all` by Default?

Database connections and query parameters often contain sensitive data or complex types that:
- Don't implement `Debug` trait (required by tracing)
- Shouldn't be logged (passwords, tokens, personal data)
- Create excessive log noise (large result sets, binary data)

Using `skip_all` by default provides safe, clean traces that focus on **which queries are running** rather than **what data they contain**.

## Function-Specific Override

You can override the default `skip_all` behavior on individual functions:

```rust
#[derive(SqliteTemplate, FromRow)]
#[table("sessions")]
#[tp_select_one(by = "token", instrument = "skip(conn)")]  // Override: skip only conn
#[tp_update(by = "id", on = "last_activity", instrument = true)]  // Override: trace all parameters
#[tp_delete(by = "token")]  // Uses default: skip_all
pub struct Session {
    pub id: i32,
    pub token: String,
    pub last_activity: DateTime<Utc>,
}

// Generated:
// #[tracing::instrument(name = "Session::find_one_by_token", skip(conn))]
// pub async fn find_one_by_token(token: &str, conn: E) -> ...  // token will be in trace
//
// #[tracing::instrument(name = "Session::update_by_id")]  // All params in trace
// pub async fn update_by_id(...) -> ...
//
// #[tracing::instrument(name = "Session::delete_by_token", skip_all)]  // Default
// pub async fn delete_by_token(token: &str, conn: E) -> ...
```

## Builder Pattern Support

Builder methods are also automatically instrumented with `skip_all`:

```rust
#[derive(SqliteTemplate, FromRow)]
#[table("products")]
#[tp_select_builder]
#[tp_update_builder]
#[tp_delete_builder]
pub struct Product {
    pub id: i32,
    pub name: String,
    pub price: f64,
}

// All builder methods automatically get skip_all:
// SELECT Builder:
// - builder_find_one: #[instrument(name = "Product::builder_find_one", skip_all)]
// - builder_find_all: #[instrument(name = "Product::builder_find_all", skip_all)]
// - builder_find_page: #[instrument(name = "Product::builder_find_page", skip_all)]
// - builder_count: #[instrument(name = "Product::builder_count", skip_all)]
// - builder_stream: #[instrument(name = "Product::builder_stream", skip_all)]
//
// UPDATE Builder:
// - builder_update_execute: #[instrument(name = "Product::builder_update_execute", skip_all)]
//
// DELETE Builder:
// - builder_delete_execute: #[instrument(name = "Product::builder_delete_execute", skip_all)]
```

## Raw Query Macros

Raw query macros (`#[query]`, `#[select]`, `#[insert]`, `#[update]`, `#[delete]`, `#[multi_query]`) also get automatic instrumentation:

```rust
#[query(query = "DELETE FROM old_data WHERE created_at < :cutoff")]
pub async fn cleanup_old_data(cutoff: &str, conn: &SqlitePool) -> Result<(), sqlx::Error> {}

// Generated:
// #[tracing::instrument(name = "cleanup_old_data", skip_all)]
// pub async fn cleanup_old_data(cutoff: &str, conn: &SqlitePool) -> Result<(), sqlx::Error> { ... }
```

## Override Attribute Values

You can use these values in the `instrument` parameter of `tp_*` attributes:

- `instrument = true` - Trace all parameters (be careful with sensitive data!)
- `instrument = "skip_all"` - Skip all parameters (same as default, usually not needed)
- `instrument = "skip(param1, param2)"` - Skip specific parameters

Examples:
```rust
// Skip only the connection parameter, include query params in trace
#[tp_select_all(by = "user_id", instrument = "skip(conn)")]

// Skip both the struct parameter and connection
#[tp_update(by = "id", on = "status", instrument = "skip(re, conn)")]

// Include everything (useful for debugging, but be careful!)
#[tp_insert(instrument = true)]
```

## Instrument Name Format

All generated instrument names follow these patterns:

### Template Functions
Format: `{StructName}::{function_name}`
- `User::find_all`
- `User::insert`
- `Session::find_one_by_token`
- `Product::update_by_id`

### Builder Functions
Format: `{StructName}::builder_{method_name}`
- `User::builder_find_one`
- `User::builder_find_all`
- `User::builder_find_page`
- `User::builder_count`
- `User::builder_stream`
- `User::builder_update_execute`
- `User::builder_delete_execute`

### Raw Query Functions
Format: `{function_name}` (just the function name)
- `cleanup_old_data`
- `get_user_stats`
- `update_batch`

## Parameter Name Convention

All generated functions use `conn` as the parameter name for the database executor. This makes it consistent and easy to skip when overriding:

```rust
#[tp_select_all(by = "name", instrument = "skip(conn)")]  // Works for ALL generated functions
```

You can be confident that using `skip(conn)` will work across:
- Insert functions: `insert(re: &T, conn: E)`
- Update functions: `update_by_*(args..., re: &T, conn: E)`
- Select functions: `find_*(args..., conn: E)`
- Delete functions: `delete_by_*(args..., conn: E)`
- Builder methods: `find_all(self, conn: E)`, `execute(self, conn: E)`

## Complete Example

```rust
use sqlx_template::{SqliteTemplate, FromRow};
use chrono::{DateTime, Utc};

#[derive(SqliteTemplate, FromRow, Debug)]
#[table("api_requests")]
// No #[instrument] attribute needed at struct level!
// All functions automatically get skip_all when tracing feature is enabled
#[tp_select_all(by = "endpoint")]  // Auto: skip_all
#[tp_select_one(by = "id", instrument = "skip(conn)")]  // Override: trace id parameter
#[tp_insert]  // Auto: skip_all (good for sensitive data)
#[tp_update(by = "id", on = "status")]  // Auto: skip_all
#[tp_select_builder]  // Builder methods: auto skip_all
#[tp_update_builder]
pub struct ApiRequest {
    #[auto]
    pub id: i32,
    pub endpoint: String,
    pub status: i32,
    pub created_at: DateTime<Utc>,
}

// Raw query macro - also auto-instrumented
#[query(query = "DELETE FROM api_requests WHERE created_at < :cutoff")]
pub async fn cleanup_old_requests(cutoff: &str, conn: &SqlitePool) -> Result<(), sqlx::Error> {}

// Usage with tracing:
async fn example(pool: &SqlitePool) {
    // All calls automatically traced with skip_all
    let requests = ApiRequest::find_all_by_endpoint("/api/users", pool).await?;
    // Trace: User::find_all_by_endpoint (no params in log)
    
    // find_one_by_id traces with id parameter visible
    let req = ApiRequest::find_one_by_id(&123, pool).await?;
    // Trace: User::find_one_by_id { id: 123 }
    
    // Builder pattern also traced
    let active = ApiRequest::builder_select()
        .status(&200)?
        .find_all(pool)  // Traced: "ApiRequest::builder_find_all", skip_all
        .await?;
    
    // Cleanup traced automatically
    cleanup_old_requests("2024-01-01", pool).await?;
    // Trace: cleanup_old_requests (no params in log)
}
```

## Without Tracing Feature

If you don't enable the `tracing` feature in `Cargo.toml`, **no tracing code is generated at all**. This allows you to:

1. Develop with tracing enabled for debugging
2. Deploy without tracing overhead by disabling the feature
3. Toggle tracing on/off via feature flags without code changes

```toml
# Development with tracing
[dependencies]
sqlx-template = { version = "0.1", features = ["tracing"] }
tracing = "0.1"

# Production without tracing (smaller binary, no overhead)
[dependencies]
sqlx-template = { version = "0.1" }
```

## Key Differences from Previous Versions

**Before (v0.1.x):**
- Required `#[instrument = "..."]` attribute on struct to enable tracing
- Functions without global attribute had no tracing

**Now (v0.2.x+):**
- **Automatic**: All functions traced when feature is enabled
- **No struct-level attribute needed**: Just enable the `tracing` feature
- **Default `skip_all`**: Safe and clean traces by default
- **Override at function level**: Use `instrument` on `tp_*` macros to customize

## See Also

- [Tracing Documentation](https://docs.rs/tracing/)
- [Builder Pattern](./builder_pattern.md)
- [Null Value Handling](./handling_null_values.md)
