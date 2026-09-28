use std::collections::HashSet;

use syn::punctuated::Pair;
use syn::visit::{self, Visit};

use crate::internals::{ast, attr};

pub fn without_default(generics: &syn::Generics) -> syn::Generics {
    syn::Generics {
        params: generics
            .params
            .iter()
            .map(|param| match param {
                syn::GenericParam::Type(param) => syn::GenericParam::Type(syn::TypeParam {
                    eq_token: None,
                    default: None,
                    ..param.clone()
                }),
                _ => param.clone(),
            })
            .collect(),
        ..generics.clone()
    }
}

#[allow(unused)]
pub fn with_bound(
    container: &ast::Container,
    generics: &syn::Generics,
    filter: fn(&attr::Field, Option<&attr::Variant>) -> bool,
    bound: &syn::Path,
) -> syn::Generics {
    struct FindTyParams<'ast> {
        all_type_params: HashSet<syn::Ident>,
        relevant_type_params: HashSet<syn::Ident>,
        associated_type_usage: Vec<&'ast syn::TypePath>,
    }

    impl<'ast> Visit<'ast> for FindTyParams<'ast> {
        fn visit_field(&mut self, field: &'ast syn::Field) {
            if let syn::Type::Path(ty) = ungroup(&field.ty)
                && let Some(Pair::Punctuated(t, _)) = ty.path.segments.pairs().next()
                && self.all_type_params.contains(&t.ident)
            {
                self.associated_type_usage.push(ty);
            }
            self.visit_type(&field.ty);
        }

        fn visit_macro(&mut self, _mac: &'ast syn::Macro) {}

        fn visit_path(&mut self, path: &'ast syn::Path) {
            if let Some(seg) = path.segments.last()
                && seg.ident == "PhantomData"
            {
                return;
            }
            if path.leading_colon.is_none() && path.segments.len() == 1 {
                let id = &path.segments[0].ident;
                if self.all_type_params.contains(id) {
                    self.relevant_type_params.insert(id.clone());
                }
            }
            visit::visit_path(self, path);
        }
    }

    let all_type_params = generics
        .type_params()
        .map(|param| param.ident.clone())
        .collect();

    let mut visitor = FindTyParams {
        all_type_params,
        relevant_type_params: HashSet::new(),
        associated_type_usage: Vec::new(),
    };

    match &container.data {
        ast::Data::Enum(variants) => {
            for variant in variants.iter() {
                let relevant_fields = variant
                    .fields
                    .iter()
                    .filter(|field| filter(&field.attrs, Some(&variant.attrs)));

                for field in relevant_fields {
                    visitor.visit_field(field.original);
                }
            }
        }
        ast::Data::Struct(_, fields) => {
            for field in fields.iter().filter(|field| filter(&field.attrs, None)) {
                visitor.visit_field(field.original);
            }
        }
    }

    let relevant_type_params = visitor.relevant_type_params;
    let associated_type_params = visitor.associated_type_usage;

    let new_predicates = generics
        .type_params()
        .map(|param| param.ident.clone())
        .filter(|ident| relevant_type_params.contains(ident))
        .map(|ident| syn::TypePath {
            qself: None,
            path: ident.into(),
        })
        .chain(associated_type_params.into_iter().cloned())
        .map(|bounded_ty| {
            syn::WherePredicate::Type(syn::PredicateType {
                lifetimes: None,
                bounded_ty: syn::Type::Path(bounded_ty),
                colon_token: <syn::Token![:]>::default(),
                bounds: vec![syn::TypeParamBound::Trait(syn::TraitBound {
                    paren_token: None,
                    modifier: syn::TraitBoundModifier::None,
                    lifetimes: None,
                    path: bound.clone(),
                })]
                .into_iter()
                .collect(),
            })
        });

    let mut generics = generics.clone();
    generics
        .make_where_clause()
        .predicates
        .extend(new_predicates);
    generics
}

fn ungroup(mut ty: &syn::Type) -> &syn::Type {
    while let syn::Type::Group(group) = ty {
        ty = &group.elem;
    }
    ty
}
