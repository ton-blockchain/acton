use crate::commands::common::error_fmt;
use crate::context::{BuildCache, KnownAddresses};
use crate::ffi::emulation::{
    V3TraceTransaction, V3TraceTransactions, build_v3_trace_transactions,
    tx_cell_to_send_result_tuple_with_relations, v3_message_hash,
};
use crate::file_build_cache::FileBuildCache;
use crate::formatter::FormatterContext;
use acton_config::color::OwoColorize;
use acton_config::config::{ActonConfig, ContractConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use clap::Subcommand;
use log::warn;
use num_bigint::{BigInt, Sign};
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tolk_compiler::TolkSourceMap;
use tolk_compiler::abi::ContractABI as CompilerContractABI;
use ton_abi::abi_serde::Data as CompilerAbiData;
use ton_abi::{ContractAbi, compiler_abi_serde, contract_abi};
use ton_api::{
    AccountState as TonApiAccountState, Network, TonApiClient, V3MessageSummary, V3Trace,
    V3TransactionSummary,
};
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes, Lazy};
use tycho_types::models::{
    Account, AccountState as TychoAccountState, Base64StdAddrFlags, CurrencyCollection,
    DisplayBase64StdAddr, IntAddr, OptionalAccount, ShardAccount, StateInit, StdAddr,
    StdAddrFormat, StorageInfo, Transaction,
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
    },
    #[command(about = "Print the latest masterchain block number for a network")]
    LatestBlock {
        #[arg(
            long,
            help = "Network to query (defaults to testnet). Supported values: mainnet, testnet, localnet, custom:<name>"
        )]
        net: Option<String>,
    },
    #[command(about = "Print a TonCenter v3 transaction trace as an Acton transaction tree")]
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
        RpcCommand::LatestBlock { net } => rpc_latest_block_cmd(net),
        RpcCommand::Trace {
            hash,
            net,
            summary,
            tree,
            verbose,
            show_bodies,
        } => rpc_trace_cmd(
            &hash,
            net,
            trace_output_mode(summary, tree, verbose),
            show_bodies,
        ),
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

