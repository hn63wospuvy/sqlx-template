# Builder Pattern Support

The macro supports fluent builder patterns for query construction with custom WHERE conditions.

## Builder Attributes

- `tp_select_builder`: Builder pattern configuration for SELECT operations
- `tp_update_builder`: Builder pattern configuration for UPDATE operations  
- `tp_delete_builder`: Builder pattern configuration for DELETE operations

## Custom Condition Syntax

Custom conditions are defined using the following syntax:

```rust
#[tp_select_builder(
    method_name = "SQL_expression_with_placeholders"
)]
```

### Parameter Types

- **Auto-mapping**: `:field_name` automatically maps to struct field types
- **Explicit types**: `:param$Type` for custom parameter types
- **Multiple parameters**: Single condition can accept multiple parameters

### Examples

```rust
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range = "score BETWEEN :min$i32 AND :max$i32",
    with_active_status = "active = :active"  // Auto-mapped to bool
)]
```

## Generated Methods

For each field, the builder generates:

### Field-based Methods
- **Equality**: `.field_name(value)`, `.field_name_not(value)`
- **Comparison**: `.field_name_gt(value)`, `.field_name_gte(value)`, `.field_name_lt(value)`, `.field_name_lte(value)`
- **String operations**: `.field_name_like(pattern)`, `.field_name_start_with(prefix)`, `.field_name_end_with(suffix)`
- **Ordering**: `.order_by_field_asc()`, `.order_by_field_desc()`

### Builder-specific Methods

#### SELECT Builder
- **Query execution**: `.find_all()`, `.find_one()`, `.count()`, `.find_page()`, `.stream()`
- **SQL generation**: `.build_sql()`

#### UPDATE Builder  
- **SET clauses**: `.on_field_name(value)` - specify which fields to update
- **WHERE clauses**: `.by_field_name(value)` - specify which records to update
- **Execution**: `.execute()` - returns number of affected rows

#### DELETE Builder
- **WHERE clauses**: `.field_name(value)` - specify which records to delete
- **Execution**: `.execute()` - returns number of deleted rows

## Usage Examples

### SELECT Builder

```rust,no_run
# use sqlx_template::SqliteTemplate;
# use sqlx::{FromRow, SqlitePool};
# #[derive(SqliteTemplate, FromRow, Debug, Clone)]
# #[table("users")]
# #[tp_select_builder(
#     with_email_domain = "email LIKE :domain$String",
#     with_score_range = "score BETWEEN :min$i32 AND :max$i32"
# )]
# pub struct User {
#     pub id: i32,
#     pub email: String,
#     pub score: i32,
# }
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
# let pool = SqlitePool::connect(":memory:").await?;
let users = User::builder_select()
    .email("john@example.com")?           // Field-based condition
    .score_gte(&75)?                      // Generated comparison method
    .with_email_domain("%@company.com")?  // Custom condition
    .with_score_range(60, 90)?            // Custom condition with multiple params
    .order_by_score_desc()?               // Generated ORDER BY method
    .find_all(&pool)
    .await?;
# Ok(())
# }
```

### UPDATE Builder

```rust,no_run
# use sqlx_template::SqliteTemplate;
# use sqlx::{FromRow, SqlitePool};
# #[derive(SqliteTemplate, FromRow, Debug, Clone)]
# #[table("users")]
# #[tp_update_builder(
#     with_high_score = "score > :threshold$i32"
# )]
# pub struct User {
#     pub id: i32,
#     pub email: String,
#     pub active: bool,
#     pub score: i32,
# }
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
# let pool = SqlitePool::connect(":memory:").await?;
# let user_id = 1;
let affected = User::builder_update()
    .on_email("newemail@example.com")?    // SET email = ?
    .on_active(&true)?                    // SET active = ?
    .by_id(&user_id)?                     // WHERE id = ?
    .with_high_score(80)?                 // Custom WHERE condition
    .execute(&pool)
    .await?;
# Ok(())
# }
```

### DELETE Builder

```rust,no_run
# use sqlx_template::SqliteTemplate;
# use sqlx::{FromRow, SqlitePool};
# #[derive(SqliteTemplate, FromRow, Debug, Clone)]
# #[table("users")]
# #[tp_delete_builder(
#     with_old_accounts = "created_at < :cutoff$String"
# )]
# pub struct User {
#     pub id: i32,
#     pub active: bool,
#     pub created_at: String,
# }
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
# let pool = SqlitePool::connect(":memory:").await?;
let deleted = User::builder_delete()
    .active(&false)?                      // WHERE active = false
    .with_old_accounts("2023-01-01")?     // Custom WHERE condition
    .execute(&pool)
    .await?;
# Ok(())
# }
```

## Validation

- **Table alias validation**: Prevents use of table aliases (e.g., `u.field`) in custom conditions
- **Column validation**: Ensures referenced columns exist in the struct
- **Type safety**: Compile-time parameter type checking
- **SQL injection protection**: Uses parameterized queries

## Performance Features

- **Compile-time optimization**: SQL templates pre-generated at compile time
- **Minimal runtime overhead**: Reduced `format!` calls and string allocations
- **Efficient parameter binding**: Direct parameter binding without intermediate formatting

# Document generated by LLMs

