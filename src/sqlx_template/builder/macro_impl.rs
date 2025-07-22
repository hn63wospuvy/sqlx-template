use proc_macro2::{TokenStream, Literal};
use quote::{quote, ToTokens};
use syn::{DeriveInput, Data, Fields, Field, Type as SynType, Ident};

use crate::sqlx_template::{Database, get_field_name, get_field_name_as_column, get_database_type, get_table_name};

/// Generate select builder implementation cho struct
pub fn impl_select_builder(input: &DeriveInput, config: &super::BuilderConfig) -> TokenStream {
    let struct_name = &input.ident;
    let builder_name = quote::format_ident!("{}SelectBuilder", struct_name);
    let args_struct_name = quote::format_ident!("{}QueryBuilderArgs", struct_name);
    let table_name = &config.table_name;
    let database_type = get_database_type(config.database);

    // Parse fields to generate methods
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    // Generate field methods
    let field_methods = fields.iter().map(|field| {
        generate_field_methods(field, config.database)
    }).collect::<Vec<_>>();

    // Generate order by methods
    let order_methods = fields.iter().map(|field| {
        generate_order_methods(field, config.database)
    }).collect::<Vec<_>>();

    // Generate custom condition methods
    let custom_methods = config.custom_conditions.iter().map(|condition| {
        generate_custom_condition_method(condition, config.database, &config.fields)
    }).collect::<Vec<_>>();

    // Generate column list for SELECT
    let column_names = config.fields.iter().map(|field| {
        crate::sqlx_template::get_field_name_as_column(field, config.database)
    }).collect::<Vec<_>>();
    let columns_list = column_names.join(", ");

    // Pre-generate SQL templates at compile time
    let select_base_sql = format!("SELECT {} FROM {}", columns_list, config.table_name);
    let select_base_literal = proc_macro2::Literal::string(&select_base_sql);

    let count_base_sql = format!("SELECT COUNT(*) FROM {}", config.table_name);
    let count_base_literal = proc_macro2::Literal::string(&count_base_sql);

    // Build builder with simple parameter storage and manual binding
    quote! {
        /// QueryBuilderArgs for parameter binding

        #[derive(Clone)]
        pub struct #args_struct_name<'q, DB: sqlx::Database>(pub Box<DB::Arguments<'q>>, usize);
        impl<'q, DB: sqlx::Database> Default for #args_struct_name<'q, DB> {
            fn default() -> Self {
                Self(Box::default(), 0)
            }
        }

        impl<'q, DB: sqlx::Database> #args_struct_name<'q, DB> {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn add_param<T: 'q + Send + sqlx::Encode<'q, DB> + sqlx::Type<DB>>(&mut self, arg: T) -> Result<(), sqlx::Error> {
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
        pub struct #builder_name<'q> {
            table_name: String,
            where_conditions: Vec<String>,
            where_args: #args_struct_name<'q, #database_type>,
            order_by_clauses: Vec<String>,
            stream_sql: String,
        }

        impl <'q> #builder_name<'q> {

            #[inline]
            pub fn clone(&self) -> #builder_name<'q> {
                let cloned_where_args = #args_struct_name(Box::new(self.where_args.0.as_ref().clone()), self.where_args.1);
                #builder_name {
                    table_name: self.table_name.clone(),
                    where_conditions: self.where_conditions.clone(),
                    where_args: cloned_where_args,
                    order_by_clauses: self.order_by_clauses.clone(),
                    stream_sql: self.stream_sql.clone(),
                }
            }
        
            pub fn new() -> Self {
                Self {
                    table_name: #table_name.to_string(),
                    where_conditions: Vec::new(),
                    where_args: #args_struct_name::default(),
                    order_by_clauses: Vec::new(),
                    stream_sql: "".to_string(),
                }
            }

            #(#field_methods)*
            #(#order_methods)*
            #(#custom_methods)*

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                let mut sql = #select_base_literal.to_string();

                if !self.where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&self.where_conditions.join(" AND "));
                }

                if !self.order_by_clauses.is_empty() {
                    sql.push_str(" ORDER BY ");
                    sql.push_str(&self.order_by_clauses.join(", "));
                }

                sql
            }

            /// Build SQL query with parameter placeholders
            pub fn build_sql_with_params(&self) -> (String, usize) {
                let sql = self.build_sql();
                (sql, self.where_args.len())
            }

            /// Execute query và return single result
            /// Note: This uses parameterized queries with proper parameter binding
            pub async fn find_one<'c, E>(self, executor: E) -> Result<Option<#struct_name>, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;

                // Build SQL manually since we destructured self
                let mut sql = #select_base_literal.to_string();
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }
                if !order_by_clauses.is_empty() {
                    sql.push_str(" ORDER BY ");
                    sql.push_str(&order_by_clauses.join(", "));
                }

                // Manually bind parameters
                sqlx::query_as_with(&sql, *where_args.0).fetch_optional(executor).await
            }

            /// Execute query và return all results
            /// Note: This uses parameterized queries with proper parameter binding
            pub async fn find_all<'c, E>(self, executor: E) -> Result<Vec<#struct_name>, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;

                // Build SQL manually since we destructured self
                let mut sql = #select_base_literal.to_string();
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }
                if !order_by_clauses.is_empty() {
                    sql.push_str(" ORDER BY ");
                    sql.push_str(&order_by_clauses.join(", "));
                }

                // Manually bind parameters
                sqlx::query_as_with(&sql, *where_args.0)
                    .fetch_all(executor)
                    .await
            }

            pub async fn find_page<'c, E>(
                self,
                page: impl Into<(i64, i32, bool)>,
                executor: E,
            ) -> Result<(Vec<#struct_name>, Option<i64>), sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type> +'c + Copy,
            {
                let (offset, limit, count) = page.into();
                let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;
                let mut sql = #select_base_literal.to_string();
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
                        let mut count_sql = #count_base_literal.to_string();
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
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, where_conditions, where_args, order_by_clauses, .. } = self;
                let mut count_sql = #count_base_literal.to_string();
                if !where_conditions.is_empty() {
                    count_sql.push_str(" WHERE ");
                    count_sql.push_str(&where_conditions.join(" AND "));
                }
                sqlx::query_scalar_with(&count_sql, *where_args.0).fetch_one(executor).await
            }


            pub async fn stream<E>(
                &'q mut self,
                executor: E,
            ) -> futures::stream::BoxStream<'q, core::result::Result<#struct_name, sqlx::Error>>
            where
                E: sqlx::Executor<'q, Database = #database_type> + 'q,
            {
                self.stream_sql.clear();
                self.stream_sql.push_str(#select_base_literal);
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

        impl #struct_name {
            /// Create a new SELECT query builder for this table.
            ///
            /// The builder provides a fluent interface for constructing SELECT queries with:
            /// - WHERE conditions using field methods (e.g., `.field_name(value)`, `.field_name_gt(value)`)
            /// - Custom WHERE conditions (if defined with `#[tp_select_builder(...)]`)
            /// - ORDER BY clauses using `.order_by_field_asc()` and `.order_by_field_desc()` methods
            /// - Query execution methods: `.find_all()`, `.find_one()`, `.count()`, `.find_page()`, `.stream()`
            ///
            /// # Example
            ///
            /// ```rust
            /// // Basic usage
            /// let users = User::builder_select()
            ///     .email("john@example.com")?
            ///     .active(&true)?
            ///     .order_by_created_at_desc()?
            ///     .find_all(&pool)
            ///     .await?;
            ///
            /// // With custom conditions (if defined)
            /// let users = User::builder_select()
            ///     .with_email_domain("@company.com")?  // Custom condition
            ///     .find_all(&pool)
            ///     .await?;
            /// ```
            ///
            /// # Returns
            ///
            /// A new `SelectBuilder` instance ready for method chaining.
            pub fn builder_select<'q>() -> #builder_name<'q> {
                #builder_name::new()
            }

        }
    }
}

