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
    for field in all_fields {
        let field_name = field.ident.as_ref().unwrap().to_string();
        all_fields_str.push(field_name.clone());
        let attrs = &field.attrs;

        for attr in attrs {
            if attr.path.is_ident("group") {
                if let Ok(meta) = attr.parse_meta() {
                    if let Meta::NameValue(meta) = meta {
                        if let syn::Lit::Str(lit) = meta.lit {
                            let group_name = lit.value();
                            let entry = group_map.entry(group_name).or_default();
                            entry.push(field_name.clone());
                        }
                    }
                }
            }
        }
    }
    let all_str = all_fields_str.join(", ");
    let return_ = "&' static ";
    let expanded = quote!{
        impl #struct_name {
            pub const fn as_select_all_fields() -> &'static str {
                #all_str
            }
        }
    };

    Ok(expanded.into())

}