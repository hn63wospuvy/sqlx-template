use std::collections::HashMap;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, Field, Type as SynType};

use crate::sqlx_template::{Database, get_field_name, get_field_name_as_column, get_database_type};

pub mod macro_impl;

/// Convert CamelCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch.is_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }

    result
}



/// Common traits and utilities for builder pattern
pub trait BuilderField {
    fn field_name(&self) -> String;
    fn field_type(&self) -> &SynType;
    fn is_string_type(&self) -> bool;
    fn is_numeric_type(&self) -> bool;
    fn is_datetime_type(&self) -> bool;
}

impl BuilderField for Field {
    fn field_name(&self) -> String {
        self.ident.as_ref().unwrap().to_string()
    }

    fn field_type(&self) -> &SynType {
        &self.ty
    }

    fn is_string_type(&self) -> bool {
        match &self.ty {
            SynType::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    let type_name = segment.ident.to_string();
                    type_name == "String" || type_name == "str"
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn is_numeric_type(&self) -> bool {
        match &self.ty {
            SynType::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    let type_name = segment.ident.to_string();
                    matches!(type_name.as_str(), 
                        "i8" | "i16" | "i32" | "i64" | "i128" |
                        "u8" | "u16" | "u32" | "u64" | "u128" |
                        "f32" | "f64" | "isize" | "usize"
                    )
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn is_datetime_type(&self) -> bool {
        match &self.ty {
            SynType::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    let type_name = segment.ident.to_string();
                    type_name == "DateTime" || type_name == "OffsetDateTime" || type_name == "NaiveDateTime"
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

// Note: QueryBuilderArgs implementation sẽ được generate trong macro
// để tránh dependency issues với sqlx trong builder module

/// Determine database type from existing attributes
fn determine_database_from_attributes(ast: &DeriveInput) -> Result<Database, syn::Error> {
    // Check for #[db("type")] attribute
    for attr in &ast.attrs {
        if attr.path.is_ident("db") {
            if let Ok(meta) = attr.parse_args::<syn::LitStr>() {
                return match meta.value().as_str() {
                    "sqlite" => Ok(Database::Sqlite),
                    "postgres" => Ok(Database::Postgres),
                    "mysql" => Ok(Database::Mysql),
                    "any" => Ok(Database::Any),
                    db_type => Err(syn::Error::new(
                        meta.span(),
                        format!("Unsupported database type: {}", db_type)
                    )),
                };
            }
        }
    }

    // Check derive macros to determine database type
    for attr in &ast.attrs {
        if attr.path.is_ident("derive") {
            if let Ok(syn::Meta::List(meta_list)) = attr.parse_meta() {
                for nested in meta_list.nested {
                    if let syn::NestedMeta::Meta(syn::Meta::Path(path)) = nested {
                        if let Some(ident) = path.get_ident() {
                            match ident.to_string().as_str() {
                                "SqliteTemplate" => return Ok(Database::Sqlite),
                                "PostgresTemplate" => return Ok(Database::Postgres),
                                "MysqlTemplate" => return Ok(Database::Mysql),
                                "SqlxTemplate" => {
                                    // For SqlxTemplate, must have #[db] attribute
                                    // Continue checking for #[db] attribute below
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    // No database info found - this is an error
    Err(syn::Error::new(
        proc_macro2::Span::call_site(),
        "No database type found. Use #[derive(SqliteTemplate)], #[derive(PostgresTemplate)], #[derive(MysqlTemplate)], or #[derive(SqlxTemplate)] with #[db(\"type\")]"
    ))
}

/// Re-export macro implementation
pub use macro_impl::impl_select_builder;

/// Generate builder struct name from original struct name
pub fn get_builder_struct_name(original_name: &str, builder_type: &str) -> String {
    format!("{}{}Builder", original_name, builder_type)
}

/// Generate method names for different filter types
pub fn generate_filter_method_names(field_name: &str) -> HashMap<String, String> {
    let mut methods = HashMap::new();
    
    // Basic equality
    methods.insert("eq".to_string(), field_name.to_string());
    methods.insert("not_eq".to_string(), format!("{}_not", field_name));
    
    // String-specific methods
    methods.insert("like".to_string(), format!("{}_like", field_name));
    methods.insert("not_like".to_string(), format!("{}_not_like", field_name));
    methods.insert("start_with".to_string(), format!("{}_start_with", field_name));
    methods.insert("not_start_with".to_string(), format!("{}_not_start_with", field_name));
    methods.insert("end_with".to_string(), format!("{}_end_with", field_name));
    methods.insert("not_end_with".to_string(), format!("{}_not_end_with", field_name));
    methods.insert("in".to_string(), format!("{}_in", field_name));
    methods.insert("not_in".to_string(), format!("{}_not_in", field_name));
    
    // Numeric/DateTime-specific methods
    methods.insert("gt".to_string(), format!("{}_gt", field_name));
    methods.insert("gte".to_string(), format!("{}_gte", field_name));
    methods.insert("lt".to_string(), format!("{}_lt", field_name));
    methods.insert("lte".to_string(), format!("{}_lte", field_name));
    
    methods
}

/// Generate order by method names
pub fn generate_order_method_names(field_name: &str) -> HashMap<String, String> {
    let mut methods = HashMap::new();
    
    methods.insert("asc".to_string(), format!("order_by_{}", field_name));
    methods.insert("asc_explicit".to_string(), format!("order_by_{}_asc", field_name));
    methods.insert("desc".to_string(), format!("order_by_{}_desc", field_name));
    
    methods
}

/// Common builder configuration
#[derive(Clone)]
pub struct BuilderConfig {
    pub struct_name: String,
    pub table_name: String,
    pub database: Database,
    pub debug_slow: Option<i32>,
    pub fields: Vec<Field>,
}

impl BuilderConfig {
    pub fn from_ast(ast: &DeriveInput, db: Database) -> Self {
        let struct_name = ast.ident.to_string();
        let table_name = super::get_table_name(ast);
        let debug_slow = super::get_debug_slow_from_table_scope(ast);
        
        let fields = if let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
            ..
        }) = ast.data
        {
            named.iter().cloned().collect()
        } else {
            panic!("Builder macro only works with structs with named fields");
        };

        Self {
            struct_name,
            table_name,
            database: db,
            debug_slow,
            fields,
        }
    }

    pub fn from_attributes(args: &str, attrs: &[syn::Attribute], ast: &DeriveInput) -> Result<Self, syn::Error> {
        // Parse macro arguments
        let mut database = Database::Sqlite; // default
        let mut table_name = String::new();

        // Parse args string like 'db = "sqlite", table = "users"'
        if !args.is_empty() {
            for part in args.split(',') {
                let part = part.trim();
                if let Some((key, value)) = part.split_once('=') {
                    let key = key.trim();
                    let value = value.trim().trim_matches('"');

                    match key {
                        "db" => {
                            database = match value {
                                "sqlite" => Database::Sqlite,
                                "postgres" => Database::Postgres,
                                "mysql" => Database::Mysql,
                                "any" => Database::Any,
                                _ => return Err(syn::Error::new(
                                    proc_macro2::Span::call_site(),
                                    format!("Unsupported database: {}", value)
                                )),
                            };
                        }
                        "table" => {
                            table_name = value.to_string();
                        }
                        _ => {}
                    }
                }
            }
        }

        // If table name not specified in args, try to get from attributes or derive from struct name
        if table_name.is_empty() {
            // Look for #[table("name")] attribute
            for attr in attrs {
                if attr.path.is_ident("table") {
                    if let Ok(meta) = attr.parse_args::<syn::LitStr>() {
                        table_name = meta.value();
                        break;
                    }
                }
            }

            // If still empty, derive from struct name
            if table_name.is_empty() {
                // Fallback: convert struct name to snake_case
                let struct_name = ast.ident.to_string();
                table_name = to_snake_case(&struct_name);
            }
        }

        let struct_name = ast.ident.to_string();

        // Skip debug_slow for now to avoid calling get_table_name
        let debug_slow = None; // super::get_debug_slow_from_table_scope(ast);

        let fields = if let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
            ..
        }) = ast.data
        {
            named.iter().cloned().collect()
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Builder macro only works with structs with named fields"
            ));
        };

        Ok(Self {
            struct_name,
            table_name,
            database,
            debug_slow,
            fields,
        })
    }

    pub fn from_existing_attributes(ast: &DeriveInput) -> Result<Self, syn::Error> {
        let struct_name = ast.ident.to_string();

        // Parse existing #[table("name")] attribute
        let table_name = super::get_table_name(ast);

        // Parse existing #[db("type")] attribute or derive macro to determine database
        let database = determine_database_from_attributes(ast)?;

        // Skip debug_slow for now
        let debug_slow = None;

        let fields = if let syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
            ..
        }) = ast.data
        {
            named.iter().cloned().collect()
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "Builder macro only works with structs with named fields"
            ));
        };

        Ok(Self {
            struct_name,
            table_name,
            database,
            debug_slow,
            fields,
        })
    }
}

/// Filter condition for WHERE clause
#[derive(Debug, Clone)]
pub enum FilterCondition {
    Eq(String, String),      // field = value
    NotEq(String, String),   // field != value
    Like(String, String),    // field LIKE value
    NotLike(String, String), // field NOT LIKE value
    In(String, String),      // field IN (values)
    NotIn(String, String),   // field NOT IN (values)
    Gt(String, String),      // field > value
    Gte(String, String),     // field >= value
    Lt(String, String),      // field < value
    Lte(String, String),     // field <= value
}

impl FilterCondition {
    pub fn to_sql(&self, db: Database) -> String {
        match self {
            FilterCondition::Eq(field, _) => format!("{} = ?", super::check_column_name(field.clone(), db)),
            FilterCondition::NotEq(field, _) => format!("{} != ?", super::check_column_name(field.clone(), db)),
            FilterCondition::Like(field, _) => format!("{} LIKE ?", super::check_column_name(field.clone(), db)),
            FilterCondition::NotLike(field, _) => format!("{} NOT LIKE ?", super::check_column_name(field.clone(), db)),
            FilterCondition::In(field, _) => format!("{} IN (?)", super::check_column_name(field.clone(), db)),
            FilterCondition::NotIn(field, _) => format!("{} NOT IN (?)", super::check_column_name(field.clone(), db)),
            FilterCondition::Gt(field, _) => format!("{} > ?", super::check_column_name(field.clone(), db)),
            FilterCondition::Gte(field, _) => format!("{} >= ?", super::check_column_name(field.clone(), db)),
            FilterCondition::Lt(field, _) => format!("{} < ?", super::check_column_name(field.clone(), db)),
            FilterCondition::Lte(field, _) => format!("{} <= ?", super::check_column_name(field.clone(), db)),
        }
    }

    pub fn get_value(&self) -> &String {
        match self {
            FilterCondition::Eq(_, v) | FilterCondition::NotEq(_, v) |
            FilterCondition::Like(_, v) | FilterCondition::NotLike(_, v) |
            FilterCondition::In(_, v) | FilterCondition::NotIn(_, v) |
            FilterCondition::Gt(_, v) | FilterCondition::Gte(_, v) |
            FilterCondition::Lt(_, v) | FilterCondition::Lte(_, v) => v,
        }
    }
}

/// Order by clause
#[derive(Debug, Clone)]
pub struct OrderBy {
    pub field: String,
    pub ascending: bool,
}

impl OrderBy {
    pub fn to_sql(&self, db: Database) -> String {
        let direction = if self.ascending { "ASC" } else { "DESC" };
        format!("{} {}", super::check_column_name(self.field.clone(), db), direction)
    }
}
