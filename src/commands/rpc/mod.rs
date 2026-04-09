use crate::commands::common::error_fmt;
use crate::file_build_cache::FileBuildCache;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use log::warn;
use num_bigint::{BigInt, Sign};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tolkc::abi::ContractABI as CompilerContractABI;
use ton_abi::abi_serde::Data as CompilerAbiData;
use ton_abi::{ContractAbi, compiler_abi_serde, contract_abi};
use ton_api::{Network, TonApiClient};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::models::{
    Base64StdAddrFlags, DisplayBase64StdAddr, IntAddr, StdAddr, StdAddrFormat,
};

#[derive(Subcommand, Clone)]
pub enum RpcCommand {
    #[command(about = "Show remote account information and decode storage when ABI is known")]
    Info {
        #[arg(help = "Contract address in friendly or raw format")]
        address: String,
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
        #[arg(long, help = "TonCenter API key for blockchain queries")]
        api_key: Option<String>,
    },
}

pub fn rpc_cmd(command: RpcCommand) -> anyhow::Result<()> {
    match command {
        RpcCommand::Info {
            address,
            net,
            api_key,
        } => rpc_info_cmd(&address, net, api_key),
    }
}

fn rpc_info_cmd(address: &str, net: Option<String>, api_key: Option<String>) -> anyhow::Result<()> {
    let (address, _) = StdAddr::from_str_ext(address, StdAddrFormat::any())
        .map_err(|_| anyhow!("Invalid address"))
        .with_context(|| error_fmt::invalid_address(address))?;

    let network = net
        .as_deref()
        .map(Network::from_str)
        .transpose()?
        .unwrap_or(Network::Testnet);

    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks(), api_key)?;

    let remote = client
        .get_account_info(None, &address.to_string())
        .with_context(|| format!("Failed to fetch account info for {address} from {network}"))?;

    let balance = remote.balance.to_bigint()?;
    let code = TonApiClient::decode_optional_cell(&remote.code)?;
    let data = TonApiClient::decode_optional_cell(&remote.data)?;

    let matched_contract = code
        .as_ref()
        .map(|code| find_local_contract_match(code.repr_hash(), &config))
        .transpose()?
        .flatten();

    let decoded_storage = match (&data, matched_contract.as_ref()) {
        (Some(data), Some(contract)) => contract
            .compiler_abi
            .as_ref()
            .map(|abi| decode_storage_json(data, abi, &network))
            .transpose()?,
        _ => None,
    };

    print_section("Remote Account");
    print_kv("Network", network.to_string());
    print_kv(
        "Address",
        format_std_address(&address, &network).cyan().to_string(),
    );
    print_kv("Raw Address", address.to_string().cyan().to_string());
    print_kv("Status", format_account_status(&remote.state));
    print_kv("Balance", format_nanotons(&balance).white().to_string());
    print_kv("Last Tx LT", remote.last_transaction_id.lt.as_str());
    print_kv(
        "Last Tx Hash",
        remote
            .last_transaction_id
            .hash
            .as_str()
            .yellow()
            .to_string(),
    );

    if let Some(code) = &code {
        print_kv("Code Hash", format_hash(code.repr_hash()));
    }
    if let Some(data) = &data {
        print_kv("Data Hash", format_hash(data.repr_hash()));
    }
    if remote.state == "frozen" && !remote.frozen_hash.is_empty() {
        print_kv(
            "Frozen Hash",
            remote.frozen_hash.as_str().yellow().to_string(),
        );
    }

    if code.is_some() {
        print_section("Local Match");
        if let Some(contract) = &matched_contract {
            print_kv(
                "Contract",
                format!("{} ({})", contract.contract_id, contract.contract_name)
                    .green()
                    .to_string(),
            );
            if let Some(abi) = &contract.abi {
                print_kv("ABI", abi.name.as_str().green().to_string());
            }
        } else {
            print_kv("Contract", "<none>".dimmed().to_string());
        }
    }

    match decoded_storage {
        Some(decoded_storage) => {
            print_section("Decoded Storage");
            print_yaml_value(None, &decoded_storage, 2);
        }
        None if data.is_some() && matched_contract.is_some() => {
            print_section("Decoded Storage");
            println!("  {}", "<unavailable>".dimmed());
        }
        None if data.is_some() => {
            print_section("Decoded Storage");
            println!("  {}", "<local ABI not found>".dimmed());
        }
        None => {}
    }

    Ok(())
}

