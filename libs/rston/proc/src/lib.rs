use proc_macro::TokenStream;
use quote::quote;

mod bound;
mod derive_load;
mod derive_store;
mod internals;

#[derive(Copy, Clone, Eq, PartialEq)]
enum Derive {
    Debug,
}

/// Implements `Load` for the type.
#[proc_macro_derive(Load, attributes(bounds, tlb))]
pub fn derive_load(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_load::impl_derive(input)
        .unwrap_or_else(to_compile_errors)
        .into()
}

/// Implements `Store` for the type.
#[proc_macro_derive(Store, attributes(bounds, tlb))]
pub fn derive_store(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    derive_store::impl_derive(input)
        .unwrap_or_else(to_compile_errors)
        .into()
}

fn to_compile_errors(errors: Vec<syn::Error>) -> proc_macro2::TokenStream {
    let compile_errors = errors.iter().map(syn::Error::to_compile_error);
    quote!(#(#compile_errors)*)
}
