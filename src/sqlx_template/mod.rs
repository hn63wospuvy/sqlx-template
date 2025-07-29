use std::{
    backtrace::Backtrace, collections::{HashMap, HashSet}, fmt::format
};

use proc_macro2::{Span, TokenStream};
use quote::quote;
use rust_format::{Formatter, RustFmt};
use sqlparser::{dialect::{Dialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect}, parser::Parser};
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
pub mod builder;

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

pub(super) fn check_column_name(column: String, db: Database) -> String {
    match db {
        Database::Postgres | Database::Any => {
            // Reference: https://www.postgresql.org/docs/current/sql-keywords-appendix.html
            if [
                "all", "analyse", "analyze", "and", "any", "array", "as", "asc", "asymmetric", "authorization",
                "binary", "both", "case", "cast", "check", "collate", "collation", "column", "concurrently",
                "constraint", "create", "cross", "current_catalog", "current_date", "current_role", "current_schema",
                "current_time", "current_timestamp", "current_user", "default", "deferrable", "desc", "distinct",
                "do", "else", "end", "except", "false", "fetch", "for", "foreign", "freeze", "from", "full", "grant",
                "group", "having", "ilike", "in", "initially", "inner", "intersect", "into", "is", "isnull", "join",
                "lateral", "leading", "left", "like", "limit", "localtime", "localtimestamp", "natural", "not",
                "notnull", "null", "offset", "on", "only", "or", "order", "outer", "overlaps", "placing", "primary",
                "references", "returning", "right", "select", "session_user", "similar", "some", "symmetric", "table",
                "then", "to", "trailing", "true", "union", "unique", "user", "using", "variadic", "verbose", "when",
                "where", "window", "with", "authorization", "between"
            ].contains(&column.to_lowercase().as_str()) {
                format!("\"{}\"", column)
            } else {
                column
            }
        },
        Database::Mysql => {
            // Reference: https://dev.mysql.com/doc/refman/8.0/en/keywords.html
            if [
                "add", "all", "alter", "analyze", "and", "as", "asc", "before", "between", "both", "by", "call",
                "cascade", "case", "change", "check", "column", "condition", "constraint", "continue", "convert",
                "create", "cross", "current_date", "current_time", "current_timestamp", "current_user", "cursor",
                "database", "databases", "day_hour", "day_microsecond", "day_minute", "day_second", "dec", "declare",
                "default", "delayed", "delete", "desc", "describe", "deterministic", "distinct", "distinctrow", "div",
                "drop", "dual", "each", "else", "elseif", "enclosed", "escaped", "exists", "exit", "explain", "false",
                "fetch", "for", "force", "foreign", "from", "fulltext", "generated", "get", "grant", "group", "having",
                "high_priority", "if", "ignore", "in", "index", "infile", "inner", "inout", "insensitive", "insert",
                "int", "integer", "interval", "into", "is", "iterate", "join", "key", "keys", "kill", "leading", "leave",
                "left", "like", "limit", "linear", "lines", "load", "localtime", "localtimestamp", "lock", "long",
                "loop", "low_priority", "master_bind", "master_ssl_verify_server_cert", "match", "maxvalue", "mediumint",
                "middleint", "minute_microsecond", "minute_second", "mod", "modifies", "natural", "not", "no_write_to_binlog",
                "null", "numeric", "on", "optimize", "optimizer_costs", "option", "optionally", "or", "order", "out",
                "outer", "outfile", "partition", "precision", "primary", "procedure", "purge", "range", "read", "reads",
                "read_write", "real", "references", "regexp", "release", "rename", "repeat", "replace", "require",
                "resignal", "restrict", "return", "revoke", "right", "rlike", "schema", "schemas", "second_microsecond",
                "select", "sensitive", "separator", "set", "show", "signal", "smallint", "spatial", "specific", "sql",
                "sqlexception", "sqlstate", "sqlwarning", "sql_big_result", "sql_calc_found_rows", "sql_small_result",
                "ssl", "starting", "stored", "straight_join", "table", "terminated", "then", "tinyint", "to", "trailing",
                "trigger", "true", "undo", "union", "unique", "unlock", "unsigned", "update", "usage", "use", "using",
                "utc_date", "utc_time", "utc_timestamp", "values", "varbinary", "varchar", "varcharacter", "varying",
                "when", "where", "while", "with", "write", "xor", "year_month", "zerofill"
            ].contains(&column.to_lowercase().as_str()) {
                format!("`{}`", column)
            } else {
                column
            }
        },
        Database::Sqlite => {
            // Reference: https://www.sqlite.org/lang_keywords.html
            if [
                "abort", "action", "add", "after", "all", "alter", "analyze", "and", "as", "asc", "attach", "autoincrement",
                "before", "begin", "between", "by", "cascade", "case", "cast", "check", "collate", "column", "commit",
                "conflict", "constraint", "create", "cross", "current_date", "current_time", "current_timestamp", "database",
                "default", "deferrable", "deferred", "delete", "desc", "detach", "distinct", "drop", "each", "else", "end",
                "escape", "except", "exclusive", "exists", "explain", "fail", "for", "foreign", "from", "full", "glob",
                "group", "having", "if", "ignore", "immediate", "in", "index", "indexed", "initially", "inner", "insert",
                "instead", "intersect", "into", "is", "isnull", "join", "key", "left", "like", "limit", "match", "natural",
                "no", "not", "notnull", "null", "of", "offset", "on", "or", "order", "outer", "plan", "pragma", "primary",
                "query", "raise", "recursive", "references", "regexp", "reindex", "release", "rename", "replace", "restrict",
                "right", "rollback", "row", "savepoint", "select", "set", "table", "temp", "temporary", "then", "to",
                "transaction", "trigger", "union", "unique", "update", "using", "vacuum", "values", "view", "virtual",
                "when", "where", "with", "without"
            ].contains(&column.to_lowercase().as_str()) {
                format!("\"{}\"", column)
            } else {
                column
            }
        }
    }
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
        _ => panic!("More than one `tp_scope` attribute was found"),
    }
}

