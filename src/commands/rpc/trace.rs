use super::{
    format_nanotons, format_std_address, load_local_contract_candidates, load_rpc_config, print_kv,
    resolve_rpc_network,
};
use crate::context::{BuildCache, KnownAddresses};
use crate::ffi::emulation::{
    V3TraceTransaction, V3TraceTransactions, build_v3_trace_transactions, v3_message_hash,
};
use crate::formatter::FormatterContext;
use acton_config::color::OwoColorize;
use acton_config::config::ActonConfig;
use anyhow::{Context, anyhow};
use chrono::{TimeZone, Utc};
use log::warn;
use num_bigint::BigInt;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;
use tolk_compiler::SourceMap;
use ton_api::{
    AccountState as TonApiAccountState, Network, TonApiClient, V3MessageSummary, V3Trace,
    V3TransactionSummary,
};
use tvm_ffi::stack::TupleItem;
use tycho_types::boc::Boc;
use tycho_types::cell::{HashBytes, Lazy};
use tycho_types::models::{
    Account, AccountState as TychoAccountState, CurrencyCollection, IntAddr, OptionalAccount,
    ShardAccount, StateInit, StdAddr, StdAddrFormat, StorageInfo,
};

pub(super) fn rpc_trace_cmd(
    hash: &str,
    net: Option<String>,
    summary: bool,
    verbose: bool,
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

    if summary {
        print_rpc_trace_summary(hash, &trace);
        return Ok(());
    }

    let trace_txs = match build_v3_trace_transactions(&trace)? {
        V3TraceTransactions::Ready(transactions) => transactions,
        V3TraceTransactions::Pending { tx_hash } => {
            anyhow::bail!("Trace references missing transaction {tx_hash}");
        }
    };
    let formatter = rpc_trace_formatter(&trace_txs, &client, &network, &config, show_bodies)?;

    let send_result_list: Vec<TupleItem> = trace_txs
        .iter()
        .map(V3TraceTransaction::to_send_result_tuple)
        .collect();
    let formatted_tree = formatter.format_transaction_list(&send_result_list);
    println!("{}", formatted_tree.trim_end());
    println!();

    print_rpc_trace_summary(hash, &trace);

    if verbose {
        println!();
        print_rpc_trace_details(&trace_txs, Some(&formatter), &network);
    }

    if !show_bodies {
        println!();
        println!(
            "{}",
            "Hint: pass --show-bodies to include decoded message bodies when local ABI matches."
                .dimmed()
        );
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
    print_kv("Query Hash", format_trace_hash(query_hash));
    print_kv("Trace ID", format_trace_hash(&trace.trace_id));
    print_kv("Time", format_trace_time_range(trace));
    print_kv("Block", format_trace_block_range(trace));
    print_kv("Trace Complete", format_trace_bool(!trace.is_incomplete));
    print_kv(
        "Total Txs",
        format_trace_count(trace.transactions_order.len()),
    );
    print_kv(
        "Total Messages",
        format_trace_count(trace_message_count(trace)),
    );
}

fn trace_message_count(trace: &V3Trace) -> usize {
    let mut unique = BTreeSet::new();

    for tx_hash in &trace.transactions_order {
        let Some(tx) = trace.transactions.get(tx_hash) else {
            continue;
        };

        if let Some(hash) = tx.in_msg.as_ref().and_then(v3_message_hash) {
            unique.insert(hash.to_owned());
        } else if tx.in_msg.is_some() {
            unique.insert(format!("{tx_hash}:in"));
        }

        for (idx, msg) in tx.out_msgs.iter().enumerate() {
            if let Some(hash) = v3_message_hash(msg) {
                unique.insert(hash.to_owned());
            } else {
                unique.insert(format!("{tx_hash}:out:{idx}"));
            }
        }
    }

    unique.len()
}

fn print_rpc_trace_details(
    trace_txs: &[V3TraceTransaction],
    formatter: Option<&FormatterContext<'_>>,
    network: &Network,
) {
    for (idx, tx) in trace_txs.iter().enumerate() {
        let prefix = format!("tx[{}]", idx + 1);
        println!("  {prefix}:");
        println!("    hash: {}", format_trace_hash(&tx.hash));
        println!("    lt: {}", format_trace_u64(tx.transaction.lt));
        println!(
            "    account: {}",
            format_trace_address(&tx.summary.account, network)
        );
        println!("    parent_lt: {}", format_optional_u64(tx.parent_lt));
        println!("    child_lts: {}", format_child_lts(&tx.child_lts));

        if let Some(message) = &tx.summary.in_msg {
            let message_name = formatter
                .and_then(|formatter| formatter.transaction_inbound_message_name(&tx.transaction));
            println!(
                "    from: {}",
                format_optional_address(message.source.as_deref(), network)
            );
            println!(
                "    to: {}",
                format_optional_address(message.destination.as_deref(), network)
            );
            println!("    value: {}", format_message_value(message));
            println!(
                "    opcode: {}",
                format_message_opcode(message, message_name.as_deref())
            );
            println!(
                "    bounced: {}",
                format_trace_bool(message.bounced.unwrap_or(false))
            );
            println!(
                "    branch: {}",
                trace_branch_kind(&tx.summary, message, message_name.as_deref()).cyan()
            );
        } else {
            println!("    from: {}", format_trace_none());
            println!("    to: {}", format_trace_none());
            println!("    value: {}", format_trace_none());
            println!("    opcode: {}", format_trace_none());
            println!("    bounced: {}", format_trace_bool(false));
            println!("    branch: {}", "system".cyan());
        }

        println!(
            "    success: {}",
            format_trace_bool(trace_tx_success(&tx.summary))
        );
        println!("    exit_code: {}", format_compute_exit_code(&tx.summary));
        println!(
            "    action_result_code: {}",
            format_action_result_code(&tx.summary)
        );
    }
}

fn format_optional_u64(value: Option<u64>) -> String {
    value.map_or_else(format_trace_null, format_trace_u64)
}

fn format_child_lts(child_lts: &[u64]) -> String {
    if child_lts.is_empty() {
        return "[]".dimmed().to_string();
    }
    format!(
        "[{}]",
        child_lts
            .iter()
            .map(|value| format_trace_u64(*value))
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
        .map_or_else(format_trace_null, format_trace_code)
}

fn format_action_result_code(tx: &V3TransactionSummary) -> String {
    tx.description
        .as_ref()
        .and_then(|description| description.action.as_ref())
        .and_then(|action| action.result_code)
        .map_or_else(format_trace_null, format_trace_code)
}

fn trace_branch_kind(
    tx: &V3TransactionSummary,
    message: &V3MessageSummary,
    message_name: Option<&str>,
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
    if message_name.is_some_and(|name| name.to_ascii_lowercase().contains("notification")) {
        return "notification";
    }
    "message"
}

fn format_message_opcode(message: &V3MessageSummary, message_name: Option<&str>) -> String {
    let opcode = extract_message_opcode(message);
    let opcode_text = if opcode == 0 {
        "0x00000000".to_owned()
    } else {
        format!("0x{opcode:08x}")
    };
    let name = message_name.or_else(|| (opcode == 0).then_some("empty"));
    match name {
        Some(name) => format!("{} ({})", opcode_text.yellow(), name.cyan()),
        None => opcode_text.yellow().to_string(),
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

fn format_message_value(message: &V3MessageSummary) -> String {
    let Some(value) = message.value.as_deref() else {
        return format_trace_none();
    };
    match BigInt::from_str(value) {
        Ok(value) => format_nanotons(&value).white().to_string(),
        Err(_) => value.yellow().to_string(),
    }
}

fn format_optional_address(address: Option<&str>, network: &Network) -> String {
    address.map_or_else(format_trace_none, |address| {
        format_trace_address(address, network)
    })
}

fn format_trace_address(address: &str, network: &Network) -> String {
    StdAddr::from_str_ext(address, StdAddrFormat::any()).map_or_else(
        |_| address.yellow().to_string(),
        |(address, _)| format_std_address(&address, network).cyan().to_string(),
    )
}

fn format_trace_hash(hash: &str) -> String {
    hash.yellow().to_string()
}

fn format_trace_time_range(trace: &V3Trace) -> String {
    let timestamps = trace
        .transactions_order
        .iter()
        .filter_map(|tx_hash| trace.transactions.get(tx_hash))
        .filter_map(|tx| (tx.now != 0).then_some(u64::from(tx.now)));
    let range = trace_u64_range(timestamps);

    match range {
        Some((start, end)) => format_trace_range(format_trace_time(start), format_trace_time(end)),
        None => format_trace_range(format_trace_null(), format_trace_null()),
    }
}

fn format_trace_time(timestamp: u64) -> String {
    Utc.timestamp_opt(timestamp as i64, 0)
        .single()
        .map_or_else(
            || timestamp.to_string(),
            |datetime| datetime.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        )
        .white()
        .to_string()
}

fn format_trace_block_range(trace: &V3Trace) -> String {
    let blocks = trace
        .transactions_order
        .iter()
        .filter_map(|tx_hash| trace.transactions.get(tx_hash))
        .filter_map(|tx| tx.mc_block_seqno.map(u64::from));
    let range = trace_u64_range(blocks);

    match range {
        Some((start, end)) => format_trace_range(format_trace_u64(start), format_trace_u64(end)),
        None => format_trace_range(format_trace_null(), format_trace_null()),
    }
}

fn trace_u64_range(values: impl IntoIterator<Item = u64>) -> Option<(u64, u64)> {
    values.into_iter().fold(None, |range, value| {
        Some(match range {
            Some((start, end)) => (start.min(value), end.max(value)),
            None => (value, value),
        })
    })
}

fn format_trace_range(start: String, end: String) -> String {
    format!("{start} {} {end}", "—".dimmed())
}

fn format_trace_bool(value: bool) -> String {
    if value {
        "true".green().to_string()
    } else {
        "false".red().to_string()
    }
}

fn format_trace_count(value: usize) -> String {
    value.to_string().white().to_string()
}

fn format_trace_u64(value: u64) -> String {
    value.to_string().white().to_string()
}

fn format_trace_code(value: i32) -> String {
    if value == 0 {
        value.to_string().green().to_string()
    } else {
        value.to_string().red().to_string()
    }
}

fn format_trace_null() -> String {
    "null".dimmed().to_string()
}

fn format_trace_none() -> String {
    "<none>".dimmed().to_string()
}

fn fetch_trace_accounts(
    trace_txs: &[V3TraceTransaction],
    client: &TonApiClient,
) -> anyhow::Result<FxHashMap<StdAddr, ShardAccount>> {
    let mut addresses = BTreeSet::new();
    for tx in trace_txs {
        collect_trace_address(&tx.summary.account, &mut addresses);
        if let Some(in_msg) = &tx.summary.in_msg {
            for address in [&in_msg.source, &in_msg.destination].into_iter().flatten() {
                collect_trace_address(address, &mut addresses);
            }
        }
        for out_msg in &tx.summary.out_msgs {
            for address in [&out_msg.source, &out_msg.destination]
                .into_iter()
                .flatten()
            {
                collect_trace_address(address, &mut addresses);
            }
        }
    }

    let address_strings = addresses.into_iter().collect::<Vec<_>>();
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

fn collect_trace_address(address: &str, addresses: &mut BTreeSet<String>) {
    let Ok((address, _)) = StdAddr::from_str_ext(address, StdAddrFormat::any()) else {
        return;
    };
    addresses.insert(address.to_string());
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
            &candidate.contract_id,
            &candidate.contract_name,
            &candidate.contract_path,
            &candidate.code_boc64,
            candidate.code_hash,
            candidate
                .source_map
                .unwrap_or_else(|| Arc::new(SourceMap::without_debug_info())),
            candidate.abi,
        );
    }
    Ok(build_cache)
}
