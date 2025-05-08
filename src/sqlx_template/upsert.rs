use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, GenericArgument,
    Ident, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, PathArguments, Token, Type,
};

use crate::sqlx_template::get_field_name;

use super::get_table_name;

pub fn derive(ast: DeriveInput) -> syn::Result<TokenStream> {
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
        panic!("UpdateTemplate macro only works with structs with named fields");
    };

    let mut functions = Vec::new();
    for attr in ast.attrs {
        if let Ok(Meta::List(MetaList {
            ref path,
            ref nested,
            ..
        })) = attr.parse_meta()
        {
            if path.is_ident("tp_upsert") {
                let mut by_fields = Vec::new();
                let mut on_fields = Vec::new();
                let mut version_fields = Vec::new();
                let mut fn_name_attr = None;
                let mut return_entity = false;
                let mut do_nothing = false;
                let mut debug_slow = debug_slow.clone();

                let mut insert_fields = vec![];
                if let syn::Data::Struct(syn::DataStruct {
                    fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
                    ..
                }) = ast.data
                {
                    named.iter().for_each(|f| {
                        if !has_auto_attribute(f) {
                            if let Some(ident) = f.ident.as_ref() {
                                insert_fields.push(ident);
                            }
                        };
                    })
                } else {
                    panic!("InsertTemplate macro only works with structs with named fields");
                };

                if insert_fields.is_empty() {
                    panic!("Must have at least one field with no auto attribute");
                }
                for meta in nested {
                    match meta {
                        NestedMeta::Meta(Meta::NameValue(nv)) => {
                            if nv.path.is_ident("by") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    by_fields =
                                        super::check_fields(&fields_str, all_fields.clone());
                                    if has_duplicates(&by_fields) {
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
                            } else if nv.path.is_ident("on") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    on_fields =
                                        super::check_fields(&fields_str, all_fields.clone());
                                    if has_duplicates(&on_fields) {
                                        panic!("Found duplicated fields: {:?}", fields_str);
                                    }
                                    if on_fields.len() != fields_str.len() {
                                        panic!(
                                            "One of those value is duplicated or not a field in the struct: {:?}",
                                            fields_str
                                        );
                                    }
                                } else {
                                    panic!("Expected string value order = \"...\"");
                                }
                            } else if nv.path.is_ident("op_lock") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    version_fields =
                                        super::check_fields(&fields_str, all_fields.clone());
                                    if version_fields.len() != 1 {
                                        panic!("Expected exactly one 'version' field");
                                    }
                                } else {
                                    panic!("Expected string value order = \"...\"");
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
                            } else if nv.path.is_ident("do_nothing") {
                                if let Lit::Bool(lit) = &nv.lit {
                                    let lit = lit.value();
                                    do_nothing = lit;
                                }
                            } else if nv.path.is_ident("debug") {
                                if let Lit::Int(lit) = &nv.lit {
                                    let slow_in_ms = lit
                                        .base10_parse()
                                        .expect("Invalid debug value. Must be integer");
                                    debug_slow.replace(slow_in_ms);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(version_field) = version_fields.get(0) {
                    if super::contains(&on_fields, version_field)
                        || super::contains(&by_fields, version_field)
                    {
                        panic!(
                            "Version field {:?} must not be in 'on'  fields or 'by' fields",
                            version_field
                                .ident
                                .as_ref()
                                .map(|x| x.to_string())
                                .unwrap_or_default()
                        );
                    }

                    if !super::is_integer_type(&version_field.ty) {
                        panic!("'version' field must be signed number type. Eg i8, i16, i32, i64");
                    }
                }

                let intersection = by_fields
                    .iter()
                    .filter(|&item| super::contains(&on_fields, &item))
                    .collect::<Vec<_>>();
                if !intersection.is_empty() {
                    let intersection = intersection
                        .iter()
                        .map(|x| get_field_name(*x))
                        .collect::<Vec<_>>();
                    panic!(
                        "Fields: {:?} must be either on 'by' fields or 'on' fields",
                        intersection
                    );
                }
                if by_fields.is_empty() {
                    panic!("'by' fields must not be empty");
                }
                by_fields.sort_by_key(|x| x.ident.clone());
                on_fields.sort_by_key(|x| x.ident.clone());

                let func_name_by_field = by_fields
                    .iter()
                    .map(|x| x.ident.clone().unwrap().to_string())
                    .collect::<Vec<_>>()
                    .join("_and_");
                let (fn_name, fn_name_return) = if let Some(fn_name) = fn_name_attr {
                    if fn_name.len() == 0 {
                        panic!("fn_name must not be empty");
                    } else {
                        (fn_name.clone(), fn_name.clone())
                    }
                } else if version_fields.len() > 0 {
                    let version_field = version_fields.get(0).unwrap();
                    let version_field_name = version_field.ident.clone().unwrap().to_string();
                    (
                        format!("upsert_by_{func_name_by_field}_lock_on_{version_field_name}"),
                        format!(
                            "upsert_by_{func_name_by_field}_lock_on_{version_field_name}_return",
                        ),
                    )
                } else {
                    (
                        format!("upsert_by_{}", func_name_by_field),
                        format!("upsert_by_{}_return", func_name_by_field),
                    )
                };
                let fn_name = Ident::new(&fn_name, proc_macro2::Span::call_site());
                let fn_name_return = Ident::new(&fn_name_return, proc_macro2::Span::call_site());
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

                let insert_field_stmt = insert_fields
                    .iter()
                    .map(|f| f.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                let insert_placeholders = (1..=insert_fields.len())
                    .map(|i| format!("${}", i))
                    .collect::<Vec<_>>()
                    .join(", ");

                let conflict_field_stmt = by_fields
                    .iter()
                    .map(|f| get_field_name(f))
                    .collect::<Vec<_>>()
                    .join(", ");
                let set_fields = all_fields
                    .iter()
                    .filter(|x| {
                        !super::contains(&by_fields, x) && !super::contains(&version_fields, x)
                    })
                    .collect::<Vec<_>>();
                if set_fields.is_empty() {
                    panic!("No set fields remains");
                }

                let do_update_stmt = if do_nothing {
                    format!("DO NOTHING")
                } else if on_fields.is_empty() {
                    let mut set_stmt = insert_fields
                        .iter()
                        .filter(|x| !version_fields
                            .iter()
                            .any(|y| 
                                get_field_name(y) == x.to_string()
                            )
                        )
                        .map(|x| format!(" {x} = EXCLUDED.{x}"))
                        .collect::<Vec<_>>()
                        ;
                    if !version_fields.is_empty() {
                        let version_set_stmt = version_fields.iter().map(|x| {
                            let x = get_field_name(x);
                            format!(" {x} = {table_name}.{x} + 1")
                        });
                        set_stmt = set_stmt.into_iter().chain(version_set_stmt).collect();
                    }
                    let set_stmt = set_stmt.join(", ");
                    format!(" DO UPDATE SET {set_stmt}")
                } else {
                    let mut set_stmt = on_fields
                        .iter()
                        .filter(|x| !version_fields
                            .iter()
                            .any(|y| 
                                get_field_name(y) == get_field_name(x)
                            )
                        )
                        .map(|x| {
                            let x = get_field_name(x);
                            format!(" {x} = EXCLUDED.{x}")
                        })
                        .collect::<Vec<_>>()
                        ;
                    if !version_fields.is_empty() {
                        let version_set_stmt = version_fields.iter().map(|x| {
                            let x = get_field_name(x);
                            format!(" {x} = {table_name}.{x} + 1")
                        });
                        set_stmt = set_stmt.into_iter().chain(version_set_stmt).collect();
                    }
                    let set_stmt = set_stmt.join(", ");
                    format!(" DO UPDATE SET {set_stmt}")
                };
                let mut set_stmt = set_fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        format!(
                            "{} = ${}",
                            field.ident.clone().unwrap().to_string(),
                            index + 1
                        )
                    })
                    .collect::<Vec<_>>();
                if version_fields.len() > 0 {
                    let version_field = version_fields.get(0).unwrap();
                    let arg_name = version_field.ident.as_ref().unwrap();
                    let set_version = format!("{arg_name} = {table_name}.{arg_name} + 1");
                    set_stmt.push(set_version);
                }
                let set_stmt = set_stmt.join(", ");
                let current_idx = set_fields.len();

                let sql = format!(
                    "INSERT INTO {table_name} ({insert_field_stmt}) VALUES ({insert_placeholders}) ON CONFLICT ({conflict_field_stmt}) {do_update_stmt}",
                );
                let sql_return = format!(
                    "INSERT INTO {table_name} ({insert_field_stmt}) VALUES ({insert_placeholders}) ON CONFLICT ({conflict_field_stmt}) {do_update_stmt} RETURNING *",
                );
                let binds = insert_fields.iter().map(|field| {
                    quote! {
                        .bind(&re.#field)
                    }
                });

                let binds_return = binds.clone();
                let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
                let database = super::get_database();
                let generated = if return_entity && cfg!(feature = "postgres") {
                    quote! {
                        pub async fn #fn_name_return<'c, E: sqlx::Executor<'c, Database = #database>>(#(#fn_args),* , re: &#struct_name, conn: E) -> core::result::Result<#struct_name, sqlx::Error> {
                            let sql = #sql_return;
                            #dbg_before
                            let res = sqlx::query_as::<_, #struct_name>(sql)
                                #(#binds_return)*
                                .fetch_one(conn)
                                .await;
                            #dbg_after
                            Ok(res?)
                        }
                    }
                } else {
                    quote! {
                        pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#(#fn_args),* , re: &#struct_name, conn: E) -> core::result::Result<u64, sqlx::Error> {
                            let sql = #sql;
                            #dbg_before
                            let query = sqlx::query(sql)
                                #(#binds)*
                                .execute(conn)
                                .await;
                            #dbg_after
                            Ok(query?.rows_affected())
                        }
                    }
                };
                functions.push(super::gen_with_doc(generated));
            }
        }
    }
    let expanded = quote! {
        impl #struct_name {
            #(#functions)*
        }
    };

    Ok(expanded.into())
}

fn has_duplicates(vec: &Vec<Field>) -> bool {
    let vec = vec.iter().map(|x| get_field_name(x)).collect::<Vec<_>>();
    for (i, item1) in vec.iter().enumerate() {
        for (j, item2) in vec.iter().enumerate() {
            if i != j && item1 == item2 {
                return true;
            }
        }
    }
    false
}

fn has_version_attribute(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("version"))
}

fn has_auto_attribute(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("auto"))
}
