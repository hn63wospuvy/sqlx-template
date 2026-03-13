use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use proc_macro2::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use rust_format::RustFmt;
use sqlparser::dialect::PostgreSqlDialect;
use syn::token::Eq;
use syn::{AttributeArgs, Ident, ItemFn, Lit, Meta, MetaNameValue, NestedMeta, PathArguments, ReturnType, Type};
use syn::GenericArgument;
use rust_format::Formatter;

use crate::parser::{self, Mode, ValidateQueryResult};
use crate::sqlx_template::Database;

/// Extract `$StructName` patterns from SQL and return (cleaned_sql, Vec<struct_name>).
/// Replaces `$StructName` with `*` for SQL validation purposes.
fn extract_struct_column_templates(sql: &str) -> (String, Vec<String>) {
    let mut result = String::new();
    let mut struct_names = Vec::new();
    let mut chars = sql.chars().peekable();
    
    while let Some(ch) = chars.next() {
        if ch == '$' {
            // Check if next char is an uppercase letter (struct name)
            if let Some(&next_ch) = chars.peek() {
                if next_ch.is_ascii_uppercase() {
                    // Collect the struct name
                    let mut name = String::new();
                    while let Some(&c) = chars.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            name.push(c);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if !struct_names.contains(&name) {
                        struct_names.push(name.clone());
                    }
                    // Replace with * for SQL validation
                    result.push('*');
                    continue;
                }
            }
            result.push(ch);
        } else {
            result.push(ch);
        }
    }
    
    (result, struct_names)
}

enum QueryType {
    Data,
    Scalar,
    RowAfftected,
    Void,
    Page,
}

#[derive(PartialEq)]
enum DataType {
    Single,
    Vec,
    Option,
    Stream
}

fn get_query_string(nested_meta: Option<&NestedMeta>) -> syn::Result<String> {
    let res = match nested_meta {
        Some(NestedMeta::Meta(Meta::NameValue(MetaNameValue {path, lit, eq_token}))) => {
            let path_name = path.segments
                .first()
                .expect("Invalid name-value marco at first attribute")
                .ident
                .to_string()
                ;
            let value = match lit {
                    Lit::Str(lit_str) => {
                        lit_str.value()
                    },
                    _ => panic!("Expected a string literal for the query in the first name-value attribute")
                };
            let query = match path_name.as_str() {
                "sql" => value,
                "file" => {
                    let current_dir = std::env::current_dir().unwrap();
                    let manifest_dir = env!("CARGO_MANIFEST_DIR");
                    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
                    let rooted = root.join(&value);
                    if rooted.exists() {
                        let mut file = File::open(rooted).expect("Unable to open file");
                        let mut contents = String::new();
                        file.read_to_string(&mut contents).expect("Failed to read file");
                        contents
                    } else {
                        let message = format!("{rooted:?} not found");
                        return Err(syn::Error::new(Span::call_site(), message))
                    }
                }
                _ => panic!("First attribute name must be 'sql' or 'file'")
            };
            query
        }
        Some(NestedMeta::Lit(Lit::Str(lit_str))) => {
            lit_str.value()
        },
        _ => panic!("Expected a string literal for the query in the first attribute")
    };
    Ok(res)
}

fn get_debug_slow(args: &AttributeArgs) -> syn::Result<i32> {
    for arg in args {
        match arg {
            NestedMeta::Lit(Lit::Int(slow_in_ms)) => {
                return slow_in_ms.base10_parse().map_err(|x| syn::Error::new(Span::call_site(), "Value is not valid integer"));
            }
            NestedMeta::Meta(Meta::NameValue(MetaNameValue {path, lit, ..})) => {
                if path.is_ident("debug") {
                    match lit {
                        Lit::Int(slow_in_ms) => {
                            return slow_in_ms.base10_parse().map_err(|x| syn::Error::new(Span::call_site(), "Value is not valid integer"));
                        },
                        _ => panic!("Expected a number for debug attribute")
                    }
                }
            }
            NestedMeta::Meta(Meta::Path(path)) => {
                if path.is_ident("debug") {
                    return Ok(0);
                }
            }
            _ => {}
        }
    }
    Ok(-1)
}

