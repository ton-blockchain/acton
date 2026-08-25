use crate::cell_codec::CellCodec;
use crate::const_values::ConstValueEmitter;
use crate::names::{stack_struct_ident, type_ident, union_ident, value_ident};
use crate::rust_types::RustTypes;
use crate::symbols::Symbols;
use crate::{CodegenError, generate_error};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeSet;
use tolk_source_map::abi::{ABIDeclaration, ABIOpcode, ContractABI};
use tolk_source_map::types_kernel::{Ty, TyIdx};

pub(crate) struct DeclarationEmitter<'abi> {
    abi: &'abi ContractABI,
    symbols: Symbols<'abi>,
    types: RustTypes<'abi>,
    cells: CellCodec<'abi>,
    const_values: ConstValueEmitter<'abi>,
}

impl<'abi> DeclarationEmitter<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self {
            abi,
            symbols: Symbols::new(abi),
            types: RustTypes::new(abi),
            cells: CellCodec::new(abi),
            const_values: ConstValueEmitter::new(abi),
        }
    }

    pub(crate) fn emit(&self) -> Result<TokenStream, CodegenError> {
        let unions = self
            .abi
            .unique_types
            .iter()
            .enumerate()
            .filter_map(|(ty_idx, ty)| match ty {
                Ty::Union { .. } => Some(self.emit_union(ty_idx)),
                _ => None,
            })
            .collect::<Result<Vec<_>, _>>()?;
        let declarations = self
            .abi
            .declarations
            .iter()
            .map(|declaration| self.emit_declaration(declaration))
            .collect::<Result<Vec<_>, _>>()?;
        let mut stack_structs = Vec::new();
        for (ty_idx, ty) in self.abi.unique_types.iter().enumerate() {
            let Ty::StructRef { struct_name, .. } = ty else {
                continue;
            };
            let fields = self.symbols.struct_fields(ty_idx, true)?;
            if fields
                .iter()
                .any(|field| field.field.client_ty_idx.is_some())
            {
                stack_structs.push(self.emit_stack_struct(ty_idx, struct_name, fields)?);
            }
        }
        Ok(quote! {
            #(#unions)*
            #(#declarations)*
            #(#stack_structs)*
        })
    }

    fn emit_declaration(&self, declaration: &ABIDeclaration) -> Result<TokenStream, CodegenError> {
        let (kind, name, ty_idx) = match declaration {
            ABIDeclaration::Struct { name, ty_idx, .. } => ("struct", name, *ty_idx),
            ABIDeclaration::Alias { name, ty_idx, .. } => ("alias", name, *ty_idx),
            ABIDeclaration::Enum { name, ty_idx, .. } => ("enum", name, *ty_idx),
        };

        let result = (|| {
            self.validate_dictionary_keys(ty_idx, name, &mut BTreeSet::new())?;
            match declaration {
                ABIDeclaration::Struct { .. } => self.emit_struct(declaration),
                ABIDeclaration::Alias { .. } => self.emit_alias(declaration),
                ABIDeclaration::Enum { .. } => self.emit_enum(declaration),
            }
        })();

        result.map_err(|error| {
            generate_error(format!("Error while generating {kind} '{name}': {error}"))
        })
    }

    fn validate_dictionary_keys(
        &self,
        ty_idx: TyIdx,
        field_path: &str,
        visiting: &mut BTreeSet<TyIdx>,
    ) -> Result<(), CodegenError> {
        if !visiting.insert(ty_idx) {
            return Ok(());
        }

        let result = match self.symbols.ty(ty_idx)? {
            Ty::Nullable { .. } | Ty::AliasRef { .. } => {
                let inner_ty_idx =
                    if let Ty::Nullable { inner_ty_idx, .. } = self.symbols.ty(ty_idx)? {
                        *inner_ty_idx
                    } else {
                        self.symbols.alias_target(ty_idx)?.ty_idx
                    };
                self.validate_dictionary_keys(inner_ty_idx, field_path, visiting)
            }
            Ty::CellOf { inner_ty_idx } => {
                self.validate_dictionary_keys(*inner_ty_idx, &format!("{field_path}.ref"), visiting)
            }
            Ty::ArrayOf { inner_ty_idx } | Ty::LispListOf { inner_ty_idx } => self
                .validate_dictionary_keys(*inner_ty_idx, &format!("{field_path}[ith]"), visiting),
            Ty::Tensor { items_ty_idx } | Ty::ShapedTuple { items_ty_idx } => items_ty_idx
                .iter()
                .enumerate()
                .try_for_each(|(index, ty_idx)| {
                    self.validate_dictionary_keys(
                        *ty_idx,
                        &format!("{field_path}[{index}]"),
                        visiting,
                    )
                }),
            Ty::MapKV {
                key_ty_idx,
                value_ty_idx,
            } => {
                if !matches!(
                    self.symbols.ty(*key_ty_idx)?,
                    Ty::IntN { .. } | Ty::UintN { .. } | Ty::Address
                ) {
                    return Err(generate_error(format!(
                        "[NonStandardDictKey] '{field_path}' is 'map<{}, ...>': such a non-standard map key can not be handled by @ton/core library",
                        self.abi.render_type(*key_ty_idx)
                    )));
                }
                self.validate_dictionary_keys(*value_ty_idx, field_path, visiting)
            }
            Ty::EnumRef { enum_name } => {
                let ABIDeclaration::Enum {
                    encoded_as_ty_idx, ..
                } = self.symbols.declaration(enum_name)?
                else {
                    unreachable!("Symbols checked the declaration kind")
                };
                self.validate_dictionary_keys(*encoded_as_ty_idx, field_path, visiting)
            }
            Ty::StructRef { .. } => self
                .symbols
                .struct_fields(ty_idx, false)?
                .into_iter()
                .try_for_each(|field| {
                    self.validate_dictionary_keys(
                        field.field.ty_idx,
                        &format!("{field_path}.{}", field.field.name),
                        visiting,
                    )
                }),
            Ty::Union { variants, .. } => variants.iter().try_for_each(|variant| {
                self.validate_dictionary_keys(variant.variant_ty_idx, field_path, visiting)
            }),
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
            | Ty::Callable
            | Ty::Void
            | Ty::Unknown
            | Ty::GenericT { .. } => Ok(()),
        };

        visiting.remove(&ty_idx);
        result
    }

    fn emit_union(&self, ty_idx: TyIdx) -> Result<TokenStream, CodegenError> {
        let Ty::Union { variants, .. } = self.symbols.ty(ty_idx)? else {
            unreachable!("caller selected a union")
        };
        let name = union_ident(ty_idx);
        let params = self.types.generic_params(ty_idx)?;
        let definitions = variants
            .iter()
            .enumerate()
            .map(|(index, variant)| {
                let name = format_ident!("Variant{index}");
                let ty = self.types.cell_type(variant.variant_ty_idx)?;
                Ok(quote! { #name(#ty) })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let labels = self
            .symbols
            .union_variants(variants, None)?
            .into_iter()
            .map(|variant| variant.label)
            .collect::<Vec<_>>();
        let generic = (!params.is_empty()).then(|| quote! { <#(#params),*> });
        Ok(quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub enum #name #generic {
                #(#definitions),*
            }

            impl #generic #name #generic {
                pub const VARIANT_LABELS: &'static [&'static str] = &[#(#labels),*];
            }
        })
    }

    fn emit_struct(&self, declaration: &ABIDeclaration) -> Result<TokenStream, CodegenError> {
        let ABIDeclaration::Struct {
            name,
            ty_idx,
            type_params,
            prefix,
            description,
            ..
        } = declaration
        else {
            unreachable!("caller selected a struct")
        };
        let struct_name = type_ident(name)?;
        let params = type_params
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|param| type_ident(param))
            .collect::<Result<Vec<_>, _>>()?;
        let generic = (!params.is_empty()).then(|| quote! { <#(#params),*> });
        let fields = self.symbols.struct_fields(*ty_idx, false)?;
        let field_defs = fields
            .iter()
            .map(|field| {
                let field_name = value_ident(&field.field.name)?;
                let ty = self.types.cell_type(field.field.ty_idx)?;
                let doc = doc(&field.field.description);
                Ok(quote! { #doc pub #field_name: #ty })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let doc = doc(description);
        let prefix_constant = prefix.as_ref().map(emit_prefix_constant);
        let codecs = if params.is_empty() {
            self.emit_struct_cell_codecs(*ty_idx, &struct_name)?
        } else {
            TokenStream::new()
        };
        let create = if params.is_empty() {
            self.emit_struct_create(&fields)?
        } else {
            TokenStream::new()
        };
        let default = if params.is_empty() {
            self.emit_struct_default(&struct_name, &fields)?
        } else {
            TokenStream::new()
        };
        Ok(quote! {
            #doc
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct #struct_name #generic {
                #(#field_defs),*
            }

            impl #generic #struct_name #generic {
                #prefix_constant
                #create
            }

            #default
            #codecs
        })
    }

    fn emit_struct_create(
        &self,
        fields: &[crate::symbols::ResolvedField],
    ) -> Result<TokenStream, CodegenError> {
        let mut has_default = false;
        let mut parameters = Vec::new();
        let mut values = Vec::with_capacity(fields.len());

        for field in fields {
            let name = value_ident(&field.field.name)?;
            if let Some(default_value) = &field.field.default_value
                && self.const_values.is_supported(field.field.ty_idx)?
            {
                has_default = true;
                let value = self.const_values.emit(default_value, field.field.ty_idx)?;
                values.push(quote! { #name: #value });
            } else {
                let ty = self.types.cell_type(field.field.ty_idx)?;
                parameters.push(quote! { #name: #ty });
                values.push(quote! { #name });
            }
        }

        if !has_default {
            return Ok(TokenStream::new());
        }

        Ok(quote! {
            #[must_use]
            pub fn create(#(#parameters),*) -> Self {
                Self {
                    #(#values),*
                }
            }
        })
    }

    fn emit_struct_default(
        &self,
        struct_name: &Ident,
        fields: &[crate::symbols::ResolvedField],
    ) -> Result<TokenStream, CodegenError> {
        for field in fields {
            if field.field.default_value.is_none()
                || !self.const_values.is_supported(field.field.ty_idx)?
            {
                return Ok(TokenStream::new());
            }
        }

        let value = if fields.is_empty() {
            quote! { Self {} }
        } else {
            quote! { Self::create() }
        };

        Ok(quote! {
            impl ::std::default::Default for #struct_name {
                fn default() -> Self {
                    #value
                }
            }
        })
    }

    fn emit_struct_cell_codecs(
        &self,
        ty_idx: TyIdx,
        struct_name: &Ident,
    ) -> Result<TokenStream, CodegenError> {
        let builder = format_ident!("builder");
        let slice = format_ident!("slice");
        let store = self
            .cells
            .store_declaration(ty_idx, quote! { self }, &builder)?;
        let load = self.cells.load_declaration(ty_idx, &slice)?;
        Ok(quote! {
            impl ::acton_client::AbiStore for #struct_name {
                fn store_into(
                    &self,
                    #builder: &mut ::acton_client::__private::tycho_types::cell::CellBuilder,
                ) -> ::std::result::Result<(), ::acton_client::AbiError> {
                    #store
                    ::std::result::Result::Ok(())
                }
            }

            impl ::acton_client::AbiLoad for #struct_name {
                fn load_from(
                    #slice: &mut ::acton_client::__private::tycho_types::cell::CellSlice<'_>,
                ) -> ::std::result::Result<Self, ::acton_client::AbiError> {
                    let value = #load;
                    ::std::result::Result::Ok(value)
                }
            }
        })
    }

    fn emit_alias(&self, declaration: &ABIDeclaration) -> Result<TokenStream, CodegenError> {
        let ABIDeclaration::Alias {
            name,
            ty_idx,
            target_ty_idx,
            type_params,
            description,
            ..
        } = declaration
        else {
            unreachable!("caller selected an alias")
        };
        let alias_name = type_ident(name)?;
        let params = type_params
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|param| type_ident(param))
            .collect::<Result<Vec<_>, _>>()?;
        let generic = (!params.is_empty()).then(|| quote! { <#(#params),*> });
        let target = self.types.cell_type(*target_ty_idx)?;
        let doc = doc(description);
        let codecs = if params.is_empty() {
            let builder = format_ident!("builder");
            let slice = format_ident!("slice");
            let value_ty = self.types.cell_type(*ty_idx)?;
            let store = self
                .cells
                .store_declaration(*ty_idx, quote! { value }, &builder)?;
            let load = self.cells.load_declaration(*ty_idx, &slice)?;
            let store_name = format_ident!("store_{}", value_ident(name)?);
            let load_name = format_ident!("load_{}", value_ident(name)?);
            quote! {
                pub fn #store_name(
                    value: &#value_ty,
                    #builder: &mut ::acton_client::__private::tycho_types::cell::CellBuilder,
                ) -> ::std::result::Result<(), ::acton_client::AbiError> {
                    #store
                    ::std::result::Result::Ok(())
                }

                pub fn #load_name(
                    #slice: &mut ::acton_client::__private::tycho_types::cell::CellSlice<'_>,
                ) -> ::std::result::Result<#value_ty, ::acton_client::AbiError> {
                    let value = #load;
                    ::std::result::Result::Ok(value)
                }
            }
        } else {
            TokenStream::new()
        };
        Ok(quote! {
            #doc
            pub type #alias_name #generic = #target;
            #codecs
        })
    }

    fn emit_enum(&self, declaration: &ABIDeclaration) -> Result<TokenStream, CodegenError> {
        let ABIDeclaration::Enum {
            name,
            ty_idx,
            members,
            description,
            ..
        } = declaration
        else {
            unreachable!("caller selected an enum")
        };
        let enum_name = type_ident(name)?;
        let mut used_names = BTreeSet::new();
        let members = members
            .iter()
            .map(|member| {
                let mut base_name = value_ident(&member.name)?.to_string();
                if matches!(base_name.as_str(), "from_slice" | "store" | "to_cell") {
                    base_name.push('_');
                }
                let mut unique_name = base_name.clone();
                let mut suffix = 2;
                while !used_names.insert(unique_name.clone()) {
                    unique_name = format!("{base_name}_{suffix}");
                    suffix += 1;
                }
                let member_name = format_ident!("{unique_name}");
                let value = &member.value;
                let doc = doc(&member.description);
                Ok(quote! {
                    #doc
                    pub fn #member_name() -> Self {
                        Self(
                            <::acton_client::__private::num_bigint::BigInt as ::std::str::FromStr>::from_str(#value)
                                .expect("Tolk ABI enum member must be an integer")
                        )
                    }
                })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let doc = doc(description);
        let builder = format_ident!("builder");
        let slice = format_ident!("slice");
        let store = self
            .cells
            .store_declaration(*ty_idx, quote! { self }, &builder)?;
        let load = self.cells.load_declaration(*ty_idx, &slice)?;
        Ok(quote! {
            #doc
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct #enum_name(pub ::acton_client::__private::num_bigint::BigInt);

            impl #enum_name {
                #(#members)*
            }

            impl ::acton_client::AbiStore for #enum_name {
                fn store_into(
                    &self,
                    #builder: &mut ::acton_client::__private::tycho_types::cell::CellBuilder,
                ) -> ::std::result::Result<(), ::acton_client::AbiError> {
                    #store
                    ::std::result::Result::Ok(())
                }
            }

            impl ::acton_client::AbiLoad for #enum_name {
                fn load_from(
                    #slice: &mut ::acton_client::__private::tycho_types::cell::CellSlice<'_>,
                ) -> ::std::result::Result<Self, ::acton_client::AbiError> {
                    let value = #load;
                    ::std::result::Result::Ok(value)
                }
            }
        })
    }

    fn emit_stack_struct(
        &self,
        ty_idx: TyIdx,
        struct_name: &str,
        fields: Vec<crate::symbols::ResolvedField>,
    ) -> Result<TokenStream, CodegenError> {
        let name = stack_struct_ident(struct_name, ty_idx)?;
        let fields = fields
            .iter()
            .map(|field| {
                let name = value_ident(&field.field.name)?;
                let ty = self.types.stack_type(field.field.ty_idx)?;
                Ok(quote! { pub #name: #ty })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        Ok(quote! {
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct #name {
                #(#fields),*
            }
        })
    }
}

fn doc(description: &str) -> TokenStream {
    if description.is_empty() {
        TokenStream::new()
    } else {
        quote! { #[doc = #description] }
    }
}

fn emit_prefix_constant(prefix: &ABIOpcode) -> TokenStream {
    let prefix_num = prefix.prefix_num;
    let prefix_len = prefix.prefix_len;
    quote! {
        pub const PREFIX: u64 = #prefix_num;
        pub const PREFIX_BITS: i32 = #prefix_len;
    }
}