/// Generate methods cho một field
fn generate_field_methods(field: &Field, database: Database) -> TokenStream {
    let field_name = field.ident.as_ref().unwrap(); // Get &Ident directly
    let column_name = get_field_name_as_column(field, database);
    let field_type = &field.ty;
    let database_type = get_database_type(database);

    // Determine field type category
    let type_str = quote!(#field_type).to_string();

    if is_string_type(&type_str) {
        generate_string_methods(field_name, &column_name, &database_type)
    } else if is_numeric_or_datetime_type(&type_str) {
        generate_numeric_datetime_methods(field_name, &column_name, &database_type, field_type)
    } else {
        generate_basic_methods(field_name, &column_name, &database_type, field_type)
    }
}

/// Generate methods cho string fields
fn generate_string_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream) -> TokenStream {
    let eq_method = quote::format_ident!("{}", field_name);
    let not_method = quote::format_ident!("{}_not", field_name);
    let like_method = quote::format_ident!("{}_like", field_name);
    let start_with_method = quote::format_ident!("{}_start_with", field_name);
    let end_with_method = quote::format_ident!("{}_end_with", field_name);

    // Pre-generate SQL condition strings at compile time
    let eq_condition = format!("{} = ?", column_name);
    let neq_condition = format!("{} != ?", column_name);
    let like_condition = format!("{} LIKE ?", column_name);

    let eq_condition_literal = Literal::string(&eq_condition);
    let neq_condition_literal = Literal::string(&neq_condition);
    let like_condition_literal = Literal::string(&like_condition);

    quote! {
        /// Equality condition
        pub fn #eq_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#eq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Not equal condition
        pub fn #not_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#neq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// LIKE condition
        pub fn #like_method(mut self, pattern: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#like_condition_literal.to_string());
            self.where_args.add_param(pattern)?;
            Ok(self)
        }

        /// STARTS WITH condition
        pub fn #start_with_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#like_condition_literal.to_string());
            self.where_args.add_param(format!("{}%", value))?;
            Ok(self)
        }

        /// ENDS WITH condition
        pub fn #end_with_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#like_condition_literal.to_string());
            self.where_args.add_param(format!("%{}", value))?;
            Ok(self)
        }
    }
}

