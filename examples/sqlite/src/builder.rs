use sqlx::{types::Text, FromRow};
use sqlx_template::{tp_select_builder, tp_update_builder, tp_delete_builder, SqlxTemplate};
use chrono::{DateTime, Utc};

#[derive(SqlxTemplate, FromRow, Debug, Clone)]
#[table("users")]
#[db("sqlite")]
#[tp_select_builder]
#[tp_update_builder]
#[tp_delete_builder]
struct User {
    id: i32,
    email: String,
    age: i32,
    score: f64,
    active: bool,
    actived: Text<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}



fn main() {
    println!("=== SQLite Builder Example ===");

    // Create proper typed values
    let age_threshold: i32 = 18;
    let score_threshold: f64 = 85.5;
    let active_flag: bool = true;
    let cutoff_date = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z").unwrap().with_timezone(&Utc);

    // SELECT builder
    let select_builder = User::builder_select()
        .email("test@example.com")
        .age_gt(age_threshold)
        .score_gte(score_threshold)
        .active(active_flag)
        .created_at_lt(cutoff_date)
        .order_by_id_desc()
        .order_by_email_asc()
        ;

    println!("SELECT SQL: {}", select_builder.build_sql());

    // UPDATE builder
    let new_age: i32 = 25;
    let new_score: f64 = 95.5;
    let new_active: bool = false;
    let user_id: i32 = 123;

    let update_builder = User::builder_update()
        .on_email("new@example.com")
        .on_age(new_age)
        .on_score(new_score)
        .on_active(new_active)
        .by_id(user_id)
        .by_email_like("%@old-domain.com");

    println!("UPDATE SQL: {}", update_builder.build_sql());

    // DELETE builder
    let min_age: i32 = 13;
    let inactive_flag: bool = false;
    let old_cutoff = DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap().with_timezone(&Utc);

    let delete_builder = User::builder_delete()
        .email_end_with("@spam.com")
        .age_lt(min_age)
        .active(inactive_flag)
        .created_at_lt(old_cutoff);

    println!("DELETE SQL: {}", delete_builder.build_sql());

    // Test type safety - these should cause compile errors
    // let wrong_builder = User::builder_select()
    //     .age_gt("not a number")  // Should fail: String is not i32
    //     .score_gte(true)         // Should fail: bool is not f64
    //     .active("yes");          // Should fail: String is not bool
}
