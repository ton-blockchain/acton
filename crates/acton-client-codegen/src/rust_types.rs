use crate::CodegenError;
use crate::names::{stack_struct_ident, type_ident, union_ident};
use crate::symbols::Symbols;
use proc_macro2::{Ident, TokenStream};
use quote::quote;
use std::collections::BTreeSet;
use tolk_source_map::types_kernel::{Ty, TyIdx};

pub(crate) struct RustTypes<'abi> {
    symbols: Symbols<'abi>,
}

impl<'abi> RustTypes<'abi> {
    pub(crate) const fn new(abi: &'abi tolk_source_map::abi::ContractABI) -> Self {
        Self {
            symbols: Symbols::new(abi),
        }
    }

    pub(crate) fn cell_type(&self, ty_idx: TyIdx) -> Result<TokenStream, CodegenError> {
        self.rust_type(ty_idx, false)
    }

    pub(crate) fn stack_type(&self, ty_idx: TyIdx) -> Result<TokenStream, CodegenError> {
        self.rust_type(ty_idx, true)
    }

    pub(crate) fn generic_params(&self, ty_idx: TyIdx) -> Result<Vec<Ident>, CodegenError> {
        let mut names = BTreeSet::new();
        self.collect_generic_params(ty_idx, &mut names)?;
        names.into_iter().map(|name| type_ident(&name)).collect()
    }

    fn rust_type(&self, ty_idx: TyIdx, is_for_stack: bool) -> Result<TokenStream, CodegenError> {
        Ok(match self.symbols.ty(ty_idx)? {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => quote! { ::acton_client::__private::num_bigint::BigInt },
            Ty::Bool => quote! { bool },
            Ty::Cell => quote! { ::acton_client::Cell },
            Ty::Builder => quote! { ::acton_client::__private::tycho_types::cell::CellBuilder },
            Ty::Slice | Ty::Remaining => quote! { ::acton_client::OwnedSlice },
            Ty::String => quote! { ::std::string::String },
            Ty::Address => {
                quote! { ::acton_client::__private::tycho_types::models::StdAddr }
            }
            Ty::AddressOpt => {
                quote! { ::std::option::Option<::acton_client::__private::tycho_types::models::StdAddr> }
            }
            Ty::AddressExt => {
                quote! { ::acton_client::__private::tycho_types::models::ExtAddr }
            }
            Ty::AddressAny => {
                quote! { ::acton_client::__private::tycho_types::models::AnyAddr }
            }
            Ty::BitsN { .. } => quote! { ::acton_client::BitString },
            Ty::NullLiteral | Ty::Void => quote! { () },
            Ty::Callable | Ty::Unknown => quote! { ::acton_client::TupleItem },
            Ty::Nullable { inner_ty_idx, .. } => {
                let inner = self.rust_type(*inner_ty_idx, is_for_stack)?;
                quote! { ::std::option::Option<#inner> }
            }
            Ty::CellOf { inner_ty_idx } => {
                let inner = self.rust_type(*inner_ty_idx, false)?;
                quote! { ::acton_client::CellRef<#inner> }
            }
            Ty::ArrayOf { inner_ty_idx } | Ty::LispListOf { inner_ty_idx } => {
                let inner = self.rust_type(*inner_ty_idx, is_for_stack)?;
                quote! { ::std::vec::Vec<#inner> }
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                let items = items_ty_idx
                    .iter()
                    .map(|ty_idx| self.rust_type(*ty_idx, is_for_stack))
                    .collect::<Result<Vec<_>, _>>()?;
                quote! { (#(#items,)*) }
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let key = self.rust_type(*key_ty_idx, false)?;
                let value = self.rust_type(*value_ty_idx, false)?;
                quote! { ::acton_client::Dictionary<#key, #value> }
            }
            Ty::EnumRef { enum_name } => {
                let name = type_ident(enum_name)?;
                quote! { #name }
            }
            Ty::StructRef {
                struct_name,
                type_args_ty_idx,
            } => {
                if is_for_stack
                    && self
                        .symbols
                        .struct_fields(ty_idx, true)?
                        .iter()
                        .any(|field| field.field.client_ty_idx.is_some())
                {
                    let name = stack_struct_ident(struct_name, ty_idx)?;
                    quote! { #name }
                } else {
                    let name = type_ident(struct_name)?;
                    let arguments = type_args_ty_idx
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(|ty_idx| self.rust_type(*ty_idx, is_for_stack))
                        .collect::<Result<Vec<_>, _>>()?;
                    if arguments.is_empty() {
                        quote! { #name }
                    } else {
                        quote! { #name<#(#arguments),*> }
                    }
                }
            }
            Ty::AliasRef {
                alias_name,
                type_args_ty_idx,
            } => {
                let name = type_ident(alias_name)?;
                let arguments = type_args_ty_idx
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|ty_idx| self.rust_type(*ty_idx, is_for_stack))
                    .collect::<Result<Vec<_>, _>>()?;
                if arguments.is_empty() {
                    quote! { #name }
                } else {
                    quote! { #name<#(#arguments),*> }
                }
            }
            Ty::GenericT { name_t } => {
                let name = type_ident(name_t)?;
                quote! { #name }
            }
            Ty::Union { .. } => {
                let name = union_ident(ty_idx);
                let params = self.generic_params(ty_idx)?;
                if params.is_empty() {
                    quote! { #name }
                } else {
                    quote! { #name<#(#params),*> }
                }
            }
        })
    }

    fn collect_generic_params(
        &self,
        ty_idx: TyIdx,
        output: &mut BTreeSet<String>,
    ) -> Result<(), CodegenError> {
        match self.symbols.ty(ty_idx)? {
            Ty::GenericT { name_t } => {
                output.insert(name_t.clone());
            }
            Ty::Nullable { inner_ty_idx, .. }
            | Ty::CellOf { inner_ty_idx }
            | Ty::ArrayOf { inner_ty_idx }
            | Ty::LispListOf { inner_ty_idx } => {
                self.collect_generic_params(*inner_ty_idx, output)?;
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                for ty_idx in items_ty_idx {
                    self.collect_generic_params(*ty_idx, output)?;
                }
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                self.collect_generic_params(*key_ty_idx, output)?;
                self.collect_generic_params(*value_ty_idx, output)?;
            }
            Ty::StructRef {
                type_args_ty_idx, ..
            }
            | Ty::AliasRef {
                type_args_ty_idx, ..
            } => {
                for ty_idx in type_args_ty_idx.as_deref().unwrap_or_default() {
                    self.collect_generic_params(*ty_idx, output)?;
                }
            }
            Ty::Union { variants, .. } => {
                for variant in variants {
                    self.collect_generic_params(variant.variant_ty_idx, output)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
