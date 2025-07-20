
pub mod builder {
    use sqlx_template::SqliteTemplate;
    pub struct Userrrrr1111 {
        pub id: i32,
    }
    #[automatically_derived]
    impl<'a, R: ::sqlx::Row> ::sqlx::FromRow<'a, R> for Userrrrr1111
    where
        &'a ::std::primitive::str: ::sqlx::ColumnIndex<R>,
        i32: ::sqlx::decode::Decode<'a, R::Database>,
        i32: ::sqlx::types::Type<R::Database>,
    {
        fn from_row(__row: &'a R) -> ::sqlx::Result<Self> {
            let id: i32 = __row.try_get("id")?;
            ::std::result::Result::Ok(Userrrrr1111 { id })
        }
    }
    #[automatically_derived]
    impl ::core::clone::Clone for Userrrrr1111 {
        #[inline]
        fn clone(&self) -> Userrrrr1111 {
            Userrrrr1111 {
                id: ::core::clone::Clone::clone(&self.id),
            }
        }
    }
    #[automatically_derived]
    impl ::core::fmt::Debug for Userrrrr1111 {
        #[inline]
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            todo!()
        }
    }
    impl Userrrrr1111 {
        #[inline]
        pub const fn table_name() -> &'static str {
            "users"
        }
    }
    impl Userrrrr1111 {
        /**Automatically generated function by sqlx-template

```rust
pub async fn insert<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
    re: &Userrrrr,
    conn: E,
) -> Result<u64, sqlx::Error> {
    let sql = "INSERT INTO users(id) VALUES ($1)";
    let query = sqlx::query(sql).bind(&re.id).execute(conn).await;
    Ok(query?.rows_affected())
}

```*/
        pub async fn insert<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
            re: &Userrrrr1111,
            conn: E,
        ) -> Result<u64, sqlx::Error> {
            let sql = "INSERT INTO users(id) VALUES ($1)";
            let query = sqlx::query(sql).bind(&re.id).execute(conn).await;
            Ok(query?.rows_affected())
        }
    }
    impl Userrrrr1111 {}
    impl Userrrrr1111 {
        /**Automatically generated function by sqlx-template

```rust
pub async fn find_all<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
    conn: E,
) -> Result<Vec<Userrrrr>, sqlx::Error> {
    let sql = "SELECT id FROM users";
    let query_result = sqlx::query_as::<_, Userrrrr>(sql).fetch_all(conn).await;
    Ok(query_result?)
}

```*/
        pub async fn find_all<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
            conn: E,
        ) -> Result<Vec<Userrrrr1111>, sqlx::Error> {
            let sql = "SELECT id FROM users";
            let query_result = sqlx::query_as::<_, Userrrrr1111>(sql).fetch_all(conn).await;
            Ok(query_result?)
        }
        /**Automatically generated function by sqlx-template

```rust
pub async fn count_all<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
    conn: E,
) -> Result<i64, sqlx::Error> {
    let sql = "SELECT COUNT(1) FROM users";
    let count = sqlx::query_scalar(sql).fetch_one(conn).await;
    Ok(count?)
}

```*/
        pub async fn count_all<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
            conn: E,
        ) -> Result<i64, sqlx::Error> {
            let sql = "SELECT COUNT(1) FROM users";
            let count = sqlx::query_scalar(sql).fetch_one(conn).await;
            Ok(count?)
        }
        /**Automatically generated function by sqlx-template

```rust
pub async fn find_page_all<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite> + Copy>(
    page: impl Into<(i64, i32, bool)>,
    conn: E,
) -> Result<(Vec<Userrrrr>, Option<i64>), sqlx::Error> {
    async fn data_query<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
        offset: i64,
        limit: i32,
        conn: E,
    ) -> Result<Vec<Userrrrr>, sqlx::Error> {
        let sql = "SELECT id FROM users LIMIT $1 OFFSET $2";
        let query_result = sqlx::query_as::<_, Userrrrr>(sql)
            .bind(limit)
            .bind(offset)
            .fetch_all(conn)
            .await;
        Ok(query_result?)
    }
    pub async fn count_query<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
        conn: E,
    ) -> Result<i64, sqlx::Error> {
        let sql = "SELECT id FROM users LIMIT $1 OFFSET $2";
        let count = sqlx::query_scalar(sql).fetch_one(conn).await;
        Ok(count?)
    }
    let page = page.into();
    let offset = page.0;
    let limit = page.1;
    let count = page.2;
    let data = data_query(offset, limit, conn).await?;
    let count = if count {
        if data.is_empty() && offset == 0 {
            Some(0)
        } else {
            Some(count_query(conn).await?)
        }
    } else {
        None
    };
    Ok((data, count))
}

```*/
        pub async fn find_page_all<
            'c,
            E: sqlx::Executor<'c, Database = sqlx::Sqlite> + Copy,
        >(
            page: impl Into<(i64, i32, bool)>,
            conn: E,
        ) -> Result<(Vec<Userrrrr1111>, Option<i64>), sqlx::Error> {
            async fn data_query<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
                offset: i64,
                limit: i32,
                conn: E,
            ) -> Result<Vec<Userrrrr1111>, sqlx::Error> {
                let sql = "SELECT id FROM users LIMIT $1 OFFSET $2";
                let query_result = sqlx::query_as::<_, Userrrrr1111>(sql)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(conn)
                    .await;
                Ok(query_result?)
            }
            pub async fn count_query<'c, E: sqlx::Executor<'c, Database = sqlx::Sqlite>>(
                conn: E,
            ) -> Result<i64, sqlx::Error> {
                let sql = "SELECT id FROM users LIMIT $1 OFFSET $2";
                let count = sqlx::query_scalar(sql).fetch_one(conn).await;
                Ok(count?)
            }
            let (offset, limit, count) = page.into();
            let data = data_query(offset, limit, conn).await?;
            let count = if count {
                if data.is_empty() && offset == 0 {
                    Some(0)
                } else {
                    Some(count_query(conn).await?)
                }
            } else {
                None
            };
            Ok((data, count))
        }

        
    }
    /// QueryBuilderArgs for parameter binding
    
    pub struct UserrrrrQueryBuilderArgs<'q, DB: sqlx::Database>(
        pub Box<DB::Arguments<'q>>,
        usize,
    );
    #[automatically_derived]
    impl<'q, DB: ::core::clone::Clone + sqlx::Database> ::core::clone::Clone
    for UserrrrrQueryBuilderArgs<'q, DB>
    where
        DB::Arguments<'q>: ::core::clone::Clone,
    {
        #[inline]
        fn clone(&self) -> UserrrrrQueryBuilderArgs<'q, DB> {
            UserrrrrQueryBuilderArgs(
                ::core::clone::Clone::clone(&self.0),
                ::core::clone::Clone::clone(&self.1),
            )
        }
    }
    impl<'q, DB: sqlx::Database> Default for UserrrrrQueryBuilderArgs<'q, DB> {
        fn default() -> Self {
            Self(Box::default(), 0)
        }
    }
    impl<'q, DB: sqlx::Database> UserrrrrQueryBuilderArgs<'q, DB> {
        pub fn new() -> Self {
            Self::default()
        }
        pub fn add_param<T: 'q + Send + sqlx::Encode<'q, DB> + sqlx::Type<DB>>(
            &mut self,
            arg: T,
        ) -> Result<(), sqlx::Error> {
            use sqlx::Arguments;
            self.0.add(arg).map_err(|e| sqlx::Error::Encode(e))?;
            self.1 += 1;
            Ok(())
        }
        pub fn len(&self) -> usize {
            self.1
        }
    }
    /// Generated select builder
    
    pub struct UserrrrrSelectBuilder<'q> {
        table_name: String,
        where_conditions: Vec<String>,
        where_args: UserrrrrQueryBuilderArgs<'q, sqlx::Sqlite>,
        order_by_clauses: Vec<String>,
        stream_sql: String,
    }
    impl<'q> UserrrrrSelectBuilder<'q> {

        #[inline]
        pub fn clone(&self) -> UserrrrrSelectBuilder<'q> {
            let cloned_where_args = UserrrrrQueryBuilderArgs(Box::new(self.where_args.0.as_ref().clone()), self.where_args.1);
            UserrrrrSelectBuilder {
                table_name: self.table_name.clone(),
                where_conditions: self.where_conditions.clone(),
                where_args: cloned_where_args,
                order_by_clauses: self.order_by_clauses.clone(),
                stream_sql: self.stream_sql.clone(),
                
            }
        }
        pub fn new() -> Self {
            Self {
                table_name: "users".to_string(),
                where_conditions: Vec::new(),
                where_args: UserrrrrQueryBuilderArgs::default(),
                order_by_clauses: Vec::new(),
                stream_sql: "".to_string(),
            }
        }
        /// Equality condition
        pub fn id(mut self, value: &'q i32) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Not equal condition
        pub fn id_not(mut self, value: &'q i32) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Greater than condition
        pub fn id_gt(mut self, value: &'q i32) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Greater than or equal condition
        pub fn id_gte(mut self, value: &'q i32) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Less than condition
        pub fn id_lt(mut self, value: &'q i32) -> Result<Self, sqlx::Error> {
           todo!()
        }
        /// Less than or equal condition
        pub fn id_lte(mut self, value: &'q i32) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Order by field ascending (default)
        pub fn order_by_id(mut self) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Order by field ascending (explicit)
        pub fn order_by_id_asc(mut self) -> Result<Self, sqlx::Error> {
            todo!()
        }
        /// Order by field descending
        pub fn order_by_id_desc(mut self) -> Result<Self, sqlx::Error> {
           todo!()
        }
        /// Build SQL query string
        pub fn build_sql(&self) -> String {
            todo!()
        }
        /// Build SQL query with parameter placeholders
        pub fn build_sql_with_params(&self) -> (String, usize) {
            let sql = self.build_sql();
            (sql, self.where_args.len())
        }
        /// Execute query v├á return single result
        /// Note: This uses parameterized queries with proper parameter binding
        pub async fn find_one<'c, E>(
            self,
            executor: E,
        ) -> Result<Option<Userrrrr1111>, sqlx::Error>
        where
            E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
        {
            let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;
            let mut sql = "SELECT * FROM users".to_string();
            if !where_conditions.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&where_conditions.join(" AND "));
            }
            if !order_by_clauses.is_empty() {
                sql.push_str(" ORDER BY ");
                sql.push_str(&order_by_clauses.join(", "));
            }
            sql.push_str(" LIMIT 1");
            sqlx::query_as_with(&sql, *where_args.0).fetch_optional(executor).await
        }
        /// Execute query v├á return all results
        /// Note: This uses parameterized queries with proper parameter binding
        pub async fn find_all<'c, E>(
            self,
            executor: E,
        ) -> Result<Vec<Userrrrr1111>, sqlx::Error>
        where
            E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
        {
            let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;
            let mut sql = format!("SELECT * FROM {0}", table_name);
            if !where_conditions.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&where_conditions.join(" AND "));
            }
            if !order_by_clauses.is_empty() {
                sql.push_str(" ORDER BY ");
                sql.push_str(&order_by_clauses.join(", "));
            }
            sqlx::query_as_with(&sql, *where_args.0).fetch_all(executor).await
        }


        pub async fn find_page<'c, E>(
            self,
            page: impl Into<(i64, i32, bool)>,
            executor: E,
        ) -> Result<(Vec<Userrrrr1111>, Option<i64>), sqlx::Error>
        where
            E: sqlx::Executor<'c, Database = sqlx::Sqlite> +'c + Copy,
        {
            let (offset, limit, count) = page.into();
            let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;
            let mut sql = format!("SELECT * FROM {0}", table_name);
            if !where_conditions.is_empty() {
                sql.push_str(" WHERE ");
                sql.push_str(&where_conditions.join(" AND "));
            }
            if !order_by_clauses.is_empty() {
                sql.push_str(" ORDER BY ");
                sql.push_str(&order_by_clauses.join(", "));
            }
            sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));
            
            let res = if count {
                let data = sqlx::query_as_with(&sql, *where_args.0.clone()).fetch_all(executor).await?;
                if data.is_empty() && offset == 0 {
                    (data, Some(0))
                } else {
                    let mut count_sql = format!("SELECT COUNT(*) FROM {0}", table_name);
                    if !where_conditions.is_empty() {
                        count_sql.push_str(" WHERE ");
                        count_sql.push_str(&where_conditions.join(" AND "));
                    }
                    
                    let count = sqlx::query_scalar_with(&count_sql, *where_args.0).fetch_one(executor).await?;
                    (data, Some(count))
                }
            } else {
                let data = sqlx::query_as_with(&sql, *where_args.0).fetch_all(executor).await?;
                (data, None)
            };
            Ok(res)
        }

        pub async fn count<'c, E>(
            self,
            executor: E,
        ) -> Result<i64, sqlx::Error>
        where
            E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
        {
            let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;
            let mut count_sql = format!("SELECT COUNT(*) FROM {0}", table_name);
            if !where_conditions.is_empty() {
                count_sql.push_str(" WHERE ");
                count_sql.push_str(&where_conditions.join(" AND "));
            }
            sqlx::query_scalar_with(&count_sql, *where_args.0).fetch_one(executor).await
        }


        pub async fn stream<E>(
            &'q mut self,
            executor: E,
        ) -> futures::stream::BoxStream<'q, core::result::Result<Userrrrr1111, sqlx::Error>>
        where
            E: sqlx::Executor<'q, Database = sqlx::Sqlite> + 'q,
        {
            self.stream_sql.clear();
            self.stream_sql.push_str(&format!("SELECT * FROM {0}", self.table_name));
            if !self.where_conditions.is_empty() {
                self.stream_sql.push_str(" WHERE ");
                self.stream_sql.push_str(&self.where_conditions.join(" AND "));
            }
            if !self.order_by_clauses.is_empty() {
                self.stream_sql.push_str(" ORDER BY ");
                self.stream_sql.push_str(&self.order_by_clauses.join(", "));
            }
            sqlx::query_as_with(&self.stream_sql, *self.where_args.0.clone()).fetch(executor)
        }


    }
    impl Userrrrr1111 {
        /// Create new select builder
        pub fn builder_select<'q>() -> UserrrrrSelectBuilder<'q> {
            UserrrrrSelectBuilder::new()
        }
    }
    impl Userrrrr1111 {}
    impl Userrrrr1111 {}
}

async fn test() {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    let res: (Vec<builder::Userrrrr1111>, Option<i64>) = builder::Userrrrr1111::builder_select().find_page((0, 10, true), &pool).await.unwrap();
    let mut builder = builder::Userrrrr1111::builder_select().id_gt(&1).unwrap();
    let mut stream = builder.stream(&pool).await;
    while let Some(a) = futures::StreamExt::next(&mut stream).await {
        dbg!(&a);
    }
    dbg!(&res);
}
