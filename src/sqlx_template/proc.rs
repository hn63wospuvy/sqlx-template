use std::collections::HashSet;
use quote::{quote, ToTokens};
use proc_macro2::TokenStream;
use syn::{AttributeArgs, DeriveInput, Lit, Meta, NestedMeta, Path};

use crate::sqlx_template::Scope;

pub fn proc_gen(input: DeriveInput, nested_metas: Vec<NestedMeta>) -> syn::Result<TokenStream> {
    let mut for_path = None;
    let mut scope_value = None;
    for arg in &nested_metas {
        if let NestedMeta::Meta(Meta::NameValue(nv)) = arg {
            if nv.path.is_ident("for") {
                if for_path.is_some() {
                    panic!("Duplicate `for` attribute in tp_gen macro")
                }
                if let Lit::Str(lit_str) = &nv.lit {
                    for_path = Some(lit_str.parse::<Path>().expect("Expect a valid path"));
                }
            } else if nv.path.is_ident("scope") {
                if let Lit::Str(lit_str) = &nv.lit {
                    if scope_value.is_some() {
                        panic!("Duplicate `scope` attribute in tp_gen macro")
                    }
                    let s = lit_str.value();
                    scope_value = match s.as_str() {
                        "struct" => Some(Scope::Struct),
                        "mod" => Some(Scope::Mod),
                        "newmod" => Some(Scope::NewMod),
                        _ => None,
                    }
                }
            }
        }
    }

    let mut derives_set = HashSet::new();
    for attr in &input.attrs {
        if attr.path.is_ident("derive") {
            // attr.tokens: ví dụ (InsertTemplate, UpdateTemplate, ...)
            // parse tokens trong derive:
            if let Ok(meta) = attr.parse_meta() {
                if let Meta::List(mlist) = meta {
                    for nested in mlist.nested {
                        if let NestedMeta::Meta(Meta::Path(path)) = nested {
                            if let Some(ident) = path.get_ident() {
                                derives_set.insert(ident.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let struct_name = &input.ident;
    let table_name = super::get_table_name(&input);

    let mut functions = vec![];
    if derives_set.contains("SqlxTemplate") {
        functions.push(super::derive_all(&input, for_path.as_ref(), Scope::Mod, None)?);
    } else {
        if derives_set.contains("InsertTemplate") {
            functions.push(super::insert::derive_insert(&input, for_path.as_ref(), Scope::Mod, None)?);
        }
        if derives_set.contains("UpdateTemplate") {
            functions.push(super::update::derive_update(&input, for_path.as_ref(), Scope::Mod, None)?);
        }
        if derives_set.contains("UpsertTemplate") {
            functions.push(super::upsert::derive_upsert(&input, for_path.as_ref(), Scope::Mod, None)?);
        }
        if derives_set.contains("SelectTemplate") {
            functions.push(super::select::derive_select(&input, for_path.as_ref(), Scope::Mod, None)?);
        }
        if derives_set.contains("DeleteTemplate") {
            functions.push(super::delete::derive_delete(&input, for_path.as_ref(), Scope::Mod, None)?);
        }
    }


    let expanded = match scope_value {
        Some(Scope::Mod) => {
            quote! {
                #(#functions)*
            }
        },
        Some(Scope::NewMod) => {
            let new_mod = super::create_ident(&table_name);
            quote! {
                pub mod #new_mod {
                    #(#functions)*
                }
            }
        },
        None | Some(Scope::Struct) => {
            quote! {
                impl #struct_name {
                    #(#functions)*
                }
            }
        },
    };
    Ok(expanded.into())

}