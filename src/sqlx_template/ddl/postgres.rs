use std::{default, fmt};

use crate::{sqlx_template::{
    check_fields, gen_with_doc, get_database, get_table_name, has_duplicates,
}, util};
use proc_macro2::TokenStream;
use quote::quote;
use sqlparser::dialect::PostgreSqlDialect;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, Lit, Meta, MetaList,
    NestedMeta, Type, TypePath,
};

use super::do_print_sql;

const DIALECT: PostgreSqlDialect = PostgreSqlDialect {};

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

    let all_fields = fields.named.iter().collect::<Vec<_>>();

    let mut primary_keys = Vec::new();
    let columns = fields
        .named
        .iter()
        .map(|field| {
            let name = &field.ident;
            let ty = &field.ty;
            let (mut column_type, nullable) = parse_type(ty);
            let mut default_value = None;

            //  #[column(type = "", default = "")]
            for attr in &field.attrs {
                if attr.path.is_ident("column") {
                    if let Ok(meta) = attr.parse_meta() {
                        if let Meta::List(meta_list) = meta {
                            for nested_meta in meta_list.nested {
                                // if let NestedMeta::Meta(Meta::NameValue(nv)) = nested_meta {
                                //     if nv.path.is_ident("type") {
                                //         if let Lit::Str(lit_str) = &nv.lit {
                                //             column_type = lit_str.value();
                                //         }
                                //     }
                                //     if nv.path.is_ident("default") {
                                //         if let Lit::Str(lit_str) = &nv.lit {
                                //             default_value = Some(lit_str.value());
                                //         }
                                //     }
                                //     if nv.path.is_ident("primary") {
                                //         primary_keys.push(name.clone().unwrap().to_string());
                                //     }
                                //     // TODO: support collation
                                // }
                                match &nested_meta {
                                    NestedMeta::Meta(Meta::NameValue(nv)) => {
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
                                    }
                                    NestedMeta::Meta(Meta::Path(path)) => {
                                        if path.is_ident("primary") {
                                            primary_keys.push(name.clone().unwrap().to_string());
                                        }
                                    }
                                    _ => {}
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

            format!(
                r#""{}" {}{}{}"#,
                name.clone().unwrap().to_string(),
                column_type,
                nullable_sql,
                default_sql
            )
        })
        .map(|x| x.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let indexes = gen_index_query(&input.attrs, &all_fields, table_name.as_str());
    let primary_key_def = if !primary_keys.is_empty() {
        format!(", PRIMARY KEY({})", primary_keys.join(", "))
    } else {
        String::new()
    };
    let create_sql = format!(
        "CREATE TABLE IF NOT EXISTS {} ({}{})",
        table_name, columns, primary_key_def
    );

    let create_sql = util::format_sql(&create_sql, &DIALECT).unwrap();

    let drop_sql = format!("DROP TABLE IF EXISTS {}", table_name,);

    let database = get_database();

    let index_creation = indexes
        .iter()
        .map(|query| {
            let print_stmt = do_print_sql(query);
            quote! {
                if print_query {
                    #print_stmt
                }
                let _ = sqlx::query(#query).execute(conn).await?;
            }
        })
        .collect::<Vec<_>>();

    let print_stmt = do_print_sql(&create_sql);
    let index_creation_for_recreate = index_creation.clone();
    let create_function_impl = gen_with_doc(quote! {
        pub async fn create_table<'c, E: sqlx::Executor<'c, Database = #database> + Copy>( conn: E, print_query: bool) -> Result<(), sqlx::Error> {
            if print_query {
                #print_stmt
            }
            let _ = sqlx::query(#create_sql)
                .execute(conn)
                .await?;
            #(#index_creation)*
            Ok(())
        }
    });

    let print_drop_stmt = do_print_sql(&drop_sql);
    let recreate_function_impl = gen_with_doc(quote! {
            pub async fn recreate_table<'c, E: sqlx::Executor<'c, Database = #database> + Copy>( conn: E, print_query: bool) -> Result<(), sqlx::Error> {
                if print_query {
                    #print_drop_stmt
                }   
                let _ = sqlx::query(#drop_sql)
                    .execute(conn)
                    .await?;

                if print_query {
                    #print_stmt
                }    
                let _ = sqlx::query(#create_sql)
                .execute(conn)
                .await?;
                #(#index_creation_for_recreate)*
                Ok(())
            }
    });

    let index_gen_function_impl = quote! {
        pub fn get_indexes_query() -> &'static [&'static str] {
            &[
                #(#indexes),*
            ]
        }
    };

    let expanded = quote! {
        impl #struct_name {

            pub const GEN_TABLE_SQL : &'static str = #create_sql;

            #create_function_impl
            #recreate_function_impl
            #index_gen_function_impl
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

#[derive(Debug, Default, PartialEq, Eq)]
enum IndexType {
    #[default]
    Btree,
    Hash,
    Gin,
    Gist,
    Brin,
    Bloom,
    SpGist,
}

impl fmt::Display for IndexType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            IndexType::Btree => write!(f, "BTREE"),
            IndexType::Hash => write!(f, "HASH"),
            IndexType::Gin => write!(f, "GIN"),
            IndexType::Gist => write!(f, "GiST"),
            IndexType::Brin => write!(f, "BRIN"),
            IndexType::Bloom => write!(f, "BLOOM"),
            IndexType::SpGist => write!(f, "SPGIST"),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
enum NullSort {
    #[default]
    NullFirst,
    NullLast,
}

#[derive(Debug, Default, PartialEq, Eq)]
enum NullUnique {
    #[default]
    NullDistinct,
    NullNotDistinct,
}

impl fmt::Display for NullSort {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            NullSort::NullFirst => write!(f, "NULLS FIRST"),
            NullSort::NullLast => write!(f, "NULLS LAST"),
        }
    }
}
impl fmt::Display for NullUnique {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            NullUnique::NullDistinct => write!(f, "NULLS DISTINCT"),
            NullUnique::NullNotDistinct => write!(f, "NULLS NOT DISTINCT"),
        }
    }
}

/// https://www.postgresql.org/docs/current/sql-createindex.html
fn gen_index_query(attrs: &[Attribute], all_fields: &[&Field], table_name: &str) -> Vec<String> {
    let mut res = vec![];
    let mut exist_index_names = vec![];
    for (index, attr) in attrs.iter().enumerate() {
        if let Ok(Meta::List(MetaList {
            ref path,
            ref nested,
            ..
        })) = attr.parse_meta()
        {
            let mut index_fields = Vec::new();
            let mut index_type = IndexType::default();
            let mut concurrently = false;
            let mut unique = false;
            let mut raw: Option<String> = None;
            let mut index_name: Option<String> = None;
            let mut with: Option<String> = None;
            let mut include: Vec<String> = vec![];
            let mut null_sort = None;
            let mut null_unique = None;
            let mut desc = false;
            if path.is_ident("index") {
                for meta in nested {
                    match meta {
                        NestedMeta::Meta(Meta::NameValue(nv)) => {
                            if nv.path.is_ident("fields") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    let mut fields = check_fields(&fields_str, all_fields.to_vec());
                                    fields.sort_by_key(|x| x.ident.clone());
                                    if has_duplicates(&fields) {
                                        panic!("#[index]: fields - Found duplicated fields: {:?}", fields_str);
                                    }
                                    if fields.len() != fields_str.len() {
                                        panic!(
                                            "#[index]: fields - One of those value is duplicated or not a field in struct: {:?}",
                                            fields_str
                                        );
                                    }
                                    index_fields = fields
                                        .into_iter()
                                        .map(|x| x.ident.map(|y| y.to_string()).unwrap_or_default())
                                        .collect();
                                } 
                            } else if nv.path.is_ident("name") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    index_name.replace(lit);
                                }
                            } else if nv.path.is_ident("with") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    with.replace(lit);
                                }
                            } else if nv.path.is_ident("include") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    let fields_str =
                                        lit.split(',').map(|x| x.trim()).collect::<Vec<_>>();
                                    let mut fields = check_fields(&fields_str, all_fields.to_vec());
                                    fields.sort_by_key(|x| x.ident.clone());
                                    if has_duplicates(&fields) {
                                        panic!("#[index]: include - Found duplicated fields: {:?}", fields_str);
                                    }
                                    if fields.len() != fields_str.len() {
                                        panic!(
                                            "#[index]: include - One of those value is duplicated or not a field in struct: {:?}",
                                            fields_str
                                        );
                                    }
                                    include = fields
                                        .into_iter()
                                        .map(|x| x.ident.map(|y| y.to_string()).unwrap_or_default())
                                        .collect();
                                }
                            } else if nv.path.is_ident("raw") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value();
                                    raw.replace(lit);
                                }
                            } else if nv.path.is_ident("null_sort") {
                                if let Lit::Str(lit) = &nv.lit {
                                    let lit = lit.value().to_uppercase();
                                    match lit.as_str() {
                                        "first" => null_sort.replace(NullSort::NullFirst),
                                        "last" => null_sort.replace(NullSort::NullLast),
                                        _ => panic!("Valid null_sort value: first | last"),
                                    };
                                }
                            } else if nv.path.is_ident("null_distinct") {
                                if let Lit::Bool(lit) = &nv.lit {
                                    if lit.value() {
                                        null_unique.replace(NullUnique::NullDistinct);
                                    } else {
                                        null_unique.replace(NullUnique::NullNotDistinct);
                                    }
                                }
                            } else if nv.path.is_ident("asc") {
                                if let Lit::Bool(lit) = &nv.lit {
                                    if !lit.value() {
                                        desc = true
                                    }
                                }
                            }
                        }
                        NestedMeta::Meta(Meta::Path(path)) => {
                            let ident_str = path
                                .get_ident()
                                .and_then(|x| Some(x.to_string().to_lowercase()));
                            let ident_str = ident_str.unwrap_or_default();
                            match ident_str.as_str() {
                                "concurrently" => concurrently = true,
                                "unique" => unique = true,
                                "hash" => index_type = IndexType::Hash,
                                "btree" => index_type = IndexType::Btree,
                                "bloom" => index_type = IndexType::Bloom,
                                "brin" => index_type = IndexType::Brin,
                                "gin" => index_type = IndexType::Gin,
                                "gist" => index_type = IndexType::Gist,
                                "spgist" => index_type = IndexType::SpGist,
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                if raw.is_none() && index_fields.is_empty() {
                    panic!("Either `fields` or `raw` must not be empty");
                }

                if unique && index_type != IndexType::Btree {
                    panic!("Unique attribute only for btree index");
                }
                let unique = if unique {
                    "UNIQUE"
                } else {
                    null_unique = None;
                    ""
                };
                let concurrently = if concurrently { "CONCURRENTLY" } else { "" };
                match raw {
                    Some(ref raw) => {
                        let index_name = match index_name.as_ref() {
                            Some(index_name) => index_name.to_string(),
                            None => format!("{table_name}_{index}_idx"),
                        };
                        if exist_index_names.contains(&index_name) {
                            panic!("Duplicate index name: {index_name}");
                        } else {
                            exist_index_names.push(index_name.clone());
                        }
                        let sql = format!("CREATE {unique} {concurrently} INDEX IF NOT EXISTS {index_name} ON {table_name} {raw}");
                        
                        let sql = util::format_sql(sql.as_str(), &DIALECT).unwrap();
                        res.push(sql);
                    }
                    None => {
                        let index_name = match index_name.as_ref() {
                            Some(index_name) => index_name.to_string(),
                            None => format!("{table_name}_{}_idx", index_fields.join("_")),
                        };
                        if exist_index_names.contains(&index_name) {
                            panic!("Duplicate index name: {index_name}");
                        } else {
                            exist_index_names.push(index_name.clone());
                        }
                        let with = match with.as_ref() {
                            Some(with) => format!("WITH ({with})"),
                            None => String::new(),
                        };
                        let include = match include.len() {
                            i if i > 0 => format!("INCLUDE ({})", include.join(", ")),
                            _ => String::new(),
                        };
                        let sort = if desc { "DESC" } else { "" };
                        let null_sort = match null_sort.as_ref() {
                            Some(value) => value.to_string(),
                            None => String::new(),
                        };

                        let null_unique = match null_unique.as_ref() {
                            Some(value) => value.to_string(),
                            None => String::new(),
                        };
                        let fields = index_fields.join(", ");
                        let sql = format!("CREATE {unique} {concurrently} INDEX IF NOT EXISTS {index_name} ON {table_name} USING {index_type} ({fields}) {sort} {null_sort} {include} {null_unique} {with} ");
                        let sql = util::format_sql(sql.as_str(), &DIALECT).unwrap();
                        res.push(sql);
                    }
                }
                
            }
        }
    }
    res
}
