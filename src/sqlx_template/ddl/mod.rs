use syn::DeriveInput;
use proc_macro2::TokenStream;

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