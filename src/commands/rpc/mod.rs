use crate::commands::common::format_nanograms;
use crate::contract_interface::{
    compile_optional_contract_interface_with_cache, is_boc_path, read_precompiled_boc,
};
use crate::file_build_cache::FileBuildCache;
use acton_config::color::{OwoColorize, colors_enabled};
use acton_config::config::{ActonConfig, ContractConfig, project_root as configured_project_root};
use acton_debug::{PrettyAddressFormat, PrettyRenderOptions, render_unpacked_value_as_tolk_type};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use log::warn;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::{ABIGetMethod, ContractABI};
use tolk_compiler::dynamic_unpack;
use ton_api::{Network, TonApiClient};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, IntAddr, StdAddr};

mod call;
mod info;
mod trace;

#[derive(Subcommand, Clone)]
pub enum RpcCommand {
    #[command(about = "Show remote account information and decode storage when ABI is known")]
    Info {
        #[arg(help = "Contract address in friendly or raw format")]
        address: String,
        #[arg(long, help = "Masterchain block seqno to query account state at")]
        block_number: Option<u64>,
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
        #[arg(long, help = "Print machine-readable JSON output")]
        json: bool,
        #[arg(long, help = "Skip domain inspectors such as jetton detection")]
        raw: bool,
    },
    #[command(about = "Call a contract get-method through TonCenter")]
    Call {
        #[arg(help = "Contract address in friendly or raw format")]
        address: String,
        #[arg(help = "Get-method name")]
        method: String,
        #[arg(
            help = "Arguments to pass to the get-method",
            allow_hyphen_values = true
        )]
        args: Vec<String>,
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
        #[arg(
            long,
            help = "Masterchain block seqno to query account state and run get-method at"
        )]
        block_number: Option<u64>,
        #[arg(long, help = "Print machine-readable JSON output")]
        json: bool,
        #[arg(long, help = "Print the raw TonCenter stack without ABI decoding")]
        raw: bool,
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
        RpcCommand::Info {
            address,
            block_number,
            net,
            json,
            raw,
        } => info::rpc_info_cmd(&address, net, block_number, json, raw),
        RpcCommand::Call {
            address,
            method,
            args,
            net,
            block_number,
            json,
            raw,
        } => call::rpc_call_cmd(&address, &method, &args, net, block_number, json, raw),
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

pub(super) fn resolve_rpc_network(net: Option<String>) -> anyhow::Result<Network> {
    net.as_deref()
        .map(Network::from_str)
        .transpose()
        .map(|network| network.unwrap_or(Network::Testnet))
}

