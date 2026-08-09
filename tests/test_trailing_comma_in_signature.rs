//! A trailing comma in a query function's parameter list must compile.
//!
//! This is a compile-time regression test: building this file **is** the assertion. If the macro
//! reintroduces the unconditional `,` join, this file stops compiling, and it does so next to the
//! shape that caused it instead of surfacing as `custom attribute panicked` at a `<stdin>`
//! location inside a rustfmt subprocess the user never ran.
//!
//! Why the shape is not exotic: `rustfmt` appends a trailing comma every time it breaks a
//! parameter list across lines, which it does for any signature past `max_width` (100 by default)
//! under the default `fn_params_layout = "Tall"`. Running `cargo fmt` on a crate that uses these
//! macros was therefore enough to break its own build.
//!
//! The commas below are deliberate. Do not "tidy" them away.

#![allow(dead_code, reason = "compiling these signatures IS the assertion")]

use sqlx::FromRow;
use sqlx_template::{multi_query, postgres_query};

#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct UserRow {
    id: i32,
    name: String,
}

// The shape rustfmt produces: parameters broken across lines, trailing comma.
#[postgres_query("SELECT id, name FROM users WHERE id = :user_id AND name = :user_name")]
async fn find_user_multiline(
    user_id: i32,
    user_name: String,
) -> Vec<UserRow> {
}

// The same trailing comma on ONE line. This is the minimal reproduction: it differs from the
// known-good signature below by exactly one character, which is what pins the comma — rather than
// the line breaking — as the cause.
#[postgres_query("SELECT id, name FROM users WHERE id = :user_id")]
async fn find_user_one_line_trailing(user_id: i32,) -> Vec<UserRow> {}

// No trailing comma: the shape that always worked. Kept so that a "fix" which breaks the ordinary
// case cannot pass by repairing only the new one.
#[postgres_query("SELECT id, name FROM users WHERE id = :user_id")]
async fn find_user_no_trailing(user_id: i32) -> Vec<UserRow> {}

// One parameter with a trailing comma — the boundary between "no parameters" and "several".
#[postgres_query("SELECT id, name FROM users WHERE name = :user_name")]
async fn find_by_name_single_trailing(
    user_name: String,
) -> Vec<UserRow> {
}

// No parameters at all: the branch that must keep emitting `(conn: E)` with no leading comma.
#[postgres_query("SELECT id, name FROM users")]
async fn all_users() -> Vec<UserRow> {}

// `multi_query` carried the same unconditional join, written inline in its own `quote!`. Fixing
// only the single-query path would have left this one broken.
#[multi_query(
    "UPDATE users SET name = :new_name WHERE id = :user_id; DELETE FROM users WHERE id = :user_id;"
)]
#[db("postgres")]
async fn rename_then_delete(
    user_id: i32,
    new_name: String,
) {
}

/// Nothing to execute: the generated bodies need a live database, and the generated functions are
/// generic over the executor so they cannot even be named without one. A test binary that links
/// is the proof — every signature above expanded into valid Rust.
#[test]
fn trailing_comma_signatures_compile() {}
