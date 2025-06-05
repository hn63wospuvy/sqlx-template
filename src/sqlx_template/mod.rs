use std::{
    collections::{HashMap, HashSet},
    fmt::format,
};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use rust_format::{Formatter, RustFmt};
use sqlparser::dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect};
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, GenericArgument, Ident, ItemFn, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, PathArguments, Token, Type
};

use crate::delete;

pub mod select;
pub mod update;
pub mod insert;
pub mod delete;
pub mod upsert;
pub mod raw;
pub mod ddl;
pub mod proc;

#[derive(Debug, Default, Clone, Copy)]
pub(super) enum Scope {
    #[default]
    Struct,
    Mod,
    NewMod
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) enum Database {
    #[default]
    Postgres,
    Sqlite,
    Mysql,
    Any
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

pub fn get_database_from_ast(ast: &DeriveInput) -> Database {
    if cfg!(feature = "postgres") {
        return Database::Postgres;
    } else if cfg!(feature = "sqlite") {
        return Database::Sqlite;
    } else if cfg!(feature = "mysql") {
        return Database::Mysql;
    } else if cfg!(feature = "any") {
        return Database::Any;
    }
    let struct_name = &ast.ident;
    let mut dbs = ast
        .attrs
        .iter()
        .filter_map(|attr| {
            if let Ok(Meta::NameValue(MetaNameValue {
                path,
                lit: Lit::Str(lit_str),
                ..
            })) = attr.parse_meta()
            {
                if path.is_ident("database") {
                    let name = lit_str.value();
                    match name.to_lowercase().as_str() {
                        "postgres" | "postgresql" => Some(Database::Postgres),
                        "mysql" => Some(Database::Mysql),
                        "sqlite" => Some(Database::Sqlite),
                        "any" => Some(Database::Any),
                        _ => panic!("`database`: {name} is not valid. Valid values: 'postgres', 'mysql', 'sqlite', 'any'")
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    match dbs.len() {
        0 => panic!("`database` attribute not found. Valid values: 'postgres', 'mysql', 'sqlite', 'any'"),
        1 => dbs.pop().unwrap(),
        _ => panic!("More than one database attribute was found"),
    }
}

pub fn get_database_from_input_fn(input: &ItemFn) -> Database {
    if cfg!(feature = "postgres") {
        return Database::Postgres;
    } else if cfg!(feature = "sqlite") {
        return Database::Sqlite;
    } else if cfg!(feature = "mysql") {
        return Database::Mysql;
    } else if cfg!(feature = "any") {
        return Database::Any;
    }
    let db = input.attrs.iter()
        .find(|attr| attr.path.is_ident("database"))
        .map(|attr| {
            let name = attr.parse_args::<syn::LitStr>().unwrap().value();
            match name.to_lowercase().as_str() {
                "postgres" | "postgresql" => Database::Postgres,
                "mysql" => Database::Mysql,
                "sqlite" => Database::Sqlite,
                "any" => Database::Any,
                _ => panic!("`database`: {name} is not valid. Valid values: 'postgres', 'mysql', 'sqlite', 'any'")
            }
        })
        .expect("Missing `database` attribute");
    db
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



pub fn table_name_derive(ast: &DeriveInput) -> syn::Result<TokenStream> {
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

pub fn get_database_type(db: Database) -> TokenStream {
    match db {
        Database::Postgres => quote! { sqlx::Postgres },
        Database::Sqlite => quote! { sqlx::Sqlite },
        Database::Mysql => quote! { sqlx::Mysql },
        Database::Any => quote! { sqlx::Any },
    }

}

pub fn get_database_dialect(db: Database) -> Box<dyn Dialect> {
    match db {
        Database::Postgres => Box::new(PostgreSqlDialect {}),
        Database::Sqlite => Box::new(SQLiteDialect {}),
        Database::Mysql => Box::new(MySqlDialect {}),
        Database::Any => Box::new(GenericDialect {}),
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

pub fn derive_all(input: &DeriveInput, for_path: Option<&syn::Path>, scope: Scope, db: Option<Database>) -> syn::Result<TokenStream> {
    let table_name = table_name_derive(&input)?;
    let insert = insert::derive_insert(&input, for_path, scope, db)?;
    let update = update::derive_update(&input, for_path, scope, db)?;
    let select = select::derive_select(&input, for_path, scope, db)?;
    let delete = delete::derive_delete(&input, for_path, scope, db)?;
    let upsert = upsert::derive_upsert(&input, for_path, scope, db)?;

    Ok(quote! {
        #table_name
        #insert
        #update
        #select
        #delete
        #upsert
    })
}