fn load_rpc_config() -> anyhow::Result<ActonConfig> {
    let manifest_path = acton_config::config::manifest_path();
    match ActonConfig::load() {
        Ok(config) => Ok(config),
        Err(_) if !manifest_path.exists() => Ok(ActonConfig::default()),
        Err(err) => Err(err).with_context(|| {
            format!(
                "Failed to load Acton config from {}",
                manifest_path.display()
            )
        }),
    }
}

#[derive(Clone)]
struct LocalContractMatch {
    contract_id: String,
    contract_name: String,
    abi: Option<Arc<ContractAbi>>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
}

fn find_local_contract_match(
    code_hash: &HashBytes,
    config: &ActonConfig,
) -> anyhow::Result<Option<LocalContractMatch>> {
    let manifest_path = acton_config::config::manifest_path();
    if !manifest_path.exists() {
        return Ok(None);
    }

    let Some(contracts) = config.contracts() else {
        return Ok(None);
    };
    let mut file_cache = FileBuildCache::new(None).ok();

    for (contract_id, contract) in contracts {
        let candidate =
            match load_local_contract_candidate(contract_id, contract, config, file_cache.as_mut())
            {
                Ok(candidate) => candidate,
                Err(err) => {
                    warn!("Skipping rpc ABI match candidate `{contract_id}`: {err:#}");
                    continue;
                }
            };
        if &candidate.code_hash == code_hash {
            return Ok(Some(LocalContractMatch {
                contract_id: candidate.contract_id,
                contract_name: candidate.contract_name,
                abi: candidate.abi,
                compiler_abi: candidate.compiler_abi,
            }));
        }
    }

    Ok(None)
}

struct LocalContractCandidate {
    contract_id: String,
    contract_name: String,
    code_hash: HashBytes,
    abi: Option<Arc<ContractAbi>>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
}

fn load_local_contract_candidate(
    contract_id: &str,
    contract: &ContractConfig,
    config: &ActonConfig,
    mut file_cache: Option<&mut FileBuildCache>,
) -> anyhow::Result<LocalContractCandidate> {
    let contract_path = resolve_project_path(&contract.src);
    if contract.src.ends_with(".boc") {
        let boc = fs::read(&contract_path)
            .with_context(|| format!("Failed to read {}", contract_path.display()))?;
        let code = Boc::decode(boc)
            .with_context(|| format!("Failed to decode {}", contract_path.display()))?;
        return Ok(LocalContractCandidate {
            contract_id: contract_id.to_owned(),
            contract_name: contract.name.clone(),
            code_hash: *code.repr_hash(),
            abi: None,
            compiler_abi: None,
        });
    }

    let cached = file_cache
        .as_mut()
        .and_then(|cache| cache.get(&contract.src, false, 2, "1.3"));
    let (code_boc64, compiler_abi) = if let Some(cached) = cached {
        (cached.code_boc64, cached.abi.map(Arc::new))
    } else {
        let compiler = tolkc::Compiler::new(2).with_mappings(&config.mappings());
        match compiler.compile(&contract_path, false) {
            tolkc::CompilerResult::Success(result) => {
                if let Some(cache) = file_cache.as_mut() {
                    let _ = cache.put(&contract.src, &result, false, 2, "1.3");
                }
                (result.code_boc64, result.abi.map(Arc::new))
            }
            tolkc::CompilerResult::Error(err) => {
                return Err(anyhow!(err.message)
                    .context(format!("Failed to compile {}", contract_path.display())));
            }
        }
    };

    let code = Boc::decode_base64(&code_boc64)
        .with_context(|| format!("Failed to decode code for {}", contract_path.display()))?;
    let content = fs::read_to_string(&contract_path)
        .with_context(|| format!("Failed to read {}", contract_path.display()))?;
    let path = contract_path.to_string_lossy();
    let abi = Arc::new(contract_abi(content.into(), &path, &config.mappings()));

    Ok(LocalContractCandidate {
        contract_id: contract_id.to_owned(),
        contract_name: contract.name.clone(),
        code_hash: *code.repr_hash(),
        abi: Some(abi),
        compiler_abi,
    })
}

