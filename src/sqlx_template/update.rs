use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, GenericArgument,
    Ident, Lit, LitStr, Meta, MetaList, MetaNameValue, NestedMeta, PathArguments, Token, Type,
};

use crate::{
    parser,
    sqlx_template::{check_column_name, get_database_from_ast, get_field_name, get_field_name_as_column, Database},
};

use super::get_table_name;

pub fn derive_update(
    ast: &DeriveInput,
    for_path: Option<&syn::Path>,
    scope: super::Scope,
    db: Option<Database>,
) -> syn::Result<TokenStream> {
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
        panic!("UpdateTemplate macro only works with structs with named fields");
    };

    let all_columns_name = all_fields
        .iter()
        .map(|x| get_field_name_as_column(x, db))
        .collect::<Vec<_>>();
    let mut functions = Vec::new();
    for attr in &ast.attrs {
        if let Ok(Meta::List(MetaList {
            ref path,
            ref nested,
            ..
        })) = attr.parse_meta()
        {
            if path.is_ident("tp_update") {
                let mut by_fields = Vec::new();
                let mut on_fields = Vec::new();
                let mut version_fields = Vec::new();
                let mut fn_name_attr = None;
                let mut return_entity = None;
                let mut debug_slow = debug_slow.clone();
                let mut where_stmt_str = None;
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
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    let return_fields = super::check_fields(&fields_str, all_fields.clone());
                                    if super::has_duplicates(&return_fields) {
                                        panic!("Found duplicated fields: {:?}", fields_str);
                                    }
                                    if return_fields.len() != fields_str.len() {
                                        panic!(
                                            "One of those value is duplicated or not a field in struct: {:?}",
                                            fields_str
                                        );
                                    }
                                    return_entity.replace(return_fields);
                                } else if let Lit::Bool(lit) = &nv.lit {
                                    if lit.value() {
                                        return_entity.replace(vec![]);
                                    }
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

                by_fields.sort_by_key(|x| x.ident.clone());
                on_fields.sort_by_key(|x| x.ident.clone());

                if on_fields.is_empty() {
                    let func_name_by_field = by_fields
                        .iter()
                        .map(|x| get_field_name(x))
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
                        (format!("update_by_{func_name_by_field}_lock_on_{version_field_name}"), format!("update_by_{func_name_by_field}_lock_on_{version_field_name}_return", ))
                    } else {
                        (
                            format!("update_by_{}", func_name_by_field),
                            format!("update_by_{}_return", func_name_by_field),
                        )
                    };
                    let fn_name = Ident::new(&fn_name, proc_macro2::Span::call_site());
                    let fn_name_return =
                        Ident::new(&fn_name_return, proc_macro2::Span::call_site());
                    let fn_name_return_stream =
                        Ident::new(&format!("{fn_name_return}_stream"), proc_macro2::Span::call_site());
                    let mut fn_args = by_fields
                        .iter()
                        .map(|field| {
                            let arg_name = field.ident.as_ref().unwrap();
                            let arg_type = &field.ty;
                            if &arg_type.to_token_stream().to_string() == "String" {
                                quote! { #arg_name: &'c str }
                            } else {
                                quote! { #arg_name: &'c #arg_type }
                            }
                        })
                        .collect::<Vec<_>>();

                    let set_fields = all_fields
                        .iter()
                        .filter(|x| {
                            !super::contains(&by_fields, **x)
                                && !super::contains(&version_fields, **x)
                        })
                        .collect::<Vec<_>>();
                    if set_fields.is_empty() {
                        panic!("No set fields remains");
                    }
                    let mut set_stmt = set_fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            match db {
                                Database::Postgres => format!(
                                    "{} = ${}",
                                    get_field_name_as_column(field, db),
                                    index + 1
                                ),
                                Database::Sqlite | Database::Mysql | Database::Any => format!(
                                    "{} = ?",
                                    get_field_name_as_column(field, db)
                                ),
                            }
                        })
                        .collect::<Vec<_>>();
                    if version_fields.len() > 0 {
                        let version_field = version_fields.get(0).unwrap();
                        let arg_name = version_field.ident.as_ref().unwrap();
                        let set_version = format!("{arg_name} = {arg_name} + 1");
                        set_stmt.push(set_version);
                    }
                    let set_stmt = set_stmt.join(", ");
                    let current_idx = set_fields.len();
                    let mut where_stmt = by_fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            match db {
                                Database::Postgres => format!(
                                    "{} = ${}",
                                    get_field_name_as_column(field, db),
                                    index + current_idx + 1
                                ),
                                Database::Sqlite | Database::Mysql | Database::Any => format!(
                                    "{} = ?",
                                    get_field_name_as_column(field, db)
                                ),
                            }
                        })
                        .collect::<Vec<_>>();
                    if version_fields.len() > 0 {
                        let version_field = version_fields.get(0).unwrap();
                        let arg_name = get_field_name_as_column(version_field, db);
                        let stmt = match db {
                            Database::Postgres => format!("{arg_name} = ${}", by_fields.len() + current_idx + 1),
                            Database::Sqlite | Database::Mysql | Database::Any => format!("{arg_name} = ?"),
                        };
                        where_stmt.push(stmt);
                    }
                    let set_binds = set_fields.iter().map(|field| {
                        let field_name = field.ident.clone().unwrap();
                        quote! {
                            .bind(&re.#field_name)
                        }
                    });

                    let where_binds = by_fields.iter().map(|field| {
                        let field_name = field.ident.clone().unwrap();
                        quote! {
                            .bind(#field_name)
                        }
                    });

                    let version_binds = version_fields.iter().map(|field| {
                        let field_name = field.ident.clone().unwrap();
                        quote! {
                            .bind(&re.#field_name)
                        }
                    });

                    let mut binds = set_binds.collect::<Vec<_>>();
                    binds.append(&mut where_binds.collect::<Vec<_>>());
                    binds.append(&mut version_binds.collect::<Vec<_>>());
                    if let Some(where_stmt_str) = where_stmt_str {
                        let par_res = parser::get_columns_and_compound_ids(
                            &where_stmt_str,
                            super::get_database_dialect(db),
                        )
                        .unwrap();
                        for col in &par_res.columns {
                            let normalized_col = check_column_name(col.clone(), db);
                            if !all_columns_name.contains(&normalized_col) {
                                panic!("Invalid where statement: {col} column is not found in field list");
                            }
                        }
                        for table in &par_res.tables {
                            if table != &table_name {
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
                            let mut extend_fields = Vec::new();

                            // Process placeholders in order they appear in the SQL
                            for placeholder in &par_res.placeholder_vars {
                                let placeholder_name = &placeholder[1..]; // Remove ':' prefix

                                // Check if placeholder has format :name$Type - if so, always use Case 2
                                if placeholder_name.contains('$') {
                                    // Case 2: Placeholder with custom type format :name$Type
                                    if let Some(dollar_pos) = placeholder_name.find('$') {
                                        let var_name = &placeholder_name[..dollar_pos];
                                        let type_name = &placeholder_name[dollar_pos + 1..];

                                        let arg_name = syn::Ident::new(var_name, proc_macro2::Span::call_site());
                                        let arg_type: syn::Type = syn::parse_str(type_name)
                                            .unwrap_or_else(|_| panic!("Invalid type in placeholder {placeholder}: {type_name}"));

                                        let bind_expr = quote! { .bind(#arg_name) };
                                        let arg_expr = Some(quote! { #arg_name: &'c #arg_type });

                                        extend_fields.push((bind_expr, arg_expr));
                                    } else {
                                        panic!("Placeholder {placeholder} contains '$' but format is invalid");
                                    }
                                }
                                // Check if placeholder is mapped to a column
                                else if let Some(columns) = par_res.get_columns_for_placeholder(placeholder) {
                                    // Case 1: Placeholder is mapped to a column (and doesn't have $ format)
                                    if columns.len() == 1 {
                                        let column_name = columns.iter().next().unwrap();
                                        // Find the corresponding field in struct
                                        if let Some(field) = all_fields_map.get(column_name) {
                                            let arg_name = syn::Ident::new(placeholder_name, proc_macro2::Span::call_site());
                                            let arg_type = &field.ty;

                                            // Check if this field is already in by_fields (avoid duplication)
                                            if !by_fields_map.contains_key(column_name) {
                                                let bind_expr = quote! { .bind(#arg_name) };
                                                let arg_expr = if &arg_type.to_token_stream().to_string() == "String" {
                                                    Some(quote! { #arg_name: &'c str })
                                                } else {
                                                    Some(quote! { #arg_name: &'c #arg_type })
                                                };

                                                extend_fields.push((bind_expr, arg_expr));
                                            } else {
                                                // Field already in by_fields, just add binding
                                                let field_arg_name = field.ident.as_ref().unwrap();
                                                extend_fields.push((quote! { .bind(#field_arg_name) }, None));
                                            }
                                        } else {
                                            panic!("Column {column_name} mapped by placeholder {placeholder} is not found in struct fields");
                                        }
                                    } else {
                                        panic!("Placeholder {placeholder} is mapped to multiple columns: {:?}", columns);
                                    }
                                } else {
                                    // Case 3: Placeholder is not mapped to any column and doesn't have $ format
                                    panic!("Placeholder {placeholder} is not mapped to any column and doesn't have format :name$Type");
                                }
                            }
                            let (mut bind_vec, mut args_vec) =
                                extend_fields.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
                            let mut args_vec =
                                args_vec.into_iter().filter_map(|x| x).collect::<Vec<_>>();
                            fn_args.append(&mut args_vec);
                            binds.append(&mut bind_vec);
                            let start_counter = by_fields.len() + set_fields.len() + 1;
                            let (sql, params) = parser::replace_placeholder_with_db(
                                &where_stmt_str,
                                par_res.placeholder_vars,
                                Some(start_counter as i32),
                                db,
                            );
                            where_stmt.push(sql);
                        } else {
                            where_stmt.push(where_stmt_str);
                        }
                    }

                    if by_fields.is_empty() && where_stmt.is_empty() {
                        panic!("`by` fields or `where` attribute must not empty");
                    }

                    let where_stmt = where_stmt.join(" AND ");

                    let sql = format!(
                        "UPDATE {table_name} SET {set_stmt} WHERE {where_stmt}",
                    );
                    
                    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
                    let database = super::get_database_type(db);
                    let args_signature = if fn_args.is_empty() {
                        quote! {}
                    } else {
                        quote! {#(#fn_args),* ,}
                    };
                    let generated = if return_entity.is_some() && matches!(db, Database::Postgres) {
                        let binds_return = binds.clone();
                        let return_entity = return_entity.unwrap();
                        let (return_type, return_columns, query_func) = match return_entity.len() {
                            0 => (quote! {#struct_name}, "*".into(), quote! {query_as}),
                            1 => {
                                let field_type = return_entity[0].clone().ty;
                                (quote! {#field_type}, get_field_name_as_column(&return_entity[0], db), quote! {query_scalar})
                            }
                            _ => {
                                let field_types = return_entity.iter().map(|field| &field.ty);
                                let field_columns = return_entity.iter().map(|field| get_field_name_as_column(field, db)).collect::<Vec<_>>();
                                (quote! {(#(#field_types),*)}, field_columns.join(", "), quote! {query_as})
                            }
                        };
                        let sql_return = format!(
                            "UPDATE {table_name} SET {set_stmt} WHERE {where_stmt} RETURNING {return_columns}",
                        );
                        super::check_valid_single_sql(&sql_return, db);
                        quote! {
                            pub async fn #fn_name_return<'c, E: sqlx::Executor<'c, Database = #database>>(#args_signature re: &'c #struct_name, conn: E) -> core::result::Result<Vec<#return_type>, sqlx::Error> {
                                let sql = #sql_return;
                                #dbg_before
                                let query_result = sqlx::#query_func::<_, #return_type>(sql)
                                    #(#binds_return)*
                                    .fetch_all(conn)
                                    .await;
                                #dbg_after
                                Ok(query_result?)
                            }
                            pub async fn #fn_name_return_stream<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#args_signature re: &'c #struct_name, conn: E) -> futures::stream::BoxStream<'c, core::result::Result<#return_type, sqlx::Error>> {
                                let sql = #sql_return;
                                #dbg_before
                                let query_result = sqlx::#query_func::<_, #return_type>(sql)
                                    #(#binds_return)*
                                    .fetch(conn)
                                    ;
                                #dbg_after
                                query_result
                            }

                           
                        }

                    } else {
                        quote! {
                            pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#args_signature re: &#struct_name, conn: E) -> core::result::Result<u64, sqlx::Error> {
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
                } else {
                    let func_name_by_field = by_fields
                        .iter()
                        .map(|x| get_field_name(x))
                        .collect::<Vec<_>>()
                        .join("_and_");
                    let func_name_on_field = on_fields
                        .iter()
                        .map(|x| get_field_name(x))
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
                        (format!("update_by_{func_name_by_field}_on_{func_name_on_field}_lock_on_{version_field_name}"), format!("update_by_{func_name_by_field}_on_{func_name_on_field}_lock_on_{version_field_name}_return"))
                    } else {
                        (
                            format!("update_by_{}_on_{func_name_on_field}", func_name_by_field),
                            format!(
                                "update_by_{}_on_{func_name_on_field}_return",
                                func_name_by_field
                            ),
                        )
                    };
                    let fn_name = Ident::new(&fn_name, proc_macro2::Span::call_site());
                    let fn_name_return =
                        Ident::new(&fn_name_return, proc_macro2::Span::call_site());
                    let fn_name_return_stream =
                        Ident::new(&format!("{fn_name_return}_stream"), proc_macro2::Span::call_site());
                    let mut fn_args = by_fields
                        .iter()
                        .map(|field| {
                            let arg_name = field.ident.as_ref().unwrap();
                            let arg_type = &field.ty;
                            if &arg_type.to_token_stream().to_string() == "String" {
                                quote! { #arg_name: &'c str }
                            } else {
                                quote! { #arg_name: &'c #arg_type }
                            }
                            
                        })
                        .collect::<Vec<_>>();

                    on_fields.iter().for_each(|field| {
                        let arg_name = field.ident.as_ref().unwrap();
                        let arg_type = &field.ty;
                        let arg = if &arg_type.to_token_stream().to_string() == "String" {
                            quote! { #arg_name: &'c str }
                        } else {
                            quote! { #arg_name: &'c #arg_type }
                        };
                        fn_args.push(arg);
                    });

                    if version_fields.len() > 0 {
                        let version_field = version_fields.get(0).unwrap();
                        let arg_name = version_field.ident.as_ref().unwrap();
                        let arg_type = &version_field.ty;
                        let ts = if &arg_type.to_token_stream().to_string() == "String" {
                            quote! { #arg_name: &'c str }
                        } else {
                            quote! { #arg_name: &'c #arg_type }
                        };
                        fn_args.push(ts);
                    }

                    let mut set_stmt = on_fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            format!(
                                "{} = ${}",
                                get_field_name_as_column(field, db),
                                index + 1
                            )
                        })
                        .collect::<Vec<_>>();
                    if version_fields.len() > 0 {
                        let version_field = version_fields.get(0).unwrap();
                        let arg_name = version_field.ident.as_ref().unwrap();
                        let set_version = format!("{arg_name} = {arg_name} + 1");
                        set_stmt.push(set_version);
                    }
                    let set_stmt = set_stmt.join(", ");
                    let current_idx = on_fields.len();
                    let mut where_stmt = by_fields
                        .iter()
                        .enumerate()
                        .map(|(index, field)| {
                            format!(
                                "{} = ${}",
                                get_field_name_as_column(field, db),
                                index + current_idx + 1
                            )
                        })
                        .collect::<Vec<_>>();
                    if version_fields.len() > 0 {
                        let version_field = version_fields.get(0).unwrap();
                        let arg_name = get_field_name_as_column(version_field, db);
                        let stmt = format!("{arg_name} = ${}", by_fields.len() + current_idx + 1);
                        where_stmt.push(stmt);
                    }
                    


                    
                    let set_binds = on_fields.iter().map(|field| {
                        let field_name = field.ident.clone().unwrap();
                        quote! {
                            .bind(#field_name)
                        }
                    });

                    let where_binds = by_fields.iter().map(|field| {
                        let field_name = field.ident.clone().unwrap();
                        quote! {
                            .bind(#field_name)
                        }
                    });
                    let version_binds = version_fields.iter().map(|field| {
                        let field_name = field.ident.clone().unwrap();
                        quote! {
                            .bind(#field_name)
                        }
                    });

                    let mut binds = set_binds.collect::<Vec<_>>();
                    binds.append(&mut where_binds.collect::<Vec<_>>());
                    binds.append(&mut version_binds.collect::<Vec<_>>());

                    if let Some(where_stmt_str) = where_stmt_str {
                        let par_res = parser::get_columns_and_compound_ids(
                            &where_stmt_str,
                            super::get_database_dialect(db),
                        )
                        .unwrap();
                        for col in &par_res.columns {
                            let normalized_col = check_column_name(col.clone(), db);
                            if !all_columns_name.contains(&normalized_col) {
                                panic!("Invalid where statement: {col} column is not found in field list");
                            }
                        }
                        for table in &par_res.tables {
                            if table != &table_name {
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
                            let mut extend_fields = Vec::new();

                            // Process placeholders in order they appear in the SQL
                            for placeholder in &par_res.placeholder_vars {
                                let placeholder_name = &placeholder[1..]; // Remove ':' prefix

                                // Check if placeholder has format :name$Type - if so, always use Case 2
                                if placeholder_name.contains('$') {
                                    // Case 2: Placeholder with custom type format :name$Type
                                    if let Some(dollar_pos) = placeholder_name.find('$') {
                                        let var_name = &placeholder_name[..dollar_pos];
                                        let type_name = &placeholder_name[dollar_pos + 1..];

                                        let arg_name = syn::Ident::new(var_name, proc_macro2::Span::call_site());
                                        let arg_type: syn::Type = syn::parse_str(type_name)
                                            .unwrap_or_else(|_| panic!("Invalid type in placeholder {placeholder}: {type_name}"));

                                        let bind_expr = quote! { .bind(#arg_name) };
                                        let arg_expr = Some(quote! { #arg_name: &'c #arg_type });

                                        extend_fields.push((bind_expr, arg_expr));
                                    } else {
                                        panic!("Placeholder {placeholder} contains '$' but format is invalid");
                                    }
                                }
                                // Check if placeholder is mapped to a column
                                else if let Some(columns) = par_res.get_columns_for_placeholder(placeholder) {
                                    // Case 1: Placeholder is mapped to a column (and doesn't have $ format)
                                    if columns.len() == 1 {
                                        let column_name = columns.iter().next().unwrap();
                                        // Find the corresponding field in struct
                                        if let Some(field) = all_fields_map.get(column_name) {
                                            let arg_name = syn::Ident::new(placeholder_name, proc_macro2::Span::call_site());
                                            let arg_type = &field.ty;

                                            // Check if this field is already in by_fields (avoid duplication)
                                            if !by_fields_map.contains_key(column_name) {
                                                let bind_expr = quote! { .bind(#arg_name) };
                                                let arg_expr = if &arg_type.to_token_stream().to_string() == "String" {
                                                    Some(quote! { #arg_name: &'c str })
                                                } else {
                                                    Some(quote! { #arg_name: &'c #arg_type })
                                                };

                                                extend_fields.push((bind_expr, arg_expr));
                                            } else {
                                                // Field already in by_fields, just add binding
                                                let field_arg_name = field.ident.as_ref().unwrap();
                                                extend_fields.push((quote! { .bind(#field_arg_name) }, None));
                                            }
                                        } else {
                                            panic!("Column {column_name} mapped by placeholder {placeholder} is not found in struct fields");
                                        }
                                    } else {
                                        panic!("Placeholder {placeholder} is mapped to multiple columns: {:?}", columns);
                                    }
                                } else {
                                    // Case 3: Placeholder is not mapped to any column and doesn't have $ format
                                    panic!("Placeholder {placeholder} is not mapped to any column and doesn't have format :name$Type");
                                }
                            }
                            let (mut bind_vec, mut args_vec) =
                                extend_fields.into_iter().unzip::<_, _, Vec<_>, Vec<_>>();
                            let mut args_vec =
                                args_vec.into_iter().filter_map(|x| x).collect::<Vec<_>>();
                            fn_args.append(&mut args_vec);
                            binds.append(&mut bind_vec);
                            let start_counter = by_fields.len() + on_fields.len() + 1;
                            let (sql, params) = parser::replace_placeholder_with_db(
                                &where_stmt_str,
                                par_res.placeholder_vars,
                                Some(start_counter as i32),
                                db,
                            );
                            where_stmt.push(sql);
                        } else {
                            where_stmt.push(where_stmt_str);
                        }
                    }
                    if by_fields.is_empty() && where_stmt.is_empty() {
                        panic!("`by` fields or `where` attribute must not empty");
                    }
                    let where_stmt = where_stmt.join(" AND ");
                    let sql = format!(
                        "UPDATE {table_name} SET {set_stmt} WHERE {where_stmt}",
                    );
                    super::check_valid_single_sql(&sql, db);

                    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
                    let database = super::get_database_type(db);
                    let args_signature = if fn_args.is_empty() {
                        quote! {}
                    } else {
                        quote! {#(#fn_args),* ,}
                    };
                    let generated = if return_entity.is_some() && matches!(db, Database::Postgres) {

                        let binds_return = binds.clone();
                        let return_entity = return_entity.unwrap();
                        let (return_type, return_columns, query_func) = match return_entity.len() {
                            0 => (quote! {#struct_name}, "*".into(), quote! {query_as}),
                            1 => {
                                let field_type = return_entity[0].clone().ty;
                                (quote! {#field_type}, get_field_name_as_column(&return_entity[0], db), quote! {query_scalar})
                            }
                            _ => {
                                let field_types = return_entity.iter().map(|field| &field.ty);
                                let field_columns = return_entity.iter().map(|field| get_field_name_as_column(field, db)).collect::<Vec<_>>();
                                (quote! {(#(#field_types),*)}, field_columns.join(", "), quote! {query_as})
                            }
                        };
                        let sql_return = format!(
                            "UPDATE {table_name} SET {set_stmt} WHERE {where_stmt} RETURNING {return_columns}",
                        );
                        super::check_valid_single_sql(&sql_return, db);
                        quote! {
                            pub async fn #fn_name_return<'c, E: sqlx::Executor<'c, Database = #database>>(#args_signature conn: E) -> core::result::Result<Vec<#return_type>, sqlx::Error> {
                                let sql = #sql_return;
                                #dbg_before
                                let query_result = sqlx::#query_func::<_, #return_type>(sql)
                                    #(#binds_return)*
                                    .fetch_all(conn)
                                    .await;
                                #dbg_after
                                Ok(query_result?)
                            }
                            pub async fn #fn_name_return_stream<'c, E: sqlx::Executor<'c, Database = #database> + 'c>(#args_signature conn: E) -> futures::stream::BoxStream<'c, core::result::Result<#return_type, sqlx::Error>> {
                                let sql = #sql_return;
                                #dbg_before
                                let query_result = sqlx::#query_func::<_, #return_type>(sql)
                                    #(#binds_return)*
                                    .fetch(conn)
                                    ;
                                #dbg_after
                                query_result
                            }

                           
                        }

                    } else {
                        quote! {
                            pub async fn #fn_name<'c, E: sqlx::Executor<'c, Database = #database>>(#args_signature conn: E) -> core::result::Result<u64, sqlx::Error> {
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
    }

    // Check for tp_update_builder attribute and generate builder if present
    let builder_code = if super::has_attribute(ast, "tp_update_builder") {
        let config = super::builder::BuilderConfig::from_update_attributes(ast, db)?;
        Some(super::builder::macro_impl::impl_update_builder(ast, &config))
    } else {
        None
    };

    let expanded = match scope {
        super::Scope::Struct => quote! {
            impl #struct_name {
                #(#functions)*
            }
            #builder_code
        },
        super::Scope::Mod => quote! {
            #(#functions)*
            #builder_code
        },
        super::Scope::NewMod => {
            let new_mod = super::create_ident(&table_name);
            quote! {
                pub mod #new_mod {
                    #(#functions)*
                    #builder_code
                }
            }
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
