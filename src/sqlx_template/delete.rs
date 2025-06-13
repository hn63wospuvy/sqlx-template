use std::{collections::HashMap, str::FromStr};

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, Path, Token
};

use crate::{parser, sqlx_template::{get_database_from_ast, get_field_name_as_column, Database}};

use super::{get_debug_slow_from_table_scope, get_field_name, get_table_name, Scope};

pub fn derive_delete(ast: &DeriveInput, for_path: Option<&syn::Path>, scope: super::Scope, db: Option<Database>) -> syn::Result<TokenStream> {
    let struct_name = &ast.ident;
    let struct_name = match for_path {
        Some(path) => quote! {#path},
        None => quote! {#struct_name},
    };
    let table_name = get_table_name(&ast);
    let db = db.or_else(|| Some(get_database_from_ast(&ast))).expect("Missing db config");
    
    let debug_slow = super::get_debug_slow_from_table_scope(&ast);
    let all_fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named.iter().collect::<Vec<_>>()
    } else {
        panic!("DeleteTemplate macro only works with structs with named fields");
    };
    let all_columns_name = all_fields.iter().map(|x| get_field_name_as_column(x, db)).collect::<Vec<_>>();
    let mut functions = Vec::new();
    
    for attr in &ast.attrs {
        if let Ok(Meta::List(MetaList {
            ref path,
            ref nested,
            ..
        })) = attr.parse_meta()
        {
            let mut by_fields = Vec::new();
            let mut fn_name_attr = None;
            let mut return_entity = false;
            let mut debug_slow = debug_slow.clone();
            let mut where_stmt_str = None;
            if path.is_ident("tp_delete") {
                for meta in nested {
                    match meta {
                        NestedMeta::Meta(Meta::NameValue(nv)) => {
                            if nv.path.is_ident("by") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    by_fields = super::check_fields(&fields_str, all_fields.clone());
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
                            } else if nv.path.is_ident("fn_name") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    fn_name_attr.replace(lit);
                                }
                            } else if nv.path.is_ident("returning") {
                                if let Lit::Bool(lit) = &nv.lit {
                                    let lit = lit.value();
                                    return_entity = lit;
                                } 
                            } else if nv.path.is_ident("where") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    if !lit.trim().is_empty() {
                                        where_stmt_str.replace(lit);
                                    }
                                }
                            } else if nv.path.is_ident("debug") {
                                if let Lit::Int(lit) = &nv.lit {
                                    let slow_in_ms = lit.base10_parse().expect("Invalid debug value. Must be integer");
                                    debug_slow.replace(slow_in_ms);
                                } 
                            }
                
                        },
                        _ => {}
                    }
                }
                if by_fields.is_empty() {
                    panic!("'by' fields must not be empty");
                }
                by_fields.sort_by_key(|x| x.ident.clone());
                let fn_name = if let Some(fn_name) = fn_name_attr {
                    Ident::new(
                        &fn_name,
                        proc_macro2::Span::call_site(),
                    )
                } else {
                    Ident::new(
                        &format!(
                            "delete_by_{}",
                            by_fields
                                .iter()
                                .map(|f| f.ident.as_ref().expect("Must be ident").to_string())
                                .collect::<Vec<_>>()
                                .join("_and_")
                        ),
                        proc_macro2::Span::call_site(),
                    )
                };

                let mut fn_args = by_fields
                    .iter()
                    .map(|field| {
                        let arg_name = field.ident.as_ref().unwrap();
                        let arg_type = &field.ty;
                        if &arg_type.to_token_stream().to_string() == "String" {
                            quote! { #arg_name: &str }
                        } else {
                            quote! { #arg_name: &#arg_type }
                        }
                    })
                    .collect::<Vec<_>>();
                let mut binds = by_fields.iter().map(|field| {
                    let arg_name = field.ident.as_ref().unwrap();
                    quote! {
                        .bind(&#arg_name)
                    }
                }).collect::<Vec<_>>();
                let mut where_condition = by_fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        format!(
                            "{} = ${}",
                            get_field_name_as_column(field, db),
                            index + 1
                        )
                    })
                    .collect::<Vec<_>>()
                    ;

                if let Some(where_stmt_str) = where_stmt_str {
                    let par_res = parser::get_columns_and_compound_ids(&where_stmt_str, super::get_database_dialect(db)).unwrap();

                    for col in &par_res.columns {
                        if !all_columns_name.contains(&col) {
                            panic!("Invalid where statement: {col} column is not found in field list");
                        }
                    }
                    for table in &par_res.tables {
                        if table != &table_name  {
                            panic!("Invalid where statement: {table} is not allowed. Only {table_name} are permitted.");
                        }
                    }
                    if !par_res.placeholder_vars.is_empty() {
                        let all_fields_map = all_fields
                            .iter()
                            .map(|x| (get_field_name(x), x.clone()))
                            .collect::<HashMap<_, _>>();
                        let by_fields_map = by_fields
                            .iter()
                            .map(|x| (get_field_name(x), x.clone()))
                            .collect::<HashMap<_, _>>();
                        let mut extend_fields = par_res.placeholder_vars.iter()
                        .filter_map(|p| {
                            let p = &p[1..];
                            if !all_fields_map.contains_key(p) {
                                panic!("Field {p} is not found in list columns name");
                            }
                            
                            all_fields_map.get(p)
                            .map(|field|{
                                let arg_name = field.ident.as_ref().unwrap();
                                let arg_type = &field.ty;
                                if by_fields_map.contains_key(p) {
                                    (quote! {
                                        .bind(&#arg_name)
                                    }, 
                                    None)
                                }
                                else if &arg_type.to_token_stream().to_string() == "String" {
                                    (quote! {
                                        .bind(&#arg_name)
                                    }, 
                                    Some(quote! { #arg_name: &str }))
                                } else {
                                    (quote! {
                                        .bind(&#arg_name)
                                    }, 
                                    Some(quote! { #arg_name: &#arg_type }))
                                }
                            })
                        }).collect::<Vec<_>>();
                        let (mut bind_vec, mut args_vec) = extend_fields.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
                        let mut args_vec = args_vec.into_iter().filter_map(|x| x).collect::<Vec<_>>();
                        fn_args.append(&mut args_vec);
                        binds.append(&mut bind_vec);
                        let start_counter = by_fields.len() + 1;
                        let (sql, params) = parser::replace_placeholder(&where_stmt_str, par_res.placeholder_vars, Some(start_counter as i32));
                        where_condition.push(sql);
                    
                    } else {
                        where_condition.push(where_stmt_str);
                    }
                    
                }
                let mut where_condition =  where_condition.join(" AND ");
                let sql = format!("DELETE FROM {} WHERE {}", &table_name, where_condition);
                super::check_valid_sql(&sql, db);
                let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
                let database = super::get_database_type(db);
                let generated = quote! {
                    pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#(#fn_args),* , conn: E) -> Result<u64, sqlx::Error> {
                        let sql = #sql;
                        #dbg_before
                        let query = sqlx::query(sql)
                            #(#binds)*
                            .execute(conn)
                            .await;
                        #dbg_after
                        Ok(query?.rows_affected())
                    }
                };
                functions.push(super::gen_with_doc(generated));
            } 
            
        }
    }

    let expanded = match scope {
        super::Scope::Struct => quote! {
            impl #struct_name {
                #(#functions)*
            }
        },
        super::Scope::Mod => quote! {
            #(#functions)*
        },
        super::Scope::NewMod => {
            let new_mod = super::create_ident(&table_name);
            quote! {
                pub mod #new_mod {
                    #(#functions)*
                }
            }
        },
    };

    Ok(expanded.into())
}