fn resolve_project_path(path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        configured_project_root().join(path)
    }
}

fn decode_storage_json(
    data: &Cell,
    abi: &CompilerContractABI,
    network: &Network,
) -> anyhow::Result<serde_json::Value> {
    let storage_ty = abi
        .storage
        .storage_at_deployment_ty
        .as_ref()
        .or(abi.storage.storage_ty.as_ref())
        .ok_or_else(|| anyhow!("Contract ABI does not declare storage"))?;
    let mut parser = data.as_slice_allow_exotic();
    let decoded = compiler_abi_serde::decode(&mut parser, abi, storage_ty)
        .context("Failed to decode storage with compiler ABI")?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        anyhow::bail!(
            "Storage cell has {} extra bits and {} extra refs after ABI decode",
            parser.size_bits(),
            parser.size_refs()
        );
    }
    Ok(compiler_data_to_json(&decoded, network))
}

fn compiler_data_to_json(data: &CompilerAbiData, network: &Network) -> serde_json::Value {
    match data {
        CompilerAbiData::Null => serde_json::Value::Null,
        CompilerAbiData::Number(value) => serde_json::Value::String(value.to_string()),
        CompilerAbiData::Bool(value) => serde_json::Value::Bool(*value),
        CompilerAbiData::String(value) | CompilerAbiData::Symbol(value) => {
            serde_json::Value::String(value.clone())
        }
        CompilerAbiData::Address(value) => {
            serde_json::Value::String(format_int_address(value, network))
        }
        CompilerAbiData::ExtAddress(value) => serde_json::json!({
            "bits": value.data_bit_len,
            "hex": hex::encode(&value.data),
        }),
        CompilerAbiData::Cell(value) => serde_json::json!({
            "boc64": Boc::encode_base64(value.clone()),
        }),
        CompilerAbiData::RemainingBitsAndRefs(value) => serde_json::json!({
            "boc64": Boc::encode_base64(value.clone()),
        }),
        CompilerAbiData::Bits((bytes, bit_len)) => serde_json::json!({
            "bits": bit_len,
            "hex": hex::encode(bytes),
        }),
        CompilerAbiData::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| compiler_data_to_json(value, network))
                .collect(),
        ),
        CompilerAbiData::Map(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|(key, value)| {
                    serde_json::json!({
                        "key": compiler_data_to_json(key, network),
                        "value": compiler_data_to_json(value, network),
                    })
                })
                .collect(),
        ),
        CompilerAbiData::Object(object) => {
            let mut result = serde_json::Map::new();
            for field in &object.fields {
                result.insert(
                    field.name.clone(),
                    compiler_data_to_json(&field.value, network),
                );
            }
            serde_json::Value::Object(result)
        }
    }
}

fn format_int_address(address: &IntAddr, network: &Network) -> String {
    match address {
        IntAddr::Std(address) => format_std_address(address, network),
        IntAddr::Var(address) => IntAddr::Var(address.clone()).to_string(),
    }
}

fn format_std_address(address: &StdAddr, network: &Network) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: network.uses_testnet_address_format(),
            bounceable: true,
            base64_url: false,
        },
    }
    .to_string()
}

fn format_nanotons(value: &BigInt) -> String {
    let sign = if value.sign() == Sign::Minus { "-" } else { "" };
    let digits = value.to_str_radix(10);
    let digits = digits.trim_start_matches('-');

    let formatted = if digits.len() <= 9 {
        let fractional = format!("{digits:0>9}");
        trim_fractional(format!("0.{fractional}"))
    } else {
        let (whole, fractional) = digits.split_at(digits.len() - 9);
        trim_fractional(format!("{whole}.{fractional}"))
    };

    format!("{sign}{formatted} TON")
}

