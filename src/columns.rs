use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, token::Eq, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, LitStr,
    Meta, MetaList, MetaNameValue, NestedMeta, Token,
};

pub fn derive(ast: DeriveInput) -> syn::Result<TokenStream> {
    let struct_name = &ast.ident;
    let all_fields = if let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(syn::FieldsNamed { ref named, .. }),
        ..
    }) = ast.data
    {
        named.iter().collect::<Vec<_>>()
    } else {
        panic!("Columns macro only works with structs with named fields");
    };

    let all_fields_str = all_fields.iter().filter_map(|x| x.ident.clone().and_then(|y| Some(y.to_string()))).collect::<Vec<String>>();
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