fn rpc_latest_block_cmd(net: Option<String>) -> anyhow::Result<()> {
    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

    let seqno = client
        .get_last_block_seqno()
        .with_context(|| format!("Failed to fetch latest block from {network}"))?;
    println!("{seqno}");

    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TraceOutputMode {
    Summary,
    Tree,
    Verbose,
}

const fn trace_output_mode(summary: bool, _tree: bool, verbose: bool) -> TraceOutputMode {
    if summary {
        TraceOutputMode::Summary
    } else if verbose {
        TraceOutputMode::Verbose
    } else {
        TraceOutputMode::Tree
    }
}

fn rpc_trace_cmd(
    hash: &str,
    net: Option<String>,
    mode: TraceOutputMode,
    show_bodies: bool,
) -> anyhow::Result<()> {
    let network = resolve_rpc_network(net)?;
    let config = load_rpc_config()?;
    let client = TonApiClient::new(network.clone(), config.custom_networks())?;

    let mut traces = client
        .get_traces_by_tx_hash(hash, 1)
        .with_context(|| format!("Failed to fetch trace {hash} from {network}"))?;
    let trace = traces
        .pop()
        .ok_or_else(|| anyhow!("No trace found for transaction hash {hash} on {network}"))?;

    if mode == TraceOutputMode::Summary {
        print_rpc_trace_summary(hash, &trace);
        return Ok(());
    }

    let trace_txs = match build_v3_trace_transactions(&trace)? {
        V3TraceTransactions::Ready(transactions) => transactions,
        V3TraceTransactions::Pending { tx_hash } => {
            anyhow::bail!("Trace references missing transaction {tx_hash}");
        }
    };
    ensure_rpc_trace_in_msgs(&trace_txs)?;
    print_rpc_trace_summary(hash, &trace);
    let formatter = rpc_trace_formatter(&trace_txs, &client, &network, &config, show_bodies)?;

    print_section("Trace Tree");
    let send_results = trace_txs
        .iter()
        .map(|tx| {
            tx_cell_to_send_result_tuple_with_relations(
                tx.tx_cell.clone(),
                &tx.transaction,
                &tx.child_lts,
                tx.parent_lt,
            )
        })
        .collect::<Vec<_>>();
    let send_result_list = TupleItem::TypedTuple {
        type_name: "SendResultList".to_owned(),
        inner: Tuple(send_results),
    };
    let formatted_tree = formatter.format(&send_result_list);
    println!("{}", formatted_tree.trim_end());

    if mode == TraceOutputMode::Verbose {
        print_section("Trace Details");
        print_rpc_trace_details(&trace_txs, Some(&formatter), &network);
    }

    Ok(())
}

fn ensure_rpc_trace_in_msgs(trace_txs: &[V3TraceTransaction]) -> anyhow::Result<()> {
    for tx in trace_txs {
        if tx.summary.in_msg.is_none() {
            anyhow::bail!(
                "Trace transaction {} has no in_msg in TonCenter v3 /traces response",
                tx.hash
            );
        }
    }
    Ok(())
}

fn rpc_trace_formatter(
    trace_txs: &[V3TraceTransaction],
    client: &TonApiClient,
    network: &Network,
    config: &ActonConfig,
    show_bodies: bool,
) -> anyhow::Result<FormatterContext<'static>> {
    let build_cache = load_local_build_cache(config)?;
    let accounts = if build_cache.built.is_empty() {
        FxHashMap::default()
    } else {
        match fetch_trace_accounts(trace_txs, client) {
            Ok(accounts) => accounts,
            Err(err) => {
                warn!("Skipping rpc trace local ABI matching: {err:#}");
                FxHashMap::default()
            }
        }
    };

    let mut formatter = FormatterContext::empty();
    formatter.accounts = Cow::Owned(accounts);
    formatter.build_cache = Cow::Owned(build_cache);
    formatter.known_addresses = Cow::Owned(KnownAddresses::new());
    formatter.show_bodies = show_bodies;
    formatter.network = Some(network.clone());
    Ok(formatter)
}

fn print_rpc_trace_summary(query_hash: &str, trace: &V3Trace) {
    print_section("Trace Summary");
    print_kv("Query Hash", query_hash);
    print_kv("Trace ID", trace.trace_id.as_str());
    print_kv(
        "Root Tx Hash",
        trace
            .transactions_order
            .first()
            .map(String::as_str)
            .unwrap_or("<none>"),
    );
    print_kv("Trace Complete", (!trace.is_incomplete).to_string());
    print_kv("Total Txs", trace.transactions_order.len().to_string());
    print_kv("Total Messages", trace_message_count(trace).to_string());
}

fn trace_message_count(trace: &V3Trace) -> usize {
    let mut unique = BTreeSet::new();

    for tx_hash in &trace.transactions_order {
        let Some(tx) = trace.transactions.get(tx_hash) else {
            continue;
        };
        count_trace_message(tx_hash, "in", tx.in_msg.as_ref(), &mut unique);
        for (idx, msg) in tx.out_msgs.iter().enumerate() {
            let synthetic_id = format!("out:{idx}");
            count_trace_message(tx_hash, &synthetic_id, Some(msg), &mut unique);
        }
    }

    unique.len()
}

fn count_trace_message(
    tx_hash: &str,
    synthetic_id: &str,
    message: Option<&V3MessageSummary>,
    unique: &mut BTreeSet<String>,
) {
    let Some(message) = message else {
        return;
    };
    if let Some(hash) = v3_message_hash(Some(message)) {
        unique.insert(hash.to_owned());
    } else {
        unique.insert(format!("{tx_hash}:{synthetic_id}"));
    }
}

