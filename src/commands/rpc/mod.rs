use crate::commands::common::{error_fmt, format_nanotons};
use crate::contract_interface::{
    compile_optional_contract_interface, is_boc_path, read_precompiled_boc,
};
use crate::file_build_cache::FileBuildCache;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use log::warn;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use tolk_compiler::dynamic_unpack::{self, UnpackedValue};
use ton_api::{Network, TonApiClient};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::models::{
    Base64StdAddrFlags, DisplayBase64StdAddr, IntAddr, StdAddr, StdAddrFormat,
};

mod trace;

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
    },
    #[command(about = "Print the latest masterchain block info returned by TonCenter")]
    Block {
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
    },
    #[command(about = "Print the latest masterchain block number for a network")]
    BlockNumber {
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
    },
    #[command(about = "Render a TonCenter v3 trace as a decoded transaction tree")]
    Trace {
        #[arg(help = "Root transaction hash to query through TonCenter v3 /traces")]
        hash: String,
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
        #[arg(
            long,
            conflicts_with_all = ["tree", "verbose"],
            help = "Print only the trace summary"
        )]
        summary: bool,
        #[arg(
            long,
            conflicts_with_all = ["summary", "verbose"],
            help = "Print the trace summary and transaction tree (default)"
        )]
        tree: bool,
        #[arg(
            long,
            conflicts_with_all = ["summary", "tree"],
            help = "Print the summary, tree, and stable per-transaction fields"
        )]
        verbose: bool,
        #[arg(long, help = "Print decoded message bodies in the transaction tree")]
        show_bodies: bool,
    },
}

pub fn rpc_cmd(command: RpcCommand) -> anyhow::Result<()> {
    match command {
        RpcCommand::Info { address, net } => rpc_info_cmd(&address, net),
        RpcCommand::Block { net } => rpc_block_cmd(net),
        RpcCommand::BlockNumber { net } => rpc_block_number_cmd(net),
        RpcCommand::Trace {
            hash,
            net,
            summary,
            tree: _,
            verbose,
            show_bodies,
        } => trace::rpc_trace_cmd(&hash, net, summary, verbose, show_bodies),
    }
}

fn rpc_info_cmd(address: &str, net: Option<String>) -> anyhow::Result<()> {
    let (address, _) = StdAddr::from_str_ext(address, StdAddrFormat::any())
        .map_err(|_| anyhow!("Invalid address"))
        .with_context(|| error_fmt::invalid_address(address))?;

    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

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
            .abi
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

fn rpc_block_cmd(net: Option<String>) -> anyhow::Result<()> {
    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

    let block = client
        .get_masterchain_info()
        .with_context(|| format!("Failed to fetch latest block from {network}"))?;
    println!("{}", serde_json::to_string_pretty(&block)?);

    Ok(())
}

fn rpc_block_number_cmd(net: Option<String>) -> anyhow::Result<()> {
    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

    let seqno = client
        .get_last_block_seqno()
        .with_context(|| format!("Failed to fetch latest block from {network}"))?;
    println!("{seqno}");

    Ok(())
}

fn resolve_rpc_network(net: Option<String>) -> anyhow::Result<Network> {
    net.as_deref()
        .map(Network::from_str)
        .transpose()
        .map(|network| network.unwrap_or(Network::Testnet))
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
    abi: Option<Arc<ContractABI>>,
}

fn find_local_contract_match(
    code_hash: &HashBytes,
    config: &ActonConfig,
) -> anyhow::Result<Option<LocalContractMatch>> {
    for candidate in load_local_contract_candidates(config)? {
        if &candidate.code_hash == code_hash {
            return Ok(Some(LocalContractMatch {
                contract_id: candidate.contract_id,
                contract_name: candidate.contract_name,
                abi: candidate.abi,
            }));
        }
    }

    Ok(None)
}

fn load_local_contract_candidates(
    config: &ActonConfig,
) -> anyhow::Result<Vec<LocalContractCandidate>> {
    let manifest_path = acton_config::config::manifest_path();
    if !manifest_path.exists() {
        return Ok(Vec::new());
    }

    let Some(contracts) = config.contracts() else {
        return Ok(Vec::new());
    };
    let mut file_cache = FileBuildCache::new(None).ok();
    let mut candidates = Vec::new();

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
        candidates.push(candidate);
    }

    Ok(candidates)
}

struct LocalContractCandidate {
    contract_path: PathBuf,
    contract_id: String,
    contract_name: String,
    code_boc64: String,
    code_hash: HashBytes,
    abi: Option<Arc<ContractABI>>,
    source_map: Option<Arc<SourceMap>>,
}

