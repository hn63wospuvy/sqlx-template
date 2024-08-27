use syn::DeriveInput;
use proc_macro2::TokenStream;
use quote::quote;

mod postgres;
pub fn derive(input: DeriveInput) -> syn::Result<TokenStream> {
    if cfg!(feature = "postgres") {
        postgres::derive(input)
    } else if cfg!(feature = "sqlite") {
        panic!("Database is not supported")
        // postgres::derive(input)
    } else if cfg!(feature = "mysql") {
        panic!("Database is not supported")
    } else if cfg!(feature = "any") {
        panic!("Database is not supported")
    } else {
        panic!("Unknown database")
    }
}

fn do_print_sql(sql: &str) -> TokenStream {
    let message = format!("[SQLxTemplate] - DDL: {sql} ");
    if cfg!(feature = "log") {
        (
            quote! { 
                log::debug!(#message); 
            }
        )
        
    } else if cfg!(feature = "tracing") {
        (
            quote! { 
                tracing::debug!(#message);
            }
        )
    } else {
        (
            quote! { 
                println!(#message);
            }
        )
    }
}