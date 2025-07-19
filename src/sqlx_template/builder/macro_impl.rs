use proc_macro2::{TokenStream, Literal};
use quote::quote;
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

    // Build builder with simple parameter storage and manual binding
    quote! {
        /// QueryBuilderArgs for parameter binding
        #[derive(Default, Clone)]
        pub struct #args_struct_name {
            params: Vec<String>,
        }

        impl #args_struct_name {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn add_param(&mut self, value: String) {
                self.params.push(value);
            }

            pub fn len(&self) -> usize {
                self.params.len()
            }

            pub fn is_empty(&self) -> bool {
                self.params.is_empty()
            }

            pub fn get_param_count(&self) -> usize {
                self.params.len()
            }

            pub fn get_params(&self) -> &[String] {
                &self.params
            }
        }

        /// Generated select builder
        #[derive(Clone)]
        pub struct #builder_name {
            table_name: String,
            where_conditions: Vec<String>,
            where_args: #args_struct_name,
            order_by_clauses: Vec<String>,
        }

        impl #builder_name {
            pub fn new() -> Self {
                Self {
                    table_name: #table_name.to_string(),
                    where_conditions: Vec::new(),
                    where_args: #args_struct_name::default(),
                    order_by_clauses: Vec::new(),
                }
            }

            #(#field_methods)*
            #(#order_methods)*

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                let mut sql = format!("SELECT * FROM {}", self.table_name);

                if !self.where_conditions.is_empty() {
                    sql.push_str(&format!(" WHERE {}", self.where_conditions.join(" AND ")));
                }

                if !self.order_by_clauses.is_empty() {
                    sql.push_str(&format!(" ORDER BY {}", self.order_by_clauses.join(", ")));
                }

                sql
            }

            /// Build SQL query with parameter placeholders
            pub fn build_sql_with_params(&self) -> (String, usize) {
                let sql = self.build_sql();
                (sql, self.where_args.get_param_count())
            }

            /// Execute query và return single result
            /// Note: This uses parameterized queries with proper parameter binding
            pub async fn find_one<'c, E>(self, executor: E) -> Result<Option<#struct_name>, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, where_conditions, where_args, order_by_clauses } = self;

                // Build SQL manually since we destructured self
                let mut sql = format!("SELECT * FROM {}", table_name);
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }
                if !order_by_clauses.is_empty() {
                    sql.push_str(" ORDER BY ");
                    sql.push_str(&order_by_clauses.join(", "));
                }
                sql.push_str(" LIMIT 1");

                // Manually bind parameters
                let mut query = sqlx::query_as::<_, #struct_name>(&sql);
                for param in where_args.get_params() {
                    query = query.bind(param);
                }
                query.fetch_optional(executor).await
            }

            /// Execute query và return all results
            /// Note: This uses parameterized queries with proper parameter binding
            pub async fn find_all<'c, E>(self, executor: E) -> Result<Vec<#struct_name>, sqlx::Error>
            where
                E: sqlx::Executor<'c, Database = #database_type>,
            {
                let Self { table_name, where_conditions, where_args, order_by_clauses } = self;

                // Build SQL manually since we destructured self
                let mut sql = format!("SELECT * FROM {}", table_name);
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }
                if !order_by_clauses.is_empty() {
                    sql.push_str(" ORDER BY ");
                    sql.push_str(&order_by_clauses.join(", "));
                }

                // Manually bind parameters
                let mut query = sqlx::query_as::<_, #struct_name>(&sql);
                for param in where_args.get_params() {
                    query = query.bind(param);
                }
                query.fetch_all(executor).await
            }
        }

        impl #struct_name {
            /// Create new select builder
            pub fn builder_select() -> #builder_name {
                #builder_name::new()
            }

            pub fn test_macro() -> String {
                "Macro works!".to_string()
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

    let column_literal = Literal::string(column_name);

    quote! {
        /// Equality condition
        pub fn #eq_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value.as_ref().to_string());
            self
        }

        /// Not equal condition
        pub fn #not_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value.as_ref().to_string());
            self
        }

        /// LIKE condition
        pub fn #like_method(mut self, pattern: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} LIKE ?", #column_literal));
            self.where_args.add_param(pattern.as_ref().to_string());
            self
        }

        /// STARTS WITH condition
        pub fn #start_with_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} LIKE ?", #column_literal));
            self.where_args.add_param(format!("{}%", value.as_ref()));
            self
        }

        /// ENDS WITH condition
        pub fn #end_with_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} LIKE ?", #column_literal));
            self.where_args.add_param(format!("%{}", value.as_ref()));
            self
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

    let column_literal = Literal::string(column_name);

    // Determine the appropriate add method based on type
    let type_str = quote!(#field_type).to_string();
    let add_method = if type_str.contains("i32") {
        quote!(add_i32)
    } else if type_str.contains("f64") {
        quote!(add_f64)
    } else if type_str.contains("bool") {
        quote!(add_bool)
    } else {
        // For DateTime and other types, convert to string
        quote!(add_string)
    };

    quote! {
        /// Equality condition
        pub fn #eq_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// Not equal condition
        pub fn #not_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// Greater than condition
        pub fn #gt_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} > ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// Greater than or equal condition
        pub fn #gte_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} >= ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// Less than condition
        pub fn #lt_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} < ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// Less than or equal condition
        pub fn #lte_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} <= ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
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
        pub fn #eq_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// Not equal condition
        pub fn #not_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
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

    let column_literal = Literal::string(&column_name);

    quote! {
        /// Order by field ascending (default)
        pub fn #order_asc_method(mut self) -> Self {
            self.order_by_clauses.push(format!("{} ASC", #column_literal));
            self
        }

        /// Order by field ascending (explicit)
        pub fn #order_asc_explicit_method(mut self) -> Self {
            self.order_by_clauses.push(format!("{} ASC", #column_literal));
            self
        }

        /// Order by field descending
        pub fn #order_desc_method(mut self) -> Self {
            self.order_by_clauses.push(format!("{} DESC", #column_literal));
            self
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

        // For string fields, use AsRef<str>, for others use AsRef<field_type>
        let type_str = quote!(#field_type).to_string();
        if is_string_type(&type_str) {
            quote! {
                /// Set field value for UPDATE
                pub fn #on_method(mut self, value: impl AsRef<str>) -> Self {
                    self.set_clauses.push(format!("{} = ?", #column_literal));
                    self.where_args.add_param(value.as_ref().to_string());
                    self
                }
            }
        } else {
            quote! {
                /// Set field value for UPDATE
                pub fn #on_method(mut self, value: #field_type) -> Self {
                    self.set_clauses.push(format!("{} = ?", #column_literal));
                    self.where_args.add_param(value.to_string());
                    self
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

    quote! {
        /// UpdateBuilderArgs for parameter binding
        #[derive(Default, Clone)]
        pub struct #args_struct_name {
            params: Vec<String>,
        }

        impl #args_struct_name {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn add_param(&mut self, value: String) {
                self.params.push(value);
            }

            pub fn len(&self) -> usize {
                self.params.len()
            }

            pub fn is_empty(&self) -> bool {
                self.params.is_empty()
            }

            pub fn get_param_count(&self) -> usize {
                self.params.len()
            }

            pub fn get_params(&self) -> &[String] {
                &self.params
            }
        }

        /// Generated update builder
        #[derive(Clone)]
        pub struct #builder_name {
            table_name: String,
            set_clauses: Vec<String>,
            where_conditions: Vec<String>,
            where_args: #args_struct_name,
        }

        impl #builder_name {
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

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                if self.set_clauses.is_empty() {
                    panic!("UPDATE query must have at least one SET clause. Use on_* methods.");
                }

                let mut sql = format!("UPDATE {} SET {}", self.table_name, self.set_clauses.join(", "));

                if !self.where_conditions.is_empty() {
                    sql.push_str(&format!(" WHERE {}", self.where_conditions.join(" AND ")));
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
                let mut sql = format!("UPDATE {}", table_name);
                if !set_clauses.is_empty() {
                    sql.push_str(" SET ");
                    sql.push_str(&set_clauses.join(", "));
                }
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }

                // Manually bind parameters
                let mut query = sqlx::query(&sql);
                for param in where_args.get_params() {
                    query = query.bind(param);
                }
                let result = query.execute(executor).await?;
                Ok(result.rows_affected())
            }
        }

        impl #struct_name {
            /// Create new update builder
            pub fn builder_update() -> #builder_name {
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

    let column_literal = Literal::string(column_name);

    quote! {
        /// WHERE equality condition
        pub fn #by_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value.as_ref().to_string());
            self
        }

        /// WHERE not equal condition
        pub fn #by_not_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value.as_ref().to_string());
            self
        }

        /// WHERE LIKE condition
        pub fn #by_like_method(mut self, pattern: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} LIKE ?", #column_literal));
            self.where_args.add_param(pattern.as_ref().to_string());
            self
        }

        /// WHERE STARTS WITH condition
        pub fn #by_start_with_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} LIKE ?", #column_literal));
            self.where_args.add_param(format!("{}%", value.as_ref()));
            self
        }

        /// WHERE ENDS WITH condition
        pub fn #by_end_with_method(mut self, value: impl AsRef<str>) -> Self {
            self.where_conditions.push(format!("{} LIKE ?", #column_literal));
            self.where_args.add_param(format!("%{}", value.as_ref()));
            self
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

    let column_literal = Literal::string(column_name);

    quote! {
        /// WHERE equality condition
        pub fn #by_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// WHERE not equal condition
        pub fn #by_not_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// WHERE greater than condition
        pub fn #by_gt_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} > ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// WHERE greater than or equal condition
        pub fn #by_gte_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} >= ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// WHERE less than condition
        pub fn #by_lt_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} < ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// WHERE less than or equal condition
        pub fn #by_lte_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} <= ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }
    }
}

/// Generate by_* basic methods cho other types trong update builder
fn generate_update_basic_methods(field_name: &Ident, column_name: &str, _database_type: &TokenStream, field_type: &SynType) -> TokenStream {
    let by_method = quote::format_ident!("by_{}", field_name);
    let by_not_method = quote::format_ident!("by_{}_not", field_name);

    let column_literal = Literal::string(column_name);

    quote! {
        /// WHERE equality condition
        pub fn #by_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} = ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
        }

        /// WHERE not equal condition
        pub fn #by_not_method(mut self, value: #field_type) -> Self {
            self.where_conditions.push(format!("{} != ?", #column_literal));
            self.where_args.add_param(value.to_string());
            self
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

    quote! {
        /// DeleteBuilderArgs for parameter binding
        #[derive(Default, Clone)]
        pub struct #args_struct_name {
            params: Vec<String>,
        }

        impl #args_struct_name {
            pub fn new() -> Self {
                Self::default()
            }

            pub fn add_param(&mut self, value: String) {
                self.params.push(value);
            }

            pub fn len(&self) -> usize {
                self.params.len()
            }

            pub fn is_empty(&self) -> bool {
                self.params.is_empty()
            }

            pub fn get_param_count(&self) -> usize {
                self.params.len()
            }

            pub fn get_params(&self) -> &[String] {
                &self.params
            }
        }

        /// Generated delete builder
        #[derive(Clone)]
        pub struct #builder_name {
            table_name: String,
            where_conditions: Vec<String>,
            where_args: #args_struct_name,
        }

        impl #builder_name {
            pub fn new() -> Self {
                Self {
                    table_name: #table_name.to_string(),
                    where_conditions: Vec::new(),
                    where_args: #args_struct_name::default(),
                }
            }

            #(#field_methods)*

            /// Build SQL query string
            pub fn build_sql(&self) -> String {
                let mut sql = format!("DELETE FROM {}", self.table_name);

                if !self.where_conditions.is_empty() {
                    sql.push_str(&format!(" WHERE {}", self.where_conditions.join(" AND ")));
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
                let mut sql = format!("DELETE FROM {}", table_name);
                if !where_conditions.is_empty() {
                    sql.push_str(" WHERE ");
                    sql.push_str(&where_conditions.join(" AND "));
                }

                // Manually bind parameters
                let mut query = sqlx::query(&sql);
                for param in where_args.get_params() {
                    query = query.bind(param);
                }
                let result = query.execute(executor).await?;
                Ok(result.rows_affected())
            }
        }

        impl #struct_name {
            /// Create new delete builder
            pub fn builder_delete() -> #builder_name {
                #builder_name::new()
            }
        }
    }
}