/// Generate methods cho numeric/datetime fields
fn generate_numeric_datetime_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream, field_type: &SynType) -> TokenStream {
    let eq_method = quote::format_ident!("{}", field_name);
    let not_method = quote::format_ident!("{}_not", field_name);
    let gt_method = quote::format_ident!("{}_gt", field_name);
    let gte_method = quote::format_ident!("{}_gte", field_name);
    let lt_method = quote::format_ident!("{}_lt", field_name);
    let lte_method = quote::format_ident!("{}_lte", field_name);

    // Pre-generate SQL condition strings at compile time
    let eq_condition = format!("{} = ?", column_name);
    let neq_condition = format!("{} != ?", column_name);
    let gt_condition = format!("{} > ?", column_name);
    let gte_condition = format!("{} >= ?", column_name);
    let lt_condition = format!("{} < ?", column_name);
    let lte_condition = format!("{} <= ?", column_name);

    let eq_condition_literal = Literal::string(&eq_condition);
    let neq_condition_literal = Literal::string(&neq_condition);
    let gt_condition_literal = Literal::string(&gt_condition);
    let gte_condition_literal = Literal::string(&gte_condition);
    let lt_condition_literal = Literal::string(&lt_condition);
    let lte_condition_literal = Literal::string(&lte_condition);

    quote! {
        /// Equality condition
        pub fn #eq_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#eq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Not equal condition
        pub fn #not_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#neq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Greater than condition
        pub fn #gt_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#gt_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Greater than or equal condition
        pub fn #gte_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#gte_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Less than condition
        pub fn #lt_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#lt_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Less than or equal condition
        pub fn #lte_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#lte_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }
    }
}

/// Generate basic methods cho other types (chỉ equality)
fn generate_basic_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream, field_type: &SynType) -> TokenStream {
    let eq_method = quote::format_ident!("{}", field_name);
    let not_method = quote::format_ident!("{}_not", field_name);

    let column_literal = Literal::string(column_name);

    quote! {
        /// Equality condition
        pub fn #eq_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// Not equal condition
        pub fn #not_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value)?;
            Ok(self)
        }
    }
}

/// Generate order by methods
fn generate_order_methods(field: &Field, database: Database) -> TokenStream {
    let field_name = field.ident.as_ref().unwrap(); // Get &Ident directly
    let column_name = get_field_name_as_column(field, database);

    let order_asc_method = quote::format_ident!("order_by_{}", field_name);
    let order_asc_explicit_method = quote::format_ident!("order_by_{}_asc", field_name);
    let order_desc_method = quote::format_ident!("order_by_{}_desc", field_name);

    // Pre-generate ORDER BY clauses at compile time
    let asc_clause = format!("{} ASC", column_name);
    let desc_clause = format!("{} DESC", column_name);

    let asc_clause_literal = Literal::string(&asc_clause);
    let desc_clause_literal = Literal::string(&desc_clause);

    quote! {
        /// Order by field ascending (default)
        pub fn #order_asc_method(mut self) -> Result<Self, sqlx::Error> {
            self.order_by_clauses.push(#asc_clause_literal.to_string());
            Ok(self)
        }

        /// Order by field ascending (explicit)
        pub fn #order_asc_explicit_method(mut self) -> Result<Self, sqlx::Error> {
            self.order_by_clauses.push(#asc_clause_literal.to_string());
            Ok(self)
        }

        /// Order by field descending
        pub fn #order_desc_method(mut self) -> Result<Self, sqlx::Error> {
            self.order_by_clauses.push(#desc_clause_literal.to_string());
            Ok(self)
        }
    }
}

