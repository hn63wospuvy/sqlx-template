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

    #[cfg(not(feature = "postgres"))]
    let insert_returning = quote! {};

    let gen = quote! {
        impl #struct_name {

            #insert

            #insert_returning

        }
    };

    Ok(gen.into())
}

fn has_auto_attribute(field: &Field) -> bool {
    field.attrs.iter().any(|attr| attr.path.is_ident("auto"))
}
