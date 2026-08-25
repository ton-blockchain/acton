use crate::cell_codec::CellCodec;
use crate::const_values::ConstValueEmitter;
use crate::names::{const_ident, type_ident, value_ident};
use crate::rust_types::RustTypes;
use crate::stack_codec::StackCodec;
use crate::symbols::Symbols;
use crate::{CodegenError, generate_error};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use tolk_source_map::abi::{ABIGetMethod, ContractABI};
use tolk_source_map::types_kernel::{Ty, TyIdx};

/// Emits the contract-facing part of the bindings.
///
/// This is the Rust counterpart of upstream's `generate-ts-wrappers.ts`: it
/// composes declaration codecs into storage helpers, message helpers, sends,
/// get methods, and named error codes. The actual codecs remain in their own
/// emitters.
pub(crate) struct WrapperEmitter<'abi> {
    abi: &'abi ContractABI,
    symbols: Symbols<'abi>,
    types: RustTypes<'abi>,
    cells: CellCodec<'abi>,
    stack: StackCodec<'abi>,
    const_values: ConstValueEmitter<'abi>,
}

struct EmittedGetMethod {
    args: TokenStream,
    method: TokenStream,
}

impl<'abi> WrapperEmitter<'abi> {
    pub(crate) const fn new(abi: &'abi ContractABI) -> Self {
        Self {
            abi,
            symbols: Symbols::new(abi),
            types: RustTypes::new(abi),
            cells: CellCodec::new(abi),
            stack: StackCodec::new(abi),
            const_values: ConstValueEmitter::new(abi),
        }
    }

    pub(crate) fn emit(&self) -> Result<TokenStream, CodegenError> {
        let contract_name = type_ident(&self.abi.contract_name)?;
        let storage = self.emit_storage_helpers()?;
        let from_storage = self.emit_from_storage(&contract_name)?;
        let create_cells = self.emit_create_cell_methods()?;
        let sends = self.emit_send_methods()?;
        let getters = self
            .abi
            .get_methods
            .iter()
            .filter(|method| method.tvm_method_id != 0)
            .map(|method| self.emit_get_method(method))
            .collect::<Result<Vec<_>, _>>()?;
        let getter_args = getters.iter().map(|getter| &getter.args);
        let getter_methods = getters.iter().map(|getter| &getter.method);
        let get_method_metadata = self
            .abi
            .get_methods
            .iter()
            .filter(|method| method.tvm_method_id != 0)
            .map(|method| {
                let name = &method.name;
                let method_id = method.tvm_method_id;
                quote! { ::acton_client::GetMethod { name: #name, method_id: #method_id } }
            })
            .collect::<Vec<_>>();
        let errors = self.emit_errors()?;

        Ok(quote! {
            #errors

            pub const GET_METHODS: &[::acton_client::GetMethod] = &[
                #(#get_method_metadata),*
            ];

            #(#getter_args)*

            #[derive(Debug, Clone)]
            pub struct #contract_name<P = ()> {
                address: ::acton_client::StdAddr,
                init: ::std::option::Option<::acton_client::ContractInit>,
                provider: P,
            }

            impl<P> #contract_name<P> {
                #[must_use]
                pub const fn from_address(address: ::acton_client::StdAddr, provider: P) -> Self {
                    Self { address, init: ::std::option::Option::None, provider }
                }

                #[must_use]
                pub const fn address(&self) -> &::acton_client::StdAddr {
                    &self.address
                }

                #[must_use]
                pub const fn provider(&self) -> &P {
                    &self.provider
                }

                #[must_use]
                pub const fn init(&self) -> ::std::option::Option<&::acton_client::ContractInit> {
                    self.init.as_ref()
                }

                #[must_use]
                pub fn with_provider<Q>(self, provider: Q) -> #contract_name<Q> {
                    #contract_name {
                        address: self.address,
                        init: self.init,
                        provider,
                    }
                }

                pub fn code_cell() -> ::std::result::Result<::acton_client::Cell, ::acton_client::AbiError> {
                    ::acton_client::decode_code_boc64(CODE_BOC64)
                }

                #[must_use]
                pub fn into_parts(self) -> (::acton_client::StdAddr, P) {
                    (self.address, self.provider)
                }

                #storage
                #(#create_cells)*
            }

            #from_storage

            impl<P: ::acton_client::ContractSender> #contract_name<P> {
                pub async fn send_deploy(
                    &self,
                    via: &P::Sender,
                    msg_value: ::acton_client::BigInt,
                    options: ::acton_client::SendOptions,
                ) -> ::std::result::Result<P::Output, ::acton_client::ClientError<P::Error>> {
                    let init = self.init.clone().ok_or_else(|| {
                        ::acton_client::AbiError::InvalidData(
                            ::std::string::String::from(
                                "contract deployment init is unavailable for an address-only client",
                            ),
                        )
                    })?;
                    let body = ::acton_client::__private::tycho_types::cell::CellBuilder::new()
                        .build()?;
                    self.provider
                        .send_internal(
                            via,
                            &self.address,
                            ::acton_client::InternalMessage {
                                value: msg_value,
                                body,
                                options,
                                init: ::std::option::Option::Some(init),
                            },
                        )
                        .await
                        .map_err(::acton_client::ClientError::Provider)
                }

                #(#sends)*
            }

