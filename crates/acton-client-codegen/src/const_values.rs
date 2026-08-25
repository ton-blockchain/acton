use crate::names::{type_ident, union_ident, value_ident};
use crate::symbols::Symbols;
use crate::{CodegenError, generate_error};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use tolk_source_map::abi::{ABIConstValue, ContractABI};
use tolk_source_map::types_kernel::{Ty, TyIdx};

/// Emits Rust expressions for constant values embedded in a Tolk ABI.
///
/// Upstream uses the same field-default emitter for struct constructors and
/// getter parameter defaults. Keeping that stage shared is important: both
/// APIs must interpret ABI constants in exactly the same way.
pub(crate) struct ConstValueEmitter<'abi> {
    symbols: Symbols<'abi>,
}

impl<'abi> ConstValueEmitter<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self {
            symbols: Symbols::new(abi),
        }
    }

    pub(crate) fn is_supported(&self, ty_idx: TyIdx) -> Result<bool, CodegenError> {
        Ok(match self.symbols.ty(ty_idx)? {
            Ty::ArrayOf { inner_ty_idx } | Ty::LispListOf { inner_ty_idx } => {
                self.is_supported(*inner_ty_idx)?
            }
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                for item_ty_idx in items_ty_idx {
                    if !self.is_supported(*item_ty_idx)? {
                        return Ok(false);
                    }
                }
                true
            }
            Ty::Union { .. } | Ty::MapKV { .. } => false,
            _ => true,
        })
    }

    pub(crate) fn emit(
        &self,
        value: &ABIConstValue,
        ty_idx: TyIdx,
    ) -> Result<TokenStream, CodegenError> {
        let ty = self.symbols.ty(ty_idx)?;
        match ty {
            Ty::AliasRef { .. } => {
                let target = self.symbols.alias_target(ty_idx)?;
                return self.emit(value, target.ty_idx);
            }
            Ty::Union { variants, .. } => return self.emit_union(value, ty_idx, variants),
            _ => {}
        }

        if let ABIConstValue::CastTo { inner, .. } = value {
            // The cast records the Tolk expression's intermediate type. The
            // resolved ABI type is what the Rust expression must implement.
            return self.emit(inner, ty_idx);
        }

        match ty {
            Ty::AliasRef { .. } | Ty::Union { .. } => unreachable!("handled above"),
            Ty::Nullable { inner_ty_idx, .. } => {
                if matches!(value, ABIConstValue::Null) {
                    Ok(quote! { ::std::option::Option::None })
                } else {
                    let inner = self.emit(value, *inner_ty_idx)?;
                    Ok(quote! { ::std::option::Option::Some(#inner) })
                }
            }
            Ty::CellOf { inner_ty_idx } => {
                let inner = self.emit(value, *inner_ty_idx)?;
                Ok(quote! { ::acton_client::CellRef::new(#inner) })
            }
            Ty::Int
            | Ty::IntN { .. }
            | Ty::UintN { .. }
            | Ty::VarintN { .. }
            | Ty::VaruintN { .. }
            | Ty::Coins => self.emit_integer(value, None, ty_idx),
            Ty::EnumRef { enum_name } => {
                let enum_name = type_ident(enum_name)?;
                self.emit_integer(value, Some(&enum_name), ty_idx)
            }
            Ty::Bool => match value {
                ABIConstValue::Bool { v } => Ok(quote! { #v }),
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::Cell => match value {
                ABIConstValue::Slice { hex } => emit_raw_cell(hex),
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::Builder => match value {
                ABIConstValue::Slice { hex } => emit_raw_builder(hex),
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::Slice | Ty::Remaining => match value {
                ABIConstValue::Slice { hex } => {
                    let cell = emit_raw_cell(hex)?;
                    Ok(quote! { ::acton_client::OwnedSlice::full(#cell) })
                }
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::BitsN { .. } => match value {
                ABIConstValue::Slice { hex } => {
                    let cell = emit_raw_cell(hex)?;
                    Ok(quote! {
                        ::acton_client::BitString(::acton_client::OwnedSlice::full(#cell))
                    })
                }
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::String => match value {
                ABIConstValue::String { str } => Ok(quote! { ::std::string::String::from(#str) }),
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::Address => match value {
                ABIConstValue::Address { addr } => Ok(emit_std_address(addr)),
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::AddressOpt => match value {
                ABIConstValue::Null => Ok(quote! { ::std::option::Option::None }),
                ABIConstValue::Address { addr } => {
                    let address = emit_std_address(addr);
                    Ok(quote! { ::std::option::Option::Some(#address) })
                }
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::AddressAny => match value {
                ABIConstValue::Null => Ok(quote! {
                    ::acton_client::__private::tycho_types::models::AnyAddr::None
                }),
                ABIConstValue::Address { addr } => {
                    let address = emit_std_address(addr);
                    Ok(quote! {
                        ::acton_client::__private::tycho_types::models::AnyAddr::Std(#address)
                    })
                }
                _ => Err(self.type_mismatch(value, ty_idx)),
            },
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => {
                self.emit_tuple(value, items_ty_idx, ty_idx)
            }
            Ty::ArrayOf { inner_ty_idx } | Ty::LispListOf { inner_ty_idx } => {
                self.emit_vec(value, *inner_ty_idx, ty_idx)
            }
            Ty::StructRef { struct_name, .. } => self.emit_object(value, struct_name, ty_idx),
            Ty::NullLiteral | Ty::Void if matches!(value, ABIConstValue::Null) => Ok(quote! { () }),
            Ty::AddressExt
            | Ty::NullLiteral
            | Ty::Void
            | Ty::Callable
            | Ty::Unknown
            | Ty::GenericT { .. }
            | Ty::MapKV { .. } => Err(self.type_mismatch(value, ty_idx)),
        }
    }

    fn emit_union(
        &self,
        value: &ABIConstValue,
        ty_idx: TyIdx,
        variants: &[tolk_source_map::types_kernel::UnionVariant],
    ) -> Result<TokenStream, CodegenError> {
        let preferred_ty_idx = match value {
            ABIConstValue::CastTo { cast_to_ty_idx, .. } => Some(*cast_to_ty_idx),
            ABIConstValue::Null => {
                let mut null_variant = None;
                for variant in variants {
                    if self.is_null_literal(variant.variant_ty_idx)? {
                        null_variant = Some(variant.variant_ty_idx);
                        break;
                    }
                }
                null_variant
            }
            _ => None,
        };
        let selected = preferred_ty_idx
            .and_then(|preferred| {
                variants
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| variant.variant_ty_idx == preferred)
            })
            .or_else(|| {
                variants
                    .iter()
                    .enumerate()
                    .find(|(_, variant)| self.emit(value, variant.variant_ty_idx).is_ok())
            })
            .ok_or_else(|| self.type_mismatch(value, ty_idx))?;
        let (variant_index, variant) = selected;
        let inner_value = match value {
            ABIConstValue::CastTo { inner, .. } => inner.as_ref(),
            _ => value,
        };
        let inner = self.emit(inner_value, variant.variant_ty_idx)?;
        let union = union_ident(ty_idx);
        let variant = format_ident!("Variant{variant_index}");
        Ok(quote! { #union::#variant(#inner) })
    }

    fn is_null_literal(&self, ty_idx: TyIdx) -> Result<bool, CodegenError> {
        match self.symbols.ty(ty_idx)? {
            Ty::NullLiteral => Ok(true),
            Ty::AliasRef { .. } => {
                let target = self.symbols.alias_target(ty_idx)?;
                self.is_null_literal(target.ty_idx)
            }
            _ => Ok(false),
        }
    }

    fn emit_integer(
        &self,
        value: &ABIConstValue,
        wrapper: Option<&Ident>,
        ty_idx: TyIdx,
    ) -> Result<TokenStream, CodegenError> {
        let ABIConstValue::Int { v } = value else {
            return Err(self.type_mismatch(value, ty_idx));
        };
        let integer = quote! {
            <::acton_client::__private::num_bigint::BigInt as ::std::str::FromStr>::from_str(#v)
                .expect("Tolk ABI integer default must be valid")
        };
        Ok(wrapper.map_or_else(|| integer.clone(), |wrapper| quote! { #wrapper(#integer) }))
    }

    fn emit_tuple(
        &self,
        value: &ABIConstValue,
        item_types: &[TyIdx],
        ty_idx: TyIdx,
    ) -> Result<TokenStream, CodegenError> {
        let items = const_items(value).ok_or_else(|| self.type_mismatch(value, ty_idx))?;
        if items.len() != item_types.len() {
            return Err(generate_error(format!(
                "ABI default at type index {ty_idx} contains {} items, expected {}",
                items.len(),
                item_types.len()
            )));
        }
        let items = items
            .iter()
            .zip(item_types)
            .map(|(item, item_ty_idx)| self.emit(item, *item_ty_idx))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { (#(#items,)*) })
    }

    fn emit_vec(
        &self,
        value: &ABIConstValue,
        item_ty_idx: TyIdx,
        ty_idx: TyIdx,
    ) -> Result<TokenStream, CodegenError> {
        let items = const_items(value).ok_or_else(|| self.type_mismatch(value, ty_idx))?;
        let items = items
            .iter()
            .map(|item| self.emit(item, item_ty_idx))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { ::std::vec![#(#items),*] })
    }

    fn emit_object(
        &self,
        value: &ABIConstValue,
        expected_name: &str,
        ty_idx: TyIdx,
    ) -> Result<TokenStream, CodegenError> {
        let ABIConstValue::Object {
            struct_name,
            fields: values,
        } = value
        else {
            return Err(self.type_mismatch(value, ty_idx));
        };
        if struct_name != expected_name {
            return Err(generate_error(format!(
                "ABI default at type index {ty_idx} constructs `{struct_name}`, expected `{expected_name}`"
            )));
        }

        let fields = self.symbols.struct_fields(ty_idx, false)?;
        if values.len() != fields.len() {
            return Err(generate_error(format!(
                "ABI default for `{struct_name}` contains {} fields, expected {}",
                values.len(),
                fields.len()
            )));
        }
        let fields = fields
            .iter()
            .zip(values)
            .map(|(field, value)| {
                let name = value_ident(&field.field.name)?;
                let value = self.emit(value, field.field.ty_idx)?;
                Ok(quote! { #name: #value })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let struct_name = type_ident(struct_name)?;
        Ok(quote! {
            #struct_name {
                #(#fields),*
            }
        })
    }

    fn type_mismatch(&self, value: &ABIConstValue, ty_idx: TyIdx) -> CodegenError {
        generate_error(format!(
            "ABI constant {value:?} cannot initialize type index {ty_idx}"
        ))
    }
}

fn const_items(value: &ABIConstValue) -> Option<&[ABIConstValue]> {
    match value {
        ABIConstValue::Tensor { items } | ABIConstValue::ShapedTuple { items } => Some(items),
        _ => None,
    }
}

fn emit_std_address(address: &str) -> TokenStream {
    quote! {
        ::acton_client::__private::tycho_types::models::StdAddr::from_str_ext(
            #address,
            ::acton_client::__private::tycho_types::models::StdAddrFormat::any(),
        )
        .expect("Tolk ABI address default must be valid")
        .0
    }
}

fn emit_raw_builder(hex: &str) -> Result<TokenStream, CodegenError> {
    let (bytes, bit_len) = decode_hex_bits(hex)?;
    Ok(quote! {{
        let mut builder =
            ::acton_client::__private::tycho_types::cell::CellBuilder::new();
        builder
            .store_raw(&[#(#bytes),*], #bit_len)
            .expect("Tolk ABI slice default must fit into a cell");
        builder
    }})
}

fn emit_raw_cell(hex: &str) -> Result<TokenStream, CodegenError> {
    let builder = emit_raw_builder(hex)?;
    Ok(quote! {{
        let builder = #builder;
        builder
            .build()
            .expect("Tolk ABI slice default cell must build")
    }})
}

fn decode_hex_bits(hex: &str) -> Result<(Vec<u8>, u16), CodegenError> {
    let bit_len = hex
        .len()
        .checked_mul(4)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| generate_error("ABI slice default is too large"))?;
    let mut bytes = Vec::with_capacity(hex.len().div_ceil(2));
    for pair in hex.as_bytes().chunks(2) {
        let high = decode_hex_nibble(pair[0])
            .ok_or_else(|| generate_error(format!("invalid hex in ABI slice default `{hex}`")))?;
        let low = if pair.len() == 2 {
            decode_hex_nibble(pair[1]).ok_or_else(|| {
                generate_error(format!("invalid hex in ABI slice default `{hex}`"))
            })?
        } else {
            0
        };
        bytes.push((high << 4) | low);
    }
    Ok((bytes, bit_len))
}

const fn decode_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
