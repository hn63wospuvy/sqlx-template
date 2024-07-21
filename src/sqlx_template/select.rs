use std::collections::HashSet;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, LitStr,
    Meta, MetaList, MetaNameValue, NestedMeta, Token,
};

use super::{get_database, get_table_name};

#[derive(Debug, PartialEq)]
enum SelectType {
    All,
    One,
    Page,
    Stream,
    Count,
}

pub fn derive_select(ast: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &ast.ident;
    let table_name = get_table_name(&ast);
    let debug_slow = super::get_debug_slow_from_table_scope(&ast);
    let all_fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named.iter().collect::<Vec<_>>()
    } else {
        panic!("SelectTemplate macro only works with structs with named fields");
    };
    let mut functions = Vec::new();
    for attr in ast.attrs {
        if let Ok(Meta::List(MetaList {
            ref path,
            ref nested,
            ..
        })) = attr.parse_meta()
        {
            if path.is_ident("tp_select_all")
                || path.is_ident("tp_select_one")
                || path.is_ident("tp_select_page")
                || path.is_ident("tp_select_stream")
                || path.is_ident("tp_select_count")
            {
                let mut by_fields = Vec::new();
                let mut order_fields = Vec::new();
                let mut fn_name = None;
                let mut debug_slow = debug_slow.clone();
                for meta in nested {
                    match meta {
                        NestedMeta::Meta(Meta::NameValue(nv)) => {
                            if nv.path.is_ident("by") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    by_fields = check_by_fields(&fields_str, all_fields.clone());
                                    if super::has_duplicates(&by_fields) {
                                        panic!("Found duplicated fields: {:?}", fields_str);
                                    }
                                    if by_fields.len() != fields_str.len() {
                                        panic!(
                                            "One of those value is duplicated or not a field in struct: {:?}",
                                            fields_str
                                        );
                                    }
                                } else {
                                    panic!("Expected string value by = \"...\"");
                                }
                            } else if nv.path.is_ident("order") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    order_fields =
                                        check_order_fields(&fields_str, all_fields.clone());
                                    let  order_fields_only = order_fields.iter()
                                        .map(|x| x.0.clone())
                                        .collect::<Vec<_>>();
                                    if super::has_duplicates(&order_fields_only) {
                                        panic!("Found duplicated fields: {:?}", fields_str);
                                    }
                                    if order_fields.len() != fields_str.len() {
                                        panic!(
                                            "One of those value is duplicated or not a field in struct: {:?}",
                                            fields_str
                                        );
                                    }
                                } else {
                                    panic!("Expected string value order = \"...\"");
                                }
                            } else if nv.path.is_ident("fn_name") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    fn_name.replace(lit);
                                } else {
                                    panic!("Expected string value fn_name = \"...\"");
                                }
                            } else if nv.path.is_ident("debug") {
                                if let Lit::Int(lit) = &nv.lit {
                                    let slow_in_ms = lit.base10_parse().expect("Invalid debug value. Must be integer");
                                    debug_slow.replace(slow_in_ms);
                                } 
                            }
                        }
                        _ => {}
                    }
                }

                by_fields.sort_by_key(|x| x.ident.clone());
                order_fields.sort_by_key(|x| x.0.ident.clone());

                let generated = match path.get_ident().unwrap().to_string().as_str() {
                    "tp_select_all" => build_query(
                        SelectType::All,
                        struct_name,
                        &table_name,
                        &all_fields,
                        by_fields,
                        order_fields,
                        fn_name,
                        debug_slow,
                    )?,
                    "tp_select_one" => build_query(
                        SelectType::One,
                        struct_name,
                        &table_name,
                        &all_fields,
                        by_fields,
                        order_fields,
                        fn_name,
                        debug_slow,
                    )?,
                    "tp_select_page" => build_query(
                        SelectType::Page,
                        struct_name,
                        &table_name,
                        &all_fields,
                        by_fields,
                        order_fields,
                        fn_name,
                        debug_slow,
                    )?,
                    "tp_select_stream" => build_query(
                        SelectType::Stream,
                        struct_name,
                        &table_name,
                        &all_fields,
                        by_fields,
                        order_fields,
                        fn_name,
                        debug_slow,
                    )?,
                    "tp_select_count" => build_query(
                        SelectType::Count,
                        struct_name,
                        &table_name,
                        &all_fields,
                        by_fields,
                        order_fields,
                        fn_name,
                        debug_slow,
                    )?,
                    _ => None,
                };
                if let Some(generated) = generated {
                    functions.push(super::gen_with_doc(generated));
                }
            } else {
            }
        }
    }
    functions.push(super::gen_with_doc(build_default_find_all_query(&struct_name, &table_name, debug_slow, &all_fields)));
    functions.push(super::gen_with_doc(build_default_count_all_query(&struct_name, &table_name, debug_slow)));
    functions.push(super::gen_with_doc(build_default_find_page_all_query(&struct_name, &table_name, debug_slow, &all_fields)));
    let expanded = quote! {
        impl #struct_name {
            #(#functions)*
        }
    };

    Ok(expanded.into())
}