/// Check if type is string-like
fn is_string_type(type_str: &str) -> bool {
    type_str.contains("String") || type_str.contains("str")
}

/// Check if type is numeric or datetime
fn is_numeric_or_datetime_type(type_str: &str) -> bool {
    type_str.contains("i32") || type_str.contains("i64") ||
    type_str.contains("f32") || type_str.contains("f64") ||
    type_str.contains("DateTime") || type_str.contains("OffsetDateTime") ||
    type_str.contains("NaiveDateTime") || type_str.contains("NaiveDate") ||
    type_str.contains("NaiveTime")
}

/// Implement update builder macro
pub fn impl_update_builder(input: &DeriveInput, config: &super::BuilderConfig) -> TokenStream {
    let struct_name = &input.ident;
    let builder_name = quote::format_ident!("{}UpdateBuilder", struct_name);
    let args_struct_name = quote::format_ident!("{}UpdateBuilderArgs", struct_name);
    let table_name = &config.table_name;
    let database_type = get_database_type(config.database);

    // Parse fields to generate methods
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    // Generate on_* methods (for SET clause)
    let on_methods = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let column_name = get_field_name_as_column(field, config.database);
        let on_method = quote::format_ident!("on_{}", field_name);
        let column_literal = Literal::string(&column_name);
        let field_type = &field.ty;

        // Pre-generate SET clause string at compile time
        let set_clause = format!("{} = ?", column_name);
        let set_clause_literal = Literal::string(&set_clause);

        let type_str = quote!(#field_type).to_string();
        if is_string_type(&type_str) {
            quote! {
                /// Set field value for UPDATE
                pub fn #on_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
                    self.set_clauses.push(#set_clause_literal.to_string());
                    self.where_args.add_param(value)?;
                    Ok(self)
                }
            }
        } else {
            quote! {
                /// Set field value for UPDATE
                pub fn #on_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
                    self.set_clauses.push(#set_clause_literal.to_string());
                    self.where_args.add_param(value)?;
                    Ok(self)
                }
            }
        }
    }).collect::<Vec<_>>();

    // Generate by_* methods (for WHERE clause) - reuse field methods but rename them
    let by_methods = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let column_name = get_field_name_as_column(field, config.database);
        let field_type = &field.ty;
        let database_type = get_database_type(config.database);

        // Determine field type category
        let type_str = quote!(#field_type).to_string();

        if is_string_type(&type_str) {
            generate_update_string_methods(field_name, &column_name, &database_type)
        } else if is_numeric_or_datetime_type(&type_str) {
            generate_update_numeric_datetime_methods(field_name, &column_name, &database_type, field_type)
        } else {
            generate_update_basic_methods(field_name, &column_name, &database_type, field_type)
        }
    }).collect::<Vec<_>>();

    // Generate custom condition methods
    let custom_methods = config.custom_conditions.iter().map(|condition| {
        generate_custom_condition_method(condition, config.database, &config.fields)
    }).collect::<Vec<_>>();

    // Pre-generate UPDATE SQL template
    let update_base_sql = format!("UPDATE {}", config.table_name);
    let update_base_literal = proc_macro2::Literal::string(&update_base_sql);

    quote! {
        /// UpdateBuilderArgs for parameter binding

        #[derive(Clone)]
        pub struct #args_struct_name<'q, DB: sqlx::Database>(pub Box<DB::Arguments<'q>>, usize);
        impl<'q, DB: sqlx::Database> Default for #args_struct_name<'q, DB> {
            fn default() -> Self {
                Self(Box::default(), 0)
            }
        }

        impl<'q, DB: sqlx::Database> #args_struct_name<'q, DB> {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn add_param<T: 'q + Send + sqlx::Encode<'q, DB> + sqlx::Type<DB>>(&mut self, arg: T) -> Result<(), sqlx::Error> {
                use sqlx::Arguments;
                self.0.add(arg).map_err(|e| sqlx::Error::Encode(e))?;
                self.1 += 1;
                Ok(())
            }
            pub fn len(&self) -> usize {
                self.1
            }
        }


        /// Generated update builder
        pub struct #builder_name<'q> {
            table_name: String,
            set_clauses: Vec<String>,
            where_conditions: Vec<String>,
            where_args: #args_struct_name<'q, #database_type>,
        }

        impl <'q> #builder_name<'q> {

            #[inline]
            pub fn clone(&self) -> #builder_name<'q> {
                let cloned_where_args = #args_struct_name(Box::new(self.where_args.0.as_ref().clone()), self.where_args.1);
                #builder_name {
                    table_name: self.table_name.clone(),
                    where_conditions: self.where_conditions.clone(),
                    where_args: cloned_where_args,
                    set_clauses: self.set_clauses.clone(),
                }
            }

            pub fn new() -> Self {
                Self {
                    table_name: #table_name.to_string(),
                    set_clauses: Vec::new(),
                    where_conditions: Vec::new(),
                    where_args: #args_struct_name::default(),
                }
            }

            #(#on_methods)*
            #(#by_methods)*
            #(#custom_methods)*

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                if self.set_clauses.is_empty() {
                    panic!("UPDATE query must have at least one SET clause. Use on_* methods.");
                }

                let mut sql = #update_base_literal.to_string();
                sql.push_str(" SET ");
                sql.push_str(&self.set_clauses.join(", "));

                if !self.where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&self.where_conditions.join(" AND "));
                }

                sql
            }

            /// Execute update query
            pub async fn execute<'c, E>(self, executor: E) -> Result<u64, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, set_clauses, where_conditions, where_args } = self;

                // Build SQL manually since we destructured self
                let mut sql = #update_base_literal.to_string();
                if !set_clauses.is_empty() {
                    sql.push_str(" SET ");
                    sql.push_str(&set_clauses.join(", "));
                }
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }


                let result = sqlx::query_with(&sql, *where_args.0).execute(executor).await?;
                Ok(result.rows_affected())
            }
        }

        impl #struct_name {
            /// Create a new UPDATE query builder for this table.
            ///
            /// The builder provides a fluent interface for constructing UPDATE queries with:
            /// - SET clauses using `.on_field_name(value)` methods to specify which fields to update
            /// - WHERE conditions using `.by_field_name(value)` methods to specify which records to update
            /// - Custom WHERE conditions (if defined with `#[tp_update_builder(...)]`)
            /// - Query execution with `.execute()` method that returns the number of affected rows
            ///
            /// # Example
            ///
            /// ```rust
            /// // Update user email and status
            /// let affected_rows = User::builder_update()
            ///     .on_email("newemail@example.com")?     // SET email = ?
            ///     .on_active(&true)?                     // SET active = ?
            ///     .by_id(&user_id)?                      // WHERE id = ?
            ///     .execute(&pool)
            ///     .await?;
            ///
            /// // Bulk update with custom conditions (if defined)
            /// let affected_rows = User::builder_update()
            ///     .on_status("verified")?
            ///     .with_email_domain("@company.com")?    // Custom WHERE condition
            ///     .execute(&pool)
            ///     .await?;
            /// ```
            ///
            /// # Returns
            ///
            /// A new `UpdateBuilder` instance ready for method chaining.
            pub fn builder_update<'q>() -> #builder_name<'q> {
                #builder_name::new()
            }
        }
    }
}

