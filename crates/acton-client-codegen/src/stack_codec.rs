use crate::CodegenError;
use crate::cell_codec::CellCodec;
use crate::names::{stack_struct_ident, type_ident, union_ident, value_ident};
use crate::rust_types::RustTypes;
use crate::symbols::Symbols;
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeSet;
use syn::Index;
use tolk_source_map::abi::ContractABI;
use tolk_source_map::types_kernel::{Ty, TyIdx};

pub(crate) struct StackCodec<'abi> {
    symbols: Symbols<'abi>,
    types: RustTypes<'abi>,
    cells: CellCodec<'abi>,
}

impl<'abi> StackCodec<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self {
            symbols: Symbols::new(abi),
            types: RustTypes::new(abi),
            cells: CellCodec::new(abi),
        }
    }

    pub(crate) fn store_at_path(
        &self,
        ty_idx: TyIdx,
        value: TokenStream,
        output: &Ident,
        tuple_if_wide: bool,
        u_label_ty_idx: Option<TyIdx>,
        field_path: &str,
    ) -> Result<TokenStream, CodegenError> {
        self.validate_supported_on_stack(ty_idx, field_path)?;
        self.store(ty_idx, value, output, tuple_if_wide, u_label_ty_idx)
    }

    pub(crate) fn load_at_path(
        &self,
        ty_idx: TyIdx,
        reader: &Ident,
        untuple_if_wide: bool,
        u_label_ty_idx: Option<TyIdx>,
        field_path: &str,
    ) -> Result<TokenStream, CodegenError> {
        self.validate_supported_on_stack(ty_idx, field_path)?;
        self.load(ty_idx, reader, untuple_if_wide, u_label_ty_idx)
    }

    fn validate_supported_on_stack(
        &self,
        ty_idx: TyIdx,
        field_path: &str,
    ) -> Result<(), CodegenError> {
        self.validate_supported_on_stack_inner(ty_idx, field_path, &mut BTreeSet::new())
    }

    fn validate_supported_on_stack_inner(
        &self,
        ty_idx: TyIdx,
        field_path: &str,
        visiting: &mut BTreeSet<TyIdx>,
    ) -> Result<(), CodegenError> {
        if !visiting.insert(ty_idx) {
            return Ok(());
        }

        let result = match self.symbols.ty(ty_idx)? {
            Ty::Callable => Err(crate::generate_error(format!(
                "[NotSupportedTypeOnStack] '{field_path}' can not be used in get methods, because it contains 'continuation'"
            ))),
            Ty::Nullable { inner_ty_idx, .. }
            | Ty::ArrayOf { inner_ty_idx }
            | Ty::LispListOf { inner_ty_idx } => {
                self.validate_supported_on_stack_inner(*inner_ty_idx, field_path, visiting)
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => items_ty_idx
                .iter()
                .enumerate()
                .try_for_each(|(index, ty_idx)| {
                    self.validate_supported_on_stack_inner(
                        *ty_idx,
                        &format!("{field_path}[{index}]"),
                        visiting,
                    )
                }),
            Ty::StructRef { .. } => self
                .symbols
                .struct_fields(ty_idx, true)?
                .into_iter()
                .try_for_each(|field| {
                    self.validate_supported_on_stack_inner(
                        field.field.ty_idx,
                        &format!("{field_path}.{}", field.field.name),
                        visiting,
                    )
                }),
            Ty::AliasRef { .. } => self.validate_supported_on_stack_inner(
                self.symbols.alias_target(ty_idx)?.ty_idx,
                field_path,
                visiting,
            ),
            Ty::Union { variants, .. } => {
                variants
                    .iter()
                    .enumerate()
                    .try_for_each(|(index, variant)| {
                        let type_id = variant.stack_type_id.unwrap_or(index);
                        self.validate_supported_on_stack_inner(
                            variant.variant_ty_idx,
                            &format!("{field_path}#{type_id}"),
                            visiting,
                        )
                    })
            }
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins
            | Ty::Bool
            | Ty::Cell
            | Ty::Builder
            | Ty::Slice
            | Ty::String
            | Ty::Remaining
            | Ty::Address
            | Ty::AddressOpt
            | Ty::AddressExt
            | Ty::AddressAny
            | Ty::BitsN { .. }
            | Ty::NullLiteral
            | Ty::CellOf { .. }
            | Ty::MapKV { .. }
            | Ty::EnumRef { .. }
            | Ty::Void
            | Ty::Unknown
            | Ty::GenericT { .. } => Ok(()),
        };

        visiting.remove(&ty_idx);
        result
    }

    pub(crate) fn store(
        &self,
        ty_idx: TyIdx,
        value: TokenStream,
        output: &Ident,
        tuple_if_wide: bool,
        u_label_ty_idx: Option<TyIdx>,
    ) -> Result<TokenStream, CodegenError> {
        let width = self.symbols.stack_width(ty_idx)?;
        if tuple_if_wide && width != 1 {
            let nested = format_ident!("nested");
            let store = self.store(ty_idx, value, &nested, false, u_label_ty_idx)?;
            return Ok(quote! {
                ::acton_client::stack::write_tuple(#output, |#nested| {
                    #store
                    ::std::result::Result::Ok(())
                })?;
            });
        }

        Ok(match self.symbols.ty(ty_idx)? {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => quote! { ::acton_client::stack::write_int(#value, #output); },
            Ty::Bool => quote! { ::acton_client::stack::write_bool(*#value, #output); },
            Ty::Cell => quote! { ::acton_client::stack::write_cell(#value, #output); },
            Ty::Builder => quote! {
                #output.push(::acton_client::TupleItem::Builder(
                    <::acton_client::__private::tycho_types::cell::CellBuilder as ::std::clone::Clone>::clone(#value).build()?
                ));
            },
            Ty::Slice | Ty::Remaining => {
                quote! { ::acton_client::stack::write_slice(#value, #output)?; }
            }
            Ty::String => quote! { ::acton_client::stack::write_string(#value, #output); },
            Ty::Address | Ty::AddressExt | Ty::AddressAny => {
                quote! { ::acton_client::stack::write_tlb_slice(#value, #output)?; }
            }
            Ty::AddressOpt => quote! {
                if let ::std::option::Option::Some(value) = (#value).as_ref() {
                    ::acton_client::stack::write_tlb_slice(value, #output)?;
                } else {
                    #output.push(::acton_client::TupleItem::Null);
                }
            },
            Ty::BitsN { .. } => quote! { ::acton_client::stack::write_bits(#value, #output)?; },
            Ty::NullLiteral => quote! { #output.push(::acton_client::TupleItem::Null); },
            Ty::Callable => {
                return Err(crate::generate_error(
                    "continuation cannot be used in contract get methods",
                ));
            }
            Ty::Void => TokenStream::new(),
            Ty::Unknown => quote! { #output.push(#value.clone()); },
            Ty::Nullable {
                inner_ty_idx,
                stack_type_id,
                stack_width,
            } => {
                if let Some(stack_type_id) = stack_type_id {
                    let stack_width = stack_width
                        .ok_or_else(|| crate::generate_error("wide nullable has no stack_width"))?;
                    let nested = format_ident!("nested");
                    let store_inner =
                        self.store(*inner_ty_idx, quote! { inner }, &nested, false, None)?;
                    quote! {
                        ::acton_client::stack::write_wide_nullable(
                            (#value).as_ref(),
                            #stack_width,
                            #stack_type_id,
                            #output,
                            |inner, #nested| {
                                #store_inner
                                ::std::result::Result::Ok(())
                            },
                        )?;
                    }
                } else {
                    let store_inner =
                        self.store(*inner_ty_idx, quote! { inner }, output, false, None)?;
                    quote! {
                        if let ::std::option::Option::Some(inner) = (#value).as_ref() {
                            #store_inner
                        } else {
                            #output.push(::acton_client::TupleItem::Null);
                        }
                    }
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let builder = format_ident!("builder_{ty_idx}");
                let owned_builder = format_ident!("owned_builder_{ty_idx}");
                let store = self.cells.store(
                    *inner_ty_idx,
                    quote! { (#value).r#ref.as_ref() },
                    &builder,
                    None,
                )?;
                quote! {
                    {
                        let mut #owned_builder = ::acton_client::__private::tycho_types::cell::CellBuilder::new();
                        let #builder = &mut #owned_builder;
                        #store
                        #output.push(::acton_client::TupleItem::Cell(#owned_builder.build()?));
                    }
                }
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let nested = format_ident!("nested");
                let is_wide = self.symbols.stack_width(*inner_ty_idx)? != 1;
                let store = self.store(*inner_ty_idx, quote! { item }, &nested, false, None)?;
                quote! {
                    ::acton_client::stack::write_array(
                        #value,
                        #output,
                        |item, #nested| {
                            #store
                            ::std::result::Result::Ok(())
                        },
                        #is_wide,
                    )?;
                }
            }
            Ty::LispListOf { inner_ty_idx } => {
                let nested = format_ident!("nested");
                let store = self.store(*inner_ty_idx, quote! { item }, &nested, false, None)?;
                quote! {
                    ::acton_client::stack::write_lisp_list(
                        #value,
                        #output,
                        |item, #nested| {
                            #store
                            ::std::result::Result::Ok(())
                        },
                    )?;
                }
            }
            Ty::Tensor { items_ty_idx } => {
                let stores = items_ty_idx
                    .iter()
                    .enumerate()
                    .map(|(index, ty_idx)| {
                        let index = Index::from(index);
                        self.store(*ty_idx, quote! { &(#value).#index }, output, false, None)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                quote! { #(#stores)* }
            }
            Ty::ShapedTuple { items_ty_idx } => {
                let nested = format_ident!("nested");
                let stores = items_ty_idx
                    .iter()
                    .enumerate()
                    .map(|(index, ty_idx)| {
                        let index = Index::from(index);
                        self.store(*ty_idx, quote! { &(#value).#index }, &nested, true, None)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                quote! {
                    ::acton_client::stack::write_tuple(#output, |#nested| {
                        #(#stores)*
                        ::std::result::Result::Ok(())
                    })?;
                }
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let key_bits = self.dictionary_key_bits(*key_ty_idx)?;
                let key_builder = format_ident!("key_builder");
                let value_builder = format_ident!("value_builder");
                let store_key =
                    self.cells
                        .store(*key_ty_idx, quote! { key }, &key_builder, None)?;
                let store_value =
                    self.cells
                        .store(*value_ty_idx, quote! { value }, &value_builder, None)?;
                quote! {
                    if #value.is_empty() {
                        #output.push(::acton_client::TupleItem::Null);
                    } else {
                        let root = ::acton_client::cell::build_dictionary_root::<#key_bits, _, _>(
                            #value,
                            |key, #key_builder| {
                                #store_key
                                ::std::result::Result::Ok(())
                            },
                            |value, #value_builder| {
                                #store_value
                                ::std::result::Result::Ok(())
                            },
                        )?.expect("non-empty dictionary has a root");
                        #output.push(::acton_client::TupleItem::Cell(root));
                    }
                }
            }
            Ty::EnumRef { .. } => quote! { ::acton_client::stack::write_int(&#value.0, #output); },
            Ty::StructRef { .. } => {
                let stores = self
                    .symbols
                    .struct_fields(ty_idx, true)?
                    .into_iter()
                    .map(|field| {
                        let name = value_ident(&field.field.name)?;
                        self.store(
                            field.field.ty_idx,
                            quote! { &(#value).#name },
                            output,
                            false,
                            field.u_label_ty_idx,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                quote! { #(#stores)* }
            }
            Ty::AliasRef { .. } => {
                let target = self.symbols.alias_target(ty_idx)?;
                self.store(
                    target.ty_idx,
                    value,
                    output,
                    tuple_if_wide,
                    target.u_label_ty_idx,
                )?
            }
            Ty::GenericT { name_t } => unsupported(&format!("unresolved generic {name_t}")),
            Ty::Union {
                variants,
                stack_width,
            } => {
                let total_width = stack_width.ok_or_else(|| {
                    crate::generate_error(format!(
                        "union at type index {ty_idx} has no stack_width"
                    ))
                })?;
                let union_name = union_ident(self.symbols.union_identity(ty_idx, u_label_ty_idx));
                let variants = self.symbols.union_variants(variants, u_label_ty_idx)?;
                let arms = variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        let variant_name = format_ident!("Variant{index}");
                        let type_id = variant.stack_type_id.ok_or_else(|| {
                            crate::generate_error("concrete union variant has no stack_type_id")
                        })?;
                        let variant_width = variant.stack_width.ok_or_else(|| {
                            crate::generate_error("concrete union variant has no stack_width")
                        })?;
                        let nested = format_ident!("nested");
                        let store = self.store(
                            variant.variant_ty_idx,
                            quote! { inner },
                            &nested,
                            false,
                            None,
                        )?;
                        Ok(quote! {
                            #union_name::#variant_name(inner) => {
                                ::acton_client::stack::write_union_variant(
                                    inner,
                                    #total_width,
                                    #variant_width,
                                    #type_id,
                                    #output,
                                    |inner, #nested| {
                                        #store
                                        ::std::result::Result::Ok(())
                                    },
                                )?;
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
        reader: &Ident,
        untuple_if_wide: bool,
        u_label_ty_idx: Option<TyIdx>,
    ) -> Result<TokenStream, CodegenError> {
        let width = self.symbols.stack_width(ty_idx)?;
        if untuple_if_wide && width != 1 {
            let nested = format_ident!("nested_{ty_idx}");
            let owned_nested = format_ident!("owned_nested_{ty_idx}");
            let load = self.load(ty_idx, &nested, false, u_label_ty_idx)?;
            return Ok(quote! {
                {
                    let mut #owned_nested = #reader.read_tuple(::std::option::Option::Some(#width))?;
                    let #nested = &mut #owned_nested;
                    let value = #load;
                    #nested.ensure_empty()?;
                    value
                }
            });
        }

        Ok(match self.symbols.ty(ty_idx)? {
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => quote! { #reader.read_int()? },
            Ty::Bool => quote! { #reader.read_bool()? },
            Ty::Cell => quote! { #reader.read_cell()? },
            Ty::Builder => quote! { #reader.read_builder()? },
            Ty::Slice | Ty::Remaining => quote! { #reader.read_owned_slice()? },
            Ty::String => quote! { #reader.read_string()? },
            Ty::Address | Ty::AddressExt | Ty::AddressAny => {
                let ty = self.types.stack_type(ty_idx)?;
                quote! { ::acton_client::stack::read_tlb_slice::<#ty>(#reader)? }
            }
            Ty::AddressOpt => quote! {
                #reader.read_nullable(|reader| {
                    ::acton_client::stack::read_tlb_slice(reader)
                })?
            },
            Ty::BitsN { .. } => quote! { ::acton_client::BitString(#reader.read_owned_slice()?) },
            Ty::NullLiteral => quote! {
                match #reader.pop()? {
                    ::acton_client::TupleItem::Null => (),
                    _ => ::acton_client::stack::invalid_data("expected null on TVM stack")?,
                }
            },
            Ty::Callable => {
                return Err(crate::generate_error(
                    "continuation cannot be used in contract get methods",
                ));
            }
            Ty::Void => quote! { () },
            Ty::Unknown => quote! { #reader.pop()? },
            Ty::Nullable {
                inner_ty_idx,
                stack_type_id,
                stack_width,
            } => {
                let nested = format_ident!("nested");
                let load = self.load(*inner_ty_idx, &nested, false, None)?;
                if stack_type_id.is_some() {
                    let stack_width = stack_width
                        .ok_or_else(|| crate::generate_error("wide nullable has no stack_width"))?;
                    quote! {
                        #reader.read_wide_nullable(#stack_width, |#nested| {
                            let value = #load;
                            ::std::result::Result::Ok(value)
                        })?
                    }
                } else {
                    quote! {
                        #reader.read_nullable(|#nested| {
                            let value = #load;
                            ::std::result::Result::Ok(value)
                        })?
                    }
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let cell_slice = format_ident!("cell_slice_{ty_idx}");
                let owned_cell_slice = format_ident!("owned_cell_slice_{ty_idx}");
                let load = self.cells.load(*inner_ty_idx, &cell_slice, None)?;
                quote! {
                    {
                        let cell = #reader.read_cell()?;
                        let mut #owned_cell_slice = cell.as_slice()?;
                        let #cell_slice = &mut #owned_cell_slice;
                        ::acton_client::CellRef::new(#load)
                    }
                }
            }
            Ty::ArrayOf { inner_ty_idx } => {
                let nested = format_ident!("nested");
                let is_wide = self.symbols.stack_width(*inner_ty_idx)? != 1;
                let load = self.load(*inner_ty_idx, &nested, false, None)?;
                quote! {
                    ::acton_client::stack::read_array(
                        #reader,
                        |#nested| {
                            let value = #load;
                            ::std::result::Result::Ok(value)
                        },
                        #is_wide,
                    )?
                }
            }
            Ty::LispListOf { inner_ty_idx } => {
                let nested = format_ident!("nested");
                let is_wide = self.symbols.stack_width(*inner_ty_idx)? != 1;
                let load = self.load(*inner_ty_idx, &nested, false, None)?;
                quote! {
                    ::acton_client::stack::read_lisp_list(
                        #reader,
                        |#nested| {
                            let value = #load;
                            ::std::result::Result::Ok(value)
                        },
                        #is_wide,
                    )?
                }
            }
            Ty::Tensor { items_ty_idx } => {
                let values = items_ty_idx
                    .iter()
                    .map(|ty_idx| self.load(*ty_idx, reader, false, None))
                    .collect::<Result<Vec<_>, _>>()?;
                quote! { (#(#values,)*) }
            }
            Ty::ShapedTuple { items_ty_idx } => {
                let nested = format_ident!("nested_{ty_idx}");
                let owned_nested = format_ident!("owned_nested_{ty_idx}");
                let item_count = items_ty_idx.len();
                let values = items_ty_idx
                    .iter()
                    .map(|ty_idx| self.load(*ty_idx, &nested, true, None))
                    .collect::<Result<Vec<_>, _>>()?;
                quote! {
                    {
                        let mut #owned_nested = #reader.read_tuple(::std::option::Option::Some(#item_count))?;
                        let #nested = &mut #owned_nested;
                        let value = (#(#values,)*);
                        #nested.ensure_empty()?;
                        value
                    }
                }
            }
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                let key_bits = self.dictionary_key_bits(*key_ty_idx)?;
                let key_slice = format_ident!("key_slice");
                let value_slice = format_ident!("value_slice");
                let load_key = self.cells.load(*key_ty_idx, &key_slice, None)?;
                let load_value = self.cells.load(*value_ty_idx, &value_slice, None)?;
                quote! {
                    match #reader.pop()? {
                        ::acton_client::TupleItem::Null => ::acton_client::Dictionary::new(),
                        ::acton_client::TupleItem::Cell(root)
                        | ::acton_client::TupleItem::Slice(root) => {
                            ::acton_client::cell::load_dictionary_root::<#key_bits, _, _>(
                                ::std::option::Option::Some(&root),
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
                        _ => ::acton_client::stack::invalid_data(
                            "expected dictionary cell or null on TVM stack",
                        )?,
                    }
                }
            }
            Ty::EnumRef { enum_name } => {
                let name = type_ident(enum_name)?;
                quote! { #name(#reader.read_int()?) }
            }
            Ty::StructRef { struct_name, .. } => {
                let fields = self.symbols.struct_fields(ty_idx, true)?;
                let names = fields
                    .iter()
                    .map(|field| value_ident(&field.field.name))
                    .collect::<Result<Vec<_>, _>>()?;
                let values = fields
                    .iter()
                    .map(|field| self.load(field.field.ty_idx, reader, false, field.u_label_ty_idx))
                    .collect::<Result<Vec<_>, _>>()?;
                let has_client_type = fields
                    .iter()
                    .any(|field| field.field.client_ty_idx.is_some());
                let struct_ident = if has_client_type {
                    let name = stack_struct_ident(struct_name, ty_idx)?;
                    quote! { #name }
                } else {
                    let name = type_ident(struct_name)?;
                    quote! { #name }
                };
                quote! { #struct_ident { #(#names: #values),* } }
            }
            Ty::AliasRef { .. } => {
                let target = self.symbols.alias_target(ty_idx)?;
                self.load(
                    target.ty_idx,
                    reader,
                    untuple_if_wide,
                    target.u_label_ty_idx,
                )?
            }
            Ty::GenericT { name_t } => unsupported_load(&format!("unresolved generic {name_t}")),
            Ty::Union {
                variants,
                stack_width,
            } => {
                let total_width = stack_width.ok_or_else(|| {
                    crate::generate_error(format!(
                        "union at type index {ty_idx} has no stack_width"
                    ))
                })?;
                let union_name = union_ident(self.symbols.union_identity(ty_idx, u_label_ty_idx));
                let variants = self.symbols.union_variants(variants, u_label_ty_idx)?;
                let branches = variants
                    .iter()
                    .enumerate()
                    .map(|(index, variant)| {
                        let variant_name = format_ident!("Variant{index}");
                        let type_id = variant.stack_type_id.ok_or_else(|| {
                            crate::generate_error("concrete union variant has no stack_type_id")
                        })?;
                        let variant_width = variant.stack_width.ok_or_else(|| {
                            crate::generate_error("concrete union variant has no stack_width")
                        })?;
                        let load = self.load(variant.variant_ty_idx, reader, false, None)?;
                        Ok(quote! {
                            if type_id == ::acton_client::BigInt::from(#type_id) {
                                #reader.prepare_union_variant(#total_width, #variant_width)?;
                                let value = #load;
                                #reader.finish_union_variant()?;
                                return ::std::result::Result::Ok(#union_name::#variant_name(value));
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, CodegenError>>()?;
                quote! {
                    {
                        let type_id = #reader.read_union_tag(#total_width)?;
                        let mut load_variant = || {
                            #(#branches)*
                            ::std::result::Result::Err(::acton_client::AbiError::InvalidData(
                                format!("unexpected union type id {type_id}"),
                            ))
                        };
                        load_variant()?
                    }
                }
            }
        })
    }

    fn dictionary_key_bits(&self, ty_idx: TyIdx) -> Result<u16, CodegenError> {
        match self.symbols.ty(ty_idx)? {
            Ty::IntN { n } | Ty::UintN { n } => u16::try_from(*n)
                .map_err(|_| crate::generate_error(format!("bit width {n} exceeds u16"))),
            Ty::Address => Ok(267),
            _ => Err(crate::generate_error(format!(
                "map key type at index {ty_idx} is not supported; expected intN, uintN, or address"
            ))),
        }
    }
}

fn unsupported(message: &str) -> TokenStream {
    quote! {
        {
            ::acton_client::stack::unsupported::<()>(#message)?;
        }
    }
}

fn unsupported_load(message: &str) -> TokenStream {
    quote! {
        ::acton_client::stack::unsupported(#message)?
    }
}