pub fn get_table_name(ast: &DeriveInput) -> String {
    let mut res = None;
    ast.attrs.iter()
        .filter(|attr| attr.path.is_ident("table"))
        .for_each(|attr| {
            if res.is_some() {
                panic!("More than one `table` attribute was found")
            }
            let name = attr.parse_args::<syn::LitStr>().expect("Expected #[table(\"table_name\")]").value();
            res.replace(name);
        })
        ;
    res.expect("Missing `table` attribute")
}

pub fn get_database_from_ast(ast: &DeriveInput) -> Database {

    let mut res = None;
    ast.attrs.iter()
        .filter(|attr| attr.path.is_ident("db"))
        .for_each(|attr| {
            if res.is_some() {
                panic!("More than one `db` attribute was found")
            }
            let name = attr.parse_args::<syn::LitStr>().expect("Expected #[db(\"postgres\")]").value();
            let db = match name.to_lowercase().as_str() {
                "postgres" | "postgresql" => Database::Postgres,
                "mysql" => Database::Mysql,
                "sqlite" => Database::Sqlite,
                "any" => Database::Any,
                _ => panic!("`db`: {name} is not valid. Valid values: 'postgres', 'mysql', 'sqlite', 'any'")
            };
            res.replace(db);
        })
        ;
    res.expect("Missing `db` attribute")
}

pub fn get_database_from_input_fn(input: &ItemFn) -> Database {

    let mut res = None;
    input.attrs.iter()
        .filter(|attr| attr.path.is_ident("db"))
        .for_each(|attr| {
            if res.is_some() {
                panic!("More than one `db` attribute was found")
            }
            let name = attr.parse_args::<syn::LitStr>().expect("Expected #[db(\"postgres\")]").value();
            let db = match name.to_lowercase().as_str() {
                "postgres" | "postgresql" => Database::Postgres,
                "mysql" => Database::Mysql,
                "sqlite" => Database::Sqlite,
                "any" => Database::Any,
                _ => panic!("`db`: {name} is not valid. Valid values: 'postgres', 'mysql', 'sqlite', 'any'")
            };
            res.replace(db);
        })
        ;
    res.expect("Missing `db` attribute")
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
        Database::Mysql => quote! { sqlx::MySql },
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

pub fn get_field_name_as_column(field: &syn::Field, db: Database) -> String {
    check_column_name(field.ident.clone().unwrap().to_string(), db)
}

pub fn check_valid_single_sql(sql: &str, db: Database) {
    let dialect = get_database_dialect(db);
    let parse = Parser::parse_sql(dialect.as_ref(), sql);
    match parse {
        Err(e) => panic!("Invalid generated query: {sql}. Error: {e}. Please report"),
        Ok(x) if x.is_empty() => panic!("Empty statement"),
        Ok(x) if x.len() > 1 => panic!("Found multiple statements which is not allowed. Generated query: {sql}. Please report"),
        _ => {}
    }

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

pub fn has_attribute(input: &DeriveInput, attr_name: &str) -> bool {
    input.attrs.iter().any(|attr| attr.path.is_ident(attr_name))
}

pub fn derive_all(input: &DeriveInput, for_path: Option<&syn::Path>, scope: Scope, db: Option<Database>) -> syn::Result<TokenStream> {
    let table_name = table_name_derive(&input)?;
    let insert = insert::derive_insert(&input, for_path, scope, db)?;
    let update = update::derive_update(&input, for_path, scope, db)?;
    let select = select::derive_select(&input, for_path, scope, db)?;
    let delete = delete::derive_delete(&input, for_path, scope, db)?;
    let upsert = upsert::derive_upsert(&input, for_path, scope, db)?;

    // Note: Builder generation is handled by individual derive functions
    // to avoid duplicate generation when using SqlxTemplate

    Ok(quote! {
        #table_name
        #insert
        #update
        #select
        #delete
        #upsert
    })
}