fn print_rpc_trace_details(
    trace_txs: &[V3TraceTransaction],
    formatter: Option<&FormatterContext<'_>>,
    network: &Network,
) {
    for (idx, tx) in trace_txs.iter().enumerate() {
        let prefix = format!("tx[{}]", idx + 1);
        println!("  {prefix}:");
        println!("    hash: {}", tx.hash);
        println!("    lt: {}", tx.transaction.lt);
        println!(
            "    account: {}",
            format_trace_address(&tx.summary.account, network)
        );
        println!("    parent_lt: {}", format_optional_u64(tx.parent_lt));
        println!("    child_lts: {}", format_child_lts(&tx.child_lts));

        if let Some(message) = &tx.summary.in_msg {
            println!(
                "    from: {}",
                format_optional_address(&message.source, network)
            );
            println!(
                "    to: {}",
                format_optional_address(&message.destination, network)
            );
            println!("    value: {}", format_message_value(message));
            println!(
                "    opcode: {}",
                format_message_opcode(&tx.transaction, message, formatter)
            );
            println!("    bounced: {}", message.bounced.unwrap_or(false));
            println!(
                "    branch: {}",
                trace_branch_kind(&tx.transaction, &tx.summary, message, formatter)
            );
        } else {
            println!("    from: <none>");
            println!("    to: <none>");
            println!("    value: <none>");
            println!("    opcode: <none>");
            println!("    bounced: false");
            println!("    branch: system");
        }

        println!("    success: {}", trace_tx_success(&tx.summary));
        println!("    exit_code: {}", format_compute_exit_code(&tx.summary));
        println!(
            "    action_result_code: {}",
            format_action_result_code(&tx.summary)
        );
    }
}

fn format_optional_u64(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), |value| value.to_string())
}

