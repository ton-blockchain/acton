use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::meta::ParseNestedMeta;

use super::ctxt::*;
use super::symbol::*;

pub struct Container {
    pub tlb_tag: ContainerTag,
    pub tlb_validate_with: Option<syn::Expr>,
}

impl Container {
    pub fn from_ast(cx: &Ctxt, item: &syn::DeriveInput) -> Self {
        let mut tlb_tag = Attr::none(cx, TAG);
        let mut tlb_validate_with = Attr::none(cx, VALIDATE_WITH);

        for attr in &item.attrs {
            if attr.path() != TLB {
                continue;
            }

            if let syn::Meta::List(meta) = &attr.meta
                && meta.tokens.is_empty()
            {
                continue;
            }

            if let Err(e) = attr.parse_nested_meta(|meta| {
                if meta.path == TAG {
                    // Parse `#[tlb(tag = "#ab"]` or `#[tlb(tag = ["#1", "#2"])]`
                    if let Some(value) = parse_lit_into_tlb_tag(cx, TAG, &meta)? {
                        tlb_tag.set(&meta.path, value);
                    }
                } else if meta.path == VALIDATE_WITH {
                    // Parse `#[tlb(validate_with = "some_module"]`
                    if let Some(expr) = parse_lit_into_expr(cx, VALIDATE_WITH, &meta)? {
                        tlb_validate_with.set(&meta.path, expr);
                    }
                } else {
                    let path = meta.path.to_token_stream().to_string().replace(' ', "");
                    return Err(meta.error(format!("unknown TLB container attribute `{path}`")));
                }
                Ok(())
            }) {
                cx.syn_error(e);
            }
        }

        Self {
            tlb_tag: tlb_tag.get().unwrap_or_default(),
            tlb_validate_with: tlb_validate_with.get(),
        }
    }
}

pub struct Variant;

impl Variant {
    pub fn from_ast(cx: &Ctxt, item: &syn::Variant) -> Self {
        for attr in &item.attrs {
            if attr.path() != TLB {
                continue;
            }

            if let syn::Meta::List(meta) = &attr.meta
                && meta.tokens.is_empty()
            {
                continue;
            }

            if let Err(e) = attr.parse_nested_meta(|meta| {
                let path = meta.path.to_token_stream().to_string().replace(' ', "");
                Err(meta.error(format!("unknown tl variant attribute `{path}`")))
            }) {
                cx.syn_error(e);
            }
        }

        Self
    }
}

pub struct Field {
    pub since_tag: Option<usize>,
    pub default: Option<syn::Expr>,
}

