use sqlx::FromRow;

// Test 1: Global instrument = true
#[derive(sqlx_template::SqliteTemplate, FromRow, Debug, Clone)]
#[table("users1")]
#[instrument = true]
struct User1 {
    id: i64,
    name: String,
}

// Test 2: Global instrument = "skip_all"
#[derive(sqlx_template::SqliteTemplate, FromRow, Debug, Clone)]
#[table("users2")]
#[instrument = "skip_all"]
struct User2 {
    id: i64,
    name: String,
}

// Test 3: Function-specific instrument in tp_select_all
#[derive(sqlx_template::SqliteTemplate, FromRow, Debug, Clone)]
#[table("users3")]
#[tp_select_all(by = "name", instrument = "skip(conn)")]
struct User3 {
    id: i64,
    name: String,
}

// Test 4: Global + function-specific override
#[derive(sqlx_template::SqliteTemplate, FromRow, Debug, Clone)]
#[table("users4")]
#[instrument = true]
#[tp_select_all(by = "name", instrument = "skip_all")]
struct User4 {
    id: i64,
    name: String,
}

#[test]
fn test_instrument_compiles() {
    // This test just verifies that the code compiles successfully
    // The actual tracing behavior would need to be tested at runtime
    // with the tracing feature enabled
}
