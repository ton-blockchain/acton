use std::collections::HashSet;

use syn::Token;
use syn::punctuated::Punctuated;

use super::attr;
use super::ctxt::*;
use crate::Derive;

pub struct Container<'a> {
    pub ident: syn::Ident,
    pub attrs: attr::Container,
    pub data: Data<'a>,
    pub generics: &'a syn::Generics,
    #[allow(unused)]
    pub original: &'a syn::DeriveInput,
}

impl<'a> Container<'a> {
    pub(crate) fn from_ast(cx: &Ctxt, item: &'a syn::DeriveInput, _derive: Derive) -> Option<Self> {
        let attrs = attr::Container::from_ast(cx, item);

        let data = match &item.data {
            syn::Data::Enum(data) => Data::Enum(enum_from_ast(cx, &data.variants)),
            syn::Data::Struct(data) => {
                let (style, fields) = struct_from_ast(cx, &data.fields);
                Data::Struct(style, fields)
            }
            syn::Data::Union(_) => {
                cx.error_spanned_by(item, "tl doesn't support derive for unions");
                return None;
            }
        };

        let container = Self {
            ident: item.ident.clone(),
            attrs,
            data,
            generics: &item.generics,
            original: item,
        };

        container.validate(cx);

        Some(container)
    }

    fn validate(&self, cx: &Ctxt) {
        let check_default = |field: &Field<'_>| {
            if field.attrs.default.is_some() && field.attrs.since_tag.is_none() {
                cx.error_spanned_by(
                    field.original,
                    "`tlb(default = ...)` can only be used with `since_tag`",
                );
            }
        };
        match &self.data {
            Data::Enum(variants) => {
                for variant in variants {
                    for field in &variant.fields {
                        check_default(field);
                    }
                }
            }
            Data::Struct(_, fields) => {
                for field in fields {
                    check_default(field);
                }
            }
        }

        match &self.data {
            Data::Enum(variants) => {
                if matches!(self.attrs.tlb_tag, attr::ContainerTag::Multiple(_)) {
                    cx.error_spanned_by(self.original, "multi-tags for enum are not supported");
                }

                for var in variants {
                    for field in &var.fields {
                        if field.attrs.since_tag.is_some() {
                            cx.error_spanned_by(
                                field.original,
                                "`since_tag` is not supported for enum fields",
                            );
                        }
                    }
                }
            }
            Data::Struct(_, fields) => {
                let mut unused_tags = HashSet::new();
                let tag_count = match &self.attrs.tlb_tag {
                    attr::ContainerTag::None => 0,
                    attr::ContainerTag::Single(_) => 1,
                    attr::ContainerTag::Multiple(tags) => {
                        let mut tag_len = None;
                        for (i, tag) in tags.iter().enumerate() {
                            if i > 0 {
                                unused_tags.insert(i);
                            }

                            match tag_len {
                                None => tag_len = Some(tag.bits),
                                Some(bits) if tag.bits != bits => {
                                    cx.error_spanned_by(
                                        self.original,
                                        "all tags must have the same bit length",
                                    );
                                }
                                Some(_) => {}
                            }
                        }

                        tags.len()
                    }
                };

                for field in fields {
                    if let Some(since) = field.attrs.since_tag {
                        unused_tags.remove(&since);

                        if tag_count == 0 {
                            cx.error_spanned_by(
                                field.original,
                                "`since_tag` is specified but there are no tags for this struct",
                            );
                        } else if since >= tag_count {
                            cx.error_spanned_by(
                                field.original,
                                format!(
                                    "`since_tag` is out of bounds (max tag index is {})",
                                    tag_count - 1
                                ),
                            );
                        }
                    }
                }

                for i in unused_tags {
                    cx.error_spanned_by(self.original, format!("tag {i} is not used"));
                }
            }
        }
    }
}

fn enum_from_ast<'a>(
    cx: &Ctxt,
    variants: &'a Punctuated<syn::Variant, Token![,]>,
) -> Vec<Variant<'a>> {
    variants
        .iter()
        .map(|variant| {
            let attrs = attr::Variant::from_ast(cx, variant);
            let (style, fields) = struct_from_ast(cx, &variant.fields);
            Variant {
                ident: variant.ident.clone(),
                attrs,
                style,
                fields,
                original: variant,
            }
        })
        .collect()
}

fn struct_from_ast<'a>(cx: &Ctxt, fields: &'a syn::Fields) -> (Style, Vec<Field<'a>>) {
    match fields {
        syn::Fields::Named(fields) => (Style::Struct, fields_from_ast(cx, &fields.named)),
        syn::Fields::Unnamed(fields) => (Style::Tuple, fields_from_ast(cx, &fields.unnamed)),
        syn::Fields::Unit => (Style::Unit, Vec::new()),
    }
}

fn fields_from_ast<'a>(cx: &Ctxt, fields: &'a Punctuated<syn::Field, Token![,]>) -> Vec<Field<'a>> {
    fields
        .iter()
        .enumerate()
        .map(|(i, field)| Field {
            member: match &field.ident {
                Some(ident) => syn::Member::Named(ident.clone()),
                None => syn::Member::Unnamed(i.into()),
            },
            attrs: attr::Field::from_ast(cx, field),
            ty: &field.ty,
            original: field,
        })
        .collect()
}

pub enum Data<'a> {
    Enum(Vec<Variant<'a>>),
    Struct(Style, Vec<Field<'a>>),
}

pub struct Variant<'a> {
    #[allow(unused)]
    pub ident: syn::Ident,
    pub attrs: attr::Variant,
    #[allow(unused)]
    pub style: Style,
    pub fields: Vec<Field<'a>>,
    #[allow(unused)]
    pub original: &'a syn::Variant,
}

pub struct Field<'a> {
    pub member: syn::Member,
    pub attrs: attr::Field,
    pub ty: &'a syn::Type,
    pub original: &'a syn::Field,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Style {
    Struct,
    Tuple,
    Unit,
}