/// Generate by_* methods cho string fields trong update builder
fn generate_update_string_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);
    let by_like_method = quote::format_ident!("by_{}_like", field_name);
    let by_start_with_method = quote::format_ident!("by_{}_start_with", field_name);
    let by_end_with_method = quote::format_ident!("by_{}_end_with", field_name);

    // Pre-generate WHERE condition strings at compile time
    let eq_condition = format!("{} = ?", column_name);
    let neq_condition = format!("{} != ?", column_name);
    let like_condition = format!("{} LIKE ?", column_name);

    let eq_condition_literal = Literal::string(&eq_condition);
    let neq_condition_literal = Literal::string(&neq_condition);
    let like_condition_literal = Literal::string(&like_condition);

    quote! {
        /// WHERE equality condition
        pub fn #by_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#eq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE not equal condition
        pub fn #by_not_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#neq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE LIKE condition
        pub fn #by_like_method(mut self, pattern: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#like_condition_literal.to_string());
            self.where_args.add_param(pattern)?;
            Ok(self)
        }

        /// WHERE STARTS WITH condition
        pub fn #by_start_with_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#like_condition_literal.to_string());
            self.where_args.add_param(format!("{}%", value))?;
            Ok(self)
        }

        /// WHERE ENDS WITH condition
        pub fn #by_end_with_method(mut self, value: &'q str) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#like_condition_literal.to_string());
            self.where_args.add_param(format!("%{}", value))?;
            Ok(self)
        }
    }
}

