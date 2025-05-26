
// #[macro_use]
// extern crate std;
// use sqlx_template::tp_proc;
// /**Automatically generated function by sqlx-template

// ```rust
// pub async fn find_one_by_code<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres> + 'c>(
//     code: &'c str,
//     conn: E,
// ) -> Result<Option<Organization>, sqlx::Error> {
//     let sql =
//     "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations WHERE code = $1";
//     let query_result = sqlx::query_as::<_, Organization>(sql)
//         .bind(code)
//         .fetch_optional(conn)
//         .await;
//     Ok(query_result?)
// }

// ```*/
// pub async fn find_one_by_code<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres> + 'c>(
//     code: &'c str,
//     conn: E,
// ) -> Result<Option<Organization>, sqlx::Error> {
//     let sql = "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations WHERE code = $1";
//     let query_result = sqlx::query_as::<_, Organization>(sql)
//         .bind(code)
//         .fetch_optional(conn)
//         .await;
//     Ok(query_result?)
// }
// /**Automatically generated function by sqlx-template

// ```rust
// pub async fn find_order_by_id_desc<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres> + 'c>(
//     conn: E,
// ) -> Result<Vec<Organization>, sqlx::Error> {
//     let sql =
//     "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations ORDER BY id DESC ";
//     let query_result = sqlx::query_as::<_, Organization>(sql).fetch_all(conn).await;
//     Ok(query_result?)
// }

// ```*/
// pub async fn find_order_by_id_desc<
//     'c,
//     E: sqlx::Executor<'c, Database = sqlx::Postgres> + 'c,
// >(conn: E) -> Result<Vec<Organization>, sqlx::Error> {
//     let sql = "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations ORDER BY id DESC ";
//     let query_result = sqlx::query_as::<_, Organization>(sql).fetch_all(conn).await;
//     Ok(query_result?)
// }
// /**Automatically generated function by sqlx-template

// ```rust
// pub async fn find_all<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//     conn: E,
// ) -> Result<Vec<Organization>, sqlx::Error> {
//     let sql =
//     "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations";
//     let query_result = sqlx::query_as::<_, Organization>(sql).fetch_all(conn).await;
//     Ok(query_result?)
// }

// ```*/
// pub async fn find_all<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//     conn: E,
// ) -> Result<Vec<Organization>, sqlx::Error> {
//     let sql = "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations";
//     let query_result = sqlx::query_as::<_, Organization>(sql).fetch_all(conn).await;
//     Ok(query_result?)
// }
// /**Automatically generated function by sqlx-template

// ```rust
// pub async fn count_all<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//     conn: E,
// ) -> Result<i64, sqlx::Error> {
//     let sql = "SELECT COUNT(1) FROM organizations";
//     let count = sqlx::query_scalar(sql).fetch_one(conn).await;
//     Ok(count?)
// }

// ```*/
// pub async fn count_all<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//     conn: E,
// ) -> Result<i64, sqlx::Error> {
//     let sql = "SELECT COUNT(1) FROM organizations";
//     let count = sqlx::query_scalar(sql).fetch_one(conn).await;
//     Ok(count?)
// }
// /**Automatically generated function by sqlx-template

// ```rust
// pub async fn find_page_all<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres> + Copy>(
//     page: impl Into<(i64, i32, bool)>,
//     conn: E,
// ) -> Result<(Vec<Organization>, Option<i64>), sqlx::Error> {
//     async fn data_query<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//         offset: i64,
//         limit: i32,
//         conn: E,
//     ) -> Result<Vec<Organization>, sqlx::Error> {
//         let sql =
//         "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations LIMIT $1 OFFSET $2";
//         let query_result = sqlx::query_as::<_, Organization>(sql)
//             .bind(limit)
//             .bind(offset)
//             .fetch_all(conn)
//             .await;
//         Ok(query_result?)
//     }
//     pub async fn count_query<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//         conn: E,
//     ) -> Result<i64, sqlx::Error> {
//         let sql =
//         "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations LIMIT $1 OFFSET $2";
//         let count = sqlx::query_scalar(sql).fetch_one(conn).await;
//         Ok(count?)
//     }
//     let page = page.into();
//     let offset = page.0;
//     let limit = page.1;
//     let count = page.2;
//     let data = data_query(offset, limit, conn).await?;
//     let count = if count {
//         if data.is_empty() && offset == 0 {
//             Some(0)
//         } else {
//             Some(count_query(conn).await?)
//         }
//     } else {
//         None
//     };
//     Ok((data, count))
// }

// ```*/
// pub async fn find_page_all<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres> + Copy>(
//     page: impl Into<(i64, i32, bool)>,
//     conn: E,
// ) -> Result<(Vec<Organization>, Option<i64>), sqlx::Error> {
//     async fn data_query<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//         offset: i64,
//         limit: i32,
//         conn: E,
//     ) -> Result<Vec<Organization>, sqlx::Error> {
//         let sql = "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations LIMIT $1 OFFSET $2";
//         let query_result = sqlx::query_as::<_, Organization>(sql)
//             .bind(limit)
//             .bind(offset)
//             .fetch_all(conn)
//             .await;
//         Ok(query_result?)
//     }
//     pub async fn count_query<'c, E: sqlx::Executor<'c, Database = sqlx::Postgres>>(
//         conn: E,
//     ) -> Result<i64, sqlx::Error> {
//         let sql = "SELECT id, name, code, image, active, created_by, created_at, updated_by, updated_at FROM organizations LIMIT $1 OFFSET $2";
//         let count = sqlx::query_scalar(sql).fetch_one(conn).await;
//         Ok(count?)
//     }
//     let page = page.into();
//     let offset = page.0;
//     let limit = page.1;
//     let count = page.2;
//     let data = data_query(offset, limit, conn).await?;
//     let count = if count {
//         if data.is_empty() && offset == 0 {
//             Some(0)
//         } else {
//             Some(count_query(conn).await?)
//         }
//     } else {
//         None
//     };
//     Ok((data, count))
// }