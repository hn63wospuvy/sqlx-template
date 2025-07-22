use sqlx_template::PostgresTemplate;
use sqlx::FromRow;

// Test case 1: Placeholder mapped to column
#[derive(PostgresTemplate, FromRow, Debug)]
#[table("test_table")]
#[tp_select_one(by = "id", where = "user = :user and name1 = :name1")]
pub struct TestStruct {
    pub id: i32,
    pub user: String,
    pub name1: String,
    pub name2: String,
    pub full: String,
}

// Test case 2: Placeholder with custom type (not mapped to column)
#[derive(PostgresTemplate, FromRow, Debug)]
#[table("test_table2")]
#[tp_select_all(where = "status = :status and age > :min_age$i32")]
pub struct TestStruct2 {
    pub id: i32,
    pub status: String,
    pub age: i32,
}

// Test case 3: Mixed placeholders - some mapped, some with custom types
#[derive(PostgresTemplate, FromRow, Debug)]
#[table("test_table3")]
#[tp_select_all(where = "name = :name and score > :min_score$f64 and data like :search_pattern", fn_name = "find_by_name_and_score")]
pub struct TestStruct3 {
    pub id: i32,
    pub name: String,
    pub score: f64,
    pub data: String,
}

#[tokio::main]
async fn main() {
    println!("=== Test Placeholder Mapping ===");

    // Test case 1: Placeholders mapped to columns
    println!("TestStruct::select_one_by_id signature:");
    println!("- Parameters: id (from by), user (from :user mapped to user column), name1 (from :name1 mapped to name1 column)");

    // Test case 2: Placeholder with custom type
    println!("TestStruct2::select_all signature:");
    println!("- Parameters: status (from :status mapped to status column), min_age (from :min_age$i32 with custom type)");

    // Test case 3: Mixed placeholders
    println!("TestStruct3::select_all signature:");
    println!("- Parameters: name (from :name mapped to name column), min_score (from :min_score$f64 with custom type)");

    println!("All tests compiled successfully!");

    

}
