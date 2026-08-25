use crate::CodegenError;
use crate::names::{type_ident, union_ident, value_ident};
use crate::rust_types::RustTypes;
use crate::symbols::Symbols;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::Index;
use syn::Lifetime;
use tolk_source_map::abi::{ABIDeclaration, ContractABI};
use tolk_source_map::types_kernel::{Ty, TyIdx};

pub(crate) struct CellCodec<'abi> {
    symbols: Symbols<'abi>,
    types: RustTypes<'abi>,
}

#[derive(Clone, Copy)]
enum CellDirection {
    Store,
    Load,
}

struct UnsupportedCellType {
    subject: String,
    field_path: String,
    type_name: String,
}

impl<'abi> CellCodec<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self {
            symbols: Symbols::new(abi),
            types: RustTypes::new(abi),
        }
    }

    pub(crate) fn store(
        &self,
        ty_idx: TyIdx,
        value: TokenStream,
        builder: &Ident,
        u_label_ty_idx: Option<TyIdx>,
    ) -> Result<TokenStream, CodegenError> {
        self.store_impl(ty_idx, value, builder, u_label_ty_idx, false)
    }

    pub(crate) fn store_declaration(
        &self,
        ty_idx: TyIdx,
        value: TokenStream,
        builder: &Ident,
    ) -> Result<TokenStream, CodegenError> {
        if let Some(unsupported) =
            self.find_unsupported_declaration(ty_idx, CellDirection::Store)?
        {
            return Ok(unsupported_declaration_store(&unsupported));
        }
        self.store_impl(ty_idx, value, builder, None, true)
    }

    fn store_impl(
        &self,
        ty_idx: TyIdx,
        value: TokenStream,
        builder: &Ident,
        u_label_ty_idx: Option<TyIdx>,
        expand_named: bool,
    ) -> Result<TokenStream, CodegenError> {
        Ok(match self.symbols.ty(ty_idx)? {
            Ty::Int => unsupported_store("unbounded int"),
            Ty::IntN { n } => {
                let bits = bits(*n)?;
                quote! { ::acton_client::cell::store_fixed_int(#builder, #value, #bits, true)?; }
            }
            Ty::UintN { n } => {
                let bits = bits(*n)?;
                quote! { ::acton_client::cell::store_fixed_int(#builder, #value, #bits, false)?; }
            }
            Ty::VarintN { n } => {
                let len_bits = varint_len_bits(*n)?;
                quote! { ::acton_client::cell::store_var_int(#builder, #value, #len_bits, true)?; }
            }
            Ty::VaruintN { n } => {
                let len_bits = varint_len_bits(*n)?;
                quote! { ::acton_client::cell::store_var_int(#builder, #value, #len_bits, false)?; }
            }
            Ty::Coins => {
                quote! { ::acton_client::cell::store_var_int(#builder, #value, 4, false)?; }
            }
            Ty::Bool => quote! { #builder.store_bit(*#value)?; },
            Ty::Cell => quote! {
                #builder.store_reference(<::acton_client::Cell as ::std::clone::Clone>::clone(#value))?;
            },
            Ty::Builder => quote! { #builder.store_builder(#value)?; },
            Ty::Slice | Ty::Remaining => {
                quote! { ::acton_client::cell::store_slice(#builder, #value)?; }
            }
            Ty::String => quote! { ::acton_client::cell::store_string(#builder, #value)?; },
            Ty::Address | Ty::AddressExt | Ty::AddressAny => {
                quote! { ::acton_client::cell::store_tlb(#builder, #value)?; }
            }
            Ty::AddressOpt => {
                quote! { ::acton_client::cell::store_address_opt(#builder, #value)?; }
            }
            Ty::BitsN { n } => {
                let bits = bits(*n)?;
                quote! { ::acton_client::cell::store_bits(#builder, #value, #bits)?; }
            }
            Ty::NullLiteral | Ty::Void => TokenStream::new(),
            Ty::Callable => unsupported_store("continuation"),
            Ty::Unknown => unsupported_store("unknown"),
            Ty::Nullable { inner_ty_idx, .. } => {
                let inner = format_ident!("inner");
                let store_inner = self.store(*inner_ty_idx, quote! { #inner }, builder, None)?;
                quote! {
                    if let ::std::option::Option::Some(#inner) = (#value).as_ref() {
                        #builder.store_bit(true)?;
                        #store_inner
                    } else {
                        #builder.store_bit(false)?;
                    }
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let nested = format_ident!("nested_{ty_idx}");
                let owned_nested = format_ident!("owned_nested_{ty_idx}");
                let store_inner = self.store(
                    *inner_ty_idx,
                    quote! { (#value).r#ref.as_ref() },
                    &nested,
                    None,
                )?;
                quote! {
                    {
                        let mut #owned_nested = ::acton_client::__private::tycho_types::cell::CellBuilder::new();
                        let #nested = &mut #owned_nested;
                        #store_inner
                        #builder.store_reference(#owned_nested.build()?)?;
                    }
                }
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let inner_builder = format_ident!("inner_builder");
                let store_inner =
                    self.store(*inner_ty_idx, quote! { item }, &inner_builder, None)?;
                quote! {
                    ::acton_client::cell::store_array(
                        #builder,
                        #value,
                        |item, #inner_builder| {
                            #store_inner
                            ::std::result::Result::Ok(())
                        },
                    )?;
                }
            }
            Ty::LispListOf { inner_ty_idx } => {
                let inner_builder = format_ident!("inner_builder");
                let store_inner =
                    self.store(*inner_ty_idx, quote! { item }, &inner_builder, None)?;
                quote! {
                    ::acton_client::cell::store_lisp_list(
                        #builder,
                        #value,
                        |item, #inner_builder| {
                            #store_inner
                            ::std::result::Result::Ok(())
                        },
                    )?;
                }
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                let statements = items_ty_idx
                    .iter()
                    .enumerate()
                    .map(|(index, ty_idx)| {
                        let index = Index::from(index);
                        self.store(*ty_idx, quote! { &(#value).#index }, builder, None)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                quote! { #(#statements)* }
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let key_bits = self.dictionary_key_bits(*key_ty_idx)?;
                let key_builder = format_ident!("key_builder");
                let value_builder = format_ident!("value_builder");
                let store_key = self.store(*key_ty_idx, quote! { key }, &key_builder, None)?;
                let store_value =
                    self.store(*value_ty_idx, quote! { value }, &value_builder, None)?;
                quote! {
                    ::acton_client::cell::store_dictionary::<#key_bits, _, _>(
                        #builder,
                        #value,
                        |key, #key_builder| {
                            #store_key
                            ::std::result::Result::Ok(())
                        },
                        |value, #value_builder| {
                            #store_value
                            ::std::result::Result::Ok(())
                        },
                    )?;
                }
            }
            Ty::EnumRef { enum_name } if !expand_named => {
                quote! { ::acton_client::AbiStore::store_into(#value, #builder)?; }
            }
            Ty::EnumRef { enum_name } => {
                let declaration = self.symbols.declaration(enum_name)?;
                let ABIDeclaration::Enum {
                    encoded_as_ty_idx,
                    custom_pack_unpack,
                    ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.pack_to_builder)
                    .unwrap_or(false)
                {
                    quote! { ::acton_client::cell::custom_store(#enum_name, #value, #builder)?; }
                } else {
                    self.store(*encoded_as_ty_idx, quote! { &#value.0 }, builder, None)?
                }
            }
            Ty::StructRef {
                type_args_ty_idx: None,
                ..
            } if !expand_named => {
                quote! { ::acton_client::AbiStore::store_into(#value, #builder)?; }
            }
            Ty::StructRef { struct_name, .. } => {
                let declaration = self.symbols.declaration(struct_name)?;
                let ABIDeclaration::Struct {
                    prefix,
                    custom_pack_unpack,
                    ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.pack_to_builder)
                    .unwrap_or(false)
                {
                    quote! { ::acton_client::cell::custom_store(#struct_name, #value, #builder)?; }
                } else {
                    let prefix = prefix
                        .as_ref()
                        .map(|prefix| store_prefix(prefix.prefix_num, prefix.prefix_len, builder))
                        .transpose()?
                        .unwrap_or_default();
                    let fields = self
                        .symbols
                        .struct_fields(ty_idx, false)?
                        .into_iter()
                        .map(|field| {
                            let name = value_ident(&field.field.name)?;
                            self.store(
                                field.field.ty_idx,
                                quote! { &(#value).#name },
                                builder,
                                field.u_label_ty_idx,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    quote! { #prefix #(#fields)* }
                }
            }
            Ty::AliasRef {
                alias_name,
                type_args_ty_idx: None,
            } if !expand_named => {
                let store_name = format_ident!("store_{}", value_ident(alias_name)?);
                quote! { #store_name(#value, #builder)?; }
            }
            Ty::AliasRef { alias_name, .. } => {
                let declaration = self.symbols.declaration(alias_name)?;
                let ABIDeclaration::Alias {
                    custom_pack_unpack, ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.pack_to_builder)
                    .unwrap_or(false)
                {
                    quote! { ::acton_client::cell::custom_store(#alias_name, #value, #builder)?; }
                } else {
                    let target = self.symbols.alias_target(ty_idx)?;
                    self.store(target.ty_idx, value, builder, target.u_label_ty_idx)?
                }
            }
            Ty::GenericT { name_t } => unsupported_store(&format!("unresolved generic {name_t}")),
            Ty::Union { variants, .. } => {
                let union_name = union_ident(self.symbols.union_identity(ty_idx, u_label_ty_idx));
                let variants = self.symbols.union_variants(variants, u_label_ty_idx)?;
                let arms = variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        let variant_name = format_ident!("Variant{index}");
                        let prefix =
                            if matches!(self.symbols.ty(variant.variant_ty_idx)?, Ty::NullLiteral)
                                || variant.is_prefix_implicit
                            {
                                store_prefix(
                                    variant.prefix_num,
                                    i32::try_from(variant.prefix_len).map_err(|_| {
                                        crate::generate_error("union prefix length exceeds i32")
                                    })?,
                                    builder,
                                )?
                            } else {
                                TokenStream::new()
                            };
                        let store_value =
                            self.store(variant.variant_ty_idx, quote! { inner }, builder, None)?;
                        Ok(quote! {
                            #union_name::#variant_name(inner) => {
                                #prefix
                                #store_value
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?;
                quote! { match #value { #(#arms),* } }
            }
        })
    }

    pub(crate) fn load(
        &self,
        ty_idx: TyIdx,
        slice: &Ident,
        u_label_ty_idx: Option<TyIdx>,
    ) -> Result<TokenStream, CodegenError> {
        self.load_impl(ty_idx, slice, u_label_ty_idx, false)
    }

    pub(crate) fn load_declaration(
        &self,
        ty_idx: TyIdx,
        slice: &Ident,
    ) -> Result<TokenStream, CodegenError> {
        if let Some(unsupported) = self.find_unsupported_declaration(ty_idx, CellDirection::Load)? {
            return Ok(unsupported_declaration_load(&unsupported));
        }
        match self.symbols.ty(ty_idx)? {
            Ty::AliasRef { alias_name, .. } => {
                self.load_at_field_path(ty_idx, slice, None, alias_name)
            }
            _ => self.load_impl(ty_idx, slice, None, true),
        }
    }

    fn load_impl(
        &self,
        ty_idx: TyIdx,
        slice: &Ident,
        u_label_ty_idx: Option<TyIdx>,
        expand_named: bool,
    ) -> Result<TokenStream, CodegenError> {
        Ok(match self.symbols.ty(ty_idx)? {
            Ty::Int => unsupported_load("unbounded int"),
            Ty::IntN { n } => {
                let bits = bits(*n)?;
                quote! { ::acton_client::cell::load_fixed_int(#slice, #bits, true)? }
            }
            Ty::UintN { n } => {
                let bits = bits(*n)?;
                quote! { ::acton_client::cell::load_fixed_int(#slice, #bits, false)? }
            }
            Ty::VarintN { n } => {
                let len_bits = varint_len_bits(*n)?;
                quote! { ::acton_client::cell::load_var_int(#slice, #len_bits, true)? }
            }
            Ty::VaruintN { n } => {
                let len_bits = varint_len_bits(*n)?;
                quote! { ::acton_client::cell::load_var_int(#slice, #len_bits, false)? }
            }
            Ty::Coins => quote! { ::acton_client::cell::load_var_int(#slice, 4, false)? },
            Ty::Bool => quote! { #slice.load_bit()? },
            Ty::Cell => quote! { #slice.load_reference_cloned()? },
            Ty::Builder => unsupported_load("builder"),
            Ty::Slice => unsupported_load("slice"),
            Ty::String => quote! { ::acton_client::cell::load_string(#slice)? },
            Ty::Remaining => quote! { ::acton_client::cell::load_remaining(#slice)? },
            Ty::Address | Ty::AddressExt | Ty::AddressAny => {
                let ty = self.types.cell_type(ty_idx)?;
                quote! { ::acton_client::cell::load_tlb::<#ty>(#slice)? }
            }
            Ty::AddressOpt => quote! { ::acton_client::cell::load_address_opt(#slice)? },
            Ty::BitsN { n } => {
                let bits = bits(*n)?;
                quote! { ::acton_client::cell::load_bits(#slice, #bits)? }
            }
            Ty::NullLiteral | Ty::Void => quote! { () },
            Ty::Callable => unsupported_load("continuation"),
            Ty::Unknown => unsupported_load("unknown"),
            Ty::Nullable { inner_ty_idx, .. } => {
                let load_inner = self.load(*inner_ty_idx, slice, None)?;
                quote! {
                    if #slice.load_bit()? {
                        ::std::option::Option::Some(#load_inner)
                    } else {
                        ::std::option::Option::None
                    }
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let nested = format_ident!("nested_{ty_idx}");
                let owned_nested = format_ident!("owned_nested_{ty_idx}");
                let load_inner = self.load(*inner_ty_idx, &nested, None)?;
                quote! {
                    {
                        let mut #owned_nested = #slice.load_reference_as_slice()?;
                        let #nested = &mut #owned_nested;
                        ::acton_client::CellRef::new(#load_inner)
                    }
                }
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let nested = format_ident!("nested");
                let load_inner = self.load(*inner_ty_idx, &nested, None)?;
                quote! {
                    ::acton_client::cell::load_array(#slice, |#nested| {
                        let value = #load_inner;
                        ::std::result::Result::Ok(value)
                    })?
                }
            }
            Ty::LispListOf { inner_ty_idx } => {
                let nested = format_ident!("nested");
                let load_inner = self.load(*inner_ty_idx, &nested, None)?;
                quote! {
                    ::acton_client::cell::load_lisp_list(#slice, |#nested| {
                        let value = #load_inner;
                        ::std::result::Result::Ok(value)
                    })?
                }
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                let items = items_ty_idx
                    .iter()
                    .map(|ty_idx| self.load(*ty_idx, slice, None))
                    .collect::<Result<Vec<_>, _>>()?;
                quote! { (#(#items,)*) }
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let key_bits = self.dictionary_key_bits(*key_ty_idx)?;
                let key_slice = format_ident!("key_slice");
                let value_slice = format_ident!("value_slice");
                let load_key = self.load(*key_ty_idx, &key_slice, None)?;
                let load_value = self.load(*value_ty_idx, &value_slice, None)?;
                quote! {
                    ::acton_client::cell::load_dictionary::<#key_bits, _, _>(
                        #slice,
                        |#key_slice| {
                            let value = #load_key;
                            ::std::result::Result::Ok(value)
                        },
                        |#value_slice| {
                            let value = #load_value;
                            ::std::result::Result::Ok(value)
                        },
                    )?
                }
            }
            Ty::EnumRef { enum_name } if !expand_named => {
                let name = type_ident(enum_name)?;
                quote! { <#name as ::acton_client::AbiLoad>::load_from(#slice)? }
            }
            Ty::EnumRef { enum_name } => {
                let declaration = self.symbols.declaration(enum_name)?;
                let ABIDeclaration::Enum {
                    encoded_as_ty_idx,
                    custom_pack_unpack,
                    ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                let name = type_ident(enum_name)?;
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.unpack_from_slice)
                    .unwrap_or(false)
                {
                    quote! { ::acton_client::cell::custom_load::<#name>(#enum_name, #slice)? }
                } else {
                    let value = self.load(*encoded_as_ty_idx, slice, None)?;
                    quote! { #name(#value) }
                }
            }
            Ty::StructRef {
                struct_name,
                type_args_ty_idx: None,
            } if !expand_named => {
                let name = type_ident(struct_name)?;
                quote! { <#name as ::acton_client::AbiLoad>::load_from(#slice)? }
            }
            Ty::StructRef { struct_name, .. } => {
                let declaration = self.symbols.declaration(struct_name)?;
                let ABIDeclaration::Struct {
                    prefix,
                    custom_pack_unpack,
                    ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                let struct_ident = type_ident(struct_name)?;
                let ty = self.types.cell_type(ty_idx)?;
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.unpack_from_slice)
                    .unwrap_or(false)
                {
                    quote! { ::acton_client::cell::custom_load::<#ty>(#struct_name, #slice)? }
                } else {
                    let prefix = prefix
                        .as_ref()
                        .map(|prefix| {
                            load_prefix(prefix.prefix_num, prefix.prefix_len, slice, struct_name)
                        })
                        .transpose()?
                        .unwrap_or_default();
                    let fields = self.symbols.struct_fields(ty_idx, false)?;
                    let names = fields
                        .iter()
                        .map(|field| value_ident(&field.field.name))
                        .collect::<Result<Vec<_>, _>>()?;
                    let loads = fields
                        .iter()
                        .map(|field| {
                            let field_path = format!("{struct_name}.{}", field.field.name);
                            self.load_at_field_path(
                                field.field.ty_idx,
                                slice,
                                field.u_label_ty_idx,
                                &field_path,
                            )
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    quote! {
                        {
                            #prefix
                            #struct_ident { #(#names: #loads),* }
                        }
                    }
                }
            }
            Ty::AliasRef {
                alias_name,
                type_args_ty_idx: None,
            } if !expand_named => {
                let load_name = format_ident!("load_{}", value_ident(alias_name)?);
                quote! { #load_name(#slice)? }
            }
            Ty::AliasRef { alias_name, .. } => {
                let declaration = self.symbols.declaration(alias_name)?;
                let ABIDeclaration::Alias {
                    custom_pack_unpack, ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                let ty = self.types.cell_type(ty_idx)?;
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.unpack_from_slice)
                    .unwrap_or(false)
                {
                    quote! { ::acton_client::cell::custom_load::<#ty>(#alias_name, #slice)? }
                } else {
                    let target = self.symbols.alias_target(ty_idx)?;
                    self.load(target.ty_idx, slice, target.u_label_ty_idx)?
                }
            }
            Ty::GenericT { name_t } => unsupported_load(&format!("unresolved generic {name_t}")),
            Ty::Union { variants, .. } => {
                self.load_union(ty_idx, variants, slice, u_label_ty_idx, None)?
            }
        })
    }

    fn load_at_field_path(
        &self,
        ty_idx: TyIdx,
        slice: &Ident,
        u_label_ty_idx: Option<TyIdx>,
        field_path: &str,
    ) -> Result<TokenStream, CodegenError> {
        match self.symbols.ty(ty_idx)? {
            Ty::Union { variants, .. } => {
                self.load_union(ty_idx, variants, slice, u_label_ty_idx, Some(field_path))
            }
            Ty::AliasRef { alias_name, .. } => {
                let declaration = self.symbols.declaration(alias_name)?;
                let ABIDeclaration::Alias {
                    custom_pack_unpack, ..
                } = declaration
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                if custom_pack_unpack
                    .as_ref()
                    .and_then(|custom| custom.unpack_from_slice)
                    .unwrap_or(false)
                {
                    self.load_impl(ty_idx, slice, u_label_ty_idx, true)
                } else {
                    let target = self.symbols.alias_target(ty_idx)?;
                    self.load_at_field_path(target.ty_idx, slice, target.u_label_ty_idx, field_path)
                }
            }
            _ => self.load(ty_idx, slice, u_label_ty_idx),
        }
    }

    fn load_union(
        &self,
        ty_idx: TyIdx,
        variants: &[tolk_source_map::types_kernel::UnionVariant],
        slice: &Ident,
        u_label_ty_idx: Option<TyIdx>,
        field_path: Option<&str>,
    ) -> Result<TokenStream, CodegenError> {
        let variants = self.symbols.union_variants(variants, u_label_ty_idx)?;
        let union_name = union_ident(self.symbols.union_identity(ty_idx, u_label_ty_idx));
        let label = Lifetime::new(
            &format!("'union_value_{ty_idx}_{}", u_label_ty_idx.unwrap_or(ty_idx)),
            proc_macro2::Span::call_site(),
        );
        let has_void = variants
            .last()
            .is_some_and(|variant| matches!(self.symbols.ty(variant.variant_ty_idx), Ok(Ty::Void)));

        if has_void && variants.len() == 2 {
            let value_variant = &variants[0];
            let load = self.load(value_variant.variant_ty_idx, slice, None)?;
            let skip = if value_variant.is_prefix_implicit {
                let len = prefix_len(value_variant.prefix_len)?;
                quote! { #slice.skip_first(#len, 0)?; }
            } else {
                TokenStream::new()
            };
            return Ok(quote! {
                if #slice.size_bits() == 0 && #slice.size_refs() == 0 {
                    #union_name::Variant1(())
                } else {
                    #skip
                    #union_name::Variant0(#load)
                }
            });
        }

        let dispatch_len = variants.len() - usize::from(has_void);
        let mut branches = TokenStream::new();
        for (index, variant) in variants.iter().take(dispatch_len).enumerate() {
            let variant_name = format_ident!("Variant{index}");
            let prefix_len = prefix_len(variant.prefix_len)?;
            let prefix_num = variant.prefix_num;
            let skip = variant.is_prefix_implicit.then(|| {
                quote! { #slice.skip_first(#prefix_len, 0)?; }
            });
            let load = self.load(variant.variant_ty_idx, slice, None)?;
            branches.extend(quote! {
                if ::acton_client::cell::matches_prefix(#slice, #prefix_num, #prefix_len)? {
                    #skip
                    break #label #union_name::#variant_name(#load);
                }
            });
        }
        let trailing_void = has_void.then(|| {
            let index = variants.len() - 1;
            let variant_name = format_ident!("Variant{index}");
            quote! {
                if #slice.size_bits() == 0 && #slice.size_refs() == 0 {
                    break #label #union_name::#variant_name(());
                }
            }
        });
        let no_match = field_path.map_or_else(
            || "none of union prefixes matched".to_owned(),
            |path| format!("Incorrect prefix for '{path}': none of variants matched"),
        );
        Ok(quote! {
            #label: {
                #branches
                #trailing_void
                break #label (::acton_client::cell::invalid_data(
                    #no_match,
                )?);
            }
        })
    }

    fn dictionary_key_bits(&self, ty_idx: TyIdx) -> Result<u16, CodegenError> {
        match self.symbols.ty(ty_idx)? {
            Ty::IntN { n } | Ty::UintN { n } => bits(*n),
            Ty::Address => Ok(267),
            _ => Err(crate::generate_error(format!(
                "map key type at index {ty_idx} is not supported; expected intN, uintN, or address"
            ))),
        }
    }

    fn find_unsupported_declaration(
        &self,
        ty_idx: TyIdx,
        direction: CellDirection,
    ) -> Result<Option<UnsupportedCellType>, CodegenError> {
        let subject = match self.symbols.ty(ty_idx)? {
            Ty::EnumRef { enum_name } => enum_name.clone(),
            Ty::StructRef { struct_name, .. } => struct_name.clone(),
            Ty::AliasRef { alias_name, .. } => alias_name.clone(),
            _ => return Ok(None),
        };
        let field_path = match direction {
            CellDirection::Store => "self".to_owned(),
            CellDirection::Load => subject.clone(),
        };
        self.find_unsupported_type(ty_idx, direction, &subject, &field_path, &mut Vec::new())
    }

    fn find_unsupported_type(
        &self,
        ty_idx: TyIdx,
        direction: CellDirection,
        subject: &str,
        field_path: &str,
        visiting: &mut Vec<TyIdx>,
    ) -> Result<Option<UnsupportedCellType>, CodegenError> {
        let unsupported = |type_name: &str| {
            Ok(Some(UnsupportedCellType {
                subject: subject.to_owned(),
                field_path: field_path.to_owned(),
                type_name: type_name.to_owned(),
            }))
        };

        match self.symbols.ty(ty_idx)? {
            Ty::Int => unsupported("int"),
            Ty::Builder if matches!(direction, CellDirection::Load) => unsupported("builder"),
            Ty::Slice if matches!(direction, CellDirection::Load) => unsupported("slice"),
            Ty::Callable => unsupported("continuation"),
            Ty::Unknown => unsupported("unknown"),
            Ty::GenericT { name_t } => unsupported(&format!("unresolved generic {name_t}")),
            Ty::Nullable { inner_ty_idx, .. } => {
                self.find_unsupported_type(*inner_ty_idx, direction, subject, field_path, visiting)
            }
            Ty::CellOf { inner_ty_idx } => self.find_unsupported_type(
                *inner_ty_idx,
                direction,
                subject,
                &format!("{field_path}.ref"),
                visiting,
            ),
            Ty::ArrayOf { inner_ty_idx } | Ty::LispListOf { inner_ty_idx } => self
                .find_unsupported_type(
                    *inner_ty_idx,
                    direction,
                    subject,
                    &format!("{field_path}[ith]"),
                    visiting,
                ),
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                for (index, item_ty_idx) in items_ty_idx.iter().enumerate() {
                    if let Some(unsupported) = self.find_unsupported_type(
                        *item_ty_idx,
                        direction,
                        subject,
                        &format!("{field_path}[{index}]"),
                        visiting,
                    )? {
                        return Ok(Some(unsupported));
                    }
                }
                Ok(None)
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                if let Some(unsupported) = self.find_unsupported_type(
                    *key_ty_idx,
                    direction,
                    subject,
                    &format!("{field_path}[key]"),
                    visiting,
                )? {
                    return Ok(Some(unsupported));
                }
                self.find_unsupported_type(
                    *value_ty_idx,
                    direction,
                    subject,
                    &format!("{field_path}[value]"),
                    visiting,
                )
            }
            Ty::EnumRef { enum_name } => {
                if visiting.contains(&ty_idx) {
                    return Ok(None);
                }
                let ABIDeclaration::Enum {
                    encoded_as_ty_idx,
                    custom_pack_unpack,
                    ..
                } = self.symbols.declaration(enum_name)?
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                let has_custom_codec = match direction {
                    CellDirection::Store => custom_pack_unpack
                        .as_ref()
                        .and_then(|custom| custom.pack_to_builder),
                    CellDirection::Load => custom_pack_unpack
                        .as_ref()
                        .and_then(|custom| custom.unpack_from_slice),
                }
                .unwrap_or(false);
                if has_custom_codec {
                    return Ok(None);
                }

                visiting.push(ty_idx);
                let result = self.find_unsupported_type(
                    *encoded_as_ty_idx,
                    direction,
                    enum_name,
                    match direction {
                        CellDirection::Store => "self",
                        CellDirection::Load => enum_name,
                    },
                    visiting,
                );
                visiting.pop();
                result
            }
            Ty::StructRef { struct_name, .. } => {
                if visiting.contains(&ty_idx) {
                    return Ok(None);
                }
                let ABIDeclaration::Struct {
                    custom_pack_unpack, ..
                } = self.symbols.declaration(struct_name)?
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                let has_custom_codec = match direction {
                    CellDirection::Store => custom_pack_unpack
                        .as_ref()
                        .and_then(|custom| custom.pack_to_builder),
                    CellDirection::Load => custom_pack_unpack
                        .as_ref()
                        .and_then(|custom| custom.unpack_from_slice),
                }
                .unwrap_or(false);
                if has_custom_codec {
                    return Ok(None);
                }

                visiting.push(ty_idx);
                let root = match direction {
                    CellDirection::Store => "self".to_owned(),
                    CellDirection::Load => struct_name.clone(),
                };
                let mut result = Ok(None);
                for field in self.symbols.struct_fields(ty_idx, false)? {
                    result = self.find_unsupported_type(
                        field.field.ty_idx,
                        direction,
                        struct_name,
                        &format!("{root}.{}", field.field.name),
                        visiting,
                    );
                    if matches!(result, Ok(Some(_)) | Err(_)) {
                        break;
                    }
                }
                visiting.pop();
                result
            }
            Ty::AliasRef { alias_name, .. } => {
                if visiting.contains(&ty_idx) {
                    return Ok(None);
                }
                let ABIDeclaration::Alias {
                    custom_pack_unpack, ..
                } = self.symbols.declaration(alias_name)?
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                let has_custom_codec = match direction {
                    CellDirection::Store => custom_pack_unpack
                        .as_ref()
                        .and_then(|custom| custom.pack_to_builder),
                    CellDirection::Load => custom_pack_unpack
                        .as_ref()
                        .and_then(|custom| custom.unpack_from_slice),
                }
                .unwrap_or(false);
                if has_custom_codec {
                    return Ok(None);
                }

                let target = self.symbols.alias_target(ty_idx)?;
                visiting.push(ty_idx);
                let result = self.find_unsupported_type(
                    target.ty_idx,
                    direction,
                    alias_name,
                    match direction {
                        CellDirection::Store => "self",
                        CellDirection::Load => alias_name,
                    },
                    visiting,
                );
                visiting.pop();
                result
            }
            Ty::Union { variants, .. } => {
                for variant in variants {
                    if let Some(unsupported) = self.find_unsupported_type(
                        variant.variant_ty_idx,
                        direction,
                        subject,
                        field_path,
                        visiting,
                    )? {
                        return Ok(Some(unsupported));
                    }
                }
                Ok(None)
            }
            Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins
            | Ty::Bool
            | Ty::Cell
            | Ty::Builder
            | Ty::String
            | Ty::Slice
            | Ty::Remaining
            | Ty::Address
            | Ty::AddressOpt
            | Ty::AddressExt
            | Ty::AddressAny
            | Ty::BitsN { .. }
            | Ty::NullLiteral
            | Ty::Void => Ok(None),
        }
    }
}

fn bits(bits: u32) -> Result<u16, CodegenError> {
    u16::try_from(bits).map_err(|_| crate::generate_error(format!("bit width {bits} exceeds u16")))
}

fn prefix_len(bits: usize) -> Result<u16, CodegenError> {
    u16::try_from(bits)
        .map_err(|_| crate::generate_error(format!("prefix width {bits} exceeds u16")))
}

fn varint_len_bits(n: u32) -> Result<u16, CodegenError> {
    if !n.is_power_of_two() {
        return Err(crate::generate_error(format!(
            "varintN size {n} is not a power of two"
        )));
    }
    bits(n.ilog2())
}

fn store_prefix(
    prefix_num: u64,
    prefix_len: i32,
    builder: &Ident,
) -> Result<TokenStream, CodegenError> {
    let prefix_len = u16::try_from(prefix_len).map_err(|_| {
        crate::generate_error(format!("invalid serialization prefix width {prefix_len}"))
    })?;
    Ok(quote! { #builder.store_uint(#prefix_num, #prefix_len)?; })
}

fn load_prefix(
    prefix_num: u64,
    prefix_len: i32,
    slice: &Ident,
    type_name: &str,
) -> Result<TokenStream, CodegenError> {
    let prefix_len = u16::try_from(prefix_len).map_err(|_| {
        crate::generate_error(format!("invalid serialization prefix width {prefix_len}"))
    })?;
    Ok(quote! {
        ::acton_client::cell::check_prefix(#slice, #prefix_num, #prefix_len, #type_name)?;
    })
}

fn unsupported_store(type_name: &str) -> TokenStream {
    let message = format!("type `{type_name}` is not cell-serializable");
    unsupported_store_message(&message)
}

fn unsupported_store_message(message: &str) -> TokenStream {
    quote! {
        {
            ::acton_client::cell::unsupported::<()>(#message)?;
        }
    }
}

fn unsupported_load(type_name: &str) -> TokenStream {
    let message = format!("type `{type_name}` is not cell-deserializable");
    unsupported_load_message(&message)
}

fn unsupported_load_message(message: &str) -> TokenStream {
    quote! {
        {
            ::acton_client::cell::unsupported(#message)?
        }
    }
}

fn unsupported_declaration_store(unsupported: &UnsupportedCellType) -> TokenStream {
    unsupported_store_message(&format!(
        "Can't pack '{}' to cell, because '{}' is '{}'",
        unsupported.subject, unsupported.field_path, unsupported.type_name
    ))
}

fn unsupported_declaration_load(unsupported: &UnsupportedCellType) -> TokenStream {
    unsupported_load_message(&format!(
        "Can't unpack '{}' from cell, because '{}' is '{}'",
        unsupported.subject, unsupported.field_path, unsupported.type_name
    ))
}