fn build_default_find_all_query(
    struct_name: &Ident,
    table_name: &str,
    debug_slow: Option<i32>,
    all_fields: &Vec<&Field>,
) -> proc_macro2::TokenStream {
    let all_fields_str = all_fields.iter().filter_map(|x| x.ident.clone().and_then(|y| Some(y.to_string()))).collect::<Vec<String>>();
    let all_fields_str = all_fields_str.join(", ");
    let sql = format!("SELECT {all_fields_str} FROM {table_name}");
    let database = super::get_database();
    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
    let expanded = quote! {
        pub async fn find_all<'c, E: sqlx::Executor<'c, Database = #database>>( conn: E) -> Result<Vec<#struct_name>, sqlx::Error> {
            let sql = #sql;
            #dbg_before
            let query_result = sqlx::query_as::<_, #struct_name>(sql)
                .fetch_all(conn)
                .await;
            #dbg_after
            Ok(query_result?)
        }
    };
    expanded.into()
}

fn build_default_find_page_all_query(
    struct_name: &Ident,
    table_name: &str,
    debug_slow: Option<i32>,
    all_fields: &Vec<&Field>,
) -> proc_macro2::TokenStream {
    let database = super::get_database();
    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
    let all_fields_str = all_fields.iter().filter_map(|x| x.ident.clone().and_then(|y| Some(y.to_string()))).collect::<Vec<String>>();
    let all_fields_str = all_fields_str.join(", ");
    let sql = format!("SELECT {all_fields_str} FROM {table_name} LIMIT $1 OFFSET $2");
    let count_sql = format!("SELECT COUNT(1) FROM {table_name}");
    let expanded = quote! {
        pub async fn find_page_all<'c, E: sqlx::Executor<'c, Database = #database> + Copy>(page: impl Into<(i64, i32, bool)>, conn: E) -> Result<(Vec<#struct_name>, Option<i64>), sqlx::Error> {
            async fn data_query<'c, E: sqlx::Executor<'c, Database = #database>>(offset: i64, limit: i32, conn: E) -> Result<Vec<#struct_name>, sqlx::Error> {
                let sql = #sql;
                #dbg_before
                let query_result = sqlx::query_as::<_, #struct_name>(sql)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(conn)
                    .await;
                #dbg_after
                Ok(query_result?)
            }
            pub async fn count_query<'c, E: sqlx::Executor<'c, Database = #database>>( conn: E) -> Result<i64, sqlx::Error> {
                let sql = #sql;
                #dbg_before
                let count = sqlx::query_scalar(sql)
                    .fetch_one(conn)
                    .await;
                #dbg_after
                Ok(count?)
            }
            let page = page.into();
            let offset = page.0;
            let limit = page.1;
            let count = page.2;
            let data = data_query(offset, limit, conn).await?;
            let count = if count {
                if data.is_empty() && offset == 0 {
                    Some(0)
                } else {
                    Some(count_query(conn).await?)
                }
                
            } else {
                None
            };
            Ok((data, count))
        }
    };
    expanded.into()
}

fn build_default_count_all_query(
    struct_name: &Ident,
    table_name: &str,
    debug_slow: Option<i32>,
) -> proc_macro2::TokenStream {
    let sql = format!("SELECT COUNT(1) FROM {table_name}");
    let database = super::get_database();
    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
    let expanded = quote! {
        pub async fn count_all<'c, E: sqlx::Executor<'c, Database = #database>>( conn: E) -> Result<i64, sqlx::Error> {
            let sql = #sql;
            #dbg_before
            let count = sqlx::query_scalar(sql)
                .fetch_one(conn)
                .await;
            #dbg_after
            Ok(count?)
        }
    };
    expanded.into()
}

