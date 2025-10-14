# Handling NULL Values in sqlx-template

When working with `Option<T>` fields in your structs, you may encounter issues where queries don't return expected results when searching for `NULL` values. This guide explains the problem and provides solutions.

## The Problem

Consider this scenario:

```rust
#[derive(PostgresTemplate, FromRow, Default, Clone, Debug)]
#[table("users")]
#[tp_select_page(by = "org", order = "id desc, org desc")]
pub struct User {
    pub id: i32,
    pub email: String,
    pub org: Option<i32>,  // This can be NULL in database
    // ... other fields
}

// This user has org = NULL
let user_4 = User { 
    email: "user4@abc.com".to_string(),
    org: None,  // NULL value
    // ...
};
```

When you try to query for users with `org = None`:

```rust
// This returns empty results even though user_4 exists!
let org_page = User::find_page_by_org_order_by_id_desc_and_org_desc(
    &None, 
    PageRequest::default(), 
    &db
).await.unwrap();
```

**Why this happens:** The generated query uses `WHERE org = :org`, which becomes `WHERE org = NULL`. In SQL, `NULL = NULL` is always `false` - you need `WHERE org IS NULL` instead.

## Root Cause & Required Fixes

This is a bug in sqlx-template's implementation that needs to be fixed in two places:

### 1. Builder Pattern Implementation Fix

The builder pattern methods need to check if the value is `None` and generate `IS NULL` conditions instead of parameter binding:

**Current problematic code in `generate_basic_methods`:**
```rust
// Always generates: column = placeholder
let eq_condition = format!("{} = {}", column_name, placeholder);
```

**Required fix:**
```rust
// Need to check at runtime if value is None
pub fn org(mut self, value: &'q Option<i32>) -> Result<Self, sqlx::Error> {
    match value {
        Some(val) => {
            self.where_conditions.push(format!("org = ${}", self.where_args.len() + 1));
            self.where_args.add_param(val)?;
        }
        None => {
            self.where_conditions.push("org IS NULL".to_string());
            // No parameter binding for IS NULL
        }
    }
    Ok(self)
}
```

### 2. Derive Macro Function Generation Fix

For `tp_select_*` attributes with `by` parameters, when the field type is `Option<T>`, the generated function should accept `T` as parameter (not `Option<T>`):

**Current problematic generation:**
```rust
// Generated function signature for Option<i32> field
pub async fn find_page_by_org(org: &Option<i32>, ...) -> Result<...>
// SQL: WHERE org = :org (doesn't work for NULL)
```

**Required fix:**
```rust
// For Option<i32> field, generate function that accepts i32
pub async fn find_page_by_org(org: &i32, ...) -> Result<...>
// SQL: WHERE org = :org (works correctly for non-NULL values)
```

This way, the generated functions only handle non-NULL cases, and users can use where attibute or builder pattern for NULL handling.

## Implementation Tasks

To fix this properly in sqlx-template:

1. **Fix builder pattern in `src/sqlx_template/builder/macro_impl.rs`:**
   - Modify `generate_basic_methods` to handle `Option<T>` types
   - Generate runtime checks for `None` values
   - Use `IS NULL` conditions instead of parameter binding for `None`

2. **Fix derive macro generation:**
   - Detect `Option<T>` fields in `by` attributes
   - Generate function signatures that unwrap Option types (accept `T` instead of `Option<T>`)
   - Only handle non-NULL cases in generated functions

3. **Add tests for NULL value scenarios**

4. **Update documentation with correct usage patterns**

## Summary

- **Root Cause**: sqlx-template doesn't properly handle `Option<T>` fields in SQL generation
- **Impact**: Queries with `None` values return empty results instead of matching NULL records
- **Fix Required**: Update both builder pattern and derive macro implementations
- **Recommended Usage**: Use where attribute or builder pattern for NULL handling