fn parse_instrument_config(args: &AttributeArgs) -> super::InstrumentConfig {
    for arg in args {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {path, lit, ..})) = arg {
            if path.is_ident("instrument") {
                match lit {
                    Lit::Str(lit_str) => {
                        let value = lit_str.value();
                        return super::InstrumentConfig::Skip(value);
                    }
                    Lit::Bool(lit_bool) => {
                        if lit_bool.value() {
                            return super::InstrumentConfig::Enabled;
                        } else {
                            return super::InstrumentConfig::None;
                        }
                    }
                    _ => panic!("instrument attribute must be a string or boolean")
                }
            }
        } else if let NestedMeta::Meta(Meta::Path(path)) = arg {
            if path.is_ident("instrument") {
                return super::InstrumentConfig::Enabled;
            }
        }
    }
    super::InstrumentConfig::SkipAll
}

pub fn multi_query_derive(input: ItemFn, args: AttributeArgs, mode: Option<Mode>, db: Option<Database>) -> syn::Result<TokenStream> { 
    let query_string = get_query_string(args.get(0))?; 
    let debug_slow = get_debug_slow(&args)?; 
    let db = db.unwrap_or_else(|| super::get_database_from_input_fn(&input));
    
    // Parse instrument configuration from attributes
    let instrument_config = parse_instrument_config(&args);
    
    // Extract the function name and arguments 
    let fn_name = &input.sig.ident;
    
    // Generate instrument attribute for tracing
    // IMPORTANT: cfg! must be OUTSIDE quote! macro
    let fn_name_str = fn_name.to_string();
    let instrument_attr = super::gen_instrument_attr(&instrument_config, &fn_name_str, &fn_name_str);
    
    let fn_args = &input.sig.inputs; 
    let mut map_args = HashMap::new(); 
    let mut param_names: Vec<String> = fn_args.iter()
        .filter_map(|arg| { 
            match arg { 
                syn::FnArg::Typed(pat_type) => { 
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat { 
                        let name = pat_ident.ident.to_string(); 
                        map_args.insert(name.clone(), pat_ident.ident.clone()); 
                        Some(name) 
                    } else { 
                        None 
                    } 
                } 
                syn::FnArg::Receiver(_) => None, 
            } }).collect(); 
    // Validate query
    let dialect = super::get_database_dialect(db);
    let queries = match parser::validate_multi_query_with_db(&query_string, &param_names, dialect.as_ref(), db) {
        Ok(r) => r,
        Err(e) => panic!("{e}"),
    };
    let mut queries_gen = vec![]; 
    for query in queries { 
        let (before, after) = super::gen_debug_code(Some(debug_slow)); 
        let binds = &query.params.iter().map(|field| { 
            // param starts with ':' 
            let arg_param = map_args.get(&field[1..]).expect("Ident not found"); 
            quote! { 
                .bind(&#arg_param) 
            } 
        }).collect::<Vec<_>>(); 
        let sql = query.sql; 
        let gen = quote! { 
            let sql = #sql; 
            let query = sqlx::query(sql)#(#binds)*; 
            #before 
            let query = query.execute(conn).await; 
            #after query?; 
        }; 
        queries_gen.push(gen); 
    } 
    let database = super::get_database_type(db); 
    
    let final_gen = if fn_args.is_empty() {
        quote! {
            #instrument_attr
            pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(conn: E) -> Result<(), sqlx::Error> { 
                #(#queries_gen)* 
                Ok(()) 
            } 
        }
    } else {
        quote! {
            #instrument_attr
            pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(#fn_args, conn: E) -> Result<(), sqlx::Error> { 
                #(#queries_gen)* 
                Ok(()) 
            } 
        }
    }; 
    let res = super::gen_with_doc(final_gen); 
    Ok(res) }

pub fn query_derive(input: ItemFn, args: AttributeArgs, mode: Option<Mode>, db: Option<Database>) -> syn::Result<TokenStream> {
    let query_string = get_query_string(args.first())?;
    let debug_slow = get_debug_slow(&args)?;
    
    let db = db.unwrap_or_else(|| super::get_database_from_input_fn(&input));

    // Parse instrument configuration from attributes
    let instrument_config = parse_instrument_config(&args);

    // Extract $StructName templates from SQL before validation
    let (query_string_for_validation, struct_templates) = extract_struct_column_templates(&query_string);
    
    // Use original query_string for final SQL generation if no templates found
    let has_struct_templates = !struct_templates.is_empty();

    // Extract the function name and arguments
    let fn_name = &input.sig.ident;
    
    // Generate instrument attribute for tracing
    // IMPORTANT: cfg! must be OUTSIDE quote! macro
    let fn_name_str = fn_name.to_string();
    let instrument_attr = super::gen_instrument_attr(&instrument_config, &fn_name_str, &fn_name_str);
    
    let fn_args = &input.sig.inputs;
    let fn_args_with_comma = if fn_args.is_empty() {
        quote! {}
    } else {
        quote! {#fn_args ,}
    };
    let mut map_args = HashMap::new();
    let mut param_names: Vec<String> = fn_args.iter().filter_map(|arg| {
        match arg {
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let name = pat_ident.ident.to_string();
                    map_args.insert(name.clone(), pat_ident.ident.clone());
                    Some(name)
                } else {
                    None
                }
            }
            syn::FnArg::Receiver(_) => None,
        }
    }).collect();

    // Validate query (use cleaned SQL with $Struct replaced by * for validation)
    let dialect = super::get_database_dialect(db);
    let ValidateQueryResult {sql: validated_sql, params} = match parser::validate_query_with_db(&query_string_for_validation, &param_names, mode, dialect.as_ref(), db) {
        Ok(r) => r,
        Err(e) => panic!("{e}"),
    };
    
    // If we have struct templates, we need to process the original query string
    // to replace $StructName with the validated result's structure but keep template references
    let sql = if has_struct_templates {
        // Re-validate using original query string but replace $Struct with COLUMNS_STR references
        // We'll generate the SQL at compile time by reconstructing from original with param replacements
        let ValidateQueryResult {sql: original_validated, params: _} = match parser::validate_query_with_db(&query_string_for_validation, &param_names, mode, dialect.as_ref(), db) {
            Ok(r) => r,
            Err(e) => panic!("{e}"),
        };
        // Replace the * back with $StructName patterns in validated SQL
        let mut final_sql = original_validated;
        for struct_name in &struct_templates {
            // The validated SQL has * where $StructName was
            // We need to replace the first occurrence of * that corresponds to each template
            // Since validation may reformat, reconstruct from original query with param replacements applied
            final_sql = final_sql.replacen("*", &format!("${}", struct_name), 1);
        }
        final_sql
    } else {
        validated_sql
    };
    let (before, after) = super::gen_debug_code(Some(debug_slow));
    
    // Generate SQL assignment: if struct templates exist, generate runtime replacement code
    let sql_assignment = if has_struct_templates {
        // Generate: let sql_str = BASE_SQL.replace("$User", User::COLUMNS_STR).replace(...);
        let mut replace_chain = quote! { let mut sql_str = #sql.to_string(); };
        for struct_name in &struct_templates {
            let search_pattern = format!("${}", struct_name);
            let struct_ident = Ident::new(struct_name, Span::call_site());
            replace_chain = quote! {
                #replace_chain
                sql_str = sql_str.replace(#search_pattern, #struct_ident::COLUMNS_STR);
            };
        }
        quote! {
            #replace_chain
            let sql: &str = &sql_str;
        }
    } else {
        quote! { let sql = #sql; }
    };

    // Generate bind statement by param extracted from query


    // Extract the return type and determine the SQLx fetch function
    let (output, fetch_call, return_type, query_type, data_type) = match &input.sig.output {
        ReturnType::Type(_, ty) => {
            match ty.as_ref() {
                Type::Path(type_path) => {
                    let segment = &type_path.path.segments.first().unwrap();
                    match segment.ident.to_string().as_str() {
                        "Option" => {
                            let generic = get_nested_type_to_token_stream(&segment.arguments).unwrap();

                            (
                                quote! { Result<Option<#generic>, sqlx::Error> },
                                quote! { query.fetch_optional(conn).await },
                                Some(generic),
                                QueryType::Data,
                                Some(DataType::Option),
                            )
                        },
                        "Vec" => {
                            let generic = get_nested_type_to_token_stream(&segment.arguments).unwrap();

                            (
                                quote! { Result<Vec<#generic>, sqlx::Error> },
                                quote! { query.fetch_all(conn).await },
                                Some(generic),
                                QueryType::Data,
                                Some(DataType::Vec),
                            )
                        },
                        "Stream" => {
                            let generic = get_nested_type_to_token_stream(&segment.arguments).unwrap();
                            (
                                quote! { futures::stream::BoxStream<'c, core::result::Result<#generic, sqlx::Error>> },
                                quote! { query.fetch(conn) },
                                Some(generic),
                                QueryType::Data,
                                Some(DataType::Stream),
                            )
                        },
                        "Scalar" => {
                            if let Some((nested_type, nested_nested_type)) = get_nested_type(&segment.arguments) {
                                match nested_type.to_string().as_str() {
                                    "Option" => {
                                        (
                                            quote! { Result<Option<#nested_nested_type>, sqlx::Error> },
                                            quote! { query.fetch_optional(conn).await },
                                            nested_nested_type,
                                            QueryType::Scalar,
                                            Some(DataType::Option),
                                        )
                                    },
                                    "Vec" => {
                                        (
                                            quote! { Result<Option<#nested_nested_type>, sqlx::Error> },
                                            quote! { query.fetch_all(conn).await },
                                            nested_nested_type,
                                            QueryType::Scalar,
                                            Some(DataType::Vec),
                                        )
                                    },
                                    "Stream" => {
                                        (
                                            quote! { futures::stream::BoxStream<'c, core::result::Result<#nested_nested_type, sqlx::Error>> },
                                            quote! { query.fetch(conn) },
                                            nested_nested_type,
                                            QueryType::Scalar,
                                            Some(DataType::Stream),
                                        )
                                    },
                                    _ => {
                                        (
                                            quote! { core::result::Result<#nested_type, sqlx::Error> },
                                            quote! { query.fetch_one(conn).await },
                                            Some(nested_type.to_token_stream()),
                                            QueryType::Scalar,
                                            Some(DataType::Single),
                                        )
                                    }
                                }
                            } else {
                                panic!("Not a valid Scalar type")
                            }
                            
                        },
                        "Page" => {
                            let generic = get_nested_type_to_token_stream(&segment.arguments).unwrap();
                            (
                                quote! { Result<(Vec<#generic>, Option<i64>), sqlx::Error> },
                                quote! { },
                                Some(generic),
                                QueryType::Page,
                                Some(DataType::Single),
                            )
                        }
                        "RowAfftected" => {
                            (
                                quote! { core::result::Result<u64, sqlx::Error> },
                                quote! { query.execute(conn).await },
                                None,
                                QueryType::RowAfftected,
                                Some(DataType::Single),
                            )
                        },
                        _ => {
                            match get_nested_type_to_token_stream(&segment.arguments) {
                                Some(_) => panic!("Unsupported return type. Valid types:  T, Vec<T>, Option<T>, Stream<T>, Page<T>, Scalar<T>, RowAfftected"),
                                None => {
                                    let ident = segment.ident.clone();
                                    (
                                        quote! { Result<#ident, sqlx::Error> },
                                        quote! { query.fetch_one(conn).await },
                                        Some(ident.to_token_stream()),
                                        QueryType::Data,
                                        Some(DataType::Single),
                                    )
                                }
                            }
                        }
                    }
                },
                Type::Tuple(tuple) => {
                    (
                        quote! { Result<#tuple, sqlx::Error> },
                        quote! { query.fetch_one(conn).await },
                        Some(tuple.to_token_stream()),
                        QueryType::Data,
                        Some(DataType::Single),
                    )
                }
                _ => panic!("Unsupported fetch method for return type")
            }
        },
        ReturnType::Default => {
            (
                quote! { Result<(), sqlx::Error> },
                quote! { query.execute(conn).await },
                None,
                QueryType::Void,
                Some(DataType::Single),
            )
        }
    };

    // Choose database by feature
    let database = super::get_database_type(db);

    let binds = if data_type == Some(DataType::Stream) {
        params.iter().map(|field| {
            // param starts with ':'
            let arg_param = map_args.get(&field[1..]).expect("Ident not found");
            quote! {
                .bind(#arg_param.to_owned())
            }
        }).collect::<Vec<_>>()
    } else {
        params.iter().map(|field| {
            // param starts with ':'
            let arg_param = map_args.get(&field[1..]).expect("Ident not found");
            quote! {
                .bind(&#arg_param)
            }
        }).collect::<Vec<_>>()
    };

    // Generate the new function with the connection parameter
    let gen = match query_type {
        QueryType::Data => {
            match data_type {
                Some(DataType::Stream) => {
                    quote! {
                        pub fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#fn_args_with_comma conn: E) -> #output {
                            #sql_assignment
                            let query = sqlx::query_as::<_, #return_type>(sql)#(#binds)*;
                            #before
                            let result = #fetch_call;
                            #after
                            result
                        }
                    }
                }
                _ => {
                    quote! {
                        #instrument_attr
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args_with_comma conn: E) -> #output {
                            #sql_assignment
                            let query = sqlx::query_as::<_, #return_type>(sql)#(#binds)*;
                            #before
                            let result = #fetch_call;
                            #after
                            Ok(result?)
                        }
                    }
                }
            }
            
        },
        QueryType::Scalar => {
            match data_type {
                Some(DataType::Stream) => {
                    quote! {
                        pub fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#fn_args_with_comma conn: E) -> #output {
                            #sql_assignment
                            let query = sqlx::query_scalar(sql)#(#binds)*;
                            #before
                            let result = #fetch_call;
                            #after
                            result
                        }
                    }
                }
                _ => {
                    quote! {
                        #instrument_attr
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args_with_comma conn: E) -> #output {
                            #sql_assignment
                            let query = sqlx::query_scalar(sql)#(#binds)*;
                            #before
                            let result = #fetch_call;
                            #after
                            Ok(result?)
                        }
                    }
                }
            }
            
        },
        QueryType::RowAfftected => {
            quote! {
                #instrument_attr
                pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args_with_comma conn: E) -> #output {
                    #sql_assignment
                    let query = sqlx::query(sql)#(#binds)*;
                    #before
                    let result = #fetch_call;
                    #after
                    Ok(result?.rows_affected())
                }
            }
        },
        QueryType::Void => {
            quote! {
                #instrument_attr
                pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args_with_comma conn: E) -> #output {
                    #sql_assignment
                    let query = sqlx::query(sql)#(#binds)*;
                    #before
                    let query = #fetch_call;
                    #after
                    query?;
                    Ok(())
                }
            }
        },
        QueryType::Page => {
            let count_query = parser::convert_to_count_query(&sql, dialect.as_ref()).unwrap();
            let count_sql_assignment = if has_struct_templates {
                let mut replace_chain = quote! { let mut sql_str = #count_query.to_string(); };
                for struct_name in &struct_templates {
                    let search_pattern = format!("${}", struct_name);
                    let struct_ident = Ident::new(struct_name, Span::call_site());
                    replace_chain = quote! {
                        #replace_chain
                        sql_str = sql_str.replace(#search_pattern, #struct_ident::COLUMNS_STR);
                    };
                }
                quote! {
                    #replace_chain
                    let sql: &str = &sql_str;
                }
            } else {
                quote! { let sql = #count_query; }
            };
            let count_query_fn = quote! {
                #instrument_attr
                pub async fn count_query<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args_with_comma conn: E) -> core::result::Result<i64, sqlx::Error> {
                    #count_sql_assignment
                    let query = sqlx::query_scalar(sql)#(#binds)*;
                    #before
                    let result = query.fetch_one(conn).await;
                    #after
                    Ok(result?)
                }
            };
            param_names.push("offset".to_string());
            param_names.push("limit".to_string());
            
            let ValidateQueryResult {sql: page_sql, params} = parser::convert_to_page_query_with_db(&query_string_for_validation, dialect.as_ref(), &param_names, db).unwrap();
            // Re-inject $StructName into page_sql if templates exist
            let page_sql = if has_struct_templates {
                let mut final_sql = page_sql;
                for struct_name in &struct_templates {
                    final_sql = final_sql.replacen("*", &format!("${}", struct_name), 1);
                }
                final_sql
            } else {
                page_sql
            };
            let page_sql_assignment = if has_struct_templates {
                let mut replace_chain = quote! { let mut sql_str = #page_sql.to_string(); };
                for struct_name in &struct_templates {
                    let search_pattern = format!("${}", struct_name);
                    let struct_ident = Ident::new(struct_name, Span::call_site());
                    replace_chain = quote! {
                        #replace_chain
                        sql_str = sql_str.replace(#search_pattern, #struct_ident::COLUMNS_STR);
                    };
                }
                quote! {
                    #replace_chain
                    let sql: &str = &sql_str;
                }
            } else {
                quote! { let sql = #page_sql; }
            };
            let page_binds = params.iter().map(|field| {
                // param starts with ':'
                if field.as_str() == ":offset" {
                    quote! {
                        .bind(&offset)
                    }
                } else if field.as_str() == ":limit" {
                    quote! {
                        .bind(&limit)
                    }
                } else {
                    let arg_param = map_args.get(&field[1..]).expect("Ident not found");
                    quote! {
                        .bind(&#arg_param)
                    }
                }
                
            });
            let data_query_fn = quote! {
                #instrument_attr
                pub async fn data_query<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args_with_comma offset: i64, limit: i32, conn: E) -> core::result::Result<Vec<#return_type>, sqlx::Error> {
                    #page_sql_assignment
                    let query = sqlx::query_as::<_, #return_type>(sql)#(#page_binds)*;
                    #before
                    let result = query.fetch_all(conn).await;
                    #after
                    Ok(result?)
                }
            };


            let call_args = fn_args.iter().filter_map(|arg| {
                match arg {
                    syn::FnArg::Typed(pat_type) => {
                        if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                            let arg_name = &pat_ident.ident;
                            Some(quote! {#arg_name})
                        } else {
                            None
                        }
                    }
                    syn::FnArg::Receiver(_) => None,
                }
            }).collect::<Vec<_>>();
            let call_args_clone = call_args.clone();
            

            quote! {
                #instrument_attr
                pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(#fn_args_with_comma page: impl Into<(i64, i32, bool)>, conn: E) -> #output {
                    #data_query_fn

                    #count_query_fn

                    let page = page.into();
                    let offset = page.0;
                    let limit = page.1;
                    let count = page.2;
                    let data = data_query(#(#call_args),* , offset, limit, conn).await?;
                    let count = if count {
                        if data.is_empty() && offset == 0 {
                            Some(0)
                        } else {
                            Some(count_query(#(#call_args_clone),* , conn).await?)
                        }
                        
                    } else {
                        None
                    };
                    Ok((data, count))
                }   
            }

            // // Parallel by async runtime
            // let generated = if cfg!(feature = "tokio") {
            //     quote! { 
            //         pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(#fn_args, offset: i64, limit: i64, count: bool, conn: E) -> #output {
            //             #data_query_fn

            //             #count_query_fn

            //             let count_task = if count {
            //                 Some(tokio::spawn(async move {
            //                     count_query(#(#call_args),* , conn).await
            //                 }))
            //             } else {
            //                 None
            //             };

            //             let data = data_query(#(#call_args_clone),* , offset, limit, conn).await?;
            //             let count = match count_task {
            //                 Some(task) => {
            //                     task.await?.map_err(|_| sqlx::Error::WorkerCrashed)?
            //                 }
            //                 None => None
            //             }
            //             Ok((data, count))
            //         }   
            //     }
            // } else {
            //     quote! {
            //         pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(#fn_args, offset: i64, limit: i64, count: bool, conn: E) -> #output {
            //             #data_query_fn

            //             #count_query_fn

            //             let data = data_query(#(#call_args),* , offset, limit, conn).await?;
            //             let count = if count {
            //                 Some(count_query(#(#call_args_clone),* , conn).await?)
            //             } else {
            //                 None
            //             };
            //             Ok((data, count))
            //         }   
            //     }
            // };
            // dbg!(generated.to_string());
            // generated
        },
    };
    let res = super::gen_with_doc(gen);
    Ok(res)
}

fn get_nested_type(path_arg: &PathArguments) -> Option<(Ident, Option<TokenStream>)> {
    match &path_arg {
        PathArguments::None => return None,
        PathArguments::AngleBracketed(arg) => {
            if arg.args.len() > 1 {
                panic!("Only 1 generic type is allowed");
            }
            if arg.args.len() == 0 {
                return None
            }

            // match &arg.args.first() {
            //     Some(&GenericArgument::Type(Type::Path(ref t))) => {
            //         if let Some(t) = t.path.segments.first() {
            //             let nested_type = t.ident.clone();
            //             let nested_nested_type = get_nested_type_to_token_stream(&t.arguments);
            //             return Some((nested_type, nested_nested_type));
            //         } else {
            //             return None
            //         }
            //     }
            //     Some(&GenericArgument::Type(Type::Tuple(ref t))) => {
                    
            //         return Some(t.to_token_stream().into())
            //     }
            //     _ => panic!("Invalid generic type 1")
            // }
            if let Some(&GenericArgument::Type(Type::Path(ref t))) = &arg.args.first() {
                if let Some(t) = t.path.segments.first() {
                    let nested_type = t.ident.clone();
                    let nested_nested_type = get_nested_type_to_token_stream(&t.arguments);
                    return Some((nested_type, nested_nested_type));
                }
            }
            panic!("Invalid generic type 1")
        },
        _ => panic!("Return type must not contain parentheses"),
    };
}

fn get_nested_type_to_token_stream(path_arg: &PathArguments) -> Option<TokenStream> {
    match &path_arg {
        PathArguments::None => return None,
        PathArguments::AngleBracketed(arg) => {
            if arg.args.len() > 1 {
                panic!("Only 1 generic type is allowed");
            }
            if arg.args.len() == 0 {
                return None
            }
            let first_arg = arg.args.first();
            match first_arg {
                Some(&GenericArgument::Type(Type::Path(ref t))) => {
                    return Some(t.to_token_stream().into())
                }
                Some(&GenericArgument::Type(Type::Tuple(ref t))) => {
                    return Some(t.to_token_stream().into())
                }
                _ => panic!("Invalid generic type 2")
            }
            // ;
            // if let Some(&GenericArgument::Type(Type::Path(ref t))) = &arg.args.first() {
            //     return Some(t.to_token_stream().into());
            // }
            // panic!("Invalid generic type")
        },
        _ => panic!("Return type must not contain parentheses"),
    };
}


    