fn build_query(
    qtype: SelectType,
    struct_name: &Ident,
    table_name: &str,
    all_fields: &Vec<&Field>,
    by_fields: Vec<Field>,
    order_fields: Vec<(Field, bool)>,
    fn_name: Option<String>,
    debug_slow: Option<i32>,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    let database = super::get_database();
    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
    let all_fields_str = all_fields.iter().filter_map(|x| x.ident.clone().and_then(|y| Some(y.to_string()))).collect::<Vec<String>>();
    let all_fields_str = all_fields_str.join(", ");
    match (by_fields.is_empty(), order_fields.is_empty()) {
        (true, true) => {
            // Do nothing. Default implemention
        }
        (true, false) => {
            let mut post_fix = format!(
                "order_by_{}",
                order_fields
                    .iter()
                    .map(|f| {
                        let mut field_str = f.0.ident.as_ref().unwrap().to_string();
                        if f.1 {
                            field_str.push_str("_asc")
                        } else {
                            field_str.push_str("_desc")
                        }
                        field_str
                    })
                    .collect::<Vec<_>>()
                    .join("_and_")
            );
            let fn_name = match fn_name {
                Some(fn_name) => Ident::new(
                    &fn_name,
                    proc_macro2::Span::call_site(),
                ),
                None => {
                    match qtype {
                        SelectType::All => Ident::new(
                            &format!("find_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::One => Ident::new(
                            &format!("find_one_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::Page => Ident::new(
                            &format!("find_page_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::Stream => Ident::new(
                            &format!("stream_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::Count => Ident::new(
                            &format!("count_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                    }
                },
            };

            let order_str = order_fields
                .iter()
                .map(|f| {
                    let mut field_str = f.0.ident.as_ref().unwrap().to_string();
                    if !f.1 {
                        field_str.push_str(" DESC ")
                    }
                    field_str
                })
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!("SELECT {all_fields_str} FROM {table_name} ORDER BY {order_str}");
            let count_sql = format!("SELECT COUNT(1) FROM {table_name} ORDER BY {order_str}");
            let generated = match qtype {
                SelectType::All => {
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>( conn: E) -> Result<Vec<#struct_name>, sqlx::Error> {
                            let sql = #sql;
                            #dbg_before
                            let query_result = sqlx::query_as::<_, #struct_name>(sql)
                                .fetch_all(conn)
                                .await;
                            #dbg_after
                            Ok(query_result?)
                        }
                    }
                }
                SelectType::One => {
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>( conn: E) -> Result<Option<#struct_name>, sqlx::Error> {
                            let sql = #sql;
                            #dbg_before
                            let query_result = sqlx::query_as::<_, #struct_name>(sql)
                                .fetch_optional(conn)
                                .await;
                            #dbg_after
                            Ok(query_result?)
                        }
                    }
                }
                SelectType::Page => {
                    let total_binds_args = by_fields.len();
                    let paging_sql = format!(
                        "{} LIMIT ${} OFFSET ${} ",
                        sql,
                        total_binds_args + 1,
                        total_binds_args + 2
                    );
                    let mut total_binds = vec![];
                    total_binds.push(quote! {
                        .bind(paging_limit)
                    });
                    total_binds.push(quote! {
                        .bind(paging_offset)
                    });
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy + 'c>( page: impl Into<(i64, i32, bool)>, conn: E) -> Result<(Vec<#struct_name>, Option<i64>), sqlx::Error> {
                            pub async fn data_query<'c, E: sqlx::Executor<'c, Database = #database> + 'c>( paging_offset: i64, paging_limit: i32, conn: E) -> Result<Vec<#struct_name>, sqlx::Error> {
                                let sql = #paging_sql;
                                #dbg_before
                                let query_result = sqlx::query_as::<_, #struct_name>(sql)
                                    #(#total_binds)*
                                    .fetch_all(conn)
                                    .await;
                                #dbg_after
                                Ok(query_result?)
                            }
                            pub async fn count_query<'c, E: sqlx::Executor<'c, Database = #database> + 'c>( conn: E) -> Result<i64, sqlx::Error> {
                                let sql = #sql;
                                #dbg_before
                                let count = sqlx::query_scalar(sql)
                                    #(#total_binds)*
                                    .fetch_one(conn)
                                    .await;
                                #dbg_after
                                Ok(query_result?)
                            }

                            let page = page.into();
                            let offset = page.0;
                            let limit = page.1;
                            let count = page.2;
                            let data = data_query(offset, limit, conn).await?;
                            let count = if count {
                                if data.is_empty() && offset == 0 {
                                    Some(0)
                                } else {
                                    Some(count_query(conn).await?)
                                }
                            } else {
                                None
                            };
                            Ok((data, count))

                        }

                    }
                }
                SelectType::Stream => {
                    quote! {
                        pub fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>( conn: E) -> futures::stream::BoxStream<'c, Result<#struct_name, sqlx::Error>> {
                            let sql = #sql;
                            #dbg_before
                            let query_result = sqlx::query_as(sql)
                                .fetch(conn)
                                ;
                            #dbg_after
                            query_result
                        }
                    }
                }
                SelectType::Count => {
                    return Ok(None); // Do nothing
                }
            };
            return Ok(Some(generated));
        }
        (false, _) => {
            let mut post_fix = format!(
                "by_{}",
                by_fields
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join("_and_")
            );
            if !order_fields.is_empty() && qtype != SelectType::Count {
                post_fix.push_str(&format!(
                    "_order_by_{}",
                    order_fields
                        .iter()
                        .map(|f| {
                            let mut field_str = f.0.ident.as_ref().unwrap().to_string();
                            if f.1 {
                                field_str.push_str("_asc")
                            } else {
                                field_str.push_str("_desc")
                            }
                            field_str
                        })
                        .collect::<Vec<_>>()
                        .join("_and_")
                ))
            }
            let fn_name = match fn_name {
                Some(fn_name) => Ident::new(
                    &fn_name,
                    proc_macro2::Span::call_site(),
                ),
                None => {
                    match qtype {
                        SelectType::All => Ident::new(
                            &format!("find_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::One => Ident::new(
                            &format!("find_one_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::Page => Ident::new(
                            &format!("find_page_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::Stream => Ident::new(
                            &format!("stream_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                        SelectType::Count => Ident::new(
                            &format!("count_{}", post_fix),
                            proc_macro2::Span::call_site(),
                        ),
                    }
                },
            };

            let fn_args = by_fields
                .iter()
                .map(|field| {
                    let arg_name = field.ident.as_ref().unwrap();
                    let arg_type = &field.ty;
                    quote! { #arg_name: &'c #arg_type }
                })
                .collect::<Vec<_>>();
            
            let condition = by_fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    format!(
                        "{} = ${}",
                        field.ident.clone().unwrap().to_string(),
                        index + 1
                    )
                })
                .collect::<Vec<_>>()
                .join(" AND ");

            let count_sql = format!("SELECT COUNT(1) FROM {} WHERE {}", &table_name, condition);
            let sql = if order_fields.is_empty() {
                format!("SELECT {all_fields_str} FROM {} WHERE {}", &table_name, condition)
            } else {
                let order_str = order_fields
                    .iter()
                    .map(|f| {
                        let mut field_str = f.0.ident.as_ref().unwrap().to_string();
                        if !f.1 {
                            field_str.push_str(" DESC ")
                        }
                        field_str
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "SELECT * FROM {} WHERE {} ORDER BY {}",
                    &table_name, condition, order_str
                )
            };
            let binds = by_fields.iter().map(|field| {
                let arg_name = field.ident.as_ref().unwrap();
                quote! {
                    .bind(#arg_name)
                }
            });
            
            let generated = match qtype {
                SelectType::All => {
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#(#fn_args),* , conn: E) -> Result<Vec<#struct_name>, sqlx::Error> {
                            let sql = #sql;
                            #dbg_before
                            let query_result = sqlx::query_as::<_, #struct_name>(sql)
                                #(#binds)*
                                .fetch_all(conn)
                                .await;
                            #dbg_after
                            Ok(query_result?)
                        }
                    }
                }
                SelectType::One => {
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#(#fn_args),* , conn: E) -> Result<Option<#struct_name>, sqlx::Error> {
                            let sql = #sql;
                            #dbg_before
                            let query_result = sqlx::query_as::<_, #struct_name>(sql)
                                #(#binds)*
                                .fetch_optional(conn)
                                .await;
                            #dbg_after
                            Ok(query_result?)
                        }
                    }
                }
                SelectType::Page => {
                    let total_binds_args = by_fields.len();
                    let paging_sql = format!(
                        "{} LIMIT ${} OFFSET ${} ",
                        sql,
                        total_binds_args + 1,
                        total_binds_args + 2
                    );
                    let mut total_binds = binds.collect::<Vec<_>>();
                    let mut total_binds_for_count = total_binds.clone();
                    total_binds.push(quote! {
                        .bind(paging_limit)
                    });
                    total_binds.push(quote! {
                        .bind(paging_offset)
                    });

                    let fn_args_name = by_fields
                    .iter()
                    .map(|field| {
                        let arg_name = field.ident.as_ref().unwrap();
                        let arg_type = &field.ty;
                        quote! { #arg_name }
                    })
                    .collect::<Vec<_>>();
                    let fn_args_name_clone = fn_args_name.clone();
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + Copy + 'c>(#(#fn_args),* , page: impl Into<(i64, i32, bool)>, conn: E) -> Result<(Vec<#struct_name>, Option<i64>), sqlx::Error> {
                            pub async fn data_query<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#(#fn_args),* , paging_offset: i64, paging_limit: i32, conn: E) -> Result<Vec<#struct_name>, sqlx::Error> {
                                let sql = #paging_sql;
                                #dbg_before
                                let query_result = sqlx::query_as::<_, #struct_name>(sql)
                                    #(#total_binds)*
                                    .fetch_all(conn)
                                    .await;
                                #dbg_after
                                Ok(query_result?)
                            }
                            pub async fn count_query<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#(#fn_args),* , conn: E) -> Result<i64, sqlx::Error> {
                                let sql = #count_sql;
                                #dbg_before
                                let query_result = sqlx::query_scalar(sql)
                                    #(#total_binds_for_count)*
                                    .fetch_one(conn)
                                    .await;
                                #dbg_after
                                Ok(query_result?)
                            }
                            let page = page.into();
                            let offset = page.0;
                            let limit = page.1;
                            let count = page.2;
                            let data = data_query(#(#fn_args_name),*, offset, limit, conn).await?;
                            let count = if count {
                                if data.is_empty() && offset == 0 {
                                    Some(0)
                                } else {
                                    Some(count_query(#(#fn_args_name),*,conn).await?)
                                }
                            } else {
                                None
                            };
                            Ok((data, count))
                        }
                    }
                }
                SelectType::Stream => {
                    quote! {
                        pub fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#(#fn_args),* , conn: E) -> futures::stream::BoxStream<'c, Result<#struct_name, sqlx::Error>> {
                            let sql = #sql;
                            #dbg_before
                            let query_result = sqlx::query_as(sql)
                                #(#binds)*
                                .fetch(conn);
                            #dbg_after
                            query_result
                        }
                    }
                }
                SelectType::Count => {
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#(#fn_args),* , conn: E) -> Result<i64, sqlx::Error> {
                            let sql = #count_sql;
                            #dbg_before
                            let count = sqlx::query_scalar(sql)
                                #(#binds)*
                                .fetch_one(conn)
                                .await;
                            #dbg_after
                            Ok(count?)
                        }
                    }
                }
            };
            return Ok(Some(generated));
        }
    }
    Ok(None)
}

