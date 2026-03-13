use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, LitStr,
    Meta, MetaList, MetaNameValue, NestedMeta, Token,
};

pub fn derive(ast: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &ast.ident;
    let mut group_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut all_fields_str = vec![];
    let all_fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named.iter().collect::<Vec<_>>()
    } else {
        panic!("Columns macro only works with structs with named fields");
    };
    for field in &all_fields {
        let field_name = field.ident.as_ref().unwrap().to_string();
        // Check for #[column("name")] attribute
        let column_name = get_column_attribute_value(field).unwrap_or_else(|| field_name.clone());
        all_fields_str.push(column_name.clone());
        let attrs = &field.attrs;

        for attr in attrs {
            if attr.path.is_ident("group") {
                if let Ok(meta) = attr.parse_meta() {
                    if let Meta::NameValue(meta) = meta {
                        if let syn::Lit::Str(lit) = meta.lit {
                            let group_name = lit.value();
                            let entry = group_map.entry(group_name).or_default();
                            entry.push(column_name.clone());
                        }
                    }
                }
            }
        }
    }
    let all_str = all_fields_str.join(", ");
    let num_columns = all_fields_str.len();
    let column_literals = all_fields_str.iter().map(|c| quote! { #c }).collect::<Vec<_>>();
    let expanded = quote!{
        impl #struct_name {
            /// Returns all column names as a comma-separated string.
            pub const fn as_select_all_fields() -> &'static str {
                #all_str
            }
            
            /// Returns all column names as an array of string slices.
            pub const COLUMNS: [&'static str; #num_columns] = [#(#column_literals),*];
            
            /// Returns all column names as a comma-separated string (same as `as_select_all_fields`).
            pub const COLUMNS_STR: &'static str = #all_str;
        }
    };

    Ok(expanded.into())
}

/// Extract the custom column name from `#[column("name")]` attribute on a field.
fn get_column_attribute_value(field: &Field) -> Option<String> {
    for attr in &field.attrs {
        if attr.path.is_ident("column") {
            if let Ok(lit) = attr.parse_args::<syn::LitStr>() {
                return Some(lit.value());
            }
        }
    }
    None
}