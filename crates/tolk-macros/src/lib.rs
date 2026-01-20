use crate::violation_metadata::violation_metadata;
use proc_macro::TokenStream;
use syn::{DeriveInput, Error, ItemFn, parse_macro_input};

mod map_codes;
mod rule_code_prefix;
mod violation_metadata;

#[proc_macro_derive(ViolationMetadata, attributes(violation_metadata))]
pub fn derive_violation_metadata(item: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(item);

    violation_metadata(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn map_codes(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let func: ItemFn = parse_macro_input!(item);

    map_codes::map_codes(&func)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
