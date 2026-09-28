use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

use crate::internals::{ast, attr, ctxt};
use crate::{Derive, bound};

pub fn impl_derive(input: syn::DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
    let cx = ctxt::Ctxt::new();
    let container = match ast::Container::from_ast(&cx, &input, Derive::Debug) {
        Some(container) => container,
        None => return Err(cx.check().unwrap_err()),
    };
    cx.check()?;

    let tlb_lifetime: syn::LifetimeParam = syn::parse_quote!('tlb);

    let ident = &container.ident;
    let generics = bound::without_default(container.generics);
    let (_, ty_generics, where_clause) = generics.split_for_impl();

    let mut alt_generics = generics.clone();

    let mut has_tlb_lifetime = false;
    for param in alt_generics.params.iter() {
        if let syn::GenericParam::Lifetime(def) = param {
            has_tlb_lifetime |= def.lifetime == tlb_lifetime.lifetime;
        }
    }

    if !has_tlb_lifetime {
        alt_generics
            .params
            .push(syn::GenericParam::Lifetime(tlb_lifetime.clone()));
    }
    let (impl_generics, _, _) = alt_generics.split_for_impl();

    let (inline, body) = match &container.data {
        ast::Data::Enum(variants) => (variants.len() < 2, build_enum(variants)),
        ast::Data::Struct(style, fields) => {
            let inline = fields.len() < 2;
            let body = build_struct(&container, &tlb_lifetime, *style, fields);
            (inline, body)
        }
    };

    let inline = if inline { quote!(#[inline]) } else { quote!() };

    let result = quote! {
        #[automatically_derived]
        impl #impl_generics ::tycho_types::cell::Load<#tlb_lifetime> for #ident #ty_generics #where_clause {
            #inline
            fn load_from(
                __slice: &mut ::tycho_types::cell::CellSlice<#tlb_lifetime>
            ) -> ::core::result::Result<Self, ::tycho_types::error::Error> {
                #body
            }
        }
    };

    Ok(result)
}

fn build_enum(_: &[ast::Variant<'_>]) -> TokenStream {
    panic!("Unsupported")
}

fn build_struct(
    container: &ast::Container<'_>,
    lifetime_def: &syn::LifetimeParam,
    style: ast::Style,
    fields: &[ast::Field<'_>],
) -> TokenStream {
    let mut tag_version = None;
    let tags = match &container.attrs.tlb_tag {
        attr::ContainerTag::None => None,
        attr::ContainerTag::Single(tag) => load_tag(*tag),
        attr::ContainerTag::Multiple(tags) => load_tags_versioned(tags).map(|load_tags| {
            let ident = quote::format_ident!("__tag_version");
            let res = quote! { let #ident = #load_tags; };
            tag_version = Some(ident);
            res
        }),
    };

    let members = fields.iter().map(|field| {
        let ident = &field.member;
        let ty = field.ty;
        let mut op = load_op(lifetime_def, ty);

        if let (Some(since), Some(tag_version)) = (field.attrs.since_tag, &tag_version) {
            let default = match &field.attrs.default {
                None => quote! { <#ty as Default>::default() },
                Some(expr) => expr.to_token_stream(),
            };

            op = quote! {
                if #tag_version >= #since {
                    #op
                } else {
                    #default
                }
            };
        }

        quote! {
            #ident: #op
        }
    });

    let result = match style {
        ast::Style::Unit => quote!(Self),
        _ => quote! {
            Self {
                #(#members),*,
            }
        },
    };

    let result = match &container.attrs.tlb_validate_with {
        Some(expr) => quote! {
            let result = #result;
            if #expr(&result) {
                ::core::result::Result::Ok(result)
            } else {
                ::core::result::Result::Err(::tycho_types::error::Error::InvalidData)
            }
        },
        None => quote!(::core::result::Result::Ok(#result)),
    };

    quote! {
        #tags
        #result
    }
}

fn load_tag(tag: attr::TlbTag) -> Option<TokenStream> {
    let (op, value) = load_tag_op_value(tag)?;

    Some(quote! {
        match #op {
            ::core::result::Result::Ok(#value) => {},
            ::core::result::Result::Ok(_) => return ::core::result::Result::Err(::tycho_types::error::Error::InvalidTag),
            ::core::result::Result::Err(e) => return ::core::result::Result::Err(e),
        }
    })
}

fn load_tags_versioned(tags: &[attr::TlbTag]) -> Option<TokenStream> {
    let mut iter = tags.iter();

    let first_tag = iter.next()?;
    let (op, first_value) = load_tag_op_value(*first_tag)?;

    let mut values = Vec::with_capacity(tags.len());
    values.push(first_value);

    for tag in iter {
        values.push(match tag.bits {
            0 => continue,
            1 => {
                let value = tag.value != 0;
                quote!(#value)
            }
            2..=8 => {
                let value = tag.value as u8;
                quote!(#value)
            }
            16 => {
                let value = tag.value as u16;
                quote!(#value)
            }
            32 => {
                let value = tag.value;
                quote!(#value)
            }
            _ => {
                let value = tag.value as u64;
                quote!(#value)
            }
        });
    }

    let values = values.into_iter().enumerate().map(|(i, value)| {
        quote! { ::core::result::Result::Ok(#value) => #i }
    });

    Some(quote! {
        match #op {
            #(#values),*,
            ::core::result::Result::Ok(_) => return ::core::result::Result::Err(::tycho_types::error::Error::InvalidTag),
            ::core::result::Result::Err(e) => return ::core::result::Result::Err(e),
        }
    })
}

fn load_tag_op_value(tag: attr::TlbTag) -> Option<(TokenStream, TokenStream)> {
    let bits = tag.bits as u16;

    Some(match bits {
        0 => return None,
        1 => {
            let value = tag.value != 0;
            (quote!(__slice.load_bit()), quote!(#value))
        }
        2..=7 => {
            let value = tag.value as u8;
            (quote!(__slice.load_small_uint(#bits)), quote!(#value))
        }
        8 => {
            let value = tag.value as u8;
            (quote!(__slice.load_u8()), quote!(#value))
        }
        16 => {
            let value = tag.value as u16;
            (quote!(__slice.load_u16()), quote!(#value))
        }
        32 => {
            let value = tag.value;
            (quote!(__slice.load_u32()), quote!(#value))
        }
        _ => {
            let value = tag.value as u64;
            (quote!(__slice.load_uint(#bits)), quote!(#value))
        }
    })
}

fn load_op(lifetime_def: &syn::LifetimeParam, ty: &syn::Type) -> TokenStream {
    #[allow(clippy::unnecessary_operation)]
    'fallback: {
        match ty {
            syn::Type::Path(syn::TypePath { path, .. }) => {
                if let Some(syn::PathSegment { ident, .. }) = path.segments.last() {
                    let (op, cast) = match ident.to_string().as_str() {
                        "bool" => (quote!(load_bit()), None),
                        "i8" => (quote!(load_u8()), Some(quote!(as i8))),
                        "u8" => (quote!(load_u8()), None),
                        "i16" => (quote!(load_u16()), Some(quote!(as i16))),
                        "u16" => (quote!(load_u16()), None),
                        "i32" => (quote!(load_u32()), Some(quote!(as i32))),
                        "u32" => (quote!(load_u32()), None),
                        "i64" => (quote!(load_u64()), Some(quote!(as i64))),
                        "u64" => (quote!(load_u64()), None),
                        "HashBytes" => (quote!(load_u256()), None),
                        "CellImpl" => (quote!(load_reference()), None),
                        "Cell" => (quote!(load_reference_cloned()), None),
                        _ => break 'fallback,
                    };

                    return quote!(match __slice.#op {
                        ::core::result::Result::Ok(val) => val #cast,
                        ::core::result::Result::Err(err) => return ::core::result::Result::Err(err),
                    });
                }
            }
            syn::Type::Reference(syn::TypeReference { elem, .. }) => {
                return load_op(lifetime_def, elem);
            }
            _ => break 'fallback,
        }
    };

    quote! {
        match <#ty as ::tycho_types::cell::Load<#lifetime_def>>::load_from(__slice) {
            ::core::result::Result::Ok(val) => val,
            ::core::result::Result::Err(err) => return ::core::result::Result::Err(err),
        }
    }
}
