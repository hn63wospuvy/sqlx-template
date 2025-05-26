use std::{
    collections::{HashMap, HashSet},
    fmt::format,
};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use rust_format::{Formatter, RustFmt};
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect};
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Ident, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, PathArguments, Token, Type
};

pub mod select;
pub mod update;
pub mod insert;
pub mod delete;
pub mod upsert;
pub mod raw;
pub mod ddl;
pub mod proc;

#[derive(Debug, Default)]
pub(super) enum Scope {
    #[default]
    Struct,
    Mod,
    NewMod
}

pub(super) fn create_ident(name: &str) -> Ident {
    Ident::new_raw(&name.to_lowercase(), Span::call_site())
}

pub(super) fn get_scope(ast: &DeriveInput) -> Scope {
    let mut scopes = ast
        .attrs
        .iter()
        .filter_map(|attr| {
            if let Ok(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(lit_str),
                ..
            })) = attr.parse_meta()
            {
                if path.is_ident("tp_scope") {
                    let scope_str = lit_str.value();
                    match scope_str.as_str() {
                        "struct" => Some(Scope::Struct),
                        "mod" => Some(Scope::Mod),
                        _ => panic!("Invalid scope: {scope_str}. Only `struct` or `mod` are permitted")
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    match scopes.len() {
        0 => Scope::default(), // Default is struct scope
        1 => scopes.pop().unwrap(),
        _ => panic!("More than one table_name attribute was found"),
    }
}

pub fn get_table_name(ast: &DeriveInput) -> String {
    let struct_name = &ast.ident;
    let mut table_names = ast
        .attrs
        .iter()
        .filter_map(|attr| {
            if let Ok(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(lit_str),
                ..
            })) = attr.parse_meta()
            {
                if path.is_ident("table_name") {
                    let name = lit_str.value();
                    Some(name)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    match table_names.len() {
        0 => panic!("table_name attribute not found"),
        1 => table_names.pop().unwrap(),
        _ => panic!("More than one table_name attribute was found"),
    }
}

pub fn get_debug_slow_from_table_scope(ast: &DeriveInput) -> Option<i32> {
    let struct_name = &ast.ident;
    let debug_slows : Vec<i32> = ast
        .attrs
        .iter()
        .filter_map(|attr| {
            if let Ok(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Int(slow_in_ms),
                ..
            })) = attr.parse_meta()
            {
                if path.is_ident("debug_slow") {
                    let name = slow_in_ms.base10_parse().expect("Invalid debug slow value");
                    Some(name)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    match debug_slows.len() {
        0 => None,
        1 => Some(*debug_slows.first().unwrap()),
        _ => panic!("More than one debug_slow attribute was found"),
    }
}

fn check_fields<'a>(fields_from_attr: &Vec<&'a str>, all_fields: Vec<&'a Field>) -> Vec<Field> {
    let by_fields = all_fields
        .iter()
        .filter_map(|f| {
            if fields_from_attr
                .iter()
                .any(|f_attr| f.ident.clone().is_some_and(|x| x.to_string().as_str() == *f_attr))
            {
                Some((*f).clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    by_fields
}

fn is_integer_type(ty: &Type) -> bool {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                if let PathArguments::AngleBracketed(args) = &segment.arguments {
                    if args.args.len() != 1 {
                        return false;
                    }
                    if let Some(inner_type) = args.args.first() {
                        if let GenericArgument::Type(inner_path) = inner_type {
                            return is_integer_type(&inner_path);
                        }
                    } 
                    false
                } else {
                    let type_name = &segment.ident.to_string();
                    type_name == "i8" || type_name == "i16" || type_name == "i32" || type_name == "i64"
                }
            } else {
                false
            }
        }
       
        _ => false,
    }
}

fn is_option_type(type_path: &syn::TypePath) -> bool {
    if let Some(segment) = type_path.path.segments.last() {
        let type_name = &segment.ident.to_string();
        type_name == "Option"
    } else {
        false
    }
}

fn gen_debug_code(debug_slow: Option<i32>) -> (TokenStream, TokenStream) {
    match debug_slow {
        Some(0) => {
            if cfg!(feature = "log") {
                (
                    quote! { 
                        log::debug!("[SQLxTemplate] - Query: {sql}"); 
                    },
                    quote! {}
                )
                
            } else if cfg!(feature = "tracing") {
                (
                    quote! { 
                        tracing::debug!("[SQLxTemplate] - Query: {sql}"); 
                    },
                    quote! {}
                )
            } else {
                (
                    quote! { 
                        println!("[SQLxTemplate] - Query: {sql}"); 
                    },
                    quote! {}
                )
            }
        },
        Some(slow) if slow > 0 => {
            let before = quote! { let t = std::time::Instant::now(); };
            let after = if cfg!(feature = "log") {
                quote! { 
                    let elapsed = (std::time::Instant::now() - t).as_millis() as i32;
                    if elapsed >= #debug_slow {
                        log::debug!("[SQLxTemplate] - Query elapsed: {elapsed}ms - {sql}"); 
                    }
                    
                }
            } else if cfg!(feature = "tracing") {
                quote! { 
                    let elapsed = (std::time::Instant::now() - t).as_millis() as i32;
                    if elapsed >= #debug_slow {
                        tracing::debug!("[SQLxTemplate] - Query elapsed: {elapsed}ms - {sql}"); 
                    }
                }
            } else {
                quote! { 
                    let elapsed = (std::time::Instant::now() - t).as_millis() as i32;
                    if elapsed >= #debug_slow {
                        println!("[SQLxTemplate] - Query elapsed: {elapsed}ms - {sql}"); 
                    }
                }
            };
            (before, after)
        }
        _ => (quote! {}, quote! {})
    }
}



pub fn table_name_derive(ast: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &ast.ident;

    let table_name = get_table_name(&ast);
    let expanded = quote!{
        impl #struct_name {
            #[inline]
            pub const fn table_name() -> &'static str {
                #table_name
            }
        }
    };

    Ok(expanded.into())

}

pub fn get_database() -> TokenStream {
    if cfg!(feature = "postgres") {
        quote! { sqlx::Postgres }
    } else if cfg!(feature = "sqlite") {
        quote! { sqlx::Sqlite }
    } else if cfg!(feature = "mysql") {
        quote! { sqlx::Mysql }
    } else if cfg!(feature = "any") {
        quote! { sqlx::Any }
    } else {
        panic!("Unknown database 1")
    }
}

pub fn get_database_dialect() -> Box<dyn Dialect> {
    if cfg!(feature = "postgres") {
        Box::new(PostgreSqlDialect {})
    } else if cfg!(feature = "sqlite") {
        Box::new(GenericDialect {})
    } else if cfg!(feature = "mysql") {
        Box::new(MySqlDialect {})
    } else if cfg!(feature = "any") {
        Box::new(GenericDialect {})
    } else {
        panic!("Unknown database 2")
    }
}

pub fn gen_with_doc(func: TokenStream) -> TokenStream {
    let doc_string = format!(
        "Automatically generated function by sqlx-template\n\n```rust\n{}\n```",
        RustFmt::default().format_str(func.to_string()).unwrap()
        
    );
    // Include the documentation in the generated function
    let gen_with_doc = quote! {
        #[doc = #doc_string]
        #func
    };
    gen_with_doc
}

pub fn contains(fields: &[syn::Field], field: &syn::Field) -> bool {
    let fields_name = fields.iter()
        .map(|x| get_field_name(x))
        .collect::<Vec<_>>();
    let field_name = get_field_name(field);
    fields_name.contains(&field_name)
}

pub fn get_field_name(field: &syn::Field) -> String {
    field.ident.clone().unwrap().to_string()
}

pub fn has_duplicates(vec: &Vec<Field>) -> bool {
    let vec = vec.iter()
        .map(|x| get_field_name(x))
        .collect::<Vec<_>>();
    for (i, item1) in vec.iter().enumerate() {
        for (j, item2) in vec.iter().enumerate() {
            if i != j && item1 == item2 {
                return true;
            }
        }
    }
    false
}

