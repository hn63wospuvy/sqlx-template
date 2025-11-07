# Tracing Instrument Attribute

The `instrument` attribute allows you to automatically add `#[tracing::instrument]` attributes to generated database functions for observability and debugging.

## Prerequisites

Enable the `tracing` feature in your `Cargo.toml`:

```toml
[dependencies]
sqlx-template = { version = "0.1", features = ["tracing"] }
tracing = "0.1"  # Also add tracing crate to your dependencies
```

## Usage

### 1. Global Level (applies to all generated functions)

```rust
use sqlx_template::SqliteTemplate;

#[derive(SqliteTemplate, FromRow)]
#[table("users")]
#[instrument = true]  // Enable tracing for all functions
#[tp_select_all(by = "name")]
#[tp_insert]
pub struct User {
    pub id: i32,
    pub name: String,
}

// Generated code will have:
// #[tracing::instrument(name = "User::find_all_by_name")]
// pub async fn find_all_by_name(...)
//
// #[tracing::instrument(name = "User::insert")]
// pub async fn insert(...)
```

### 2. Global with `skip_all` (skip all parameters from trace)

```rust
#[derive(SqliteTemplate, FromRow)]
#[table("logs")]
#[instrument = "skip_all"]  // Skip all parameters
#[tp_select_all]
pub struct Log {
    pub id: i32,
    pub message: String,
}

// Generated:
// #[tracing::instrument(name = "Log::find_all", skip_all)]
// pub async fn find_all(...)
```

### 3. Function-Specific Override

```rust
#[derive(SqliteTemplate, FromRow)]
#[table("sessions")]
#[instrument = true]  // Global default
#[tp_select_one(by = "token", instrument = "skip(conn)")]  // Skip only conn parameter
#[tp_update(by = "id", on = "last_activity", instrument = "skip_all")]  // Skip all
#[tp_delete(by = "token")]  // Uses global: true
pub struct Session {
    pub id: i32,
    pub token: String,
    pub last_activity: DateTime<Utc>,
}

// Generated:
// #[tracing::instrument(name = "Session::find_one_by_token", skip(conn))]
// pub async fn find_one_by_token(token: &str, conn: E) -> ...
//
// #[tracing::instrument(name = "Session::update_by_id", skip_all)]
// pub async fn update_by_id(...) -> ...
//
// #[tracing::instrument(name = "Session::delete_by_token")]
// pub async fn delete_by_token(...) -> ...
```

### 4. No Global, Only Function-Specific

```rust
#[derive(SqliteTemplate, FromRow)]
#[table("cache")]
#[tp_select_one(by = "key")]  // No tracing
#[tp_update(by = "key", on = "value", instrument = true)]  // Enable for this function only
pub struct CacheEntry {
    pub key: String,
    pub value: String,
}
```

## Attribute Values

### Global Level (`#[instrument = ...]`)

- `true` - Enable tracing with all parameters
- `"skip_all"` - Enable tracing but skip all parameters from the trace

**Note**: Global level does NOT support `skip(param1, param2, ...)` because different functions have different parameters.

### Function Level (inside `tp_*` attributes)

- `instrument = true` - Enable tracing with all parameters
- `instrument = "skip_all"` - Enable tracing but skip all parameters
- `instrument = "skip(param1, param2, ...)"` - Skip specific parameters

Example:
```rust
#[tp_select_all(by = "user_id", instrument = "skip(conn)")]
#[tp_update(by = "id", on = "status", instrument = "skip(re, conn)")]
```

## Instrument Name Format

All generated instrument names follow the pattern: `{StructName}::{function_name}`

Examples:
- `User::find_all`
- `User::insert`
- `Session::find_one_by_token`
- `Product::update_by_id`

## Priority

When both global and function-specific instrument attributes are present, **function-specific takes priority**.

```rust
#[derive(SqliteTemplate, FromRow)]
#[table("items")]
#[instrument = true]  // Global: trace all
#[tp_select_one(by = "id", instrument = "skip_all")]  // Override: skip all for this function
pub struct Item {
    pub id: i32,
}

// find_one_by_id will use skip_all (function-level override)
```

## Complete Example

```rust
use sqlx_template::{SqliteTemplate, FromRow};
use chrono::{DateTime, Utc};

#[derive(SqliteTemplate, FromRow, Debug)]
#[table("api_requests")]
#[instrument = "skip_all"]  // Global: skip all by default (high traffic)
#[tp_select_all(by = "endpoint")]
#[tp_select_one(by = "id", instrument = true)]  // Override: trace this specific query
#[tp_insert(instrument = "skip(re, conn)")]  // Override: skip only certain params
#[tp_update(by = "id", on = "status")]  // Uses global: skip_all
pub struct ApiRequest {
    #[auto]
    pub id: i32,
    pub endpoint: String,
    pub status: i32,
    pub created_at: DateTime<Utc>,
}
```

## Without Tracing Feature

If you don't enable the `tracing` feature in `Cargo.toml`, the `instrument` attribute is simply ignored and no tracing code is generated. This allows you to:

1. Add instrument attributes during development
2. Deploy without tracing overhead by not enabling the feature
3. Easily toggle tracing on/off via feature flags

```toml
# Development
[dependencies]
sqlx-template = { version = "0.1", features = ["tracing"] }

# Production (no tracing)
[dependencies]
sqlx-template = { version = "0.1" }
```

## See Also

- [Tracing Documentation](https://docs.rs/tracing/)
- [Builder Pattern](./builder_pattern.md)
- [Null Value Handling](./handling_null_values.md)