/// Generate by_* methods cho numeric/datetime fields trong update builder
fn generate_update_numeric_datetime_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream, field_type: &SynType) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);
    let by_gt_method = quote::format_ident!("by_{}_gt", field_name);
    let by_gte_method = quote::format_ident!("by_{}_gte", field_name);
    let by_lt_method = quote::format_ident!("by_{}_lt", field_name);
    let by_lte_method = quote::format_ident!("by_{}_lte", field_name);

    // Pre-generate WHERE condition strings at compile time
    let eq_condition = format!("{} = ?", column_name);
    let neq_condition = format!("{} != ?", column_name);
    let gt_condition = format!("{} > ?", column_name);
    let gte_condition = format!("{} >= ?", column_name);
    let lt_condition = format!("{} < ?", column_name);
    let lte_condition = format!("{} <= ?", column_name);

    let eq_condition_literal = Literal::string(&eq_condition);
    let neq_condition_literal = Literal::string(&neq_condition);
    let gt_condition_literal = Literal::string(&gt_condition);
    let gte_condition_literal = Literal::string(&gte_condition);
    let lt_condition_literal = Literal::string(&lt_condition);
    let lte_condition_literal = Literal::string(&lte_condition);

    quote! {
        /// WHERE equality condition
        pub fn #by_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#eq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE not equal condition
        pub fn #by_not_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#neq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE greater than condition
        pub fn #by_gt_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#gt_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE greater than or equal condition
        pub fn #by_gte_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#gte_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE less than condition
        pub fn #by_lt_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#lt_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE less than or equal condition
        pub fn #by_lte_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#lte_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }
    }
}

/// Generate by_* basic methods cho other types trong update builder
fn generate_update_basic_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream, field_type: &SynType) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);

    // Pre-generate WHERE condition strings at compile time
    let eq_condition = format!("{} = ?", column_name);
    let neq_condition = format!("{} != ?", column_name);

    let eq_condition_literal = Literal::string(&eq_condition);
    let neq_condition_literal = Literal::string(&neq_condition);

    quote! {
        /// WHERE equality condition
        pub fn #by_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#eq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }

        /// WHERE not equal condition
        pub fn #by_not_method(mut self, value: &'q #field_type) -> Result<Self, sqlx::Error> {
            self.where_conditions.push(#neq_condition_literal.to_string());
            self.where_args.add_param(value)?;
            Ok(self)
        }
    }
}

