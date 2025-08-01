use proc_macro2::{TokenStream, Literal};
use quote::{quote, ToTokens};
use syn::{DeriveInput, Data, Fields, Field, Type as SynType, Ident};

use crate::sqlx_template::{Database, get_field_name, get_field_name_as_column, get_database_type, get_table_name};

/// Generate appropriate placeholder for the database type
fn get_placeholder_template(database: Database) -> &'static str {
    match database {
        Database::Postgres => "$$PLACEHOLDER$$",
        Database::Sqlite | Database::Mysql | Database::Any => "?",
    }
}

/// Generate runtime placeholder replacement function
fn generate_placeholder_replacement_fn(database: Database) -> TokenStream {
    match database {
        Database::Postgres => quote! {
            fn replace_placeholders(sql: &str, param_count: usize) -> String {
                let mut result = sql.to_string();
                for i in 1..=param_count {
                    result = result.replacen("$$PLACEHOLDER$$", &format!("${}", i), 1);
                }
                result
            }
        },
        Database::Sqlite | Database::Mysql | Database::Any => quote! {
            fn replace_placeholders(sql: &str, _param_count: usize) -> String {
                sql.to_string() // No replacement needed for ? placeholders
            }
        },
    }
}

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

    // Generate placeholder replacement function based on database type
    let placeholder_replacement_fn = generate_placeholder_replacement_fn(config.database);

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

            // Add placeholder replacement function
            #placeholder_replacement_fn

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                let mut sql = #select_base_literal.to_string();

                if !self.where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    let where_clause = self.where_conditions.join(" AND ");
                    let replaced_where = Self::replace_placeholders(&where_clause, self.where_args.len());
                    sql.push_str(&replaced_where);
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
                let sql = self.build_sql();
                let where_args = self.where_args;

                // Manually bind parameters
                sqlx::query_as_with(&sql, *where_args.0).fetch_optional(executor).await
            }

            /// Execute query và return all results
            /// Note: This uses parameterized queries with proper parameter binding
            pub async fn find_all<'c, E>(self, executor: E) -> Result<Vec<#struct_name>, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let sql = self.build_sql();
                let where_args = self.where_args;

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

                // Build base SQL with WHERE and ORDER BY
                let mut sql = self.build_sql();
                sql.push_str(&format!(" LIMIT {limit} OFFSET {offset}"));

                let res = if count {
                    let data = sqlx::query_as_with(&sql, *self.where_args.0.clone()).fetch_all(executor).await?;
                    if data.is_empty() && offset == 0 {
                        (data, Some(0))
                    } else {
                        // Build count SQL
                        let mut count_sql = #count_base_literal.to_string();
                        if !self.where_conditions.is_empty() {
                            count_sql.push_str(" WHERE ");
                            let where_clause = self.where_conditions.join(" AND ");
                            let replaced_where = Self::replace_placeholders(&where_clause, self.where_args.len());
                            count_sql.push_str(&replaced_where);
                        }
                        let count = sqlx::query_scalar_with(&count_sql, *self.where_args.0).fetch_one(executor).await?;
                        (data, Some(count))
                    }
                } else {
                    let data = sqlx::query_as_with(&sql, *self.where_args.0).fetch_all(executor).await?;
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
                let mut count_sql = #count_base_literal.to_string();
                if !self.where_conditions.is_empty() {
                    count_sql.push_str(" WHERE ");
                    let where_clause = self.where_conditions.join(" AND ");
                    let replaced_where = Self::replace_placeholders(&where_clause, self.where_args.len());
                    count_sql.push_str(&replaced_where);
                }
                sqlx::query_scalar_with(&count_sql, *self.where_args.0).fetch_one(executor).await
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
                    let where_clause = self.where_conditions.join(" AND ");
                    let replaced_where = Self::replace_placeholders(&where_clause, self.where_args.len());
                    self.stream_sql.push_str(&replaced_where);
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
            /// ```rust,no_run
            /// # use sqlx_template::SqliteTemplate;
            /// # use sqlx::{FromRow, SqlitePool};
            /// # #[derive(SqliteTemplate, FromRow, Debug, Clone)]
            /// # #[table("users")]
            /// # #[tp_select_builder]
            /// # pub struct User {
            /// #     pub id: i32,
            /// #     pub email: String,
            /// #     pub active: bool,
            /// #     pub created_at: chrono::DateTime<chrono::Utc>,
            /// # }
            /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
            /// # let pool = SqlitePool::connect(":memory:").await?;
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
            /// # Ok(())
            /// # }
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

    // Determine field type category
    let type_str = quote!(#field_type).to_string();

    if is_string_type(&type_str) {
        generate_string_methods(field_name, &column_name, database)
    } else if is_numeric_or_datetime_type(&type_str) {
        generate_numeric_datetime_methods(field_name, &column_name, database, field_type)
    } else {
        generate_basic_methods(field_name, &column_name, database, field_type)
    }
}

/// Generate methods cho string fields
fn generate_string_methods(field_name: &Ident, column_name: &str, database: Database) -> TokenStream {
    let eq_method = quote::format_ident!("{}", field_name);
    let not_method = quote::format_ident!("{}_not", field_name);
    let like_method = quote::format_ident!("{}_like", field_name);
    let start_with_method = quote::format_ident!("{}_start_with", field_name);
    let end_with_method = quote::format_ident!("{}_end_with", field_name);

    // Generate placeholder based on database type
    let placeholder = get_placeholder_template(database);

    // Pre-generate SQL condition strings at compile time
    let eq_condition = format!("{} = {}", column_name, placeholder);
    let neq_condition = format!("{} != {}", column_name, placeholder);
    let like_condition = format!("{} LIKE {}", column_name, placeholder);

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
fn generate_numeric_datetime_methods(field_name: &Ident, column_name: &str, database: Database, field_type: &SynType) -> TokenStream {
    let eq_method = quote::format_ident!("{}", field_name);
    let not_method = quote::format_ident!("{}_not", field_name);
    let gt_method = quote::format_ident!("{}_gt", field_name);
    let gte_method = quote::format_ident!("{}_gte", field_name);
    let lt_method = quote::format_ident!("{}_lt", field_name);
    let lte_method = quote::format_ident!("{}_lte", field_name);

    // Generate placeholder based on database type
    let placeholder = get_placeholder_template(database);

    // Pre-generate SQL condition strings at compile time
    let eq_condition = format!("{} = {}", column_name, placeholder);
    let neq_condition = format!("{} != {}", column_name, placeholder);
    let gt_condition = format!("{} > {}", column_name, placeholder);
    let gte_condition = format!("{} >= {}", column_name, placeholder);
    let lt_condition = format!("{} < {}", column_name, placeholder);
    let lte_condition = format!("{} <= {}", column_name, placeholder);

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
fn generate_basic_methods(field_name: &Ident, column_name: &str, database: Database, field_type: &SynType) -> TokenStream {
    let eq_method = quote::format_ident!("{}", field_name);
    let not_method = quote::format_ident!("{}_not", field_name);

    // Generate placeholder based on database type
    let placeholder = get_placeholder_template(database);

    // Pre-generate SQL condition strings at compile time
    let eq_condition = format!("{} = {}", column_name, placeholder);
    let neq_condition = format!("{} != {}", column_name, placeholder);

    let eq_condition_literal = Literal::string(&eq_condition);
    let neq_condition_literal = Literal::string(&neq_condition);

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
    // Remove whitespace and check for exact matches or common patterns
    let cleaned = type_str.replace(" ", "");

    // Check for exact string types
    cleaned == "String" ||
    cleaned == "&str" ||
    cleaned == "&'_str" ||
    cleaned.starts_with("&'") && cleaned.ends_with("str") || // &'a str, &'static str, etc.

    // Check for fully qualified paths
    cleaned.ends_with("::String") || // std::string::String, alloc::string::String, etc.
    cleaned.ends_with("::str") ||    // std::str, etc.

    // Check for Option<String> and Option<&str> patterns
    cleaned == "Option<String>" ||
    cleaned == "Option<&str>" ||
    cleaned.starts_with("Option<&'") && cleaned.ends_with("str>") || // Option<&'a str>
    cleaned.starts_with("Option<") && cleaned.ends_with("::String>") || // Option<std::string::String>

    // Check for Vec<String> patterns (if needed)
    cleaned == "Vec<String>" ||
    cleaned.starts_with("Vec<") && cleaned.ends_with("::String>") || // Vec<std::string::String>

    // Check for Box<str> patterns
    cleaned == "Box<str>" ||
    cleaned.starts_with("Box<") && cleaned.ends_with("::str>") // Box<std::str>
}

/// Check if type is numeric or datetime
fn is_numeric_or_datetime_type(type_str: &str) -> bool {
    let cleaned = type_str.replace(" ", "");

    // Check for exact numeric types
    matches!(cleaned.as_str(),
        "i8" | "i16" | "i32" | "i64" | "i128" |
        "u8" | "u16" | "u32" | "u64" | "u128" |
        "f32" | "f64" | "isize" | "usize"
    ) ||

    // Check for fully qualified numeric types (less common but possible)
    cleaned.ends_with("::i8") || cleaned.ends_with("::i16") || cleaned.ends_with("::i32") ||
    cleaned.ends_with("::i64") || cleaned.ends_with("::i128") ||
    cleaned.ends_with("::u8") || cleaned.ends_with("::u16") || cleaned.ends_with("::u32") ||
    cleaned.ends_with("::u64") || cleaned.ends_with("::u128") ||
    cleaned.ends_with("::f32") || cleaned.ends_with("::f64") ||
    cleaned.ends_with("::isize") || cleaned.ends_with("::usize") ||

    // Check for Option<numeric> patterns
    matches!(cleaned.as_str(),
        "Option<i8>" | "Option<i16>" | "Option<i32>" | "Option<i64>" | "Option<i128>" |
        "Option<u8>" | "Option<u16>" | "Option<u32>" | "Option<u64>" | "Option<u128>" |
        "Option<f32>" | "Option<f64>" | "Option<isize>" | "Option<usize>"
    ) ||

    // Check for datetime types
    matches!(cleaned.as_str(),
        "DateTime" | "OffsetDateTime" | "NaiveDateTime" | "NaiveDate" | "NaiveTime"
    ) ||
    cleaned.contains("DateTime<") || // DateTime<Utc>, DateTime<Local>, etc.
    cleaned.ends_with("::DateTime") || cleaned.ends_with("::OffsetDateTime") ||
    cleaned.ends_with("::NaiveDateTime") || cleaned.ends_with("::NaiveDate") ||
    cleaned.ends_with("::NaiveTime") ||

    // Check for Option<datetime> patterns
    matches!(cleaned.as_str(),
        "Option<DateTime>" | "Option<OffsetDateTime>" | "Option<NaiveDateTime>" |
        "Option<NaiveDate>" | "Option<NaiveTime>"
    ) ||
    cleaned.starts_with("Option<DateTime<") || // Option<DateTime<Utc>>, etc.
    cleaned.starts_with("Option<") && (
        cleaned.ends_with("::DateTime>") || cleaned.ends_with("::OffsetDateTime>") ||
        cleaned.ends_with("::NaiveDateTime>") || cleaned.ends_with("::NaiveDate>") ||
        cleaned.ends_with("::NaiveTime>")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_string_type() {
        // Basic string types
        assert!(is_string_type("String"));
        assert!(is_string_type("&str"));
        assert!(is_string_type("&'static str"));
        assert!(is_string_type("&'a str"));

        // Fully qualified paths
        assert!(is_string_type("std::string::String"));
        assert!(is_string_type("alloc::string::String"));
        assert!(is_string_type("std::str"));

        // Option string types
        assert!(is_string_type("Option<String>"));
        assert!(is_string_type("Option<&str>"));
        assert!(is_string_type("Option<&'a str>"));
        assert!(is_string_type("Option<std::string::String>"));

        // Vec and Box string types
        assert!(is_string_type("Vec<String>"));
        assert!(is_string_type("Vec<std::string::String>"));
        assert!(is_string_type("Box<str>"));
        assert!(is_string_type("Box<std::str>"));

        // Should NOT match
        assert!(!is_string_type("MyString")); // Custom type containing "String"
        assert!(!is_string_type("StringBuffer")); // Custom type starting with "String"
        assert!(!is_string_type("i32"));
        assert!(!is_string_type("Vec<i32>"));
        assert!(!is_string_type("CustomStringType"));
        assert!(!is_string_type("my_module::MyString")); // Custom type ending with "String" but not std::string::String
    }

    #[test]
    fn test_is_numeric_or_datetime_type() {
        // Basic numeric types
        assert!(is_numeric_or_datetime_type("i32"));
        assert!(is_numeric_or_datetime_type("i64"));
        assert!(is_numeric_or_datetime_type("f32"));
        assert!(is_numeric_or_datetime_type("f64"));
        assert!(is_numeric_or_datetime_type("u32"));

        // Fully qualified numeric types (rare but possible)
        assert!(is_numeric_or_datetime_type("std::primitive::i32"));
        assert!(is_numeric_or_datetime_type("core::primitive::u64"));

        // Option numeric types
        assert!(is_numeric_or_datetime_type("Option<i32>"));
        assert!(is_numeric_or_datetime_type("Option<f64>"));

        // DateTime types
        assert!(is_numeric_or_datetime_type("DateTime"));
        assert!(is_numeric_or_datetime_type("NaiveDateTime"));
        assert!(is_numeric_or_datetime_type("OffsetDateTime"));
        assert!(is_numeric_or_datetime_type("DateTime<Utc>"));

        // Fully qualified DateTime types
        assert!(is_numeric_or_datetime_type("chrono::DateTime"));
        assert!(is_numeric_or_datetime_type("chrono::NaiveDateTime"));
        assert!(is_numeric_or_datetime_type("time::OffsetDateTime"));

        // Option DateTime types
        assert!(is_numeric_or_datetime_type("Option<DateTime>"));
        assert!(is_numeric_or_datetime_type("Option<DateTime<Utc>>"));
        assert!(is_numeric_or_datetime_type("Option<chrono::DateTime>"));

        // Should NOT match
        assert!(!is_numeric_or_datetime_type("String"));
        assert!(!is_numeric_or_datetime_type("&str"));
        assert!(!is_numeric_or_datetime_type("MyCustomi32Type")); // Custom type containing "i32"
        assert!(!is_numeric_or_datetime_type("Vec<String>"));
        assert!(!is_numeric_or_datetime_type("my_module::MyDateTime")); // Custom type ending with "DateTime"
    }
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
        let field_type = &field.ty;

        // Generate placeholder based on database type
        let placeholder = get_placeholder_template(config.database);

        // Pre-generate SET clause string at compile time
        let set_clause = format!("{} = {}", column_name, placeholder);
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

        // Determine field type category
        let type_str = quote!(#field_type).to_string();

        if is_string_type(&type_str) {
            generate_update_string_methods(field_name, &column_name, config.database)
        } else if is_numeric_or_datetime_type(&type_str) {
            generate_update_numeric_datetime_methods(field_name, &column_name, config.database, field_type)
        } else {
            generate_update_basic_methods(field_name, &column_name, config.database, field_type)
        }
    }).collect::<Vec<_>>();

    // Generate custom condition methods
    let custom_methods = config.custom_conditions.iter().map(|condition| {
        generate_custom_condition_method(condition, config.database, &config.fields)
    }).collect::<Vec<_>>();

    // Pre-generate UPDATE SQL template
    let update_base_sql = format!("UPDATE {}", config.table_name);
    let update_base_literal = proc_macro2::Literal::string(&update_base_sql);

    // Generate placeholder replacement function based on database type
    let placeholder_replacement_fn = generate_placeholder_replacement_fn(config.database);

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

            // Add placeholder replacement function
            #placeholder_replacement_fn

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

                // Replace all placeholders at once with correct positions
                Self::replace_placeholders(&sql, self.where_args.len())
            }

            /// Execute update query
            pub async fn execute<'c, E>(self, executor: E) -> Result<u64, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let sql = self.build_sql();
                let where_args = self.where_args;

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
            /// ```rust,no_run
            /// # use sqlx_template::SqliteTemplate;
            /// # use sqlx::{FromRow, SqlitePool};
            /// # #[derive(SqliteTemplate, FromRow, Debug, Clone)]
            /// # #[table("users")]
            /// # #[tp_update_builder]
            /// # pub struct User {
            /// #     pub id: i32,
            /// #     pub email: String,
            /// #     pub active: bool,
            /// #     pub status: String,
            /// # }
            /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
            /// # let pool = SqlitePool::connect(":memory:").await?;
            /// # let user_id = 1;
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
            /// # Ok(())
            /// # }
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
fn generate_update_string_methods(field_name: &Ident, column_name: &str, database: Database) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);
    let by_like_method = quote::format_ident!("by_{}_like", field_name);
    let by_start_with_method = quote::format_ident!("by_{}_start_with", field_name);
    let by_end_with_method = quote::format_ident!("by_{}_end_with", field_name);

    // Generate placeholder based on database type
    let placeholder = get_placeholder_template(database);

    // Pre-generate WHERE condition strings at compile time
    let eq_condition = format!("{} = {}", column_name, placeholder);
    let neq_condition = format!("{} != {}", column_name, placeholder);
    let like_condition = format!("{} LIKE {}", column_name, placeholder);

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
fn generate_update_numeric_datetime_methods(field_name: &Ident, column_name: &str, database: Database, field_type: &SynType) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);
    let by_gt_method = quote::format_ident!("by_{}_gt", field_name);
    let by_gte_method = quote::format_ident!("by_{}_gte", field_name);
    let by_lt_method = quote::format_ident!("by_{}_lt", field_name);
    let by_lte_method = quote::format_ident!("by_{}_lte", field_name);

    // Generate placeholder based on database type
    let placeholder = get_placeholder_template(database);

    // Pre-generate WHERE condition strings at compile time
    let eq_condition = format!("{} = {}", column_name, placeholder);
    let neq_condition = format!("{} != {}", column_name, placeholder);
    let gt_condition = format!("{} > {}", column_name, placeholder);
    let gte_condition = format!("{} >= {}", column_name, placeholder);
    let lt_condition = format!("{} < {}", column_name, placeholder);
    let lte_condition = format!("{} <= {}", column_name, placeholder);

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
fn generate_update_basic_methods(field_name: &Ident, column_name: &str, database: Database, field_type: &SynType) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);

    // Generate placeholder based on database type
    let placeholder = get_placeholder_template(database);

    // Pre-generate WHERE condition strings at compile time
    let eq_condition = format!("{} = {}", column_name, placeholder);
    let neq_condition = format!("{} != {}", column_name, placeholder);

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

    // Generate placeholder replacement function based on database type
    let placeholder_replacement_fn = generate_placeholder_replacement_fn(config.database);

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

            // Add placeholder replacement function
            #placeholder_replacement_fn

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                let mut sql = #delete_base_literal.to_string();

                if !self.where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    let where_clause = self.where_conditions.join(" AND ");
                    let replaced_where = Self::replace_placeholders(&where_clause, self.where_args.len());
                    sql.push_str(&replaced_where);
                }

                sql
            }

            /// Execute delete query
            pub async fn execute<'c, E>(self, executor: E) -> Result<u64, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let sql = self.build_sql();
                let where_args = self.where_args;

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
            /// ```rust,no_run
            /// # use sqlx_template::SqliteTemplate;
            /// # use sqlx::{FromRow, SqlitePool};
            /// # #[derive(SqliteTemplate, FromRow, Debug, Clone)]
            /// # #[table("users")]
            /// # #[tp_delete_builder]
            /// # pub struct User {
            /// #     pub id: i32,
            /// #     pub active: bool,
            /// #     pub created_at: chrono::DateTime<chrono::Utc>,
            /// # }
            /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
            /// # let pool = SqlitePool::connect(":memory:").await?;
            /// # let user_id = 1;
            /// # let cutoff_date = chrono::Utc::now();
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
            /// # Ok(())
            /// # }
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

    // Create a simple SQL condition by replacing placeholders with appropriate database placeholders
    let mut condition_sql = sql_expression.to_string();
    let placeholder_template = get_placeholder_template(database);
    for placeholder in &par_res.placeholder_vars {
        condition_sql = condition_sql.replace(placeholder, placeholder_template);
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