fn check_by_fields<'a>(fields_from_attr: &Vec<&'a str>, all_fields: Vec<&'a Field>) -> Vec<Field> {
    let by_fields = all_fields
        .iter()
        .filter_map(|f| {
            if fields_from_attr
                .iter()
                .any(|f_attr| f.ident.clone().is_some_and(|x| x.to_string() == **f_attr))
            {
                Some((*f).clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    by_fields
}

fn extract_field_and_asc(str: &str) -> (&str, bool) {
    let mut split = str.split_whitespace();
    let field = split.next().expect("Invalid order attribute");
    let asc = split
        .next()
        .map(|x| {
            if x.eq_ignore_ascii_case("asc") {
                return true;
            } else if x.eq_ignore_ascii_case("desc") {
                return false;
            } else {
                panic!("Expected order = \"<field name> asc|desc\"");
            }
        })
        .unwrap_or(true);
    (field, asc)
}

fn has_duplicate_fields(set: &Vec<(&str, bool)>) -> bool {
    let mut seen_strings = HashSet::new();

    for &(ref s, _) in set {
        if !seen_strings.insert(s.clone()) {
            return true;
        }
    }

    false
}

fn has_duplicates<T: PartialEq>(vec: &Vec<T>) -> bool {
    for (i, item1) in vec.iter().enumerate() {
        for (j, item2) in vec.iter().enumerate() {
            if i != j && item1 == item2 {
                return true;
            }
        }
    }
    false
}

fn check_order_fields<'a>(
    fields_from_attr: &Vec<&'a str>,
    all_fields: Vec<&'a Field>,
) -> Vec<(Field, bool)> {
    let fields_and_asc_from_attr = fields_from_attr
        .iter()
        .copied()
        .map(|x| extract_field_and_asc(x))
        .collect::<Vec<_>>();
    if (has_duplicate_fields(&fields_and_asc_from_attr)) {
        panic!("Found duplicated fields: {:?}", fields_from_attr);
    };
    let by_fields = all_fields
        .iter()
        .filter_map(|f| {
            let field_and_asc = fields_and_asc_from_attr
                .iter()
                .filter(|f_attr| f.ident.clone().is_some_and(|x| x.to_string() == *f_attr.0))
                .collect::<Vec<_>>();
            match field_and_asc.len() {
                0 => None,
                1 => {
                    let field_and_asc = field_and_asc.first().unwrap();
                    Some(((*f).clone(), field_and_asc.1))
                }
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    by_fields
}
