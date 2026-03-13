/// Test basic #[column("name")] attribute functionality
/// Verifies COLUMNS, COLUMNS_STR generation and that column names are correct.

use sqlx_template::Columns;

// Test 1: Mixed fields - some with #[column], some without
#[derive(Columns, Debug)]
pub struct UserMixed {
    pub id: i32,
    #[column("q_name")]
    pub name: String,
    pub email: String,
    #[column("q_group")]
    pub group: String,
}

// Test 2: All fields have #[column] attribute
#[derive(Columns, Debug)]
pub struct UserAllColumns {
    #[column("user_id")]
    pub id: i32,
    #[column("user_name")]
    pub name: String,
    #[column("user_email")]
    pub email: String,
}

// Test 3: No fields have #[column] attribute (default behavior)
#[derive(Columns, Debug)]
pub struct UserNoColumns {
    pub id: i32,
    pub name: String,
    pub email: String,
}

// Test 4: Single field with #[column] attribute
#[derive(Columns, Debug)]
pub struct UserSingleColumn {
    pub id: i32,
    #[column("display_name")]
    pub name: String,
}

// Test 5: #[column] with reserved SQL keyword names
#[derive(Columns, Debug)]
pub struct UserReservedKeyword {
    pub id: i32,
    #[column("order")]
    pub sort_order: String,
    #[column("select")]
    pub selection: String,
}

// Test 6: #[column] with various naming conventions
#[derive(Columns, Debug)]
pub struct UserNamingConventions {
    pub id: i32,
    #[column("first_name")]
    pub first: String,
    #[column("last_name")]
    pub last: String,
    #[column("created_at")]
    pub created: String,
    #[column("is_active")]
    pub active: bool,
}

// Test 7: Struct with many fields, some mapped
#[derive(Columns, Debug)]
pub struct LargeStruct {
    pub id: i32,
    #[column("full_name")]
    pub name: String,
    pub email: String,
    #[column("phone_number")]
    pub phone: String,
    pub address: String,
    #[column("zip_code")]
    pub zip: String,
    pub city: String,
    pub country: String,
}

// Test 8: #[column] combined with #[group]
#[derive(Columns, Debug)]
pub struct UserWithGroup {
    pub id: i32,
    #[column("user_name")]
    #[group = "basic"]
    pub name: String,
    #[column("user_email")]
    #[group = "basic"]
    pub email: String,
    #[group = "stats"]
    pub score: i32,
}

#[test]
fn test_mixed_fields_column_attribute() {
    assert_eq!(UserMixed::COLUMNS, ["id", "q_name", "email", "q_group"]);
    assert_eq!(UserMixed::COLUMNS_STR, "id, q_name, email, q_group");
    assert_eq!(UserMixed::as_select_all_fields(), "id, q_name, email, q_group");
}

#[test]
fn test_all_fields_with_column_attribute() {
    assert_eq!(UserAllColumns::COLUMNS, ["user_id", "user_name", "user_email"]);
    assert_eq!(UserAllColumns::COLUMNS_STR, "user_id, user_name, user_email");
    assert_eq!(UserAllColumns::as_select_all_fields(), "user_id, user_name, user_email");
}

#[test]
fn test_no_column_attributes_default() {
    assert_eq!(UserNoColumns::COLUMNS, ["id", "name", "email"]);
    assert_eq!(UserNoColumns::COLUMNS_STR, "id, name, email");
    assert_eq!(UserNoColumns::as_select_all_fields(), "id, name, email");
}

#[test]
fn test_single_field_with_column() {
    assert_eq!(UserSingleColumn::COLUMNS, ["id", "display_name"]);
    assert_eq!(UserSingleColumn::COLUMNS_STR, "id, display_name");
}

#[test]
fn test_reserved_keyword_column_names() {
    assert_eq!(UserReservedKeyword::COLUMNS, ["id", "order", "select"]);
    assert_eq!(UserReservedKeyword::COLUMNS_STR, "id, order, select");
}

#[test]
fn test_various_naming_conventions() {
    assert_eq!(
        UserNamingConventions::COLUMNS,
        ["id", "first_name", "last_name", "created_at", "is_active"]
    );
    assert_eq!(
        UserNamingConventions::COLUMNS_STR,
        "id, first_name, last_name, created_at, is_active"
    );
}

#[test]
fn test_large_struct_mixed_mapping() {
    assert_eq!(
        LargeStruct::COLUMNS,
        ["id", "full_name", "email", "phone_number", "address", "zip_code", "city", "country"]
    );
    assert_eq!(
        LargeStruct::COLUMNS_STR,
        "id, full_name, email, phone_number, address, zip_code, city, country"
    );
}

#[test]
fn test_column_combined_with_group() {
    assert_eq!(UserWithGroup::COLUMNS, ["id", "user_name", "user_email", "score"]);
    assert_eq!(UserWithGroup::COLUMNS_STR, "id, user_name, user_email, score");
}

#[test]
fn test_columns_array_lengths() {
    assert_eq!(UserMixed::COLUMNS.len(), 4);
    assert_eq!(UserAllColumns::COLUMNS.len(), 3);
    assert_eq!(UserNoColumns::COLUMNS.len(), 3);
    assert_eq!(LargeStruct::COLUMNS.len(), 8);
}

#[test]
fn test_columns_str_consistent_with_columns_join() {
    let joined = UserMixed::COLUMNS.join(", ");
    assert_eq!(joined, UserMixed::COLUMNS_STR);
    let joined = UserAllColumns::COLUMNS.join(", ");
    assert_eq!(joined, UserAllColumns::COLUMNS_STR);
    let joined = UserNoColumns::COLUMNS.join(", ");
    assert_eq!(joined, UserNoColumns::COLUMNS_STR);
    let joined = LargeStruct::COLUMNS.join(", ");
    assert_eq!(joined, LargeStruct::COLUMNS_STR);
    let joined = UserWithGroup::COLUMNS.join(", ");
    assert_eq!(joined, UserWithGroup::COLUMNS_STR);
}
