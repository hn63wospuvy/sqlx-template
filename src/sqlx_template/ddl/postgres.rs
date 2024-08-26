use crate::sqlx_template::{gen_with_doc, get_database, get_table_name};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Fields, Lit, Meta, NestedMeta, Type, TypePath,
};

pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &input.ident;
    let table_name = get_table_name(&input);

    let fields = if let Data::Struct(data) = &input.data {
        if let Fields::Named(fields) = &data.fields {
            fields
        } else {
            panic!("Unnamed fields are not supported");
        }
    } else {
        panic!("DDLTemplate can only be derived for structs");
    };

    let columns = fields.named.iter().map(|field| {
        let name = &field.ident;
        let ty = &field.ty;
        let  (mut column_type, nullable) = parse_type(ty);
        let mut default_value = None;

        //  #[column(type = "", default = "")]
        for attr in &field.attrs {
            if attr.path.is_ident("column") {
                if let Ok(meta) = attr.parse_meta() {
                    if let Meta::List(meta_list) = meta {
                        for nested_meta in meta_list.nested {
                            if let NestedMeta::Meta(Meta::NameValue(nv)) = nested_meta {
                                if nv.path.is_ident("type") {
                                    if let Lit::Str(lit_str) = &nv.lit {
                                        column_type = lit_str.value();
                                    }
                                }
                                if nv.path.is_ident("default") {
                                    if let Lit::Str(lit_str) = &nv.lit {
                                        default_value = Some(lit_str.value());
                                    }
                                }
                                // TODO: support collation
                            }
                        }
                    }
                }
            }
        }

        let default_sql = if let Some(default) = default_value {
            format!(" DEFAULT {}", default)
        } else {
            String::new()
        };

        let nullable_sql = if nullable { "" } else { " NOT NULL" };

            format!("`{}` {}{}{}", name.clone().unwrap().to_string(), column_type, nullable_sql, default_sql)
    })
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(", ")
        ;

    let create_sql = format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        table_name,
        columns
    );

    let drop_sql = format!(
        "DROP TABLE IF EXISTS {}",
        table_name,
    );



    let database = get_database();
    
    let create_function_impl = gen_with_doc(quote! {
            pub async fn create_table<'c, E: sqlx::Executor<'c, Database = #database>>( conn: E) -> Result<(), sqlx::Error> {
                let _ = sqlx::query(#create_sql)
                    .execute(conn)
                    .await?;
                Ok(())
            }
    });



    let recreate_function_impl = gen_with_doc(quote! {
        pub async fn recreate_table<'c, E: sqlx::Executor<'c, Database = #database> + Copy>( conn: E) -> Result<(), sqlx::Error> {
            let _ = sqlx::query(#drop_sql)
                .execute(conn)
                .await?;

            let _ = sqlx::query(#create_sql)
            .execute(conn)
            .await?;
            Ok(())
        }
});

    let expanded = quote! {
        impl #struct_name {

            pub const GEN_TABLE_SQL : &str = #create_sql;

            #create_function_impl
            #recreate_function_impl
        }
    };

    Ok(TokenStream::from(expanded))
}

fn parse_type(ty: &Type) -> (String, bool) {
    match ty {
        Type::Path(TypePath { path, .. }) => {
            let ident = path.segments.last().unwrap().ident.to_string();
            match ident.as_str() {
                "bool" => ("bool".to_string(), false),
                "i16" => ("int2".to_string(), false),
                "i32" => ("int4".to_string(), false),
                "i64" => ("int8".to_string(), false),
                "f32" => ("float4".to_string(), false),
                "f64" => ("float8".to_string(), false),
                "Decimal" | "BigDecimal" => ("decimal".to_string(), false),
                "String" | "&str" => ("TEXT".to_string(), false),
                "Vec" => {
                    if let Some(inner_ty) = extract_inner_type(ty) {
                        let data_type = format!("{}[]", parse_type(inner_ty).0);
                        (data_type, false)
                    } else {
                        panic!("Unsupported Vec type");
                    }
                }
                "Option" => {
                    if let Some(inner_ty) = extract_inner_type(ty) {
                        let data_type = format!("{}", parse_type(inner_ty).0);
                        (data_type, true)
                    } else {
                        panic!("Unsupported Vec type");
                    }
                }
                "Json" => ("jsonb".to_string(), false),
                "OffsetDateTime" | "DateTime" => ("timestamptz".to_string(), false),
                "Instant" => ("timestamp".to_string(), false),
                _ => panic!("Unsupported type: {}", ident),
            }
        }
        _ => panic!("Unsupported type"),
    }
}

fn extract_option_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if path.segments.last().unwrap().ident == "Option" {
            extract_inner_type(ty)
        } else {
            None
        }
    } else {
        None
    }
}

fn extract_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(TypePath { path, .. }) = ty {
        if let Some(segment) = path.segments.last() {
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                    return Some(inner_ty);
                }
            }
        }
    }
    None
}
