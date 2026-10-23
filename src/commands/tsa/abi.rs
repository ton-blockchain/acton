use crate::file_build_cache::FileBuildCache;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use rston::boc::Boc;
use rston::cell::Cell;
use serde_json::Value;
use std::path::PathBuf;
use tolk_compiler::abi::ContractABI as CompilerContractABI;
use tolk_compiler::dynamic_unpack::{self, UnpackedValue};
use tolk_compiler::types_kernel::render_ty;

#[derive(Clone)]
pub(super) struct TsaContractContext {
    pub(super) tsa_id: usize,
    acton_id: String,
    contract_name: String,
    compiler_abi: Option<CompilerContractABI>,
}

impl TsaContractContext {
    pub(super) fn label(&self) -> String {
        if self.contract_name == self.acton_id {
            format!("{} [{}]", self.acton_id, self.tsa_id)
        } else {
            format!(
                "{} ({}) [{}]",
                self.acton_id, self.contract_name, self.tsa_id
            )
        }
    }
}

pub(super) struct TsaDecodedMessage {
    pub(super) contract_label: String,
    pub(super) body_name: String,
    pub(super) data: Value,
}

pub(super) fn load_contract_contexts(
    config: &ActonConfig,
    contracts: &[String],
) -> Vec<TsaContractContext> {
    let mut file_cache = FileBuildCache::new(None).ok();

    contracts
        .iter()
        .enumerate()
        .map(|(index, contract_id)| {
            let contract_config = config
                .get_contract(contract_id)
                .expect("contract presence already validated");
            let compiler_abi = file_cache.as_mut().and_then(|cache| {
                match try_load_compiler_abi(cache, config, contract_config) {
                    Ok(abi) => abi,
                    Err(err) => {
                        eprintln!(
                            "{} Failed to load ABI for {}: {}",
                            "Warning:".yellow(),
                            contract_id.cyan(),
                            err
                        );
                        None
                    }
                }
            });

            TsaContractContext {
                tsa_id: index + 1,
                acton_id: contract_id.clone(),
                contract_name: contract_config.display_name(contract_id).to_owned(),
                compiler_abi,
            }
        })
        .collect()
}

fn try_load_compiler_abi(
    file_cache: &mut FileBuildCache,
    config: &ActonConfig,
    contract_config: &ContractConfig,
) -> anyhow::Result<Option<CompilerContractABI>> {
    if !contract_config.src.ends_with(".tolk") {
        return Ok(None);
    }

    if let Some(entry) = file_cache.get(&contract_config.src, false, false, false) {
        return Ok(entry.abi);
    }

    let mappings = config.mappings();
    let compiler = tolk_compiler::Compiler::new().with_mappings(&mappings);
    let contract_path = resolve_project_path(&contract_config.src);

    match compiler.compile(&contract_path, false) {
        tolk_compiler::CompilerResult::Success(result) => {
            let abi = result.abi.clone();
            let _ = file_cache.put(&contract_config.src, &result, false, false, false);
            Ok(abi)
        }
        tolk_compiler::CompilerResult::Error(error) => {
            anyhow::bail!(
                "cannot compile {} for ABI extraction: {}",
                contract_path.display(),
                error.message
            )
        }
    }
}

fn resolve_project_path(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        configured_project_root().join(path)
    }
}

pub(super) fn try_decode_storage_cell(
    cell: &Cell,
    contract: &TsaContractContext,
) -> anyhow::Result<Value> {
    let abi = contract
        .compiler_abi
        .as_ref()
        .ok_or_else(|| anyhow!("compiler ABI is not available"))?;
    let storage_ty = abi
        .storage
        .storage_at_deployment_ty_idx
        .or(abi.storage.storage_ty_idx)
        .ok_or_else(|| anyhow!("contract ABI does not declare storage"))?;
    let mut parser = cell.as_slice_allow_exotic();
    let decoded = dynamic_unpack::unpack_from_slice(&mut parser, abi, storage_ty)
        .context("failed to decode storage with compiler ABI")?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        anyhow::bail!(
            "storage has {} extra bits and {} extra refs after ABI decode",
            parser.size_bits(),
            parser.size_refs()
        );
    }
    Ok(compiler_data_to_json(&decoded))
}

pub(super) fn try_decode_input_body(
    cell: &Cell,
    input_type: Option<&str>,
    contract_contexts: &[TsaContractContext],
) -> Vec<TsaDecodedMessage> {
    let mut decoded_messages = Vec::new();

    for contract in contract_contexts {
        let Some(abi) = &contract.compiler_abi else {
            continue;
        };

        let mut candidates = Vec::new();
        match input_type {
            Some("recvExternalInput") => {
                candidates.extend(
                    abi.incoming_external
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
            }
            Some("recvInternalInput") => {
                candidates.extend(
                    abi.incoming_messages
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
            }
            _ => {
                candidates.extend(
                    abi.incoming_external
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
                candidates.extend(
                    abi.incoming_messages
                        .iter()
                        .map(|message| message.body_ty_idx),
                );
            }
        }

        for body_ty in candidates {
            let mut parser = cell.as_slice_allow_exotic();
            let Ok(data) = dynamic_unpack::unpack_from_slice(&mut parser, abi, body_ty) else {
                continue;
            };
            if parser.size_bits() != 0 || parser.size_refs() != 0 {
                continue;
            }

            decoded_messages.push(TsaDecodedMessage {
                contract_label: contract.label(),
                body_name: render_ty(abi, body_ty),
                data: compiler_data_to_json(&data),
            });
        }
    }

    decoded_messages
}

fn compiler_data_to_json(data: &UnpackedValue) -> Value {
    match data {
        UnpackedValue::Null | UnpackedValue::Void | UnpackedValue::AddressNone => Value::Null,
        UnpackedValue::Number(value) => Value::String(value.to_string()),
        UnpackedValue::Bool(value) => Value::Bool(*value),
        UnpackedValue::String(value) => Value::String(value.clone()),
        UnpackedValue::Address(value) => Value::String(value.to_string()),
        UnpackedValue::ExtAddress(value) => serde_json::json!({
            "bits": value.data_bit_len,
            "hex": hex::encode(&value.data),
        }),
        UnpackedValue::Cell(value) | UnpackedValue::RemainingBitsAndRefs(value) => {
            serde_json::json!({
                "boc64": Boc::encode_base64(value.clone()),
            })
        }
        UnpackedValue::Bits((bytes, bit_len)) => serde_json::json!({
            "bits": bit_len,
            "hex": hex::encode(bytes),
        }),
        UnpackedValue::Array(values) => {
            Value::Array(values.iter().map(compiler_data_to_json).collect())
        }
        UnpackedValue::Map(values) => Value::Array(
            values
                .iter()
                .map(|(key, value)| {
                    serde_json::json!({
                        "key": compiler_data_to_json(key),
                        "value": compiler_data_to_json(value),
                    })
                })
                .collect(),
        ),
        UnpackedValue::Object { fields, .. } => {
            let mut result = serde_json::Map::new();
            for (name, value) in fields {
                result.insert(name.clone(), compiler_data_to_json(value));
            }
            Value::Object(result)
        }
    }
}