/// Implement delete builder macro
pub fn impl_delete_builder(input: &DeriveInput, config: &super::BuilderConfig) -> TokenStream {
    let struct_name = &input.ident;
    let builder_name = quote::format_ident!("{}DeleteBuilder", struct_name);
    let args_struct_name = quote::format_ident!("{}DeleteBuilderArgs", struct_name);
    let table_name = &config.table_name;
    let database_type = get_database_type(config.database);

    // Parse fields to generate methods
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => panic!("Only named fields are supported"),
        },
        _ => panic!("Only structs are supported"),
    };

    // Generate field methods for WHERE clause - same as select builder
    let field_methods = fields.iter().map(|field| {
        generate_field_methods(field, config.database)
    }).collect::<Vec<_>>();

    // Generate custom condition methods
    let custom_methods = config.custom_conditions.iter().map(|condition| {
        generate_custom_condition_method(condition, config.database, &config.fields)
    }).collect::<Vec<_>>();

    // Pre-generate DELETE SQL template
    let delete_base_sql = format!("DELETE FROM {}", config.table_name);
    let delete_base_literal = proc_macro2::Literal::string(&delete_base_sql);

    quote! {
        /// DeleteBuilderArgs for parameter binding

        #[derive(Clone)]
        pub struct #args_struct_name<'q, DB: sqlx::Database>(pub Box<DB::Arguments<'q>>, usize);
        impl<'q, DB: sqlx::Database> Default for #args_struct_name<'q, DB> {
            fn default() -> Self {
                Self(Box::default(), 0)
            }
        }

        impl<'q, DB: sqlx::Database> #args_struct_name<'q, DB> {

            
            pub fn new() -> Self {
                Self::default()
            }

            pub fn add_param<T: 'q + Send + sqlx::Encode<'q, DB> + sqlx::Type<DB>>(&mut self, arg: T) -> Result<(), sqlx::Error> {
                use sqlx::Arguments;
                self.0.add(arg).map_err(|e| sqlx::Error::Encode(e))?;
                self.1 += 1;
                Ok(())
            }
            pub fn len(&self) -> usize {
                self.1
            }
        }

        /// Generated delete builder
        pub struct #builder_name<'q> {
            table_name: String,
            where_conditions: Vec<String>,
            where_args: #args_struct_name<'q, #database_type>,
        }

        impl <'q> #builder_name<'q> {

            #[inline]
            pub fn clone(&self) -> #builder_name<'q> {
                let cloned_where_args = #args_struct_name(Box::new(self.where_args.0.as_ref().clone()), self.where_args.1);
                #builder_name {
                    table_name: self.table_name.clone(),
                    where_conditions: self.where_conditions.clone(),
                    where_args: cloned_where_args,
                }
            }

            pub fn new() -> Self {
                Self {
                    table_name: #table_name.to_string(),
                    where_conditions: Vec::new(),
                    where_args: #args_struct_name::default(),
                }
            }

            #(#field_methods)*
            #(#custom_methods)*

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                let mut sql = #delete_base_literal.to_string();

                if !self.where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&self.where_conditions.join(" AND "));
                }

                sql
            }

            /// Execute delete query
            pub async fn execute<'c, E>(self, executor: E) -> Result<u64, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, where_conditions, where_args } = self;

                // Build SQL manually since we destructured self
                let mut sql = #delete_base_literal.to_string();
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }

                // Manually bind parameters
                
                let result = sqlx::query_with(&sql, *where_args.0).execute(executor).await?;
                Ok(result.rows_affected())
            }
        }

        impl #struct_name {
            /// Create a new DELETE query builder for this table.
            ///
            /// The builder provides a fluent interface for constructing DELETE queries with:
            /// - WHERE conditions using field methods (e.g., `.field_name(value)`, `.field_name_gt(value)`)
            /// - Custom WHERE conditions (if defined with `#[tp_delete_builder(...)]`)
            /// - Query execution with `.execute()` method that returns the number of deleted rows
            ///
            /// # Safety
            ///
            /// **⚠️ WARNING**: DELETE operations are irreversible. Always ensure you have proper WHERE conditions
            /// to avoid accidentally deleting all records in the table.
            ///
            /// # Example
            ///
            /// ```rust
            /// // Delete specific user
            /// let deleted_rows = User::builder_delete()
            ///     .id(&user_id)?                         // WHERE id = ?
            ///     .execute(&pool)
            ///     .await?;
            ///
            /// // Delete inactive users older than a certain date
            /// let deleted_rows = User::builder_delete()
            ///     .active(&false)?                       // WHERE active = false
            ///     .created_at_lt(&cutoff_date)?          // AND created_at < ?
            ///     .execute(&pool)
            ///     .await?;
            ///
            /// // Delete with custom conditions (if defined)
            /// let deleted_rows = User::builder_delete()
            ///     .with_email_domain("@oldcompany.com")? // Custom WHERE condition
            ///     .execute(&pool)
            ///     .await?;
            /// ```
            ///
            /// # Returns
            ///
            /// A new `DeleteBuilder` instance ready for method chaining.
            pub fn builder_delete<'q>() -> #builder_name<'q> {
                #builder_name::new()
            }
        }
    }
}