fn format_child_lts(child_lts: &[u64]) -> String {
    if child_lts.is_empty() {
        return "[]".to_owned();
    }
    format!(
        "[{}]",
        child_lts
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn trace_tx_success(tx: &V3TransactionSummary) -> bool {
    let Some(description) = &tx.description else {
        return false;
    };
    if description.aborted.unwrap_or(false) {
        return false;
    }
    if let Some(compute) = &description.compute_ph
        && (!compute.success.unwrap_or(false) || compute.exit_code.unwrap_or(0) != 0)
    {
        return false;
    }
    if let Some(action) = &description.action
        && (!action.success.unwrap_or(false) || action.result_code.unwrap_or(0) != 0)
    {
        return false;
    }
    true
}

fn format_compute_exit_code(tx: &V3TransactionSummary) -> String {
    tx.description
        .as_ref()
        .and_then(|description| description.compute_ph.as_ref())
        .and_then(|compute| compute.exit_code)
        .map_or_else(|| "null".to_owned(), |code| code.to_string())
}

fn format_action_result_code(tx: &V3TransactionSummary) -> String {
    tx.description
        .as_ref()
        .and_then(|description| description.action.as_ref())
        .and_then(|action| action.result_code)
        .map_or_else(|| "null".to_owned(), |code| code.to_string())
}

fn trace_branch_kind(
    parsed_tx: &Transaction,
    tx: &V3TransactionSummary,
    message: &V3MessageSummary,
    formatter: Option<&FormatterContext<'_>>,
) -> &'static str {
    if message.bounced.unwrap_or(false) {
        return "bounce";
    }
    if matches!(
        tx.orig_status.as_deref(),
        Some("nonexist" | "uninit" | "uninitialized")
    ) && tx.end_status.as_deref() == Some("active")
    {
        return "deploy";
    }
    if resolved_message_name(parsed_tx, formatter)
        .is_some_and(|name| name.to_ascii_lowercase().contains("notification"))
    {
        return "notification";
    }
    "message"
}

fn format_message_opcode(
    parsed_tx: &Transaction,
    message: &V3MessageSummary,
    formatter: Option<&FormatterContext<'_>>,
) -> String {
    let opcode = extract_message_opcode(message);
    let opcode_text = if opcode == 0 {
        "0x00000000".to_owned()
    } else {
        format!("0x{opcode:08x}")
    };
    let name = resolved_message_name(parsed_tx, formatter)
        .or_else(|| (opcode == 0).then(|| "empty".to_owned()));
    match name {
        Some(name) => format!("{opcode_text} ({name})"),
        None => opcode_text,
    }
}

fn extract_message_opcode(message: &V3MessageSummary) -> u32 {
    let Some(body_boc64) = message
        .message_content
        .as_ref()
        .and_then(|content| content.body.as_deref())
        .filter(|body| !body.is_empty())
    else {
        return 0;
    };
    let Ok(body) = Boc::decode_base64(body_boc64) else {
        return 0;
    };
    let mut parser = body.as_slice_allow_exotic();
    if message.bounced.unwrap_or(false) {
        parser.load_u32().unwrap_or(0);
    }
    parser.load_u32().unwrap_or(0)
}

fn resolved_message_name(
    parsed_tx: &Transaction,
    formatter: Option<&FormatterContext<'_>>,
) -> Option<String> {
    formatter?.transaction_inbound_message_name(parsed_tx)
}

fn format_message_value(message: &V3MessageSummary) -> String {
    let Some(value) = message.value.as_deref() else {
        return "<none>".to_owned();
    };
    match BigInt::from_str(value) {
        Ok(value) => format_nanotons(&value),
        Err(_) => value.to_owned(),
    }
}

fn format_optional_address(address: &Option<String>, network: &Network) -> String {
    address.as_deref().map_or_else(
        || "<none>".to_owned(),
        |address| format_trace_address(address, network),
    )
}

fn format_trace_address(address: &str, network: &Network) -> String {
    StdAddr::from_str_ext(address, StdAddrFormat::any()).map_or_else(
        |_| address.to_owned(),
        |(address, _)| format_std_address(&address, network),
    )
}

fn fetch_trace_accounts(
    trace_txs: &[V3TraceTransaction],
    client: &TonApiClient,
) -> anyhow::Result<FxHashMap<StdAddr, ShardAccount>> {
    let mut addresses = BTreeMap::new();
    for tx in trace_txs {
        collect_trace_address(&tx.summary.account, &mut addresses);
        if let Some(in_msg) = &tx.summary.in_msg {
            collect_optional_trace_address(&in_msg.source, &mut addresses);
            collect_optional_trace_address(&in_msg.destination, &mut addresses);
        }
        for out_msg in &tx.summary.out_msgs {
            collect_optional_trace_address(&out_msg.source, &mut addresses);
            collect_optional_trace_address(&out_msg.destination, &mut addresses);
        }
    }

    let address_strings = addresses.keys().cloned().collect::<Vec<_>>();
    let address_refs = address_strings
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let states = client
        .get_account_states(&address_refs)
        .context("Failed to fetch trace account states")?;

    let mut accounts = FxHashMap::default();
    for state in states {
        if let Some((address, account)) = shard_account_from_ton_api_state(&state)? {
            accounts.insert(address, account);
        }
    }
    Ok(accounts)
}

fn collect_optional_trace_address(address: &Option<String>, addresses: &mut BTreeMap<String, ()>) {
    if let Some(address) = address {
        collect_trace_address(address, addresses);
    }
}

fn collect_trace_address(address: &str, addresses: &mut BTreeMap<String, ()>) {
    let Ok((address, _)) = StdAddr::from_str_ext(address, StdAddrFormat::any()) else {
        return;
    };
    addresses.insert(address.to_string(), ());
}

fn shard_account_from_ton_api_state(
    state: &TonApiAccountState,
) -> anyhow::Result<Option<(StdAddr, ShardAccount)>> {
    let (address, _) =
        StdAddr::from_str_ext(&state.address, StdAddrFormat::any()).map_err(|_| {
            anyhow!(
                "Invalid account address in accountStates: {}",
                state.address
            )
        })?;
    if state.status != "active" || state.code_boc.is_none() {
        return Ok(None);
    }

    let code = state
        .code_boc
        .as_ref()
        .map(Boc::decode_base64)
        .transpose()
        .context("Failed to decode account code BoC")?;
    let balance = state
        .balance
        .as_deref()
        .and_then(|balance| balance.parse::<u128>().ok())
        .unwrap_or(0);
    let account = Account {
        balance: CurrencyCollection::new(balance),
        address: IntAddr::Std(address.clone()),
        last_trans_lt: 0,
        state: TychoAccountState::Active(StateInit {
            code,
            data: None,
            ..Default::default()
        }),
        storage_stat: StorageInfo::default(),
    };
    let shard_account = ShardAccount {
        account: Lazy::new(&OptionalAccount(Some(account)))
            .context("Failed to build account state for trace formatting")?,
        last_trans_hash: HashBytes::ZERO,
        last_trans_lt: 0,
    };
    Ok(Some((address, shard_account)))
}

fn load_local_build_cache(config: &ActonConfig) -> anyhow::Result<BuildCache> {
    let mut build_cache = BuildCache::new();
    for candidate in load_local_contract_candidates(config)? {
        build_cache.memoize(
            &candidate.contract_name,
            &candidate.contract_path,
            &candidate.code_boc64,
            candidate.code_hash,
            Arc::new(TolkSourceMap::without_debug_info()),
            candidate.abi,
            candidate.compiler_abi,
        );
    }
    Ok(build_cache)
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
    abi: Option<Arc<ContractAbi>>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
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
                compiler_abi: candidate.compiler_abi,
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
    abi: Option<Arc<ContractAbi>>,
    compiler_abi: Option<Arc<CompilerContractABI>>,
}

fn load_local_contract_candidate(
    contract_id: &str,
    contract: &ContractConfig,
    config: &ActonConfig,
    mut file_cache: Option<&mut FileBuildCache>,
) -> anyhow::Result<LocalContractCandidate> {
    let contract_path = contract.absolute_source_path(configured_project_root());
    if contract_path.extension().is_some_and(|ext| ext == "boc") {
        let boc = fs::read(&contract_path)
            .with_context(|| format!("Failed to read {}", contract_path.display()))?;
        let code = Boc::decode(boc)
            .with_context(|| format!("Failed to decode {}", contract_path.display()))?;
        let code_boc64 = Boc::encode_base64(&code);
        return Ok(LocalContractCandidate {
            contract_path,
            contract_id: contract_id.to_owned(),
            contract_name: contract.display_name(contract_id).to_owned(),
            code_boc64,
            code_hash: *code.repr_hash(),
            abi: None,
            compiler_abi: None,
        });
    }

    let contract_path_key = contract_path.to_string_lossy().to_string();
    let cached = file_cache
        .as_mut()
        .and_then(|cache| cache.get(&contract_path_key, false, false, 2, "1.3"));
    let (code_boc64, compiler_abi) = if let Some(cached) = cached {
        (cached.code_boc64, cached.abi.map(Arc::new))
    } else {
        let compiler = tolk_compiler::Compiler::new(2).with_mappings(&config.mappings());
        match compiler.compile(&contract_path, false) {
            tolk_compiler::CompilerResult::Success(result) => {
                if let Some(cache) = file_cache.as_mut() {
                    let _ = cache.put(&contract_path_key, &result, false, false, 2, "1.3");
                }
                (result.code_boc64, result.abi.map(Arc::new))
            }
            tolk_compiler::CompilerResult::Error(err) => {
                return Err(anyhow!(err.message)
                    .context(format!("Failed to compile {}", contract_path.display())));
            }
        }
    };

    let code = Boc::decode_base64(&code_boc64)
        .with_context(|| format!("Failed to decode code for {}", contract_path.display()))?;
    let content = fs::read_to_string(&contract_path)
        .with_context(|| format!("Failed to read {}", contract_path.display()))?;
    let path = contract_path.to_string_lossy().to_string();
    let abi = Arc::new(contract_abi(content.into(), &path, &config.mappings()));

    Ok(LocalContractCandidate {
        contract_path,
        contract_id: contract_id.to_owned(),
        contract_name: contract.display_name(contract_id).to_owned(),
        code_boc64,
        code_hash: *code.repr_hash(),
        abi: Some(abi),
        compiler_abi,
    })
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
        CompilerAbiData::Cell(value) | CompilerAbiData::RemainingBitsAndRefs(value) => {
            serde_json::json!({
                "boc64": Boc::encode_base64(value.clone()),
            })
        }
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

fn trim_fractional(mut formatted: String) -> String {
    while formatted.ends_with('0') {
        formatted.pop();
    }
    if formatted.ends_with('.') {
        formatted.pop();
    }
    formatted
}