impl Field {
    pub fn from_ast(cx: &Ctxt, field: &syn::Field) -> Self {
        let mut since_tag = Attr::none(cx, SINCE_TAG);
        let mut default = Attr::none(cx, DEFAULT);

        for attr in &field.attrs {
            if attr.path() != TLB {
                continue;
            }

            if let syn::Meta::List(meta) = &attr.meta
                && meta.tokens.is_empty()
            {
                continue;
            }

            if let Err(e) = attr.parse_nested_meta(|meta| {
                if meta.path == SINCE_TAG {
                    // Parse `#[tlb(since_tag = 0)]`
                    if let Some(value) = parse_number(cx, SINCE_TAG, &meta)? {
                        since_tag.set(&meta.path, value);
                    }
                } else if meta.path == DEFAULT {
                    // Parse `#[tlb(since_tag = 0, default = "123")]`
                    if let Some(value) = parse_lit_into_expr(cx, DEFAULT, &meta)? {
                        default.set(&meta.path, value);
                    }
                } else {
                    let path = meta.path.to_token_stream().to_string().replace(' ', "");
                    return Err(meta.error(format!("unknown tl field attribute `{path}`")));
                }
                Ok(())
            }) {
                cx.syn_error(e);
            }
        }

        Self {
            since_tag: since_tag.get(),
            default: default.get(),
        }
    }
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub enum ContainerTag {
    #[default]
    None,
    Single(TlbTag),
    Multiple(Vec<TlbTag>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct TlbTag {
    pub value: u32,
    pub bits: u8,
}

fn parse_lit_into_tlb_tag(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<ContainerTag>> {
    fn parse_tag(cx: &Ctxt, lit: syn::LitStr) -> Option<TlbTag> {
        let string = lit.value();
        let string = string.trim();
        if let Some(hex_tag) = string.strip_prefix('#') {
            if let Ok(value) = u32::from_str_radix(hex_tag, 16) {
                return Some(TlbTag {
                    value,
                    bits: (hex_tag.len() * 4) as u8,
                });
            }

            cx.error_spanned_by(lit, format!("failed to parse hex TLB tag: {string}"));
        } else if let Some(binary_tag) = string.strip_prefix('$') {
            if let Ok(value) = u32::from_str_radix(binary_tag, 2) {
                return Some(TlbTag {
                    value,
                    bits: binary_tag.len() as u8,
                });
            }

            cx.error_spanned_by(lit, format!("failed to parse binary TLB tag: {string}"));
        } else {
            cx.error_spanned_by(lit, format!("failed to parse TLB tag: {string}"));
        }

        None
    }

    Ok(match ungroup_meta(meta)? {
        syn::Expr::Array(array) => {
            if array.elems.is_empty() {
                cx.error_spanned_by(array, "tag list is empty");
                return Ok(None);
            }

            let mut tags = Vec::with_capacity(array.elems.len());
            let mut is_ok = true;
            for value in array.elems {
                let res = get_lit_str2(cx, attr_name, attr_name, &value)
                    .and_then(|lit| parse_tag(cx, lit));
                is_ok &= res.is_some();
                tags.extend(res);
            }

            is_ok.then_some(ContainerTag::Multiple(tags))
        }
        value => get_lit_str2(cx, attr_name, attr_name, &value)
            .and_then(|lit| parse_tag(cx, lit))
            .map(ContainerTag::Single),
    })
}

fn parse_lit_into_expr(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::Expr>> {
    let Some(s) = get_lit_str(cx, attr_name, meta)? else {
        return Ok(None);
    };

    let tokens = spanned_tokens(&s)?;
    let expr: syn::Expr = syn::parse2(tokens)?;
    Ok(Some(expr))
}

fn parse_number(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<usize>> {
    let value = ungroup_meta(meta)?;

    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(lit),
        ..
    }) = value
    {
        lit.base10_parse::<usize>().map(Some)
    } else {
        cx.error_spanned_by(
            value,
            format!("expected {attr_name} attribute to be a number: `{attr_name} = \"...\"`"),
        );
        Ok(None)
    }
}

fn spanned_tokens(s: &syn::LitStr) -> syn::parse::Result<TokenStream> {
    let stream = syn::parse_str(&s.value())?;
    Ok(respan_token_stream(stream, s.span()))
}

fn respan_token_stream(stream: TokenStream, span: Span) -> TokenStream {
    stream
        .into_iter()
        .map(|token| respan_token_tree(token, span))
        .collect()
}

fn respan_token_tree(mut token: TokenTree, span: Span) -> TokenTree {
    if let TokenTree::Group(g) = &mut token {
        *g = Group::new(g.delimiter(), respan_token_stream(g.stream(), span));
    }
    token.set_span(span);
    token
}

fn get_lit_str(
    cx: &Ctxt,
    attr_name: Symbol,
    meta: &ParseNestedMeta,
) -> syn::Result<Option<syn::LitStr>> {
    let value = ungroup_meta(meta)?;
    Ok(get_lit_str2(cx, attr_name, attr_name, &value))
}

fn get_lit_str2(
    cx: &Ctxt,
    attr_name: Symbol,
    meta_item_name: Symbol,
    value: &syn::Expr,
) -> Option<syn::LitStr> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Str(lit),
        ..
    }) = value
    {
        let suffix = lit.suffix();
        if !suffix.is_empty() {
            cx.error_spanned_by(
                lit,
                format!("unexpected suffix `{suffix}` on string literal"),
            );
        }
        Some(lit.clone())
    } else {
        cx.error_spanned_by(
            value,
            format!("expected {attr_name} attribute to be a string: `{meta_item_name} = \"...\"`"),
        );
        None
    }
}

fn ungroup_meta(meta: &ParseNestedMeta) -> syn::Result<syn::Expr> {
    let mut value = meta.value()?.parse()?;
    loop {
        match value {
            syn::Expr::Group(e) => value = *e.expr,
            value => return Ok(value),
        }
    }
}

#[allow(unused)]
struct BoolAttr<'c>(Attr<'c, ()>);

#[allow(unused)]
impl<'c> BoolAttr<'c> {
    fn none(cx: &'c Ctxt, name: Symbol) -> Self {
        BoolAttr(Attr::none(cx, name))
    }

    fn set_true<O>(&mut self, object: O)
    where
        O: ToTokens,
    {
        self.0.set(object, ());
    }

    fn get(&self) -> bool {
        self.0.value.is_some()
    }
}

struct Attr<'c, T> {
    cx: &'c Ctxt,
    name: Symbol,
    tokens: TokenStream,
    value: Option<T>,
}

impl<'c, T> Attr<'c, T> {
    fn none(cx: &'c Ctxt, name: Symbol) -> Self {
        Self {
            cx,
            name,
            tokens: TokenStream::new(),
            value: None,
        }
    }

    fn set<O>(&mut self, object: O, value: T)
    where
        O: ToTokens,
    {
        let tokens = object.into_token_stream();

        if self.value.is_some() {
            self.cx
                .error_spanned_by(tokens, format!("duplicated attribute `{}`", self.name));
        } else {
            self.tokens = tokens;
            self.value = Some(value);
        }
    }

    #[allow(unused)]
    fn set_opt<O>(&mut self, object: O, value: Option<T>)
    where
        O: ToTokens,
    {
        if let Some(value) = value {
            self.set(object, value);
        }
    }

    #[allow(unused)]
    fn set_if_none(&mut self, value: T) {
        if self.value.is_none() {
            self.value = Some(value);
        }
    }

    fn get(self) -> Option<T> {
        self.value
    }

    #[allow(unused)]
    fn get_with_tokens(self) -> Option<(TokenStream, T)> {
        match self.value {
            Some(value) => Some((self.tokens, value)),
            None => None,
        }
    }
}