/// Generate custom condition method from CustomCondition
fn generate_custom_condition_method(condition: &super::CustomCondition, database: Database, fields: &[Field]) -> TokenStream {
    use crate::parser;

    let method_name = quote::format_ident!("{}", condition.method_name);
    let sql_expression = &condition.sql_expression;

    // Use the same logic as select.rs - parse SQL expression with parser
    let par_res = match parser::get_columns_and_compound_ids(
        sql_expression,
        crate::sqlx_template::get_database_dialect(database),
    ) {
        Ok(res) => res,
        Err(e) => {
            return quote! {
                compile_error!(concat!("Failed to parse SQL expression in custom condition: ", #sql_expression, " - Error: ", stringify!(#e)));
            };
        }
    };

    // Check for table aliases in SQL expression (not allowed in builder conditions)
    if sql_expression.contains('.') {
        // Simple check for table.column patterns
        return quote! {
            compile_error!(concat!("Table aliases are not allowed in builder custom conditions. Found '.' in: ", #sql_expression));
        };
    }

    // Generate method parameters based on placeholders (same logic as select.rs)
    let mut method_params = Vec::new();
    let mut param_bindings = Vec::new();

    for placeholder in &par_res.placeholder_vars {
        let placeholder_name = &placeholder[1..]; // Remove ':' prefix

        // Check if placeholder has format :name$Type
        if placeholder_name.contains('$') {
            // Case: Placeholder with custom type format :name$Type
            if let Some(dollar_pos) = placeholder_name.find('$') {
                let var_name = &placeholder_name[..dollar_pos];
                let type_name = &placeholder_name[dollar_pos + 1..];

                let param_ident = quote::format_ident!("{}", var_name);
                let param_type = match type_name {
                    "i32" => quote!(i32),
                    "i64" => quote!(i64),
                    "f32" => quote!(f32),
                    "f64" => quote!(f64),
                    "bool" => quote!(bool),
                    "String" => quote!(&'q str),
                    "str" => quote!(&'q str),
                    _ => {
                        // Let user specify any type, compiler will validate it
                        let type_ident = syn::parse_str::<syn::Type>(type_name).unwrap_or_else(|_| {
                            syn::parse_quote!(#type_name)
                        });
                        quote!(#type_ident)
                    }
                };

                method_params.push(quote!(#param_ident: #param_type));
                param_bindings.push(quote!(self.where_args.add_param(#param_ident)?;));
            } else {
                return quote! {
                    compile_error!(concat!("Placeholder ", #placeholder, " contains '$' but format is invalid"));
                };
            }
        }
        // Check if placeholder is mapped to a column (same logic as select.rs)
        else if let Some(columns) = par_res.get_columns_for_placeholder(placeholder) {
            // Case: Placeholder is mapped to a column (and doesn't have $ format)
            if columns.len() == 1 {
                let column_name = columns.iter().next().unwrap();
                // Find the corresponding field in struct by checking condition.columns
                if condition.columns.contains(column_name) {
                    // Find the field type from the original struct fields
                    if let Some(field) = fields.iter().find(|f| {
                        crate::sqlx_template::get_field_name_as_column(f, database) == *column_name
                    }) {
                        let param_ident = quote::format_ident!("{}", placeholder_name);
                        let arg_type = &field.ty;

                        let param_type = if &arg_type.to_token_stream().to_string() == "String" {
                            quote!(&'q str)
                        } else {
                            quote!(#arg_type)
                        };

                        method_params.push(quote!(#param_ident: #param_type));
                        param_bindings.push(quote!(self.where_args.add_param(#param_ident)?;));
                    } else {
                        return quote! {
                            compile_error!(concat!("Column '", #column_name, "' not found in struct fields"));
                        };
                    }
                } else {
                    return quote! {
                        compile_error!(concat!("Column '", #column_name, "' referenced in placeholder but not found in SQL expression"));
                    };
                }
            } else {
                return quote! {
                    compile_error!(concat!("Placeholder '", #placeholder, "' maps to multiple columns, which is not supported"));
                };
            }
        } else {
            // Require explicit type specification if no column mapping
            return quote! {
                compile_error!(concat!("Placeholder '", #placeholder, "' must specify a type using format ':name$type' or map to a column"));
            };
        }
    }

    // Use parser to replace placeholders with proper parameter positions (same as select.rs)
    let sql_expression_literal = proc_macro2::Literal::string(sql_expression);

    // Convert placeholder_vars to a vector of string literals for the quote macro
    let placeholder_vars_literals = par_res.placeholder_vars.iter().map(|var| {
        proc_macro2::Literal::string(var)
    }).collect::<Vec<_>>();

    // Create a simple SQL condition by replacing placeholders with ?
    let mut condition_sql = sql_expression.to_string();
    for placeholder in &par_res.placeholder_vars {
        condition_sql = condition_sql.replace(placeholder, "?");
    }
    let condition_sql_literal = proc_macro2::Literal::string(&condition_sql);

    // Create comprehensive documentation
    let param_docs = if method_params.is_empty() {
        String::new()
    } else {
        let param_list = method_params.iter()
            .enumerate()
            .map(|(i, _)| format!("- `{}`: Parameter for placeholder in SQL condition",
                                 condition.parameters.get(i).unwrap_or(&format!("param_{}", i))))
            .collect::<Vec<_>>()
            .join("\n/// ");
        format!("\n/// \n/// # Parameters\n/// \n/// {}", param_list)
    };

    let doc_string = format!(
        "Custom WHERE condition: `{}`{}",
        sql_expression,
        param_docs
    );
    let doc_literal = proc_macro2::Literal::string(&doc_string);

    quote! {
        #[doc = #doc_literal]
        pub fn #method_name(mut self, #(#method_params),*) -> Result<Self, sqlx::Error> {
            // Add parameters to args
            #(#param_bindings)*

            // Add condition to where clauses
            self.where_conditions.push(#condition_sql_literal.to_string());

            Ok(self)
        }
    }
}
