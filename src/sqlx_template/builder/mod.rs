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

    

    pub fn from_existing_attributes(ast: &DeriveInput) -> Result<Self, syn::Error> {
        let database = super::get_database_from_ast(ast);
        Ok(Self::from_ast(ast, database))
    }
}



// /// Order by clause
// #[derive(Debug, Clone)]
// pub struct OrderBy {
//     pub field: String,
//     pub ascending: bool,
// }

// impl OrderBy {
//     pub fn to_sql(&self, db: Database) -> String {
//         let direction = if self.ascending { "ASC" } else { "DESC" };
//         format!("{} {}", super::check_column_name(self.field.clone(), db), direction)
//     }
// }
