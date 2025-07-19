use sqlx::FromRow;
use sqlx_template::{tp_select_builder, SqlxTemplate};

#[derive(SqlxTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_select_builder]
struct MinimalUser {
    id: i32,
}

#[test]
fn test_minimal_macro() {
    // Test that the macro generates the expected method
    let result = MinimalUser::test_macro();
    assert_eq!(result, "Macro works!");
    println!("Macro compiled and works successfully: {}", result);

    // Test field methods
    let builder = MinimalUser::builder_select();
    let sql = builder.build_sql();
    assert!(sql.contains("SELECT * FROM users"));
    println!("Basic SQL: {}", sql);

    // Test field methods
    let builder = MinimalUser::builder_select()
        .id(123).unwrap();
    let sql = builder.build_sql();
    assert!(sql.contains("WHERE"));
    assert!(sql.contains("id = 123"));
    println!("With WHERE: {}", sql);

    // Test order by methods
    let builder = MinimalUser::builder_select()
        .id(123).unwrap()
        .order_by_id_desc().unwrap();
    let sql = builder.build_sql();
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("id DESC"));
    println!("With ORDER BY: {}", sql);
}
