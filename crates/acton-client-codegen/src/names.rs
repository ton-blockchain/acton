use crate::{CodegenError, generate_error};
use heck::{ToShoutySnakeCase, ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, Span};

pub(crate) fn type_ident(name: &str) -> Result<Ident, CodegenError> {
    ident(&sanitize(name).to_upper_camel_case(), "type")
}

pub(crate) fn value_ident(name: &str) -> Result<Ident, CodegenError> {
    ident(&sanitize(name).to_snake_case(), "value")
}

pub(crate) fn const_ident(name: &str) -> Result<Ident, CodegenError> {
    ident(&sanitize(name).to_shouty_snake_case(), "constant")
}

pub(crate) fn union_ident(ty_idx: usize) -> Ident {
    Ident::new(&format!("UnionTy{ty_idx}"), Span::call_site())
}

pub(crate) fn stack_struct_ident(name: &str, ty_idx: usize) -> Result<Ident, CodegenError> {
    let name = sanitize(name).to_upper_camel_case();
    ident(&format!("{name}StackTy{ty_idx}"), "stack struct")
}

fn sanitize(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut last_was_separator = false;
    for character in name.chars() {
        if character.is_alphanumeric() || character == '_' {
            result.push(character);
            last_was_separator = false;
        } else if !last_was_separator {
            result.push('_');
            last_was_separator = true;
        }
    }
    result.trim_matches('_').to_owned()
}

fn ident(name: &str, kind: &str) -> Result<Ident, CodegenError> {
    if name.is_empty() {
        return Err(generate_error(format!("ABI {kind} name is empty")));
    }
    syn::parse_str::<Ident>(name)
        .or_else(|_| syn::parse_str::<Ident>(&format!("r#{name}")))
        .map_err(|_| generate_error(format!("ABI {kind} name `{name}` is invalid in Rust")))
}