            impl<P: ::acton_client::ContractProvider> #contract_name<P> {
                #(#getter_methods)*
            }
        })
    }

    fn emit_storage_helpers(&self) -> Result<TokenStream, CodegenError> {
        let storage_ty_idx = self
            .abi
            .storage
            .storage_at_deployment_ty_idx
            .or(self.abi.storage.storage_ty_idx);
        let Some(ty_idx) = storage_ty_idx else {
            return Ok(TokenStream::new());
        };
        if matches!(self.symbols.ty(ty_idx)?, Ty::NullLiteral) {
            return Ok(TokenStream::new());
        }

        let ty = self.types.cell_type(ty_idx)?;
        let encode = self.encode_cell(ty_idx, quote! { storage })?;
        let decode = self.decode_cell(ty_idx, quote! { cell })?;
        Ok(quote! {
            pub fn storage_to_cell(
                storage: &#ty,
            ) -> ::std::result::Result<::acton_client::Cell, ::acton_client::AbiError> {
                #encode
            }

            pub fn storage_from_cell(
                cell: &::acton_client::Cell,
            ) -> ::std::result::Result<#ty, ::acton_client::AbiError> {
                #decode
            }
        })
    }

    fn emit_from_storage(&self, contract_name: &Ident) -> Result<TokenStream, CodegenError> {
        let storage_ty_idx = self
            .abi
            .storage
            .storage_at_deployment_ty_idx
            .or(self.abi.storage.storage_ty_idx);
        let Some(ty_idx) = storage_ty_idx else {
            return Ok(TokenStream::new());
        };
        if matches!(self.symbols.ty(ty_idx)?, Ty::NullLiteral) {
            return Ok(TokenStream::new());
        }

        let ty = self.types.cell_type(ty_idx)?;
        Ok(quote! {
            impl #contract_name<()> {
                pub fn from_storage(
                    storage: &#ty,
                ) -> ::std::result::Result<Self, ::acton_client::AbiError> {
                    Self::from_storage_with_options(
                        storage,
                        ::acton_client::DeployedAddressOptions::default(),
                    )
                }

                pub fn from_storage_with_options(
                    storage: &#ty,
                    options: ::acton_client::DeployedAddressOptions,
                ) -> ::std::result::Result<Self, ::acton_client::AbiError> {
                    let data = Self::storage_to_cell(storage)?;
                    let code = match options.override_contract_code.clone() {
                        ::std::option::Option::Some(code) => code,
                        ::std::option::Option::None => Self::code_cell()?,
                    };
                    let address = ::acton_client::calculate_deployed_address(&code, &data, &options)?;
                    ::std::result::Result::Ok(Self {
                        address,
                        init: ::std::option::Option::Some(::acton_client::ContractInit { code, data }),
                        provider: (),
                    })
                }
            }
        })
    }

    fn emit_create_cell_methods(&self) -> Result<Vec<TokenStream>, CodegenError> {
        let messages = self
            .abi
            .incoming_messages
            .iter()
            .map(|message| message.body_ty_idx)
            .chain(
                self.abi
                    .incoming_external
                    .iter()
                    .map(|message| message.body_ty_idx),
            );
        self.unique_types(messages)
            .into_iter()
            .map(|(name, ty_idx)| {
                let method_name = value_ident(&format!("create_cell_of_{name}"))?;
                let ty = self.types.cell_type(ty_idx)?;
                let encode = self.encode_cell(ty_idx, quote! { body })?;
                Ok(quote! {
                    pub fn #method_name(
                        body: &#ty,
                    ) -> ::std::result::Result<::acton_client::Cell, ::acton_client::AbiError> {
                        #encode
                    }
                })
            })
            .collect()
    }

    fn emit_send_methods(&self) -> Result<Vec<TokenStream>, CodegenError> {
        self.unique_types(
            self.abi
                .incoming_messages
                .iter()
                .map(|message| message.body_ty_idx),
        )
        .into_iter()
        .map(|(name, ty_idx)| {
            let method_name = value_ident(&format!("send_{name}"))?;
            let ty = self.types.cell_type(ty_idx)?;
            let encode = self.encode_cell(ty_idx, quote! { body })?;
            Ok(quote! {
                pub async fn #method_name(
                    &self,
                    via: &P::Sender,
                    msg_value: ::acton_client::BigInt,
                    body: &#ty,
                    options: ::acton_client::SendOptions,
                ) -> ::std::result::Result<P::Output, ::acton_client::ClientError<P::Error>> {
                    let body = #encode?;
                    self.provider
                        .send_internal(
                            via,
                            &self.address,
                            ::acton_client::InternalMessage {
                                value: msg_value,
                                body,
                                options,
                                init: ::std::option::Option::None,
                            },
                        )
                        .await
                        .map_err(::acton_client::ClientError::Provider)
                }
            })
        })
        .collect()
    }

    fn emit_get_method(&self, method: &ABIGetMethod) -> Result<EmittedGetMethod, CodegenError> {
        self.emit_get_method_impl(method).map_err(|error| {
            generate_error(format!(
                "Error while generating get method '{}': {error}",
                method.name
            ))
        })
    }

    fn emit_get_method_impl(
        &self,
        method: &ABIGetMethod,
    ) -> Result<EmittedGetMethod, CodegenError> {
        let bare_name = value_ident(&method.name)?;
        let bare_name_text = bare_name.to_string();
        let method_name = if bare_name_text.trim_start_matches("r#").starts_with("get") {
            bare_name
        } else {
            value_ident(&format!("get_{}", method.name))?
        };
        let method_id = method.tvm_method_id;
        let return_ty = self.types.stack_type(method.return_ty_idx)?;
        let return_width = self.symbols.stack_width(method.return_ty_idx)?;

        let parameters = method
            .parameters
            .iter()
            .map(|parameter| {
                let name = value_ident(&parameter.name)?;
                let ty = self.types.stack_type(parameter.ty_idx)?;
                let default = parameter
                    .default_value
                    .as_ref()
                    .map(|value| self.const_values.emit(value, parameter.ty_idx))
                    .transpose()?;
                Ok((name, ty, parameter.ty_idx, default))
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        let parameter_names = parameters
            .iter()
            .map(|(name, _, _, _)| name.clone())
            .collect::<Vec<_>>();
        let required_parameter_defs = parameters
            .iter()
            .filter(|(_, _, _, default)| default.is_none())
            .map(|(name, ty, _, _)| quote! { #name: &#ty })
            .collect::<Vec<_>>();

        let default_args_type = parameters
            .iter()
            .any(|(_, _, _, default)| default.is_some())
            .then(|| type_ident(&format!("{}_args", method.name)))
            .transpose()?;
        let default_args_name = unique_local_ident("default_args", &parameter_names)?;

        let default_args = if let Some(args_type) = &default_args_type {
            let fields = parameters
                .iter()
                .filter(|(_, _, _, default)| default.is_some())
                .map(|(name, ty, _, _)| quote! { pub #name: #ty })
                .collect::<Vec<_>>();
            let values = parameters
                .iter()
                .filter_map(|(name, _, _, default)| {
                    default.as_ref().map(|value| quote! { #name: #value })
                })
                .collect::<Vec<_>>();
            quote! {
                #[derive(Debug, Clone)]
                pub struct #args_type {
                    #(#fields),*
                }

                impl ::std::default::Default for #args_type {
                    fn default() -> Self {
                        Self {
                            #(#values),*
                        }
                    }
                }
            }
        } else {
            TokenStream::new()
        };
        let default_args_parameter = default_args_type
            .as_ref()
            .map(|args_type| quote! { #default_args_name: #args_type });

        let arguments = unique_local_ident("arguments", &parameter_names)?;
        let owned_arguments = unique_local_ident("owned_arguments", &parameter_names)?;
        let owned_reader = unique_local_ident("owned_reader", &parameter_names)?;
        let reader = unique_local_ident("reader", &parameter_names)?;
        let output = unique_local_ident("output", &parameter_names)?;
        let result = unique_local_ident("result", &parameter_names)?;
        let load = self
            .stack
            .load_at_path(method.return_ty_idx, &reader, false, None, "result")?;
        let stores = parameters
            .iter()
            .zip(&method.parameters)
            .map(|((name, _, ty_idx, default), parameter)| {
                let value = if default.is_some() {
                    quote! { &#default_args_name.#name }
                } else {
                    quote! { #name }
                };
                self.stack
                    .store_at_path(*ty_idx, value, &arguments, false, None, &parameter.name)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let arguments_setup = if stores.is_empty() {
            quote! {
                let #owned_arguments = ::std::vec::Vec::new();
            }
        } else {
            quote! {
                let mut #owned_arguments = ::std::vec::Vec::new();
                let #arguments = &mut #owned_arguments;
            }
        };
        let doc = doc(&method.description);

        let method = quote! {
            #doc
            pub async fn #method_name(
                &self,
                #(#required_parameter_defs,)*
                #default_args_parameter
            ) -> ::std::result::Result<#return_ty, ::acton_client::ClientError<P::Error>> {
                #arguments_setup
                #(#stores)*
                let #output = self
                    .provider
                    .run_get_method(
                        &self.address,
                        #method_id,
                        ::acton_client::Tuple(#owned_arguments),
                    )
                    .await
                    .map_err(::acton_client::ClientError::Provider)?;
                let mut #owned_reader = ::acton_client::StackReader::from_tuple(#output, #return_width)?;
                let #reader = &mut #owned_reader;
                let #result = #load;
                #reader.ensure_empty()?;
                ::std::result::Result::Ok(#result)
            }
        };

        Ok(EmittedGetMethod {
            args: default_args,
            method,
        })
    }

    fn emit_errors(&self) -> Result<TokenStream, CodegenError> {
        let named = self
            .abi
            .thrown_errors
            .iter()
            .filter(|error| !error.name.is_empty())
            .map(|error| {
                let name = const_ident(&error.name)?;
                let code = error.err_code;
                let doc = doc(&error.description);
                Ok(quote! { #doc pub const #name: i32 = #code; })
            })
            .collect::<Result<Vec<_>, CodegenError>>()?;
        if named.is_empty() {
            Ok(TokenStream::new())
        } else {
            Ok(quote! { pub mod errors { #(#named)* } })
        }
    }

    fn encode_cell(&self, ty_idx: TyIdx, value: TokenStream) -> Result<TokenStream, CodegenError> {
        let builder = format_ident!("builder");
        let store = self.cells.store(ty_idx, value, &builder, None)?;
        Ok(quote! {
            (|| -> ::std::result::Result<::acton_client::Cell, ::acton_client::AbiError> {
                let mut owned_builder = ::acton_client::__private::tycho_types::cell::CellBuilder::new();
                let #builder = &mut owned_builder;
                #store
                ::std::result::Result::Ok(owned_builder.build()?)
            })()
        })
    }

    fn decode_cell(&self, ty_idx: TyIdx, cell: TokenStream) -> Result<TokenStream, CodegenError> {
        let slice = format_ident!("slice");
        let load = self.cells.load(ty_idx, &slice, None)?;
        Ok(quote! {
            (|| -> ::std::result::Result<_, ::acton_client::AbiError> {
                let mut owned_slice = (#cell).as_slice()?;
                let #slice = &mut owned_slice;
                let value = #load;
                ::acton_client::cell::ensure_empty(#slice)?;
                ::std::result::Result::Ok(value)
            })()
        })
    }

    fn unique_types(&self, types: impl Iterator<Item = TyIdx>) -> BTreeMap<String, TyIdx> {
        types
            .map(|ty_idx| (self.type_name(ty_idx), ty_idx))
            .collect()
    }

    fn type_name(&self, ty_idx: TyIdx) -> String {
        match self.symbols.ty(ty_idx) {
            Ok(Ty::StructRef { struct_name, .. }) => struct_name.clone(),
            Ok(Ty::AliasRef { alias_name, .. }) => alias_name.clone(),
            Ok(Ty::EnumRef { enum_name }) => enum_name.clone(),
            _ => format!("type_{ty_idx}"),
        }
    }
}

fn unique_local_ident(base: &str, parameter_names: &[Ident]) -> Result<Ident, CodegenError> {
    let mut candidate = value_ident(base)?;
    let mut suffix = 2;
    while parameter_names.contains(&candidate) {
        candidate = value_ident(&format!("{base}_{suffix}"))?;
        suffix += 1;
    }
    Ok(candidate)
}

fn doc(description: &str) -> TokenStream {
    if description.is_empty() {
        TokenStream::new()
    } else {
        quote! { #[doc = #description] }
    }
}
