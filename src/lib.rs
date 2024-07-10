#![allow(warnings)]
use proc_macro::TokenStream;
use quote::quote;
use sqlx_template::raw;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Field, Fields, Ident, Lit, Meta,
    MetaNameValue, NestedMeta,
};

mod sqlx_template;
mod columns;
mod parser;

#[proc_macro_derive(InsertTemplate, attributes(table_name, auto, debug_slow))]
pub fn insert_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::insert::derive_insert(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_derive(UpdateTemplate, attributes(table_name, tp_update, debug_slow))]
pub fn update_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::update::derive_update(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}


#[proc_macro_derive(DeleteTemplate, attributes(table_name, tp_delete, debug_slow))]
pub fn delete_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::delete::derive_delete(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}
#[proc_macro_derive(QueryTemplate, attributes(table_name, debug_slow, tp_query_all, tp_query_one, tp_query_page, tp_query_stream, tp_query_count))]
pub fn query_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::query::derive_query(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_derive(Columns)]
pub fn columns_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match columns::derive(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_derive(TableName, attributes(table_name))]
pub fn table_name_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    match sqlx_template::table_name_derive(input) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_attribute]
pub fn multi_query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::multi_query_derive(input, args, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_attribute]
pub fn query(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, None) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_attribute]
pub fn select(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Select)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_attribute]
pub fn update(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Update)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_attribute]
pub fn insert(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Insert)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}

#[proc_macro_attribute]
pub fn delete(args: TokenStream, item: TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);
    match raw::query_derive(input, args, Some(parser::Mode::Delete)) {
        Ok(ok) => ok,
        Err(err) => err.to_compile_error().into(),
    }
    .into()
}