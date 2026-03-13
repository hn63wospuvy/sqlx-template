/// Test edge cases for #[column("name")] attribute:
/// - Single-field struct
/// - Column name same as field name (no-op)
/// - Column name with special characters (underscores, numbers)
/// - Multiple derive macros on same struct (InsertTemplate + SelectTemplate + Columns)
/// - #[column] with #[auto] combined

use sqlx_template::{Columns, InsertTemplate, SelectTemplate, DeleteTemplate, UpdateTemplate};
use sqlx::FromRow;

// Edge case 1: Single field struct
#[derive(Columns, Debug)]
pub struct SingleField {
    #[column("the_value")]
    pub value: String,
}

// Edge case 2: Column name same as field name (explicit but redundant)
#[derive(Columns, Debug)]
pub struct RedundantColumn {
    #[column("id")]
    pub id: i32,
    #[column("name")]
    pub name: String,
}

// Edge case 3: Column names with underscores and numbers
#[derive(Columns, Debug)]
pub struct SpecialNames {
    #[column("field_1")]
    pub first: String,
    #[column("field_2_value")]
    pub second: String,
    #[column("x123")]
    pub third: String,
    #[column("__internal")]
    pub internal: String,
}

// Edge case 4: #[column] with #[auto] combined
#[derive(Columns, Debug)]
pub struct AutoWithColumn {
    #[column("record_id")]
    pub id: i32,
    #[column("record_name")]
    pub name: String,
    pub data: String,
}

// Edge case 5: Multiple derive macros working together
#[derive(InsertTemplate, SelectTemplate, DeleteTemplate, UpdateTemplate, Columns, FromRow, Debug, Clone)]
#[table("items")]
#[db("sqlite")]
#[tp_select_all(by = "id")]
#[tp_select_one(by = "id")]
#[tp_delete(by = "id")]
#[tp_update(by = "id")]
pub struct Item {
    #[auto]
    pub id: i32,
    #[column("item_name")]
    pub name: String,
    #[column("item_desc")]
    pub description: String,
    pub quantity: i32,
}

// Edge case 6: Struct with only #[auto] field having #[column]
#[derive(Columns, Debug)]
pub struct AutoOnlyColumn {
    #[column("record_id")]
    pub id: i32,
    pub name: String,
    pub value: String,
}

// Edge case 7: Large column name
#[derive(Columns, Debug)]
pub struct LongColumnName {
    #[column("very_long_column_name_that_is_quite_descriptive")]
    pub short: String,
    pub normal: String,
}

#[test]
fn test_single_field_column() {
    assert_eq!(SingleField::COLUMNS, ["the_value"]);
    assert_eq!(SingleField::COLUMNS_STR, "the_value");
}

#[test]
fn test_redundant_column_name() {
    assert_eq!(RedundantColumn::COLUMNS, ["id", "name"]);
    assert_eq!(RedundantColumn::COLUMNS_STR, "id, name");
}

#[test]
fn test_special_naming_characters() {
    assert_eq!(SpecialNames::COLUMNS, ["field_1", "field_2_value", "x123", "__internal"]);
    assert_eq!(SpecialNames::COLUMNS_STR, "field_1, field_2_value, x123, __internal");
}

#[test]
fn test_auto_with_column() {
    assert_eq!(AutoWithColumn::COLUMNS, ["record_id", "record_name", "data"]);
    assert_eq!(AutoWithColumn::COLUMNS_STR, "record_id, record_name, data");
}

#[test]
fn test_multiple_derive_macros_with_column() {
    assert_eq!(Item::COLUMNS, ["id", "item_name", "item_desc", "quantity"]);
    assert_eq!(Item::COLUMNS_STR, "id, item_name, item_desc, quantity");
}

#[test]
fn test_auto_only_column() {
    assert_eq!(AutoOnlyColumn::COLUMNS, ["record_id", "name", "value"]);
    assert_eq!(AutoOnlyColumn::COLUMNS_STR, "record_id, name, value");
}

#[test]
fn test_long_column_name() {
    assert_eq!(
        LongColumnName::COLUMNS,
        ["very_long_column_name_that_is_quite_descriptive", "normal"]
    );
}

#[test]
fn test_all_columns_str_consistency() {
    let test_cases: Vec<(&[&str], &str)> = vec![
        (&SingleField::COLUMNS, SingleField::COLUMNS_STR),
        (&RedundantColumn::COLUMNS, RedundantColumn::COLUMNS_STR),
        (&SpecialNames::COLUMNS, SpecialNames::COLUMNS_STR),
        (&AutoWithColumn::COLUMNS, AutoWithColumn::COLUMNS_STR),
        (&Item::COLUMNS, Item::COLUMNS_STR),
        (&AutoOnlyColumn::COLUMNS, AutoOnlyColumn::COLUMNS_STR),
        (&LongColumnName::COLUMNS, LongColumnName::COLUMNS_STR),
    ];
    for (cols, str_val) in test_cases {
        assert_eq!(cols.join(", "), str_val);
    }
}