pub(super) fn load_rpc_config() -> anyhow::Result<ActonConfig> {
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

pub(super) struct LocalContractMatch {
    pub(super) contract_name: String,
    pub(super) abi: Option<Arc<ContractABI>>,
}

pub(super) fn find_local_contract_match(
    code_hash: &HashBytes,
    config: &ActonConfig,
) -> anyhow::Result<Option<LocalContractMatch>> {
    for candidate in load_local_contract_candidates(config)? {
        if &candidate.code_hash == code_hash {
            return Ok(Some(LocalContractMatch {
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
        let interface = compile_optional_contract_interface_with_cache(
            config,
            configured_project_root(),
            contract_id,
            contract,
            file_cache,
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

fn decode_storage(data: &Cell, abi: &ContractABI, network: &Network) -> anyhow::Result<String> {
    let storage_ty_idx = abi
        .storage
        .storage_at_deployment_ty_idx
        .or(abi.storage.storage_ty_idx)
        .ok_or_else(|| anyhow!("Contract ABI does not declare storage"))?;
    let mut parser = data.as_slice_allow_exotic();
    let decoded = dynamic_unpack::unpack_from_slice(&mut parser, abi, storage_ty_idx)
        .context("Failed to decode storage with compiler ABI")?;
    if parser.size_bits() != 0 || parser.size_refs() != 0 {
        anyhow::bail!(
            "Storage cell has {} extra bits and {} extra refs after type decode",
            parser.size_bits(),
            parser.size_refs()
        );
    }
    let rendered = render_unpacked_value_as_tolk_type(abi, decoded, storage_ty_idx);
    let options = PrettyRenderOptions {
        address_format: pretty_address_format(network),
        address_labels: Default::default(),
        colorize: colors_enabled(),
    };
    Ok(rendered.to_pretty_string(options))
}

const fn pretty_address_format(network: &Network) -> PrettyAddressFormat {
    if network.uses_testnet_address_format() {
        PrettyAddressFormat::Testnet
    } else {
        PrettyAddressFormat::Mainnet
    }
}

pub(super) fn format_int_address(address: &IntAddr, network: &Network) -> String {
    match address {
        IntAddr::Std(address) => format_std_address(address, network),
        IntAddr::Var(address) => IntAddr::Var(address.clone()).to_string(),
    }
}

pub(super) fn format_std_address(address: &StdAddr, network: &Network) -> String {
    DisplayBase64StdAddr {
        addr: address,
        flags: Base64StdAddrFlags {
            testnet: network.uses_testnet_address_format(),
            bounceable: true,
            base64_url: true,
        },
    }
    .to_string()
}

const LABEL_WIDTH: usize = 18;

pub(super) fn print_section_title(title: &str) {
    println!("{}", title.bold().cyan());
}

pub(super) fn print_section(title: &str) {
    println!("\n{}", title.bold().cyan());
}

pub(super) fn print_kv(label: &str, value: impl AsRef<str>) {
    let key = format!("{label}:");
    println!(
        "  {} {}",
        format!("{key:<LABEL_WIDTH$}").dimmed(),
        value.as_ref()
    );
}

fn print_indented_block(value: &str, indent: usize) {
    let prefix = " ".repeat(indent);
    for line in value.lines() {
        println!("{prefix}{line}");
    }
}

fn print_get_methods(abi: &ContractABI) {
    if abi.get_methods.is_empty() {
        println!("  {}", "none".dimmed());
        return;
    }

    for method in &abi.get_methods {
        println!("  {}", format_get_method_signature_colored(abi, method));
    }
}

fn print_get_method_hint(address: &str, network: &Network, block_number: Option<u64>) {
    let net_arg = format_rpc_network_arg(network);
    let block_arg = block_number
        .map(|block_number| format!(" --block-number {block_number}"))
        .unwrap_or_default();
    let command = format!("acton rpc call --net {net_arg}{block_arg} {address} <METHOD> [ARGS...]");

    println!(
        "\n{}",
        format!("hint: To run get method: {command}").dimmed()
    );
}

fn format_rpc_network_arg(network: &Network) -> String {
    match network {
        Network::Custom(name) => format!("custom:{name}"),
        Network::Mainnet | Network::Testnet | Network::Localnet => network.to_string(),
    }
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

pub(super) fn format_get_method_signature(abi: &ContractABI, method: &ABIGetMethod) -> String {
    format_get_method_signature_with_name(
        method.name.to_owned(),
        format_get_method_signature_suffix(abi, method),
    )
}

pub(super) fn format_get_method_signature_colored(
    abi: &ContractABI,
    method: &ABIGetMethod,
) -> String {
    format_get_method_signature_with_name(
        method.name.yellow().to_string(),
        format_get_method_signature_suffix(abi, method)
            .dimmed()
            .to_string(),
    )
}

pub(super) fn format_get_method_signature_with_name(
    method_name: String,
    signature_suffix: String,
) -> String {
    format!("{method_name}{signature_suffix}")
}

pub(super) fn format_get_method_signature_suffix(
    abi: &ContractABI,
    method: &ABIGetMethod,
) -> String {
    let params = method
        .parameters
        .iter()
        .map(|param| format!("{}: {}", param.name, abi.render_type(param.ty_idx)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("({}): {}", params, abi.render_type(method.return_ty_idx))
}
