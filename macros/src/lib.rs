#![recursion_limit = "256"]
extern crate proc_macro;
use proc_macro::{TokenStream, Ident, Span};
use quote::quote;

#[proc_macro]
pub fn struct_from_tsv(_input: TokenStream) -> TokenStream {
    let mut input = proc_macro2::TokenStream::from(_input).into_iter();
    let mut name = input.next();

    proc_macro::TokenStream::from(
        quote! {
            #[derive(Debug, serde::Deserialize, Clone)]
            pub struct #name {
                #(pub #input : String),*
            }
        }
    )
}
