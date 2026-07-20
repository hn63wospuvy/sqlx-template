---
name: sqlx-template
description: >
  Use when writing Rust database code with the sqlx-template crate. Covers generating
  CRUD query functions from a struct via derive macros (tp_select_all/one/page/count/stream,
  tp_insert, tp_update, tp_delete, tp_upsert), building dynamic queries with
  tp_select_builder / tp_update_builder / tp_delete_builder, writing raw-SQL functions with
  #[query]/#[select]/#[insert]/#[update]/#[delete]/#[multi_query] and their postgres_/mysql_/sqlite_
  variants, and using Columns / COLUMNS_STR / the $StructName template / tp_gen. Works with
  Postgres, MySQL and SQLite. (Does not cover DDL / DDLTemplate.)
---

# sqlx-template

## Overview

`sqlx-template` is a proc-macro crate on top of [`sqlx`](https://crates.io/crates/sqlx) that
generates database-access code at compile time so you don't hand-write `sqlx::query*` boilerplate.
It targets **Postgres, MySQL, SQLite** (and `sqlx::Any`).

There are **three code-generation mechanisms**. Pick by what the query needs:

| Mechanism | Use it for | Produces |
|---|---|---|
| **Derive macros** + `#[tp_*]` attributes on a struct | Single-table CRUD with a **fixed shape** (no JOIN, no `SELECT ... AS` projection) | Associated fns: `User::find_one_by_email(&email, &db)`, `User::insert(&u, &db)` |
| **Query builder** (`#[tp_select_builder]` …) | **Dynamic** queries where conditions/order are decided at runtime | Fluent builder: `User::builder_select().active(&true)?.find_all(&db).await?` |
| **Raw-SQL macros** (`#[query]`, `#[select]`, …) | **Anything else** — JOINs, `SELECT col AS alias`, aggregates, hand-written SQL, migrations | Free async fns from a SQL string with `:named` params |

> **Rule of thumb (as the user framed it):** *every query that has no JOIN and no `SELECT ... AS`
> aliasing can be written with a derive macro.* The moment you need a JOIN or a custom projection,
> drop down to a raw-SQL macro (section 4).

---

## 1. General usage & setup

### 1.1 Dependencies

`sqlx-template` needs `sqlx` alongside it. Enable the `sqlx` features for your database(s):

```toml
[dependencies]
sqlx-template = "0.2"
sqlx = { version = "0.8.6", features = ["runtime-tokio", "postgres", "macros", "chrono", "uuid"] }
# swap/add "sqlite" / "mysql" for those backends
tokio   = { version = "1", features = ["rt-multi-thread", "macros"] }
chrono  = { version = "0.4", features = ["serde"] }
futures = "0.3"   # needed to consume streams (StreamExt::next)
```

### 1.2 Logging & tracing features

`sqlx-template` declares exactly two cargo features, both **off by default**:

- `log`  → `#[debug_slow]` logging goes through `log::debug!` (add the `log` crate).
- `tracing` → logging goes through `tracing::debug!` **and** every generated fn gets
  `#[tracing::instrument(name = "Struct::fn", skip_all, err)]` (add the `tracing` crate).
- **Neither (default)** → logging uses `println!`, and no instrument attributes are generated.

### 1.3 A minimal struct

Every templated struct needs `sqlx::FromRow`, a `#[table("...")]`, and a database choice.

```rust
use sqlx_template::PostgresTemplate;

#[derive(PostgresTemplate, sqlx::FromRow, Default, Clone, Debug)]
#[table("users")]                       // mandatory — panics if missing
#[tp_select_one(by = "email")]          // -> User::find_one_by_email
#[tp_delete(by = "id")]                 // -> User::delete_by_id
pub struct User {
    #[auto] pub id: i32,                 // #[auto] = excluded from INSERT column list
    pub email: String,
    pub password: String,
    #[auto] pub version: i32,
    #[auto] pub created_at: chrono::DateTime<chrono::Utc>,
}
```

### 1.4 Choosing the database — two styles

**A. Database-specific derive (no `#[db]` needed):**

| Derive | Target |
|---|---|
| `PostgresTemplate` | Postgres — `$1` placeholders, `RETURNING`, `ON CONFLICT` upsert |
| `MysqlTemplate` | MySQL — `?` placeholders, `ON DUPLICATE KEY UPDATE` |
| `SqliteTemplate` | SQLite — `?` placeholders |
| `AnyTemplate` | `sqlx::Any` |

**B. Generic `SqlxTemplate` + `#[db("...")]`** — `SqlxTemplate` reads the DB from a mandatory
`#[db]` attribute. Accepts `"postgres"`/`"postgresql"`, `"mysql"`, `"sqlite"`, `"any"`:

```rust
#[derive(SqlxTemplate, sqlx::FromRow)]
#[table("organizations")]
#[db("postgres")]
pub struct Organization { /* ... */ }
```

The granular derives `InsertTemplate` / `UpdateTemplate` / `SelectTemplate` / `DeleteTemplate` /
`UpsertTemplate` / `TableName` / `Columns` also require `#[db("...")]` and can be combined.
`SqlxTemplate` and the four DB-specific macros are the aggregate of all of them
(+ `TableName` + `Columns`).

### 1.5 Calling generated functions — the `conn` argument

Every generated function takes the executor as its **last parameter, named `conn`**
(generic `E: sqlx::Executor`). Pass either a pool reference or a transaction:

```rust
let db = sqlx::PgPool::connect(&dsn).await?;

User::insert(&user, &db).await?;                         // &Pool
let u = User::find_one_by_email(&email, &db).await?;     // -> Option<User>

// Transaction: pass &mut *tx
let mut tx = db.begin().await?;
User::insert(&user, &mut *tx).await?;
User::delete_by_id(&1, &mut *tx).await?;
tx.commit().await?;
```

### 1.6 Field attributes

- **`#[auto]`** — the field is DB-managed (auto-increment PK, `version`, `created_at`). It is
  **excluded from generated INSERT** statements. (It is still part of `COLUMNS` and SELECTs.)
- **`#[column("db_name")]`** — maps a Rust field name to a different DB column. Generated SQL uses
  `db_name`, but **`by` / `on` / `order` keys and builder method names always use the Rust FIELD
  name**, never the column name:

```rust
#[column("org_name")] pub name: String,
// #[tp_select_one(by = "name")]  ->  ... WHERE org_name = $1   (fn: find_one_by_name)
```

### 1.7 `#[debug_slow]`

Per-query logging, settable at struct level (default for all fns) and overridable per `#[tp_*]`:

- `debug_slow = 0` → always logs the SQL before running it.
- `debug_slow = 1000` → logs only queries that take **≥ 1000 ms**.
- `debug_slow = -1` → disabled (use to override an inherited struct-level default).
- absent → no logging code emitted.

Output sink follows the feature flags in §1.2 (`log` / `tracing` / `println!`).

---

## 2. Derive macros (`#[tp_*]` attributes)

All attributes below go **on the struct**. `by` / `on` / `order` values are comma-separated lists of
**Rust field names**.

> ⚠️ **Field names are sorted ALPHABETICALLY in the generated function name.**
> `by = "id, email"` and `by = "email, id"` both generate `..._by_email_and_id` (e→i).
> Multi-field names join with `_and_`. Use `fn_name = "..."` to pick an exact name and avoid surprises.

### 2.1 SELECT — `tp_select_all / one / count / page / stream`

Common keys: `by`, `order`, `where`, `fn_name`, `debug`, `instrument`.
`order` entries are `"field"` or `"field desc"` (default `asc`).

| Attribute | Fn name prefix | Returns |
|---|---|---|
| `tp_select_all` | `find_` | `Result<Vec<T>>` |
| `tp_select_one` | `find_one_` | `Result<Option<T>>` (fetch_optional) |
| `tp_select_count` | `count_` | `Result<i64>` (ignores `order`; **`order`-only generates nothing**) |
| `tp_select_page` | `find_page_` | `Result<(Vec<T>, Option<i64>)>` — extra `page: impl Into<(i64,i32,bool)>` param |
| `tp_select_stream` | `stream_` | `BoxStream<'c, Result<T>>` — **not `async`** (returns a stream) |

`SelectTemplate` also **always** generates `find_all`, `count_all`, and `find_page_all` (no attribute needed).

```rust
#[tp_select_all(by = "org", order = "id desc")]   // find_by_org_order_by_id_desc(org, conn) -> Vec<User>
#[tp_select_one(by = "id", fn_name = "get_last")] // get_last(id, conn) -> Option<User>
#[tp_select_page(by = "org", order = "id desc")]  // find_page_by_org_order_by_id_desc(org, page, conn)
#[tp_select_stream(order = "id desc")]            // stream_order_by_id_desc(conn) -> BoxStream

// page call:  (offset, limit, want_count)
let (rows, total) = User::find_page_all((0, 20, true), &db).await?;  // total: Option<i64>
```

Param typing: `String` field → `&str`; `Option<U>` field → `&U`; else `&FieldType`.

### 2.2 INSERT — `#[tp_insert]`

`InsertTemplate` always generates `insert(re: &T, conn) -> Result<u64>` (rows affected), skipping
`#[auto]` fields. `#[tp_insert]` is just a keyless marker. **On Postgres** you also get
`insert_return(re: &T, conn) -> Result<T>` (`INSERT ... RETURNING *`), useful to read back the
auto-assigned `id`.

### 2.3 UPDATE — `#[tp_update]`

Keys: `by` (required), `on`, `where`, `op_lock`, `returning`, `fn_name`, `debug`, `instrument`.

- `on` = columns to SET. **If `on` is empty, all columns except `by`/`op_lock` are updated** from the
  `re: &T` argument. If `on` is given, values are passed as explicit params (no `re`).
- `op_lock = "version"` = optimistic locking. Adds `AND version = ?` to WHERE and `SET version = version + 1`;
  a stale in-memory version updates 0 rows. The `version` field should be `#[auto]` and a signed integer.
- `returning` (**Postgres only**): `true` (→ `Vec<T>`), `"id"` (→ `Vec<i32>` via `query_scalar`), or
  `"id, email"` (→ `Vec<(i32, String)>`). Also generates a `<fn>_stream` variant.

```rust
#[tp_update(by = "id", op_lock = "version", fn_name = "update_user")]                 // -> u64
#[tp_update(by = "id", fn_name = "update_ret",    returning = true)]                  // -> Vec<User> (+ update_ret_stream)
#[tp_update(by = "id", fn_name = "update_ret_id", returning = "id")]                  // -> Vec<i32>

// on empty -> SET all-but-key from `re`:
User::update_user(&user.id, &user, &db).await?;   // update_by_id(id, re, conn)
```

Generated names (no `fn_name`): `update_by_<by>`, `update_by_<by>_on_<on>`, `..._lock_on_version`.

**Argument order** (all by reference; `String` → `&str`; `by` and `on` groups each sorted alphabetically):

- **`on` empty:** `(<by fields...>, re: &T, conn)` — SET values come from `re`.
  `#[tp_update(by = "id")]` → `update_by_id(id: &i32, re: &T, conn)`.
- **`on` given:** `re` is dropped; SET values are explicit params in the order **`by` fields, then `on`
  fields, then the `op_lock` version, then `conn`**:
  ```rust
  // #[tp_update(by = "id", on = "price", op_lock = "version")]
  Product::update_by_id_on_price_lock_on_version(&id, &new_price, &current_version, &db).await?;
  //                                              ^by   ^on         ^op_lock          ^conn
  ```

### 2.4 DELETE — `#[tp_delete]`

Keys: `by` (or `where`), `where`, `returning` (Postgres), `fn_name`, `debug`, `instrument`.

```rust
#[tp_delete(by = "id")]                                    // delete_by_id(id, conn) -> u64
#[tp_delete(where = "active = :active and version < :max_version", fn_name = "purge_inactive")]
```

### 2.5 UPSERT — `#[tp_upsert]`

Keys: `by` (conflict target, required), `on`, `op_lock`, `where`, `returning` (bool),
`do_nothing` (bool), `fn_name`, `debug`.

- `by` = the unique/PK columns to conflict on. `on` = columns to update on conflict; **if `on` is
  empty, all non-`by`/non-version columns are updated**.
- `returning = true` → `Result<T>` via `fetch_one` — supported on **Postgres and SQLite**; **MySQL
  panics at compile time**.
- Dialects: PG/SQLite `INSERT ... ON CONFLICT (by) DO UPDATE SET c = EXCLUDED.c`; MySQL
  `INSERT ... ON DUPLICATE KEY UPDATE c = VALUES(c)`. `do_nothing = true` → `DO NOTHING`. `Any` unsupported.

```rust
#[tp_upsert(by = "email")]                                    // upsert_by_email(re, conn) -> u64
#[tp_upsert(by = "id", on = "password, updated_at")]          // update only those cols on conflict
#[tp_upsert(by = "id", fn_name = "upsert_ret", returning = true)]  // -> User (PG/SQLite)
```

> ⚠️ There is **no `update = "..."` key** on `tp_upsert`. It is silently ignored (the crate's own
> examples contain `update = "..."`, but it does nothing). Use **`on = "..."`** to pick update columns.

### 2.6 `where` placeholders (select / update / delete / upsert)

A `where = "..."` clause is `AND`-joined with the `by` conditions. Two placeholder forms:

- **`:field`** — auto-mapped: `field` must be a struct field in a direct comparison; the param type is
  inferred from the field. `where = "email = :email"` → param `email: &str`.
- **`:name$Type`** — explicit type, required inside expressions/functions. `where = "score > :min$f64"`
  → param `min: &f64`. e.g. `:since$String`, `:cutoff$chrono::DateTime<chrono::Utc>`.

Only the struct's own table may be referenced (JOINs are a compile error — use a raw macro instead).

### 2.7 Tracing override (`instrument`)

Only active with the `tracing` feature. Put `instrument` on any `#[tp_*]` (or struct-level as a default):
`instrument = true` (record all params), `"skip_all"` (default), or `"skip(conn)"` / `"skip(re, conn)"`.

---

## 3. Query builder (dynamic queries)

Enable per struct (the struct already derives a template macro). Bare form = per-field methods only;
add `name = "SQL"` pairs for custom conditions:

```rust
#[tp_select_builder(
    with_email_domain = "email LIKE :domain$String",
    with_score_range  = "version BETWEEN :min$i32 AND :max$i32",
)]
#[tp_update_builder]
#[tp_delete_builder]
pub struct User { /* ... */ }
```

Entry points: `User::builder_select()`, `User::builder_update()`, `User::builder_delete()`.

> **Every builder method returns `Result<Self, sqlx::Error>`** — chain with `?` (or `.unwrap()`).

### 3.1 SELECT builder

Methods generated per field, by Rust type:

| Field type | Generated methods (`{f}` = field name) |
|---|---|
| String-like | `{f}(&str)`, `{f}_not`, `{f}_like`, `{f}_start_with`, `{f}_end_with` |
| Numeric / datetime | `{f}(&v)`, `{f}_not`, `{f}_gt`, `{f}_gte`, `{f}_lt`, `{f}_lte` |
| Other (e.g. `bool`) | `{f}(&v)`, `{f}_not` |
| **All fields** | `order_by_{f}()`, `order_by_{f}_asc()`, `order_by_{f}_desc()` |

Terminal ops: `.find_all(db)`, `.find_one(db)` → `Option<T>`, `.find_page((offset,limit,count), db)` →
`(Vec<T>, Option<i64>)`, `.count(db)` → `i64`, `.stream(db)`, `.build_sql()` → `String` (debug).

```rust
let users = User::builder_select()
    .active(&true)?
    .org(&Some(1))?
    .email_like("%@abc.com")?
    .id_gt(&100)?
    .order_by_id_desc()?
    .find_all(&db).await?;

let (page, total) = User::builder_select()
    .active(&true)?
    .find_page((0, 20, true), &db).await?;
```

> **`.stream()` borrows `&mut self`** — bind the builder to a `let mut` first:
> ```rust
> let mut b = User::builder_select().active(&true)?;
> let mut s = b.stream(&db).await;
> while let Some(row) = s.next().await { /* ... */ }
> ```

`Option<T>` fields: passing `&None` produces `IS NULL` / `IS NOT NULL` (eq/not); `gt/lt/like` on `None`
returns an `Err`.

### 3.2 UPDATE builder (two-phase)

`on_*` adds SET clauses; the first `by_*` switches to the WHERE phase (you can't add SET after). Both
phases have `.execute(db)` → `Result<u64>`.

```rust
User::builder_update()
    .on_password("new_hash")?   // SET password = ?
    .on_version(&2)?            // SET version = ?
    .by_id(&42)?               // WHERE id = ?   (now in WHERE phase)
    .execute(&db).await?;      // -> rows affected
```

### 3.3 DELETE builder

Same per-field WHERE methods as SELECT (bare names, no `by_` prefix), then `.execute(db)` → `Result<u64>`:

```rust
User::builder_delete().active(&false)?.email_like("%test.com")?.execute(&db).await?;
```

### 3.4 Custom conditions

`method_name = "SQL predicate with placeholders"`. The method name is used verbatim
(`with_score_range` → `.with_score_range(min, max)`). **No table aliases** (`.` is a compile error);
referenced columns are validated at compile time. Placeholders: `:name$Type` (explicit) or `:name`
(auto-mapped to a column in the same expression). Custom-condition params are passed **by value**:

```rust
User::builder_select().with_email_domain("%@company.com")?.count(&db).await?;
User::builder_select().with_score_range(0, 10)?.find_all(&db).await?;
```

Placeholder rendering is per-DB: Postgres `$1, $2, …`; SQLite/MySQL `?`.

---

## 4. Raw-SQL (procedural) macros

Use these for **JOINs, `SELECT ... AS`, aggregates, or any hand-written SQL** — anything the derive
macros can't express. You write an `async fn` with an **empty body**; the macro replaces it. The
executor is **appended automatically as the last param `conn`** — you never write it.

```rust
// #[db("...")] required for the generic macros (query/select/insert/update/delete/multi_query)
#[select("SELECT * FROM users WHERE email = :email AND org = :org")]
#[db("postgres")]
pub async fn find_by_email_org(email: &str, org: i32) -> Vec<User> {}

// call: find_by_email_org("a@b.com", 1, &db).await?
```

DB-specific variants bake in the database (no `#[db]` needed):
`#[postgres_query]`, `#[sqlite_select]`, `#[mysql_insert]`, … for each of
`query` / `select` / `insert` / `update` / `delete` / `multi_query`.

- `#[query]` runs any statement (no kind check). `#[select]`/`#[insert]`/`#[update]`/`#[delete]`
  additionally validate the statement kind at compile time. Behaviour is otherwise identical —
  the **return type** decides the fetch mode.
- **Named params `:name`** map to fn params by name; a param may be reused (bound once per occurrence);
  a declared param need not appear in the SQL.

### 4.1 Return type → fetch mode

| Declared return type | Behaviour |
|---|---|
| *(none)* | `execute` → `Result<()>` — fire-and-forget writes |
| `Vec<T>` | `query_as::<_,T>().fetch_all` |
| `Option<T>` | `query_as().fetch_optional` |
| `T` (struct) / `(A, B)` tuple | `query_as().fetch_one` (needs exactly one row) |
| `Stream<T>` | `query_as().fetch` — **declare as plain `fn`, not `async`** |
| `Scalar<T>` / `Scalar<Option<T>>` / `Scalar<Stream<T>>` | `query_scalar()` (single column, e.g. `Scalar<i64>` for COUNT) |
| `Page<T>` | pagination — adds a `page: impl Into<(i64,i32,bool)>` param; returns `(Vec<T>, Option<i64>)`. Works with JOIN/GROUP BY. |
| `RowAfftected` | `execute().rows_affected()` → `Result<u64>` |

> ⚠️ **`RowAfftected` is spelled with a double-t** (crate quirk). Writing `-> RowAffected` (or a
> `type RowAffected = u64`) does **not** hit this branch — it becomes a `fetch_one` and usually fails.
> For plain writes prefer a **void return** (`{}` with no `->`), which generates `execute`.

```rust
#[query("SELECT COUNT(*) FROM users WHERE active = :active")]
#[db("sqlite")]
async fn count_active(active: bool) -> Scalar<i64> {}

#[postgres_select("SELECT u.id, o.name FROM users u JOIN orgs o ON o.id = u.org WHERE u.email LIKE :q")]
pub fn user_orgs(q: &str) -> Stream<(i32, String)> {}   // plain fn (stream)
```

### 4.2 `multi_query` and importing SQL from a file

`#[multi_query]` runs **several statements sequentially** and supports only a **void return**. The SQL
source can be an inline string, `sql = "..."`, or `file = "path.sql"` (resolved against
`CARGO_MANIFEST_DIR`, read at compile time). A **bare trailing integer is the `debug` level**, not a
statement index — all statements always run.

```rust
#[multi_query(file = "sql/init.sql", 0)]   // 0 = debug level (log each stmt); NOT an index
#[db("postgres")]
async fn migrate() {}
```

`file = "..."` also works for the single-statement macros (subject to the one-statement rule).

### 4.3 `$StructName` column template

Inside raw SQL, `$StructName` expands to that struct's comma-joined column list (its `COLUMNS_STR`).
The struct must derive `Columns` **or** any template macro (which auto-generate `COLUMNS_STR`).
`#[column]` renames are reflected, so `$User` yields **DB column names**.

```rust
#[derive(SqliteTemplate, sqlx::FromRow)]
#[table("users")]
pub struct User { #[auto] pub id: i32, #[column("user_name")] pub name: String, pub active: bool }

#[sqlite_query("SELECT $User FROM users WHERE active = :active")]   // $User -> "id, user_name, active"
pub async fn active_users(active: bool) -> Vec<User> {}
```

> `db = "..."` written **inside** the macro args (e.g. `#[select(sql = "...", db = "postgres")]`) is
> **ignored** — always use a separate `#[db("...")]` attribute or a `postgres_`/`sqlite_`/`mysql_` variant.

---

## 5. `Columns` and `tp_gen`

### 5.1 `Columns` derive

`#[derive(Columns)]` emits, on the struct:

| Item | Type | Notes |
|---|---|---|
| `COLUMNS` | `[&'static str; N]` | fixed-size **array**, in declaration order |
| `COLUMNS_STR` | `&'static str` | the names joined by `", "` (comma + space) |
| `as_select_all_fields()` | `const fn -> &'static str` | same string as `COLUMNS_STR`; **standalone `Columns` derive only** |

- `#[column("db_name")]` values are used; `#[auto]` fields **are included** (unlike INSERT).
- There is **no** per-field `COLUMN_<FIELD>` constant (the doc comments that mention one are stale).
- The **aggregate** template derives (`SqlxTemplate`, `PostgresTemplate`, `MysqlTemplate`,
  `SqliteTemplate`, `AnyTemplate`) auto-generate `COLUMNS` + `COLUMNS_STR` — **do not** also derive
  `Columns` (double definition). The single-purpose derives (`InsertTemplate`, `SelectTemplate`, …) do
  **not**; add `#[derive(Columns)]` yourself if you want the constants with those.

```rust
#[derive(Columns)]
struct User { pub id: i32, #[column("q_name")] pub name: String, pub email: String }
// User::COLUMNS      == ["id", "q_name", "email"]
// User::COLUMNS_STR  == "id, q_name, email"
```

### 5.2 `tp_gen` (advanced)

`#[tp_gen(...)]` is an **attribute macro** (not a derive) that generates the same `tp_*` functions but
lets you control **where** they land and **which type** they target. It reads the struct's own
`#[derive(...)]` list to decide what to generate (`SqlxTemplate` → all; otherwise each of
`InsertTemplate`/`UpdateTemplate`/`UpsertTemplate`/`SelectTemplate`/`DeleteTemplate` present).

> ⚠️ **`#[tp_gen]` replaces the struct.** As an attribute macro it emits *only* the generated
> functions — it does **not** re-emit the struct definition, and the struct's own `#[derive(...)]` do
> **not** run. So the annotated struct is consumed; the generated code must reference a type that exists
> elsewhere. In practice this means **`for` is effectively required**: point it at a real type (the
> annotated struct is only a field-schema donor). A bare `#[tp_gen]` with no `for` generates code
> referencing the now-removed struct → "cannot find type" compile error.

Two args (only these two are parsed):

- **`for = "path::To::Type"`** — the type the generated ops target for SQL/row-mapping. The annotated
  struct supplies the field/column schema; the functions are built for the `for` type (which you define
  separately, deriving `sqlx::FromRow`).
- **`scope = "struct" | "mod" | "newmod"`** — output placement:
  - `"mod"` → free functions in the current module
  - `"newmod"` → `pub mod <table_name> { … }` (module named after `#[table]`, lowercased)
  - `"struct"` (default) → `impl <AnnotatedStruct> { … }` — but that struct was just removed, so this
    form is generally unusable; prefer `"mod"` / `"newmod"`.

```rust
#[tp_gen(for = "crate::models::UserRow", scope = "newmod")]
#[derive(SqlxTemplate)]     // drives WHICH ops are generated
#[table("users")]          // drives table name + module name
pub struct UserSchema { #[auto] pub id: i32, pub email: String }
```

> Notes: the `table = "..."` shown in the crate's `tp_gen` doc example is a **no-op** (table comes from
> `#[table]`). `tp_gen` has no tests/examples in the crate — treat it as advanced/experimental; prefer
> the derive macros for normal use.

---

## Common mistakes / gotchas

| Symptom | Cause / fix |
|---|---|
| `find_by_id_and_email` doesn't exist, but `find_by_email_and_id` does | `by`/`on`/`order` fields are **sorted alphabetically** in the fn name. Use `fn_name = "..."` for a stable name. |
| `tp_upsert(update = "...")` doesn't limit updated columns | `update` is not a real key (silently ignored). Use **`on = "..."`**. |
| `by = "field"` never matches NULL rows | Equality (`= ?`) never matches SQL `NULL`. Use `where = "... IS NULL"` or the builder with `Option` fields. |
| Raw fn `-> RowAffected` errors at runtime | The branch requires the literal misspelling **`RowAfftected`**; for plain writes use a **void return** (no `->`). |
| `db = "..."` inside `#[select(...)]` ignored | Put database in a separate `#[db("...")]` attr, or use `postgres_`/`sqlite_`/`mysql_` variants. |
| `.stream()` won't compile on a temporary | It borrows `&mut self`; bind the builder to `let mut b = ...` first. |
| Need a JOIN or `SELECT col AS alias` | Not expressible with derive/builder — use a raw macro (`#[select]`/`#[query]`, section 4). |
| MySQL upsert with `returning = true` panics | `RETURNING` upsert is Postgres/SQLite only. |
| Duplicate `COLUMNS` constant | Don't derive `Columns` alongside `SqlxTemplate`/`*Template` (they already emit it). |
| `#[tp_gen]` → "cannot find type Struct" | `tp_gen` consumes the struct. Set `for = "ExistingType"` and use `scope = "mod"`/`"newmod"`. |
| `update_by_id_on_price...` type mismatch | With `on`, arg order is **`by`, then `on`, then `op_lock` version, then `conn`** (each group alphabetical). |

## Quick reference

```
Derive:  #[table("t")] + FromRow + (PostgresTemplate|MysqlTemplate|SqliteTemplate | SqlxTemplate + #[db]) 
Select:  tp_select_all/one/count/page/stream  -> find_/find_one_/count_/find_page_/stream_...
Write:   tp_insert -> insert (+insert_return PG) | tp_update -> update_by_ | tp_delete -> delete_by_ | tp_upsert -> upsert_by_
Keys:    by=, on=, order=, where=, fn_name=, op_lock=, returning=(PG), debug_slow=, instrument=
Builder: #[tp_select_builder|tp_update_builder|tp_delete_builder] -> builder_select()/update()/delete()
         .field()? .field_like()? .field_gt()? .order_by_field_desc()?  then .find_all/.find_one/.find_page/.count/.stream/.build_sql
         update: .on_field()? ... .by_field()? .execute()   delete: .field()? .execute()
Raw:     #[query|select|insert|update|delete|multi_query] (+ #[db]) or postgres_/mysql_/sqlite_ variants
         empty body; :named params; return type picks fetch: Vec/Option/T/(A,B)/Stream<T>/Scalar<T>/Page<T>/RowAfftected/void
         file="x.sql"; $StructName -> COLUMNS_STR
Columns: derive Columns -> COLUMNS[], COLUMNS_STR, as_select_all_fields(); auto-included by *Template
tp_gen:  #[tp_gen(scope="struct|mod|newmod", for="Path")]  (advanced)
```