const LABEL_WIDTH: usize = 18;

fn print_section(title: &str) {
    println!("\n{}", title.bold().cyan());
}

fn print_kv(label: &str, value: impl AsRef<str>) {
    let key = format!("{label}:");
    println!(
        "  {} {}",
        format!("{key:<LABEL_WIDTH$}").dimmed(),
        value.as_ref()
    );
}

fn format_account_status(status: &str) -> String {
    match status {
        "active" => status.green().to_string(),
        "frozen" => status.yellow().to_string(),
        "uninit" | "uninitialized" => status.yellow().to_string(),
        "nonexist" | "inactive" | "empty" => status.dimmed().to_string(),
        _ => status.to_string(),
    }
}

fn format_hash(hash: &HashBytes) -> String {
    format!("0x{hash}").yellow().to_string()
}

fn print_yaml_value(key: Option<&str>, value: &serde_json::Value, indent: usize) {
    let prefix = " ".repeat(indent);
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {
            let scalar = format_yaml_scalar(value);
            match key {
                Some(key) => println!("{prefix}{} {scalar}", format!("{key}:").dimmed()),
                None => println!("{prefix}{scalar}"),
            }
        }
        serde_json::Value::Array(values) => {
            if let Some(key) = key {
                if values.is_empty() {
                    println!("{prefix}{} []", format!("{key}:").dimmed());
                } else {
                    println!("{prefix}{}", format!("{key}:").dimmed());
                }
            } else if values.is_empty() {
                println!("{prefix}[]");
            }

            let next_indent = indent + usize::from(key.is_some()) * 2;
            for item in values {
                print_yaml_array_item(item, next_indent);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(key) = key {
                if map.is_empty() {
                    println!("{prefix}{} {{}}", format!("{key}:").dimmed());
                } else {
                    println!("{prefix}{}", format!("{key}:").dimmed());
                }
            } else if map.is_empty() {
                println!("{prefix}{{}}");
            }

            let next_indent = indent + usize::from(key.is_some()) * 2;
            for (child_key, child_value) in map {
                print_yaml_value(Some(child_key), child_value, next_indent);
            }
        }
    }
}

fn print_yaml_array_item(value: &serde_json::Value, indent: usize) {
    let prefix = " ".repeat(indent);
    match value {
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {
            println!("{prefix}- {}", format_yaml_scalar(value));
        }
        serde_json::Value::Array(values) => {
            if values.is_empty() {
                println!("{prefix}- []");
            } else {
                println!("{prefix}-");
                for child in values {
                    print_yaml_array_item(child, indent + 2);
                }
            }
        }
        serde_json::Value::Object(map) => {
            if map.is_empty() {
                println!("{prefix}- {{}}");
            } else {
                println!("{prefix}-");
                for (child_key, child_value) in map {
                    print_yaml_value(Some(child_key), child_value, indent + 2);
                }
            }
        }
    }
}

fn format_yaml_scalar(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "null".dimmed().to_string(),
        serde_json::Value::Bool(true) => "true".green().to_string(),
        serde_json::Value::Bool(false) => "false".bright_red().to_string(),
        serde_json::Value::Number(value) => value.to_string().white().to_string(),
        serde_json::Value::String(value) => colorize_scalar_string(value),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => unreachable!(),
    }
}

fn colorize_scalar_string(value: &str) -> String {
    if looks_like_address(value) {
        value.cyan().to_string()
    } else if looks_like_hash(value) {
        value.yellow().to_string()
    } else {
        value.to_string()
    }
}

fn looks_like_address(value: &str) -> bool {
    value.starts_with("EQ")
        || value.starts_with("UQ")
        || value.starts_with("kQ")
        || value.starts_with("0:")
        || value.starts_with("-1:")
}

fn looks_like_hash(value: &str) -> bool {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    stripped.len() == 64 && stripped.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn trim_fractional(mut formatted: String) -> String {
    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}