fn load_local_contract_candidate(
    contract_id: &str,
    contract: &ContractConfig,
    config: &ActonConfig,
    mut file_cache: Option<&mut FileBuildCache>,
) -> anyhow::Result<LocalContractCandidate> {
    let contract_path = contract.absolute_source_path(configured_project_root());
    if is_boc_path(&contract_path) {
        let precompiled = read_precompiled_boc(&contract_path, &contract.src)?;
        let interface = compile_optional_contract_interface(
            config,
            configured_project_root(),
            contract_id,
            contract,
        )?;
        let (abi, source_map) = interface.map_or((None, None), |interface| {
            (
                Some(Arc::new(interface.abi)),
                Some(Arc::new(interface.source_map)),
            )
        });
        return Ok(LocalContractCandidate {
            contract_path,
            contract_id: contract_id.to_owned(),
            contract_name: contract.display_name(contract_id).to_owned(),
            code_boc64: precompiled.code_boc64,
            code_hash: precompiled.code_hash,
            abi,
            source_map,
        });
    }

    let contract_path_key = contract_path.to_string_lossy().to_string();
    let cached = file_cache
        .as_mut()
        .and_then(|cache| cache.get(&contract_path_key, false, false, 2, "1.4"));
    let (code_boc64, abi, source_map) = if let Some(cached) = cached {
        (
            cached.code_boc64,
            cached.abi.map(Arc::new),
            cached.source_map.map(Arc::new),
        )
    } else {
        let compiler = tolk_compiler::Compiler::new(2).with_mappings(&config.mappings());
        match compiler.compile(&contract_path, false) {
            tolk_compiler::CompilerResult::Success(result) => {
                if let Some(cache) = file_cache.as_mut() {
                    let _ = cache.put(&contract_path_key, &result, false, false, 2, "1.4");
                }
                (
                    result.code_boc64,
                    result.abi.map(Arc::new),
                    result.source_map.map(Arc::new),
                )
            }
            tolk_compiler::CompilerResult::Error(err) => {
                return Err(anyhow!(err.message)
                    .context(format!("Failed to compile {}", contract_path.display())));
            }
        }
    };

    let code = Boc::decode_base64(&code_boc64)
        .with_context(|| format!("Failed to decode code for {}", contract_path.display()))?;

    Ok(LocalContractCandidate {
        contract_path,
        contract_id: contract_id.to_owned(),
        contract_name: contract.display_name(contract_id).to_owned(),
        code_boc64,
        code_hash: *code.repr_hash(),
        abi,
        source_map,
    })
}

fn decode_storage_json(
    data: &Cell,
    abi: &ContractABI,
    network: &Network,
) -> anyhow::Result<serde_json::Value> {
    let storage_ty_idx = abi
        .storage
        .storage_at_deployment_ty_idx
        .or(abi.storage.storage_ty_idx)
        .ok_or_else(|| anyhow!("Contract ABI does not declare storage"))?;
    let mut parser = data.as_slice_allow_exotic();
    let decoded = dynamic_unpack::unpack_from_abi_slice(&mut parser, abi, storage_ty_idx)
        .context("Failed to decode storage with compiler ABI")?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        anyhow::bail!(
            "Storage cell has {} extra bits and {} extra refs after type decode",
            parser.size_bits(),
            parser.size_refs()
        );
    }
    Ok(compiler_data_to_json(&decoded, network))
}

fn compiler_data_to_json(data: &UnpackedValue, network: &Network) -> serde_json::Value {
    match data {
        UnpackedValue::Null | UnpackedValue::Void => serde_json::Value::Null,
        UnpackedValue::Number(value) => serde_json::Value::String(value.to_string()),
        UnpackedValue::Bool(value) => serde_json::Value::Bool(*value),
        UnpackedValue::String(value) => serde_json::Value::String(value.clone()),
        UnpackedValue::Address(value) => {
            serde_json::Value::String(format_int_address(value, network))
        }
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
        UnpackedValue::Array(values) => serde_json::Value::Array(
            values
                .iter()
                .map(|value| compiler_data_to_json(value, network))
                .collect(),
        ),
        UnpackedValue::Map(values) => serde_json::Value::Array(
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
        UnpackedValue::Object { fields, .. } => {
            let mut result = serde_json::Map::new();
            for (name, value) in fields {
                result.insert(name.clone(), compiler_data_to_json(value, network));
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
        "frozen" | "uninit" | "uninitialized" => status.yellow().to_string(),
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
