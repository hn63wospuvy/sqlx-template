use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, LitStr,
    Meta, MetaList, MetaNameValue, NestedMeta, Token,
};

use super::get_table_name;

pub fn derive_insert(ast: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &ast.ident;
    let mut fields = vec![];
    let debug_slow = super::get_debug_slow_from_table_scope(&ast);
    if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named.iter().for_each(|f| {
            if !has_auto_attribute(f) {
                if let Some(ident) = f.ident.as_ref() {
                    fields.push(ident);
                }
            };
        })
    } else {
        panic!("InsertTemplate macro only works with structs with named fields");
    };

    if fields.is_empty() {
        panic!("Must have at least one field with no auto attribute");
    }

    let table_name = get_table_name(&ast);

    let sql_fields = fields
        .iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let sql_placeholders = (1..=fields.len())
        .map(|i| format!("${}", i))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO {}({}) VALUES ({})",
        table_name, sql_fields, sql_placeholders
    );

    let sql_return = format!(
        "INSERT INTO {}({}) VALUES ({}) RETURNING *",
        table_name, sql_fields, sql_placeholders
    );

    let binds = fields.iter().map(|field| {
        quote! {
            .bind(&re.#field)
        }
    });

    let binds_return = binds.clone();

    let database = super::get_database();
    let (dbg_before, dbg_after) = super::gen_debug_code(debug_slow);
    let insert = quote! {
        pub async fn insert<'c, E: sqlx::Executor<'c, Database = #database>>(re: &#struct_name, conn: E) -> Result<u64, sqlx::Error> {
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
    let insert = super::gen_with_doc(insert);

    #[cfg(feature = "postgres")]
    let insert_returning = {
        let insert_returning = quote! {
            pub async fn insert_return<'c, E: sqlx::Executor<'c, Database = #database>>(re: &#struct_name, conn: E) -> Result<#struct_name, sqlx::Error> {
                let sql = #sql_return;
                #dbg_before
                let res = sqlx::query_as::<_, #struct_name>(sql)
                    #(#binds_return)*
                    .fetch_one(conn)
                    .await;
                #dbg_after
                Ok(res?)
            }
        };
        super::gen_with_doc(insert_returning)
    };

    let insert_collection = if cfg!(feature = "postgres") {
        let sql_insert_slice = format!(
            "INSERT INTO {} ({}) SELECT * FROM UNNEST({})",
            table_name, sql_fields, sql_placeholders
        );

        let field_vars_insert_slice = fields.iter().map(|f| {
            let name = f;
            quote! { let mut #name = Vec::new(); }
        });

        let push_value_insert_slice = fields.iter().map(|f| {
            let name = f;
            quote! {
                for record in records.iter() {
                    #name.push(&record.#name);
                }
            }
        });

        let bind_params_insert_slice = fields.iter().map(|f| {
            let name = f;
            quote! { &#name }
        });

        let insert_collection = quote! {
            pub async fn insert_all<'c, E: sqlx::Executor<'c, Database = #database>>(records: &[#struct_name], conn: E) -> Result<u64, sqlx::Error> {

                if records.is_empty() {
                    return Ok(0);
                }

                let sql = #sql_insert_slice;

                #(#field_vars_insert_slice)*

                #(#push_value_insert_slice)*

                #dbg_before
                let result = sqlx::query(&sql)
                    #(.bind(#bind_params_insert_slice))*
                    .execute(conn)
                    .await;
                #dbg_after

                Ok(result?.rows_affected())

            }
        };
        super::gen_with_doc(insert_collection)
    } else if cfg!(any(feature = "mysql", feature = "sqlite")) {
        let sql_placeholders_question_mark = format!(
            "({})",
            (0..fields.len())
                .map(|_| "?".to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let field_idents = fields
            .iter()
            .map(|f| {
                let name = f;
                quote! { &record.#name }
            })
            .collect::<Vec<_>>();

        let insert_collection = quote! {
             pub async fn insert_all<'c, E: sqlx::Executor<'c, Database = #database >>(records: &[#struct_name], conn: E)-> Result<u64, sqlx::Error> {

                let sql = format!(
                    "INSERT INTO {} ({}) VALUES {}",
                    #table_name,
                    &#sql_fields,
                    &(0..records.len()).map(|_| #sql_placeholders_question_mark.clone()).collect::<Vec<_>>().join(", "));


                let mut query = sqlx::query(&sql);
                #dbg_before
                for record in records {
                    query = query #(.bind(#field_idents))*
                }
                let result = query.execute(conn).await;
                #dbg_after

                Ok(result?.rows_affected())
            }
        };

        super::gen_with_doc(insert_collection)
    } else {
        panic!("Only support insert_all for postgres, mysql, sqlite");
    };

    #[cfg(not(feature = "postgres"))]
    let insert_returning = quote! {};
    // #[cfg(not(feature = "postgres"))]
    // let insert_collection = quote! {};
    // #[cfg(not(feature = "mysql"))]
    // let insert_collection = quote! {};
    let gen = quote! {
        impl #struct_name {

            #insert

            #insert_returning

            #insert_collection
        }
    };

    Ok(gen.into())
}

fn has_auto_attribute(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("auto"))
}
