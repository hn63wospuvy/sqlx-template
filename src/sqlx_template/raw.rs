use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use proc_macro2::{ TokenStream};
use proc_macro2::Span;
use quote::{quote, ToTokens};
use rust_format::RustFmt;
use sqlparser::dialect::PostgreSqlDialect;
use syn::token::Eq;
use syn::{AttributeArgs, Ident, ItemFn, Lit, Meta, MetaNameValue, NestedMeta, PathArguments, ReturnType, Type};
use syn::GenericArgument;
use rust_format::Formatter;

use crate::util::{self, Mode, ValidateQueryResult};

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

fn get_debug_slow(nested_meta: Option<&NestedMeta>) -> syn::Result<i32> { 
    let res = match nested_meta {
        Some(NestedMeta::Lit(Lit::Int(slow_in_ms))) => {
            slow_in_ms.base10_parse().map_err(|x| syn::Error::new(Span::call_site(), "Value is not valid integer"))?
        }
        Some(NestedMeta::Meta(Meta::NameValue(MetaNameValue {path, lit, eq_token}))) => {
            let path_name = path.segments
            .first()
            .expect("Invalid name-value marco at second attribute")
            .ident
            .to_string()
            ;
            if "debug" != path_name.as_str() {
                panic!("Second attribute name must be 'debug'");
            }
            match lit {
                Lit::Int(slow_in_ms) => {
                    slow_in_ms.base10_parse().map_err(|x| syn::Error::new(Span::call_site(), "Value is not valid integer"))?
                },
                _ => panic!("Expected a number for the query in the second name-value attribute")
            }
        }
        Some(NestedMeta::Meta(Meta::Path(path))) => {
            let path_name = path.segments
            .first()
            .expect("Invalid name-value marco at second attribute")
            .ident
            .to_string()
            ;
            if "debug" != path_name.as_str() {
                panic!("Second attribute name must be 'debug'");
            }
            0
        }
        _ => -1
    };
    Ok(res)
}

pub fn multi_query_derive(input: ItemFn, args: AttributeArgs, mode: Option<Mode>) -> syn::Result<TokenStream> { 
    let query_string = get_query_string(args.get(0))?; 
    let debug_slow = get_debug_slow(args.get(1))?; 
    // Extract the function name and arguments 
    let fn_name = &input.sig.ident; 
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
    let dialect = super::get_database_dialect(); 
    let queries = match util::validate_multi_query(&query_string, &param_names, dialect.as_ref()) { 
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
    let database = super::get_database(); 
    
    let final_gen = if fn_args.is_empty() {
        quote! { 
            pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(conn: E) -> Result<(), sqlx::Error> { 
                #(#queries_gen)* 
                Ok(()) 
            } 
        }
    } else {
        quote! { 
            pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(#fn_args, conn: E) -> Result<(), sqlx::Error> { 
                #(#queries_gen)* 
                Ok(()) 
            } 
        }
    }; 
    let res = super::gen_with_doc(final_gen); 
    Ok(res) }

pub fn query_derive(input: ItemFn, args: AttributeArgs, mode: Option<Mode>) -> syn::Result<TokenStream> {
    let query_string = get_query_string(args.first())?;
    let debug_slow = get_debug_slow(args.get(1))?;

    // Extract the function name and arguments
    let fn_name = &input.sig.ident;
    let fn_args = &input.sig.inputs;
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

    // Validate query
    let dialect = super::get_database_dialect();
    let ValidateQueryResult {sql, params} = match util::validate_query(&query_string, &param_names, mode, dialect.as_ref()) {
        Ok(r) => r,
        Err(e) => panic!("{e}"),
    };
    let (before, after) = super::gen_debug_code(Some(debug_slow));
    

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
    let database = super::get_database();

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
                        pub fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#fn_args, conn: E) -> #output {
                            let sql = #sql;
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
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args, conn: E) -> #output {
                            let sql = #sql;
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
                        pub fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#fn_args, conn: E) -> #output {
                            let sql = #sql;
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
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args, conn: E) -> #output {
                            let sql = #sql;
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
                pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args, conn: E) -> #output {
                    let sql = #sql;
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
                pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args, conn: E) -> #output {
                    let sql = #sql;
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
            let count_query = util::convert_to_count_query(&sql, dialect.as_ref()).unwrap();
            let count_query_fn = quote! {
                pub async fn count_query<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args, conn: E) -> core::result::Result<i64, sqlx::Error> {
                    let sql = #count_query;
                    let query = sqlx::query_scalar(sql)#(#binds)*;
                    #before
                    let result = query.fetch_one(conn).await;
                    #after
                    Ok(result?)
                }
            };
            param_names.push("offset".to_string());
            param_names.push("limit".to_string());
            
            let ValidateQueryResult {sql, params} = util::convert_to_page_query(&query_string, dialect.as_ref(), &param_names).unwrap();
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
                pub async fn data_query<'c, E: sqlx::Executor<'c, Database = #database>>(#fn_args, offset: i64, limit: i32, conn: E) -> core::result::Result<Vec<#return_type>, sqlx::Error> {
                    let sql = #sql;
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
                pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(#fn_args, page: impl Into<(i64, i32, bool)>, conn: E) -> #output {
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


    