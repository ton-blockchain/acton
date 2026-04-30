use crate::commands::common::error_fmt;
use crate::context::{
    AssertFailure, Context, DebugStopRequested, GetMethodAssertFailure, KnownAddress,
    MessageIterState, ParsedSearchParams, PendingMessageStep, SearchField, Wallet, to_cell,
};
use crate::external_send::{SendBocContext, format_send_boc_error};
use crate::paths;
use crate::retrace;
use crate::tonconnect;
use acton_config::color::OwoColorize;
use acton_config::config::Explorer;
use acton_debug::ChildDebugContextSpec;
use acton_debug::replayer::StepMode;
use anyhow::{Context as AnyhowContext, anyhow};
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use log::{debug, info, warn};
use num_bigint::{BigInt, Sign};
use num_traits::ToPrimitive;
use path_absolutize::Absolutize;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant, UNIX_EPOCH};
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use ton::ton_core::cell::TonCell;
use ton::ton_core::traits::tlb::TLB;
use ton_api::{
    Network, TonApiClient, V3MessageSummary, V3Trace, V3TransactionSummary, V3TxDescription,
};
use ton_emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use ton_emulator::world_state::WorldState;
use ton_emulator::{extension, register_ext_methods};
use ton_executor::BaseExecutor;
use ton_executor::get::step::StepGetExecutor;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::message::step::StepExecutor;
use ton_executor::{MissingLibrariesContext, missing_library_callback};
use tvm_ffi::serde::serialize_tuple;
use tvm_ffi::stack::{ContData, Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Load, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{
    AccountState, AccountStatus, AccountStatusChange, ActionPhase, ComputePhase,
    ComputePhaseSkipReason, CurrencyCollection, ExtInMsgInfo, ExtOutMsgInfo,
    ExtraCurrencyCollection, HashUpdate, IntAddr, IntMsgInfo, LibDescr, Message, MsgInfo,
    OptionalAccount, OrdinaryTxInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount,
    SkippedComputePhase, StateInit, StdAddr, StdAddrFormat, StoragePhase, StorageUsedShort,
    Transaction, TxInfo,
};
use tycho_types::num::{Tokens, Uint15};

/// Resolve the unix time to use for a get method invocation.
///
/// Prefers the emulated time set via `testing.setNow(...)` so `blockchain.now()` inside
/// getters matches `testing.getNow()` and the time used for transactions. Falls back to the
/// real wall clock when the user has never called `setNow` (default `current_now = 0`),
/// preserving existing behavior for tests that don't mock time.
fn resolve_get_method_unixtime(world_state: &WorldState) -> anyhow::Result<i64> {
    let emulated = world_state.get_now();
    if emulated != 0 {
        return Ok(emulated.into());
    }
    let duration_since_epoch = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    Ok(duration_since_epoch.as_secs().try_into()?)
}

fn run_nested_executor_until_finished(
    ctx: &mut Context,
    child_debug_started: bool,
    child_step_mode: StepMode,
    mut direct_step: impl FnMut() -> bool,
) -> anyhow::Result<()> {
    if child_debug_started {
        // StepInto / instruction stepping bootstrap the child by stopping on its
        // first user-visible location. "Continue"-style entry needs one explicit
        // kick so the nested replayer starts driving the live executor.
        let child_was_bootstrapped = matches!(
            child_step_mode,
            StepMode::StepInto | StepMode::EachAsmInstruction
        );

        if !child_was_bootstrapped {
            ctx.debug.step(child_step_mode);
        }

        if !ctx.debug.active_context_is_terminated() {
            let stopped = ctx
                .debug
                .process_incoming_requests(false)
                .context("Cannot process nested debug requests")?;
            if stopped {
                return Err(DebugStopRequested.into());
            }
        }

        anyhow::ensure!(
            ctx.debug.active_context_is_terminated(),
            "Nested debug context did not finish before finalization"
        );
    } else {
        while !direct_step() {}
    }

    Ok(())
}

extension!(build in (Context) with (path: String, id: String) using build_impl);
fn build_impl(ctx: &mut Context, stk: &mut Tuple, path: String, id: String) -> anyhow::Result<()> {
    debug!("Building {id}");
    let start_time = Instant::now();

    let name_only = path.is_empty();
    let mut path = PathBuf::from(&path);
    let mut display_name = id.clone(); // by default display name equal to ID

    if name_only {
        // > build("JettonMinter")
        debug!("No path provided, search in contracts");
        let found_contract = ctx.env.find_contract(&id);

        let Some(found_contract) = found_contract else {
            anyhow::bail!(error_fmt::contract_not_found(ctx.env.config, &id));
        };

        debug!("Found contract with info: {found_contract:?}");

        display_name = found_contract.display_name(&display_name).to_owned();
        path = found_contract.absolute_source_path(&ctx.env.project_root);
    } else if !path.is_absolute() {
        // > build("JettonMinter", "relative/to/root/path/to/contract")
        path = path
            .absolutize_from(ctx.env.project_root.clone())
            .unwrap_or_else(|_| path.clone().into())
            .to_path_buf();
    }

    // Build overrides used for mutation testing to change actual code of contract
    // with "mutated" one. This way we actually don't need to recompile each test
    // thus greatly increase performance of mutation testing
    if let Some(override_code) = ctx.env.build_override.get(&id) {
        debug!("Overriding code for {id}");
        stk.push(TupleItem::Cell(override_code.clone()));
        return Ok(());
    }

    let path_display = path.display().to_string();

    if path_display.ends_with(".boc") {
        // For BoC source we just return it as a Cell
        let binary_data =
            fs::read(&path).with_context(|| format!("Cannot read BoC file {path_display}"))?;
        let cell = Boc::decode(binary_data.as_slice())
            .with_context(|| anyhow::anyhow!("Failed to decode code BoC for {path_display}"))?;
        stk.push(TupleItem::Cell(cell));
        return Ok(());
    }

    // Build cache is runtime only cache, if this contract was already built we just
    // return cached cell for the contract.
    if let Some(cached) = ctx.build.build_cache.built.get(&path) {
        let elapsed = start_time.elapsed();
        info!("Build {path_display} from memory cache in {elapsed:?}");

        let code_cell = Boc::decode_base64(&cached.code_boc64).with_context(|| {
            anyhow::anyhow!("Failed to decode cached code BoC for {path_display}")
        })?;
        stk.push(TupleItem::Cell(code_cell));
        return Ok(());
    }

    // File build cache is persistent cache that outlives reruns. If this contract was already
    // built we return cached cell for the contract. Since lookup in this cache is quite expensive
    // we also add cache entry to runtime build cache.
    if let Some(cached_entry) =
        ctx.build
            .file_build_cache
            .get(&path_display, ctx.build.need_debug_info, false, 2, "1.3")
    {
        let elapsed = start_time.elapsed();
        info!(
            "Build {path_display} from file cache ({}) in {elapsed:?}",
            paths::DEFAULT_BUILD_CACHE_DIR
        );

        let code_cell = Boc::decode_base64(&cached_entry.code_boc64).map_err(|e| {
            anyhow::anyhow!("Failed to decode cached code BoC for {path_display}: {e}")
        })?;
        let source_map = Arc::new(cached_entry.source_map.clone().unwrap_or_default());

        ctx.build.build_cache.memoize(
            &display_name,
            &path,
            &cached_entry.code_boc64,
            HashBytes::from_str(&cached_entry.code_hash_hex)?,
            source_map,
            cached_entry.abi.clone().map(Into::into),
        );

        stk.push(TupleItem::Cell(code_cell));
        return Ok(());
    }

    // If there is no cache data, rebuild contract from sources.
    let compile_start = Instant::now();

    let mappings = ctx.env.config.mappings();
    let compiler = tolk_compiler::Compiler::new(2).with_mappings(&mappings);
    let result = compiler.compile(&path, ctx.build.need_debug_info);

    let compile_time = compile_start.elapsed();

    match result {
        tolk_compiler::CompilerResult::Success(success) => {
            info!("Build {path_display} from source (compilation: {compile_time:?}");

            if let Err(err) = ctx.build.file_build_cache.put(
                &path_display,
                &success,
                ctx.build.need_debug_info,
                false,
                2,
                "1.3",
            ) {
                warn!("Failed to build cached code BoC for {path_display}: {err}");
            }

            let code_cell = Boc::decode_base64(&success.code_boc64).map_err(|e| {
                anyhow::anyhow!("Failed to decode compiled code BoC for {path_display}: {e}")
            })?;
            let source_map = Arc::new(success.source_map.unwrap_or_default());

            ctx.build.build_cache.memoize(
                &display_name,
                &path,
                &success.code_boc64,
                HashBytes::from_str(&success.code_hash_hex)?,
                source_map,
                success.abi.clone().map(Into::into),
            );

            stk.push(TupleItem::Cell(code_cell));
        }
        tolk_compiler::CompilerResult::Error(error) => {
            info!(
                "Build {path_display} failed after {compile_time:?}: {}",
                error.message
            );

            anyhow::bail!("Compilation failed: {}", error.message);
        }
    }

    Ok(())
}

extension!(send_message in (Context) with (src: IntAddr, msg: Cell) using send_message_impl);
fn send_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: IntAddr,
    msg: Cell,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;

    let src_std = match &src {
        IntAddr::Std(addr) => addr,
        IntAddr::Var(_) => anyhow::bail!("Var addresses are not supported anymore"),
    };

    // Internal messages are serialized as RelaxedMessage; ExtIn messages are not.
    let is_external = match msg.parse::<RelaxedMessage<'_>>() {
        Ok(parsed) => match parsed.info {
            RelaxedMsgInfo::ExtOut(_) => {
                anyhow::bail!("External out messages can't initiate transactions!");
            }
            RelaxedMsgInfo::Int(_) => false,
        },
        Err(_) => true,
    };

    if is_external && ctx.can_broadcast_to_network() {
        if ctx.env.tonconnect.is_some() {
            anyhow::bail!(
                "`net.sendExternal` cannot be used with {}; use `net.send(wallet.address, createMessage(...))` so the connected wallet can sign the internal message",
                "--tonconnect".yellow()
            );
        }

        let parsed_ext_in = msg
            .parse::<Message<'_>>()
            .context("Failed to parse external-in message cell")?;
        let norm_hash = compute_normalized_ext_in_hash(&parsed_ext_in)?;
        drop(parsed_ext_in);

        let network = ctx.network();
        let custom_networks = ctx.env.config.custom_networks();
        let client = TonApiClient::new(network, custom_networks)
            .context("Failed to initialize toncenter client for external-in broadcast")?;
        client
            .send_boc(&Boc::encode_base64(&msg))
            .map_err(|error| format_send_boc_error(error, SendBocContext::Generic))?;

        let pseudo_tx = build_pseudo_broadcast_tx(ctx.chain.world_state.get_now(), msg, norm_hash);
        ctx.chain.world_state.invalidate_remote_cache();
        stack.push(TupleItem::big_array_from_items(vec![pseudo_tx]));
        return Ok(());
    }

    if ctx.can_broadcast_to_network()
        && let Some(tonconnect) = ctx.env.find_tonconnect_by_address(src_std)
    {
        let network = ctx.network();
        let (wallet_ext_in, norm_hash) = send_tonconnect_message(&msg, tonconnect, &network)
            .context("Failed to send message with TON Connect")?;

        ctx.chain.world_state.invalidate_remote_cache();

        let pseudo_tx =
            build_pseudo_broadcast_tx(ctx.chain.world_state.get_now(), wallet_ext_in, norm_hash);
        stack.push(TupleItem::big_array_from_items(vec![pseudo_tx]));
        return Ok(());
    }

    if ctx.can_broadcast_to_network()
        && let Some(wallet) = ctx.env.find_wallet_by_address(src_std)
    {
        let network = ctx.network();
        let custom_networks = ctx.env.config.custom_networks();
        if let Err(err) = register_localnet_abis(ctx, &custom_networks) {
            warn!("Failed to register compiler ABI in localnet: {err:#}");
        }

        let (wallet_ext_in, norm_hash) =
            send_wallet_message(&msg, wallet, &network, custom_networks)
                .context("Failed to send message to real network")?;

        ctx.chain.world_state.invalidate_remote_cache();

        let pseudo_tx =
            build_pseudo_broadcast_tx(ctx.chain.world_state.get_now(), wallet_ext_in, norm_hash);
        stack.push(TupleItem::big_array_from_items(vec![pseudo_tx]));
        return Ok(());
    }

    let libs = ctx.chain.build_libs(&src);
    let world_state = &mut ctx.chain.world_state;

    let emulations = if ctx.debug.is_enabled() {
        send_message_debug(ctx, &msg, &libs, Some(src))
    } else {
        emulator.send_message(world_state, msg, &libs, Some(src))
    }
    .context("Cannot send message")?;

    if let [SendMessageResult::Error(error), ..] = &emulations[..]
        && emulations.len() == 1
    {
        ctx.chain
            .emulations
            .save_message(&ctx.env.running_id, emulations.clone());

        // TODO return error with type when unions are supported in ffi
        if is_external {
            stack.push(TupleItem::Null);
            return Ok(());
        }

        anyhow::bail!("Cannot send message: {}", error.error)
    }

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some(res),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(emulation_to_send_result)
        .collect::<Vec<_>>();

    ctx.chain
        .emulations
        .save_message(&ctx.env.running_id, emulations);
    stack.push(TupleItem::big_array_from_items(transaction_cells));
    Ok(())
}

extension!(run_tick_tock in (Context) with (is_tock: BigInt, on_account: StdAddr) using run_tick_tock_impl);
fn run_tick_tock_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    is_tock: BigInt,
    on_account: StdAddr,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;
    let is_tock = is_tock != BigInt::ZERO;

    let addr = IntAddr::Std(on_account.clone());
    let libs = ctx.chain.build_libs(&addr);

    // TODO: debug mode support for tick-tock (send_message_debug equivalent)
    let emulations = emulator
        .run_tick_tock(ctx.chain.world_state, &on_account, is_tock, &libs)
        .context("Cannot run tick-tock transaction")?;

    if let [SendMessageResult::Error(error), ..] = &emulations[..]
        && emulations.len() == 1
    {
        ctx.chain
            .emulations
            .save_message(&ctx.env.running_id, emulations.clone());

        anyhow::bail!("Cannot run tick-tock transaction: {}", error.error)
    }

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some(res),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(emulation_to_send_result)
        .collect::<Vec<_>>();

    ctx.chain
        .emulations
        .save_message(&ctx.env.running_id, emulations);
    stack.push(TupleItem::big_array_from_items(transaction_cells));
    Ok(())
}

/// Build the `SendResult` tuple layout expected by Tolk from raw transaction components.
///
/// Callers that only have a transaction cell (e.g. polled from toncenter) can use
/// [`tx_cell_to_send_result_tuple`] instead.
fn build_send_result_tuple(
    tx_cell: Cell,
    parsed_tx: &Transaction,
    child_transactions: &[u64],
    parent_transaction: Option<u64>,
    out_actions: Cell,
    externals: &[Cell],
) -> TupleItem {
    let child_txs = Tuple(
        child_transactions
            .iter()
            .map(|lt| TupleItem::Int(BigInt::from(*lt)))
            .collect::<Vec<_>>(),
    );
    let parent_lt = match parent_transaction {
        Some(lt) => TupleItem::Int(BigInt::from(lt)),
        None => TupleItem::Null,
    };
    let out_messages = Tuple(
        collect_out_message_cells(parsed_tx)
            .into_iter()
            .map(TupleItem::Cell)
            .collect(),
    );
    let gas_used = match parsed_tx.load_info() {
        Ok(TxInfo::Ordinary(info)) => match info.compute_phase {
            ComputePhase::Executed(compute) => compute.gas_used.into(),
            ComputePhase::Skipped(_) => BigInt::ZERO,
        },
        Ok(TxInfo::TickTock(info)) => match info.compute_phase {
            ComputePhase::Executed(compute) => compute.gas_used.into(),
            ComputePhase::Skipped(_) => BigInt::ZERO,
        },
        _ => BigInt::ZERO,
    };
    let externals_tuple = Tuple(
        externals
            .iter()
            .cloned()
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    TupleItem::Tuple(Tuple(vec![
        TupleItem::Cell(tx_cell),
        TupleItem::Int(BigInt::from(parsed_tx.lt)),
        TupleItem::Tuple(child_txs),
        parent_lt,
        TupleItem::Cell(out_actions),
        TupleItem::Tuple(out_messages),
        TupleItem::Int(gas_used),
        TupleItem::Tuple(externals_tuple),
    ]))
}

fn collect_out_message_cells(parsed_tx: &Transaction) -> Vec<Cell> {
    parsed_tx
        .out_msgs
        .raw_values()
        .filter_map(Result::ok)
        .filter_map(|mut raw| raw.load_reference_cloned().ok())
        .collect()
}

fn collect_external_out_message_cells(parsed_tx: &Transaction) -> Vec<Cell> {
    parsed_tx
        .iter_out_msgs()
        .zip(parsed_tx.out_msgs.raw_values())
        .filter_map(|(msg, raw)| {
            let msg = msg.ok()?;
            if !matches!(msg.info, MsgInfo::ExtOut(_)) {
                return None;
            }
            raw.ok()?.load_reference_cloned().ok()
        })
        .collect()
}

/// Build `SendResult` from an already-parsed transaction (e.g. fetched from toncenter).
///
/// Externals are derived by filtering the transaction's own outgoing messages.
fn tx_cell_to_send_result_tuple(
    tx_cell: Cell,
    parsed_tx: &Transaction,
    child_transactions: &[u64],
    parent_transaction: Option<u64>,
) -> TupleItem {
    let externals = collect_external_out_message_cells(parsed_tx);

    build_send_result_tuple(
        tx_cell,
        parsed_tx,
        child_transactions,
        parent_transaction,
        Cell::default(),
        &externals,
    )
}

pub(crate) struct V3TraceTransaction {
    pub(crate) hash: String,
    pub(crate) summary: V3TransactionSummary,
    pub(crate) tx_cell: Cell,
    pub(crate) transaction: Transaction,
    pub(crate) parent_lt: Option<u64>,
    pub(crate) child_lts: Vec<u64>,
}

impl V3TraceTransaction {
    pub(crate) fn to_send_result_tuple(&self) -> TupleItem {
        tx_cell_to_send_result_tuple(
            self.tx_cell.clone(),
            &self.transaction,
            &self.child_lts,
            self.parent_lt,
        )
    }
}

pub(crate) enum V3TraceTransactions {
    Ready(Vec<V3TraceTransaction>),
    Pending { tx_hash: String },
}

pub(crate) fn build_v3_trace_transactions(trace: &V3Trace) -> anyhow::Result<V3TraceTransactions> {
    let mut transactions = Vec::with_capacity(trace.transactions_order.len());
    for tx_hash in &trace.transactions_order {
        let Some(summary) = trace.transactions.get(tx_hash) else {
            return Ok(V3TraceTransactions::Pending {
                tx_hash: tx_hash.clone(),
            });
        };
        let (tx_cell, transaction) = synthesize_tx_cell_from_v3(summary)
            .with_context(|| format!("Failed to synthesize Transaction cell for tx {tx_hash}"))?;
        transactions.push(V3TraceTransaction {
            hash: tx_hash.clone(),
            summary: summary.clone(),
            tx_cell,
            transaction,
            parent_lt: None,
            child_lts: Vec::new(),
        });
    }

    let in_msg_by_hash = transactions
        .iter()
        .enumerate()
        .filter_map(|(idx, tx)| {
            tx.summary
                .in_msg
                .as_ref()
                .and_then(v3_message_hash)
                .map(|hash| (hash.to_owned(), idx))
        })
        .collect::<HashMap<_, _>>();

    let mut edges = Vec::new();
    for (parent_idx, tx) in transactions.iter().enumerate() {
        for out_msg in &tx.summary.out_msgs {
            let Some(hash) = v3_message_hash(out_msg) else {
                continue;
            };
            let Some(child_idx) = in_msg_by_hash.get(hash).copied() else {
                continue;
            };
            if child_idx != parent_idx {
                edges.push((parent_idx, child_idx));
            }
        }
    }

    for (parent_idx, child_idx) in edges {
        let parent_lt = transactions[parent_idx].transaction.lt;
        let child_lt = transactions[child_idx].transaction.lt;
        if transactions[child_idx].parent_lt.is_none() {
            transactions[child_idx].parent_lt = Some(parent_lt);
        }
        if !transactions[parent_idx].child_lts.contains(&child_lt) {
            transactions[parent_idx].child_lts.push(child_lt);
        }
    }

    for tx in &mut transactions {
        tx.child_lts.sort_unstable();
    }

    Ok(V3TraceTransactions::Ready(transactions))
}

pub(crate) fn v3_message_hash(message: &V3MessageSummary) -> Option<&str> {
    message.hash.as_deref().filter(|hash| !hash.is_empty())
}

/// Compute the TEP-467 normalized hash of an external-in message as specified in the
/// [TON docs message-lookup guide](https://docs.ton.org/ecosystem/ton-connect/message-lookup).
///
/// Normalization rules applied to the cell:
/// - `src` is replaced with `addr_none$00`;
/// - `import_fee` is reset to 0;
/// - `init` is dropped;
/// - `body` is always stored as a cell reference (`Either right$1`).
///
/// The resulting cell's `repr_hash` is returned. This form is stable across cell-layout
/// re-serializations and is the value both sides of a lookup compute locally — so a client
/// that polled for it can match a transaction whose `inMessage` toncenter may have rebuilt.
fn compute_normalized_ext_in_hash(msg: &Message<'_>) -> anyhow::Result<HashBytes> {
    let MsgInfo::ExtIn(info) = &msg.info else {
        anyhow::bail!("TEP-467 normalization only applies to external-in messages");
    };

    // Promote the body slice (inline or ref) to a standalone cell so it can be stored as ref.
    let body_cell = {
        let mut b = CellBuilder::new();
        b.store_slice(msg.body)?;
        b.build()?
    };

    let normalized_info = ExtInMsgInfo {
        src: None,
        dst: info.dst.clone(),
        import_fee: Tokens::ZERO,
    };

    let ctx = Cell::empty_context();
    let mut b = CellBuilder::new();
    b.store_small_uint(0b10, 2)?; // MsgInfo::ExtIn tag
    normalized_info.store_into(&mut b, ctx)?;
    b.store_bit_zero()?; // init = nothing$0
    b.store_bit_one()?; // body = right$1 (stored in a ref)
    b.store_reference(body_cell)?;
    Ok(*b.build()?.repr_hash())
}

/// Extract the destination address from an external-in message cell.
///
/// Only external-in is supported — the TON docs message-lookup flow is specified for
/// messages sent to the network by the client, which are always external-in.
fn ext_in_dest_address(msg_cell: &Cell) -> anyhow::Result<String> {
    let msg = msg_cell
        .parse::<Message<'_>>()
        .context("Failed to parse inbound message cell")?;
    let MsgInfo::ExtIn(info) = &msg.info else {
        anyhow::bail!("waitForFirstTransaction expects an external-in message");
    };
    match &info.dst {
        IntAddr::Std(addr) => Ok(addr.to_string()),
        IntAddr::Var(_) => anyhow::bail!("Var addresses are not supported"),
    }
}

/// Build a placeholder `SendResult` tuple for transactions broadcast to a real network.
///
/// The pseudo `tx` carries the external-in message in `in_msg` (for its `dst`) and the
/// TEP-467 normalized hash in `prev_trans_hash` (used as the lookup key on the wait side
/// — avoids re-normalizing locally). Other `Transaction` slots are minimal defaults so the
/// cell round-trips through TL-B serialization.
fn build_pseudo_broadcast_tx(now: u32, in_msg: Cell, norm_hash: HashBytes) -> TupleItem {
    let tx = Transaction {
        account: Default::default(),
        lt: 0,
        // HACK: abused slot — carries the TEP-467 normalized hash for the Rust-side lookup.
        prev_trans_hash: norm_hash,
        prev_trans_lt: 0,
        now,
        out_msg_count: Default::default(),
        orig_status: AccountStatus::Uninit,
        end_status: AccountStatus::Uninit,
        in_msg: Some(in_msg),
        out_msgs: Default::default(),
        total_fees: Default::default(),
        state_update: Lazy::new(&HashUpdate {
            old: Default::default(),
            new: Default::default(),
        })
        .expect("Invalid state update"),
        info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: false,
            storage_phase: None,
            credit_phase: None,
            compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                reason: ComputePhaseSkipReason::NoState,
            }),
            action_phase: None,
            aborted: false,
            bounce_phase: None,
            destroyed: false,
        }))
        .expect("Invalid transaction info"),
    };

    let tx_cell = to_cell(&tx);
    TupleItem::Tuple(Tuple(vec![
        TupleItem::Cell(tx_cell),
        TupleItem::Int(BigInt::ZERO),
        TupleItem::Tuple(Tuple::empty()),
        TupleItem::Null,
        TupleItem::Cell(Cell::default()),
        TupleItem::Tuple(Tuple::empty()),
        TupleItem::Int(BigInt::ZERO),
        TupleItem::Tuple(Tuple::empty()),
    ]))
}

fn emulation_to_send_result(emulation: &SendMessageResultSuccess) -> Option<TupleItem> {
    let out_actions = match &emulation.actions {
        Some(actions_b64) => {
            Boc::decode_base64(actions_b64.as_ref()).unwrap_or_else(|_| Cell::default())
        }
        None => Cell::default(),
    };
    let tx_cell = to_cell(&emulation.transaction);
    Some(build_send_result_tuple(
        tx_cell,
        &emulation.transaction,
        &emulation.child_transactions,
        emulation.parent_transaction,
        out_actions,
        &emulation.externals,
    ))
}

#[allow(clippy::large_enum_variant)]
enum IterationStop {
    Steps(usize),
    UntilMatch(ParsedSearchParams, GetExecutor),
    Exhausted,
}

struct MessageIterBatch {
    results: Vec<SendMessageResult>,
    matched: bool,
    hard_error: Option<anyhow::Error>,
}

fn send_results_to_tuple_items(emulations: &[SendMessageResult]) -> Vec<TupleItem> {
    emulations
        .iter()
        .filter_map(|emulation| match emulation {
            SendMessageResult::Success(res) => Some(res),
            SendMessageResult::Error(_) => None,
        })
        .filter_map(emulation_to_send_result)
        .collect::<Vec<_>>()
}

fn save_send_results(ctx: &mut Context, emulations: &[SendMessageResult]) {
    if emulations.is_empty() {
        return;
    }

    ctx.chain
        .emulations
        .save_message(&ctx.env.running_id, emulations.to_vec());
}

fn save_message_iter_results(ctx: &mut Context, cursor_id: u64, emulations: &[SendMessageResult]) {
    if emulations.is_empty() {
        return;
    }

    let Some(trace_index) = ctx.message_iters.trace_index(cursor_id) else {
        save_send_results(ctx, emulations);
        return;
    };

    let saved_trace_index = match trace_index {
        Some(trace_index) => ctx.chain.emulations.append_message_to_trace(
            &ctx.env.running_id,
            trace_index,
            emulations.to_vec(),
        ),
        None => ctx
            .chain
            .emulations
            .save_message(&ctx.env.running_id, emulations.to_vec()),
    };

    let _ = ctx
        .message_iters
        .set_trace_index(cursor_id, saved_trace_index);
}

fn finish_message_iter_results(
    ctx: &mut Context,
    cursor_id: u64,
    emulations: &[SendMessageResult],
) {
    save_message_iter_results(ctx, cursor_id, emulations);
    let _ = ctx.message_iters.close_if_done(cursor_id);
}

fn push_successful_send_results(stack: &mut Tuple, emulations: &[SendMessageResult]) {
    stack.push(TupleItem::big_array_from_items(
        send_results_to_tuple_items(emulations),
    ));
}

fn backfill_batch_child_transactions(results: &mut [SendMessageResult]) {
    let mut children_by_parent = HashMap::<u64, Vec<u64>>::new();

    for result in results.iter() {
        let SendMessageResult::Success(result) = result else {
            continue;
        };

        if let Some(parent_lt) = result.parent_transaction {
            children_by_parent
                .entry(parent_lt)
                .or_default()
                .push(result.transaction.lt);
        }
    }

    for result in results.iter_mut() {
        let SendMessageResult::Success(result) = result else {
            continue;
        };

        result.child_transactions = children_by_parent
            .remove(&result.transaction.lt)
            .unwrap_or_default();
    }
}

fn execute_message_iter_batch_with<F>(
    message_iters: &mut MessageIterState,
    cursor_id: u64,
    stop: IterationStop,
    mut execute_step: F,
) -> anyhow::Result<MessageIterBatch>
where
    F: FnMut(PendingMessageStep, HashBytes) -> anyhow::Result<SendMessageResult>,
{
    if !message_iters.contains(cursor_id) {
        return Ok(MessageIterBatch {
            results: Vec::new(),
            matched: false,
            hard_error: None,
        });
    }

    let mut executed = 0usize;
    let mut matched = false;
    let mut results = Vec::new();

    loop {
        match &stop {
            IterationStop::Steps(limit) if executed >= *limit => break,
            IterationStop::Steps(_) | IterationStop::UntilMatch(..) | IterationStop::Exhausted => {}
        }

        let Some((pending, libs_owner)) = message_iters.peek_next(cursor_id) else {
            break;
        };

        let mut result = match execute_step(pending.clone(), libs_owner) {
            Ok(result) => result,
            Err(error) if results.is_empty() => return Err(error),
            Err(error) => {
                backfill_batch_child_transactions(&mut results);
                return Ok(MessageIterBatch {
                    results,
                    matched,
                    hard_error: Some(error),
                });
            }
        };
        let _ = message_iters.advance(cursor_id);

        if let SendMessageResult::Success(step) = &mut result {
            step.parent_transaction = pending.parent_lt;
            let tx_lt = step.transaction.lt;
            let mut externals = Vec::new();

            let out_msgs = step
                .transaction
                .iter_out_msgs()
                .zip(step.transaction.out_msgs.raw_values());
            for (out_msg, raw_out_msg) in out_msgs {
                let Ok(out_msg) = out_msg else {
                    continue;
                };
                let Ok(mut raw_out_msg) = raw_out_msg else {
                    continue;
                };

                match out_msg.info {
                    MsgInfo::ExtOut(_) => {
                        if let Ok(out_msg_cell) = raw_out_msg.load_reference_cloned() {
                            externals.push(out_msg_cell);
                        }
                    }
                    MsgInfo::Int(_) => {
                        let Ok(out_msg_cell) = raw_out_msg.load_reference_cloned() else {
                            continue;
                        };
                        let _ = message_iters.push_child_message(cursor_id, out_msg_cell, tx_lt);
                    }
                    MsgInfo::ExtIn(_) => {}
                }
            }

            step.externals = externals;

            if let IterationStop::UntilMatch(predicates, executor) = &stop
                && transaction_matches_predicates(&step.transaction, predicates, executor)?
            {
                matched = true;
            }
        }

        executed += 1;
        let should_stop = matched;
        results.push(result);

        if should_stop {
            break;
        }
    }

    backfill_batch_child_transactions(&mut results);
    Ok(MessageIterBatch {
        results,
        matched,
        hard_error: None,
    })
}

fn execute_message_iter_batch(
    ctx: &mut Context,
    cursor_id: u64,
    stop: IterationStop,
) -> anyhow::Result<MessageIterBatch> {
    if ctx.is_broadcasting {
        anyhow::bail!("createTraceIterationCursor() is available only in emulation mode")
    }

    if ctx.debug.is_enabled() {
        anyhow::bail!("Step-by-step execution is not supported in debug mode yet")
    }

    let chain = &mut ctx.chain;
    execute_message_iter_batch_with(
        &mut ctx.message_iters,
        cursor_id,
        stop,
        |pending, libs_owner| {
            let libs = chain.build_libs_with_hash_owner(&libs_owner);
            chain
                .emulator
                .send_transaction(chain.world_state, pending.message, &libs, pending.from)
                .context("Cannot execute step-by-step transaction")
        },
    )
}

/// Broadcast an internal message through an opened wallet. Returns the external-in cell
/// the wallet SDK built and its TEP-467 normalized hash returned by `sendBocReturnHash`.
fn send_wallet_message(
    message: &Cell,
    wallet: Wallet,
    network: &Network,
    custom_networks: HashMap<String, acton_config::config::CustomNetworkUrls>,
) -> anyhow::Result<(Cell, HashBytes)> {
    let expired_at_time = std::time::SystemTime::now() + Duration::from_secs(600);
    let expire_at = expired_at_time.duration_since(UNIX_EPOCH)?.as_secs() as u32;

    let client = TonApiClient::new(network.clone(), custom_networks)?;

    let (seqno, need_state_init) = wallet.seqno(&client)?;
    let message_ton = TonCell::from_boc(Boc::encode(message))?;
    let external =
        wallet
            .wallet
            .create_ext_in_msg(vec![message_ton], seqno, expire_at, need_state_init)?;

    let boc = &external.to_boc_base64()?;
    let network_name = network.to_string();
    let context = SendBocContext::wallet(&wallet, &network_name, seqno, need_state_init);
    client
        .send_boc(boc)
        .map_err(|error| format_send_boc_error(error, context))?;

    let external_in_cell =
        Boc::decode_base64(boc).context("Failed to decode wallet external-in BoC")?;
    let parsed_ext_in = external_in_cell
        .parse::<Message<'_>>()
        .context("Failed to parse wallet external-in message")?;
    let norm_hash = compute_normalized_ext_in_hash(&parsed_ext_in)?;
    drop(parsed_ext_in);
    Ok((external_in_cell, norm_hash))
}

fn send_tonconnect_message(
    message: &Cell,
    tonconnect: &tonconnect::TonConnectContext,
    network: &Network,
) -> anyhow::Result<(Cell, HashBytes)> {
    let transaction = tonconnect::transaction_from_message(message, network)?;
    let boc = tonconnect.session.send_transaction(transaction)?;
    let external_in_cell =
        Boc::decode_base64(&boc).context("Failed to decode TON Connect external-in BoC")?;
    let parsed_ext_in = external_in_cell
        .parse::<Message<'_>>()
        .context("Failed to parse TON Connect external-in message")?;
    let norm_hash = compute_normalized_ext_in_hash(&parsed_ext_in)?;
    drop(parsed_ext_in);
    Ok((external_in_cell, norm_hash))
}

fn send_transaction_debug(
    ctx: &mut Context,
    msg_cell: &Cell,
    libs: &Dict<HashBytes, LibDescr>,
    src_addr: Option<IntAddr>,
) -> anyhow::Result<Option<SendMessageResult>> {
    let prepared = Emulator::prepare_send_transaction(
        ctx.chain.world_state,
        msg_cell.clone(),
        libs,
        src_addr.clone(),
    )?;
    let config_b64 = ctx.chain.world_state.get_config_b64();

    // Nested send-message debugging executes the recipient transaction through a
    // live step executor. Compilation artifacts are reused only for source/ABI
    // rendering; execution itself still comes from the prepared emulator state.
    let mut step_executor =
        StepExecutor::new(Some(&config_b64)).context("Failed to create executor")?;
    let mut missing_libraries_ctx = MissingLibrariesContext::default();
    step_executor
        .register_missing_library_callback(&mut missing_libraries_ctx, missing_library_callback)
        .context("Failed to register missing library callback")?;

    let compilation_result = ctx
        .build
        .build_cache
        .result_for_code(&prepared.code)
        .map(|(_, result)| result);
    let source_map = compilation_result
        .as_ref()
        .map(|result| result.source_map.clone());
    let abi = compilation_result
        .as_ref()
        .and_then(|result| result.abi.clone());

    let prepare_result = step_executor
        .prepare_transaction(&prepared.message_b64, &prepared.run_args)
        .context("Prepare transaction failed")?;
    assert!(
        prepare_result.success,
        "Failed to prepare Emulator in debug mode"
    );
    if prepare_result.skipped {
        // SBS has no VM steps to expose for skipped compute, but the caller still
        // expects the same transaction/error shape as normal emulation.
        return ctx
            .chain
            .emulator
            .send_transaction(ctx.chain.world_state, msg_cell.clone(), libs, src_addr)
            .map(Some);
    }

    let need_to_stop_on_entry = ctx.debug.need_to_stop_child_thread_on_start();
    // Push the recipient transaction as a child debug context so a single DAP
    // session can step across `net.send*` without losing the parent stack.
    let child_debug_started = ctx
        .debug
        .begin_child_context(ChildDebugContextSpec {
            thread_id: 2,
            name: "Send message".to_string(),
            executor: step_executor.clone().into(),
            source_map,
            abi,
            stop_on_entry: need_to_stop_on_entry,
        })
        .context("Cannot start nested debug context")?;

    // Step Into should stop on the first user-visible line in the child. Otherwise,
    // let the nested runtime run until its own breakpoint / exception / completion.
    let child_step_mode = if need_to_stop_on_entry {
        match ctx.debug.performing_step() {
            Some(StepMode::EachAsmInstruction) => StepMode::EachAsmInstruction,
            _ => StepMode::StepInto,
        }
    } else {
        StepMode::RunUntilBreakpoint
    };
    run_nested_executor_until_finished(ctx, child_debug_started, child_step_mode, || {
        step_executor.step()
    })
    .context("Cannot finish nested message execution")?;

    let result = step_executor
        .finish_transaction()
        .context("Cannot finish transaction")?;

    if child_debug_started {
        ctx.debug
            .finish_child_context(2)
            .context("Cannot finish nested debug context")?;
    }

    Emulator::finalize_send_transaction(
        ctx.chain.world_state,
        prepared,
        result,
        None,
        missing_libraries_ctx.into_set(),
    )
    .map(Some)
}

fn send_message_debug(
    ctx: &mut Context,
    msg_cell: &Cell,
    libs: &Dict<HashBytes, LibDescr>,
    src_addr: Option<IntAddr>,
) -> anyhow::Result<Vec<SendMessageResult>> {
    Emulator::execute_send_message_flow(msg_cell.clone(), src_addr, &mut |message, from| {
        send_transaction_debug(ctx, &message, libs, from)
    })
}

extension!(send_single_message in (Context) with (src: IntAddr, msg: Cell) using send_single_message_impl);
fn send_single_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: IntAddr,
    msg: Cell,
) -> anyhow::Result<()> {
    let emulator = &ctx.chain.emulator;

    let msg_cell = msg;
    let libs = ctx.chain.build_libs(&src);
    let world_state = &mut ctx.chain.world_state;

    let emulation = emulator
        .send_transaction(world_state, msg_cell, &libs, Some(src))
        .context("Cannot emulate transaction")?;

    let SendMessageResult::Success(emulation) = emulation else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let Some(send_result) = emulation_to_send_result(&emulation) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    ctx.chain.emulations.save_message(
        &ctx.env.running_id,
        vec![SendMessageResult::Success(emulation)],
    );

    stack.push(send_result);
    Ok(())
}

extension!(start_message_iter in (Context) with (src: IntAddr, msg: Cell) using start_message_iter_impl);
fn start_message_iter_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    src: IntAddr,
    msg: Cell,
) -> anyhow::Result<()> {
    if ctx.is_broadcasting {
        anyhow::bail!("createTraceIterationCursor() is available only in emulation mode")
    }

    if ctx.debug.is_enabled() {
        anyhow::bail!("Step-by-step execution is not supported in debug mode yet")
    }

    let std_address = src.as_std().context("Var addresses are not supported")?;
    let libs_owner = std_address.address;
    let cursor_id = ctx
        .message_iters
        .insert_message_cursor(msg, Some(src), libs_owner);
    stack.push(TupleItem::Int(BigInt::from(cursor_id)));
    Ok(())
}

extension!(execute_message_iter_n in (Context) with (count: BigInt, cursor_id: BigInt) using execute_message_iter_n_impl);
fn execute_message_iter_n_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    count: BigInt,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let count = count.to_usize().unwrap_or(0);
    let batch = execute_message_iter_batch(ctx, cursor_id, IterationStop::Steps(count))?;

    finish_message_iter_results(ctx, cursor_id, &batch.results);
    if let Some(error) = batch.hard_error {
        return Err(error);
    }

    if let [SendMessageResult::Error(error)] = &batch.results[..] {
        anyhow::bail!("Cannot execute transaction iterator step: {}", error.error);
    }

    push_successful_send_results(stack, &batch.results);
    Ok(())
}

extension!(execute_message_iter_till in (Context) with (params: Tuple, cursor_id: BigInt) using execute_message_iter_till_impl);
fn execute_message_iter_till_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let predicates = parse_search_params_tuple(&params);
    let executor = make_predicate_executor(ctx)?;

    let batch = execute_message_iter_batch(
        ctx,
        cursor_id,
        IterationStop::UntilMatch(predicates, executor),
    )?;

    finish_message_iter_results(ctx, cursor_id, &batch.results);
    if let Some(error) = batch.hard_error {
        return Err(error);
    }

    if let [SendMessageResult::Error(error)] = &batch.results[..] {
        anyhow::bail!("Cannot execute transaction iterator step: {}", error.error);
    }

    if !batch.matched {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    push_successful_send_results(stack, &batch.results);
    Ok(())
}

extension!(execute_message_iter_from in (Context) with (cursor_id: BigInt) using execute_message_iter_from_impl);
fn execute_message_iter_from_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let batch = execute_message_iter_batch(ctx, cursor_id, IterationStop::Exhausted)?;

    finish_message_iter_results(ctx, cursor_id, &batch.results);
    if let Some(error) = batch.hard_error {
        return Err(error);
    }

    if let [SendMessageResult::Error(error)] = &batch.results[..] {
        anyhow::bail!("Cannot execute transaction iterator step: {}", error.error);
    }

    push_successful_send_results(stack, &batch.results);
    Ok(())
}

extension!(is_message_iter_done in (Context) with (cursor_id: BigInt) using is_message_iter_done_impl);
fn is_message_iter_done_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    stack.push_bool(ctx.message_iters.is_done(cursor_id));
    Ok(())
}

extension!(close_message_iter in (Context) with (cursor_id: BigInt) using close_message_iter_impl);
fn close_message_iter_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    cursor_id: BigInt,
) -> anyhow::Result<()> {
    let cursor_id = cursor_id
        .to_u64()
        .context("Transaction iterator id does not fit into u64")?;
    let _ = ctx.message_iters.close(cursor_id);
    Ok(())
}

fn root_lt_from_send_results(txs: &[TupleItem]) -> Option<u64> {
    let first = txs.first()?;
    let TupleItem::Tuple(send_result) = first else {
        return None;
    };
    match send_result.get(1) {
        Some(TupleItem::Int(lt)) => lt.to_u64(),
        _ => None,
    }
}

extension!(save_trace_name in (Context) with (trace_name: String, txs: Vec<TupleItem>) using save_trace_name_impl);
fn save_trace_name_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    trace_name: String,
    txs: Vec<TupleItem>,
) -> anyhow::Result<()> {
    let Some(root_lt) = root_lt_from_send_results(&txs) else {
        return Ok(());
    };

    ctx.chain
        .emulations
        .save_trace_name(&ctx.env.running_id, root_lt, trace_name);
    Ok(())
}

/// Call a TVM predicate continuation with a single argument. Returns the bool result.
fn call_predicate(executor: &GetExecutor, cont: &ContData, arg: TupleItem) -> anyhow::Result<bool> {
    let mut cont_builder = CellBuilder::new();
    tvm_ffi::serde::serialize_vm_cont(&mut cont_builder, cont)?;
    let cont_cell = cont_builder.build()?;
    let cont_boc = Boc::encode_base64(cont_cell);

    let args = Tuple(vec![arg]);
    let stack_boc = serialize_tuple(&args)
        .map(|t| Boc::encode_base64(&t))
        .context("Cannot serialize predicate arg")?;

    let result = executor
        .run_continuation(&cont_boc, &stack_boc)
        .context("Cannot run predicate")?;

    // Matcher predicates are expected to be pure boolean functions that always return a
    // value — they must never throw or end with a non-success VM exit code. If they do,
    // the offending transaction would be silently treated as "not matching" which hides
    // real runtime errors behind misleading "data mismatch" diagnostics. Surface the
    // failure instead so the caller reports it as a predicate error.
    match result {
        GetMethodResult::Success(r) if r.vm_exit_code == 0 || r.vm_exit_code == 1 => {
            let cell =
                Boc::decode_base64(r.stack.as_ref()).context("Failed to decode result stack")?;
            let tuple = Tuple::deserialize(&cell).context("Failed to deserialize result")?;
            match tuple.first() {
                Some(TupleItem::Int(n)) => Ok(*n != BigInt::from(0)),
                other => anyhow::bail!(
                    "Matcher predicate returned an unexpected stack value: {other:?}. \
                     Matchers must return an int (bool), not throw or produce other types."
                ),
            }
        }
        GetMethodResult::Success(r) => anyhow::bail!(
            "Matcher predicate failed with VM exit code {}. \
             Matchers must never throw — they should return a boolean value.\n\
             VM log:\n{}",
            r.vm_exit_code,
            r.vm_log,
        ),
        GetMethodResult::Error(err) => anyhow::bail!(
            "Matcher predicate execution error: {}. \
             Matchers must never throw — they should return a boolean value.",
            err.error
        ),
    }
}

/// Parse `SearchParams` tuple into `ParsedSearchParams`.
/// Each field is a sub-tuple [tag, value] where tag: 0=null, 1=user predicate, 2=value-as-predicate.
fn parse_search_params_tuple(params: &Tuple) -> ParsedSearchParams {
    let extract_field = |idx_from_end: usize| -> Option<SearchField> {
        let idx = params.0.len().checked_sub(idx_from_end + 1)?;
        let item = params.0.get(idx)?;
        let TupleItem::Tuple(sub) = item else {
            return None;
        };
        if sub.len() < 2 {
            return None;
        }
        let tag = match &sub[0] {
            TupleItem::Int(n) => n.to_u32().unwrap_or(0) as u8,
            _ => 0,
        };
        if tag == 0 {
            return None;
        }
        let cont = match &sub[1] {
            TupleItem::Cont(c) => c.clone(),
            _ => return None,
        };
        Some(SearchField {
            tag,
            predicate: cont,
        })
    };

    ParsedSearchParams {
        state_init: extract_field(0),
        body: extract_field(1),
        compute_phase_skipped: extract_field(2),
        action_exit_code: extract_field(3),
        opcode: extract_field(4),
        bounced: extract_field(5),
        bounce: extract_field(6),
        deploy: extract_field(7),
        aborted: extract_field(8),
        success: extract_field(9),
        exit_code: extract_field(10),
        value: extract_field(11),
        from: extract_field(12),
        to: extract_field(13),
    }
}

#[derive(Debug, Default)]
struct ScalarSearchParams {
    to: Option<IntAddr>,
    from: Option<IntAddr>,
    value: Option<BigInt>,
    exit_code: Option<u32>,
    success: Option<bool>,
    aborted: Option<bool>,
    deploy: Option<bool>,
    bounce: Option<bool>,
    bounced: Option<bool>,
    opcode: Option<u32>,
    action_exit_code: Option<i32>,
    compute_phase_skipped: Option<bool>,
    body: Option<Cell>,
    state_init: Option<Option<StateInit>>,
}

fn read_int_like_param(item: &TupleItem) -> Option<&BigInt> {
    match item {
        TupleItem::Int(num) => Some(num),
        TupleItem::Tuple(items) => items.first().and_then(read_int_like_param),
        _ => None,
    }
}

fn read_bool_like_param(item: &TupleItem) -> Option<bool> {
    match item {
        TupleItem::Int(num) => Some(num.to_i64() == Some(-1)),
        _ => None,
    }
}

fn read_optional_address_param(item: Option<&TupleItem>) -> Option<Option<IntAddr>> {
    let Some(item) = item else {
        return Some(None);
    };

    match item {
        TupleItem::Tuple(raw_addr) => match raw_addr.first() {
            Some(TupleItem::Slice(cell)) => {
                let mut slice = cell.as_slice().ok()?;
                if let Ok(address) = IntAddr::load_from(&mut slice) {
                    Some(Some(address))
                } else {
                    Some(None)
                }
            }
            _ => Some(None),
        },
        TupleItem::Slice(cell) => {
            let mut slice = cell.as_slice().ok()?;
            if let Ok(address) = IntAddr::load_from(&mut slice) {
                Some(Some(address))
            } else {
                Some(None)
            }
        }
        _ => Some(None),
    }
}

fn parse_scalar_search_params_tuple(params: &Tuple) -> Option<ScalarSearchParams> {
    let item_from_end = |idx_from_end: usize| {
        params
            .0
            .len()
            .checked_sub(idx_from_end + 1)
            .and_then(|idx| params.0.get(idx))
    };
    let raw_state_init = item_from_end(0);
    let raw_body = item_from_end(1);
    let raw_compute_phase_skipped = item_from_end(2);
    let raw_action_exit_code = item_from_end(3);
    let raw_opcode = item_from_end(4);
    let raw_bounced = item_from_end(5);
    let raw_bounce = item_from_end(6);
    let raw_deploy = item_from_end(7);
    let raw_aborted = item_from_end(8);
    let raw_success = item_from_end(9);
    let raw_exit_code = item_from_end(10);
    let raw_msg_value = item_from_end(11);
    let raw_from = item_from_end(12);
    let raw_to = item_from_end(13);

    let mut params = ScalarSearchParams {
        to: read_optional_address_param(raw_to)?,
        from: read_optional_address_param(raw_from)?,
        ..Default::default()
    };

    if let Some(raw_opcode) = raw_opcode {
        if raw_opcode == &TupleItem::Null {
            params.opcode = None;
        } else if let Some(num) = read_int_like_param(raw_opcode) {
            params.opcode = num.to_u32();
        }
    }
    if let Some(raw_bounced) = raw_bounced {
        if raw_bounced == &TupleItem::Null {
            params.bounced = None;
        } else if let Some(value) = read_bool_like_param(raw_bounced) {
            params.bounced = Some(value);
        }
    }
    if let Some(raw_bounce) = raw_bounce {
        if raw_bounce == &TupleItem::Null {
            params.bounce = None;
        } else if let Some(value) = read_bool_like_param(raw_bounce) {
            params.bounce = Some(value);
        }
    }
    if let Some(raw_deploy) = raw_deploy {
        if raw_deploy == &TupleItem::Null {
            params.deploy = None;
        } else if let Some(value) = read_bool_like_param(raw_deploy) {
            params.deploy = Some(value);
        }
    }
    if let Some(raw_exit_code) = raw_exit_code {
        if raw_exit_code == &TupleItem::Null {
            params.exit_code = None;
        } else if let Some(num) = read_int_like_param(raw_exit_code) {
            params.exit_code = num.to_u32();
        }
    }
    if let Some(raw_success) = raw_success {
        if raw_success == &TupleItem::Null {
            params.success = None;
        } else if let Some(value) = read_bool_like_param(raw_success) {
            params.success = Some(value);
        }
    }
    if let Some(raw_aborted) = raw_aborted {
        if raw_aborted == &TupleItem::Null {
            params.aborted = None;
        } else if let Some(value) = read_bool_like_param(raw_aborted) {
            params.aborted = Some(value);
        }
    }
    if let Some(raw_msg_value) = raw_msg_value {
        if raw_msg_value == &TupleItem::Null {
            params.value = None;
        } else if let TupleItem::Int(num) = raw_msg_value {
            params.value = Some(num.clone());
        }
    }
    if let Some(raw_action_exit_code) = raw_action_exit_code {
        if raw_action_exit_code == &TupleItem::Null {
            params.action_exit_code = None;
        } else if let Some(num) = read_int_like_param(raw_action_exit_code) {
            params.action_exit_code = Some(num.to_i32().unwrap_or(0));
        }
    }
    if let Some(raw_compute_phase_skipped) = raw_compute_phase_skipped {
        if raw_compute_phase_skipped == &TupleItem::Null {
            params.compute_phase_skipped = None;
        } else if let Some(value) = read_bool_like_param(raw_compute_phase_skipped) {
            params.compute_phase_skipped = Some(value);
        }
    }
    if let Some(raw_body) = raw_body {
        if raw_body == &TupleItem::Null {
            params.body = None;
        } else if let TupleItem::Cell(cell) = raw_body {
            params.body = Some(cell.clone());
        }
    }
    if let Some(raw_state_init) = raw_state_init {
        if raw_state_init == &TupleItem::Null {
            params.state_init = None;
        } else if let TupleItem::Cell(cell) = raw_state_init {
            params.state_init = Some(cell.parse::<Option<StateInit>>().ok()?);
        }
    }

    Some(params)
}

/// Check if a transaction matches all predicate search params by calling each predicate via `run_continuation`.
#[allow(clippy::collapsible_if)]
fn transaction_matches_predicates(
    tx: &Transaction,
    predicates: &ParsedSearchParams,
    executor: &GetExecutor,
) -> anyhow::Result<bool> {
    /// Helper: check a predicate field against a value, return false on mismatch.
    macro_rules! check {
        ($field:expr, $val:expr) => {
            if let Some(ref field) = $field {
                if !call_predicate(executor, &field.predicate, $val)? {
                    return Ok(false);
                }
            }
        };
    }

    let bool_item = |v: bool| TupleItem::Int(BigInt::from(if v { -1 } else { 0 }));
    let int_item = |v: i64| TupleItem::Int(BigInt::from(v));

    let requires_internal_in_msg = predicates.opcode.is_some()
        || predicates.bounced.is_some()
        || predicates.bounce.is_some()
        || predicates.value.is_some()
        || predicates.from.is_some()
        || predicates.to.is_some();
    let requires_in_msg = requires_internal_in_msg || predicates.state_init.is_some();

    check!(predicates.deploy, bool_item(transaction_is_deploy(tx)));

    let in_msg = tx.load_in_msg();
    if let Ok(Some(in_msg)) = &in_msg {
        if let Some(ref field) = predicates.state_init {
            let Ok(state_init_cell) = CellBuilder::build_from(&in_msg.init) else {
                return Ok(false);
            };
            if !call_predicate(executor, &field.predicate, TupleItem::Cell(state_init_cell))? {
                return Ok(false);
            }
        }

        if let MsgInfo::Int(info) = &in_msg.info {
            check!(predicates.bounced, bool_item(info.bounced));
            if let Some(ref field) = predicates.opcode {
                let mut slice = in_msg.body;
                let Ok(mut opcode) = slice.load_u32() else {
                    return Ok(false);
                };
                if info.bounced && predicates.bounced.is_some() {
                    let Ok(bounced_opcode) = slice.load_u32() else {
                        return Ok(false);
                    };
                    opcode = bounced_opcode;
                }
                if !call_predicate(executor, &field.predicate, int_item(i64::from(opcode)))? {
                    return Ok(false);
                }
            }
            check!(predicates.bounce, bool_item(info.bounce));
            check!(
                predicates.value,
                TupleItem::Int(BigInt::from(info.value.tokens.into_inner()))
            );
            check!(predicates.from, TupleItem::Slice(to_cell(&info.src)));
            check!(predicates.to, TupleItem::Slice(to_cell(&info.dst)));
            check!(predicates.body, TupleItem::Cell(to_cell(&in_msg.body)));
        } else if requires_internal_in_msg {
            return Ok(false);
        }
    } else if requires_in_msg {
        return Ok(false);
    }

    let Ok(TxInfo::Ordinary(ord_info)) = tx.load_info() else {
        return Ok(false);
    };

    let is_skipped = matches!(ord_info.compute_phase, ComputePhase::Skipped(_));
    check!(predicates.compute_phase_skipped, bool_item(is_skipped));
    check!(predicates.aborted, bool_item(ord_info.aborted));

    if let Some(ref field) = predicates.action_exit_code {
        if let Some(action_phase) = &ord_info.action_phase {
            if !call_predicate(
                executor,
                &field.predicate,
                int_item(i64::from(action_phase.result_code)),
            )? {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }

    if let Some(ref field) = predicates.exit_code {
        if let ComputePhase::Executed(compute) = &ord_info.compute_phase {
            if !call_predicate(
                executor,
                &field.predicate,
                int_item(i64::from(compute.exit_code)),
            )? {
                return Ok(false);
            }
        } else {
            return Ok(false);
        }
    }

    if let Some(ref field) = predicates.success {
        let action_success = ord_info.action_phase.as_ref().is_some_and(|a| a.success);
        let is_success = match &ord_info.compute_phase {
            ComputePhase::Executed(compute) => action_success && compute.success,
            ComputePhase::Skipped(_) => false,
        };
        if !call_predicate(executor, &field.predicate, bool_item(is_success))? {
            return Ok(false);
        }
    }

    Ok(true)
}

#[allow(clippy::collapsible_if)]
fn transaction_matches_scalar_params(tx: &Transaction, params: &ScalarSearchParams) -> bool {
    let requires_internal_in_msg = params.opcode.is_some()
        || params.bounced.is_some()
        || params.bounce.is_some()
        || params.value.is_some()
        || params.from.is_some()
        || params.to.is_some();
    let requires_in_msg = requires_internal_in_msg || params.state_init.is_some();
    let expected_body_hash = params.body.as_ref().map(|body| body.repr_hash());

    if let Some(expected_deploy) = params.deploy {
        if expected_deploy != transaction_is_deploy(tx) {
            return false;
        }
    }

    let in_msg = tx.load_in_msg();
    if let Ok(Some(in_msg)) = &in_msg {
        if let Some(expected_state_init) = &params.state_init
            && expected_state_init != &in_msg.init
        {
            return false;
        }

        if let MsgInfo::Int(info) = &in_msg.info {
            if let Some(expected_opcode) = &params.opcode {
                let mut slice = in_msg.body;
                let Ok(mut opcode) = slice.load_u32() else {
                    return false;
                };
                if info.bounced && params.bounced == Some(true) {
                    let Ok(bounced_opcode) = slice.load_u32() else {
                        return false;
                    };
                    opcode = bounced_opcode;
                }
                if *expected_opcode != opcode {
                    return false;
                }
            }

            if let Some(expected_bounced) = &params.bounced
                && *expected_bounced != info.bounced
            {
                return false;
            }

            if let Some(expected_bounce) = &params.bounce
                && *expected_bounce != info.bounce
            {
                return false;
            }

            if let Some(expected_value) = &params.value
                && *expected_value != BigInt::from(info.value.tokens.into_inner())
            {
                return false;
            }

            if let Some(expected_from_addr) = &params.from
                && *expected_from_addr != info.src
            {
                return false;
            }

            if let Some(expected_to_addr) = &params.to
                && *expected_to_addr != info.dst
            {
                return false;
            }

            if let Some(expected_hash) = expected_body_hash.as_ref() {
                let body_cell = to_cell(&in_msg.body);
                let actual_hash = body_cell.repr_hash();
                if expected_hash != &actual_hash {
                    return false;
                }
            }
        } else if requires_internal_in_msg {
            return false;
        }
    } else if requires_in_msg {
        return false;
    }

    let Ok(TxInfo::Ordinary(info)) = tx.load_info() else {
        return false;
    };

    if let Some(expected_compute_skipped) = params.compute_phase_skipped {
        let is_skipped = matches!(info.compute_phase, ComputePhase::Skipped(_));
        if expected_compute_skipped != is_skipped {
            return false;
        }
    }

    if let Some(expected_aborted) = params.aborted
        && expected_aborted != info.aborted
    {
        return false;
    }

    if let Some(expected_action_exit_code) = params.action_exit_code {
        if let Some(action_phase) = &info.action_phase {
            if action_phase.result_code != expected_action_exit_code {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(expected_exit_code) = params.exit_code {
        if let ComputePhase::Executed(compute) = &info.compute_phase {
            if compute.exit_code != expected_exit_code as i32 {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(expected_success) = params.success {
        let action_phase_success = info
            .action_phase
            .as_ref()
            .is_some_and(|action| action.success);

        let is_success = match &info.compute_phase {
            ComputePhase::Executed(compute) => action_phase_success && compute.success,
            ComputePhase::Skipped(_) => false,
        };
        if is_success != expected_success {
            return false;
        }
    }

    true
}

fn transaction_is_deploy(tx: &Transaction) -> bool {
    matches!(
        tx.orig_status,
        AccountStatus::NotExists | AccountStatus::Uninit
    ) && tx.end_status == AccountStatus::Active
}

/// Create a `GetExecutor` suitable for running predicate continuations.
fn make_predicate_executor(ctx: &mut Context) -> anyhow::Result<GetExecutor> {
    // Predicate continuations reference code defined in the test/script that built them
    // (e.g. `T.__eq` instantiations). Both `acton test` and `acton script` populate
    // `test_code` with the compiled code that owns those predicates.
    let code = ctx
        .env
        .test_code
        .as_ref()
        .map(Boc::encode_base64)
        .ok_or_else(|| {
            anyhow!(
                "Predicate-based transaction matchers require a compiled code cell to \
                 evaluate predicate continuations, but none was provided."
            )
        })?;

    let unixtime = resolve_get_method_unixtime(ctx.chain.world_state)?;

    let params = RunGetMethodArgs {
        code,
        data: Boc::encode_base64(Cell::default()),
        verbosity: ctx.env.default_log_level,
        libs: Default::default(),
        address: "0:0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        unixtime,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: 0,
        debug_enabled: false,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let mut executor = GetExecutor::new(&params).context("Cannot create predicate executor")?;
    // Predicate lambdas used inside matchers may call regular FFI helpers
    // (e.g. `build(...)`, `ffi.*`, io helpers). The executor that runs them must
    // have the same ffi surface registered as the outer test/script executor.
    // Registration stores `ctx` as a raw pointer, mirroring what
    // `script_cmd` / `test_cmd` do for their top-level executors, so re-registering
    // here on the same context is safe.
    crate::ffi::register(&mut executor, ctx);
    Ok(executor)
}

fn find_saved_trace_segment_by_tx_lt_range<'a>(
    ctx: &'a Context<'_>,
    first_tx_lt: u64,
    last_tx_lt: u64,
) -> Option<&'a [SendMessageResultSuccess]> {
    ctx.chain.emulations.find_trace_segment_by_tx_lt_range(
        ctx.env.running_id.as_ref(),
        first_tx_lt,
        last_tx_lt,
    )
}

fn find_transaction_in_saved_trace(
    ctx: &Context<'_>,
    first_tx_lt: u64,
    last_tx_lt: u64,
    mut matches: impl FnMut(&Transaction) -> anyhow::Result<bool>,
) -> anyhow::Result<Option<Cell>> {
    let Some(trace) = find_saved_trace_segment_by_tx_lt_range(ctx, first_tx_lt, last_tx_lt) else {
        return Ok(None);
    };

    for result in trace {
        if matches(&result.transaction)? {
            return Ok(Some(to_cell(&result.transaction)));
        }
    }

    Ok(None)
}

extension!(find_transaction_by_params in (Context) with (params: Tuple, last_tx_lt: BigInt, first_tx_lt: BigInt) using find_transaction_by_params_impl);
fn find_transaction_by_params_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    last_tx_lt: BigInt,
    first_tx_lt: BigInt,
) -> anyhow::Result<()> {
    let (Some(first_tx_lt), Some(last_tx_lt)) = (first_tx_lt.to_u64(), last_tx_lt.to_u64()) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let Some(params) = parse_scalar_search_params_tuple(&params) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    match find_transaction_in_saved_trace(ctx, first_tx_lt, last_tx_lt, |tx| {
        Ok(transaction_matches_scalar_params(tx, &params))
    })? {
        Some(cell) => stack.push(TupleItem::Cell(cell)),
        None => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(find_transaction_by_predicate_params in (Context) with (params: Tuple, last_tx_lt: BigInt, first_tx_lt: BigInt) using find_transaction_by_predicate_params_impl);
fn find_transaction_by_predicate_params_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    last_tx_lt: BigInt,
    first_tx_lt: BigInt,
) -> anyhow::Result<()> {
    let (Some(first_tx_lt), Some(last_tx_lt)) = (first_tx_lt.to_u64(), last_tx_lt.to_u64()) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    let predicates = parse_search_params_tuple(&params);
    let executor = make_predicate_executor(ctx)?;

    match find_transaction_in_saved_trace(ctx, first_tx_lt, last_tx_lt, |tx| {
        transaction_matches_predicates(tx, &predicates, &executor)
    })? {
        Some(cell) => stack.push(TupleItem::Cell(cell)),
        None => stack.push(TupleItem::Null),
    }
    Ok(())
}

extension!(run_get_method in (Context) with (args: Tuple, name: String, id: BigInt, code: Cell, address: StdAddr) using run_get_method_impl);
#[allow(clippy::too_many_arguments)]
fn run_get_method_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    args: Tuple,
    name: String,
    id: BigInt,
    code: Cell,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let args = args.unwrap_empty().unwrap_tuple();
    let world_state = &mut ctx.chain.world_state;
    let addr_str = addr.to_string();

    let shard_account = world_state.get_account(&addr);
    let state = shard_account
        .account
        .load()
        .context("Failed to load account")?
        .0
        .map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or_default()
    } else {
        Cell::default()
    };

    let libs = ctx.chain.build_libs_with_hash_owner(&addr.address);
    let libs_root = libs.into_root();
    let world_state = &mut ctx.chain.world_state;

    let method_id = id.to_i32().unwrap_or(0);

    let unixtime = resolve_get_method_unixtime(world_state)?;

    let params = RunGetMethodArgs {
        code: Boc::encode_base64(&code),
        data: Boc::encode_base64(data),
        verbosity: ctx.env.default_log_level,
        libs: libs_root.map(Boc::encode_base64).unwrap_or_default(),
        address: addr_str,
        unixtime,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let compilation_result = ctx
        .build
        .build_cache
        .result_for_code(&Some(code))
        .map(|(_, result)| result);
    // Remote/forked contracts may have no local debug info. We still run the live
    // executor, but the child replayer will then fall back to a synthetic source map.
    let source_map = compilation_result.as_ref().map_or_else(
        || Arc::new(SourceMap::without_debug_info()),
        |result| result.source_map.clone(),
    );
    let abi = compilation_result
        .as_ref()
        .and_then(|result| result.abi.clone());

    let config_b64 = world_state.get_config_b64();
    let args_b64 = serialize_tuple(&args)
        .map(|t| Boc::encode_base64(&t))
        .context("Cannot serialize tuple")?;

    let result = if ctx.debug.is_enabled() {
        let step_executor = StepGetExecutor::new(&args_b64, &params, Some(&config_b64))
            .context("Cannot create get executor")?;
        step_executor
            .prepare(method_id, &args_b64)
            .context("Cannot prepare get method")?;

        let need_to_stop_on_entry = ctx.debug.need_to_stop_child_thread_on_start();
        // Nested get methods reuse the same DAP session via a child context, exactly
        // like nested message sends do, so Step Into can cross the runtime boundary.
        let child_debug_started = ctx
            .debug
            .begin_child_context(ChildDebugContextSpec {
                thread_id: 2,
                name: "Run get method".to_string(),
                executor: step_executor.clone().into(),
                source_map: Some(source_map.clone()),
                abi: abi.clone(),
                stop_on_entry: need_to_stop_on_entry,
            })
            .context("Cannot send response")?;

        // Keep the child aligned with the parent stepping intent: stop on entry when
        // stepping into the call, otherwise continue until the child stops or ends.
        let child_step_mode = if need_to_stop_on_entry {
            match ctx.debug.performing_step() {
                Some(StepMode::EachAsmInstruction) => StepMode::EachAsmInstruction,
                _ => StepMode::StepInto,
            }
        } else {
            StepMode::RunUntilBreakpoint
        };
        run_nested_executor_until_finished(ctx, child_debug_started, child_step_mode, || {
            step_executor.step()
        })
        .context("Cannot finish nested get method execution")?;

        let result = step_executor
            .finish(&params.code)
            .context("Cannot run get method")?;

        if child_debug_started {
            ctx.debug
                .finish_child_context(2)
                .context("Cannot send response")?;
        }

        result
    } else {
        let executor = GetExecutor::new(&params).context("Cannot create get executor")?;
        executor
            .run_get_method(&args_b64, &params, Some(&config_b64))
            .context("Cannot run get method")?
    };

    match result {
        GetMethodResult::Success(result) => {
            ctx.chain
                .emulations
                .save_get_method(&ctx.env.running_id, result.clone());

            let cell =
                Boc::decode_base64(result.stack.as_ref()).context("Failed to decode stack BoC")?;
            let tuple = Tuple::deserialize(&cell).context("Failed to deserialize tuple")?;

            if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
                let get_method = abi
                    .as_deref()
                    .and_then(|abi| abi.find_get_method_by_id(method_id));

                let id_presentation = format!("({id})");
                let id_presentation = id_presentation.dimmed();

                let get_method_presentation = if let Some(get_method) = get_method {
                    format!("{} {id_presentation}", get_method.name.yellow())
                } else if name.is_empty() {
                    format!("'' {id_presentation}")
                } else {
                    format!("{} {id_presentation}", name.yellow())
                };

                let suggested_name = if result.vm_exit_code == 11 {
                    // TODO: right now get methods may not include all get methods
                    let get_methods: Vec<&str> = abi
                        .as_ref()
                        .map(|abi| abi.get_methods.iter().map(|m| m.name.as_str()).collect())
                        .unwrap_or_default();
                    suggest_name(&name, &get_methods).map(ToOwned::to_owned)
                } else {
                    None
                };

                let location =
                    retrace::find_exception_info(&result.vm_log, &source_map).map(|info| info.loc);

                *ctx.asserts.assert_failure =
                    Some(AssertFailure::GetMethod(GetMethodAssertFailure {
                        get_method_presentation,
                        vm_exit_code: result.vm_exit_code,
                        suggested_name,
                        vm_log: result.vm_log,
                        source_map,
                        abi: abi.clone(),
                        caller_trace: None,
                        location,
                    }));

                stack.push(TupleItem::Null);
                return Ok(());
            }

            stack.push(TupleItem::Tuple(tuple));
        }
        GetMethodResult::Error(result) => {
            println!("Error: {}", result.error);
        }
    }

    Ok(())
}

fn suggest_name<'a>(input: &str, candidates: &'a [&'a str]) -> Option<&'a str> {
    let mut best = None;
    let mut best_dist = usize::MAX;

    for &cand in candidates {
        let d = strsim::levenshtein(input, cand);
        if d < best_dist {
            best_dist = d;
            best = Some(cand);
        }
    }

    if best_dist <= 3 { best } else { None }
}

extension!(is_deployed in (Context) with (addr: StdAddr) using is_deployed_impl);
fn is_deployed_impl(ctx: &mut Context, stack: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let is_deployed = ctx.chain.world_state.check_deployed(&addr);
    stack.push_bool(is_deployed);
    Ok(())
}

extension!(get_deployed_code in (Context) with (addr: StdAddr) using get_deployed_code_impl);
fn get_deployed_code_impl(ctx: &mut Context, stk: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let is_deployed = ctx.chain.world_state.check_deployed(&addr);
    if !is_deployed {
        stk.push(TupleItem::Null);
        return Ok(());
    }

    let account = ctx.chain.world_state.get_account(&addr);
    let Some(cell) = get_address_code(&account) else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    stk.push(TupleItem::Cell(cell));
    Ok(())
}

fn get_address_code(account: &ShardAccount) -> Option<Cell> {
    let state = account.account.load().ok()?.0.map(|s| s.state);

    let Some(AccountState::Active(state)) = state else {
        return None;
    };

    state.code
}

extension!(crc16 in (Context) with (data: String) using crc16_impl);
fn crc16_impl(_ctx: &mut Context, stack: &mut Tuple, data: String) -> anyhow::Result<()> {
    const CRC16: Crc<u16> = Crc::<u16>::new(&CRC_16_XMODEM);
    let result = CRC16.checksum(data.as_bytes());
    stack.push(TupleItem::Int(BigInt::from(result)));
    Ok(())
}

extension!(type_name_by_opcode in (Context) with (id: BigInt) using type_name_by_opcode_impl);
fn type_name_by_opcode_impl(ctx: &mut Context, stk: &mut Tuple, id: BigInt) -> anyhow::Result<()> {
    let id = u32::try_from(&id).context("ID is too big for uint32 opcode")?;
    let Some(type_name) = ctx.env.source_map.as_deref().and_then(|source_map| {
        ContractABI::find_message_name_by_opcode_with_symbols(
            source_map,
            ctx.env.abi.as_deref(),
            id,
        )
    }) else {
        stk.push(TupleItem::Null);
        return Ok(());
    };
    stk.push_string(type_name);
    Ok(())
}

extension!(register_address in (Context) with (name: String, address: StdAddr) using register_address_impl);
fn register_address_impl(
    ctx: &mut Context,
    _: &mut Tuple,
    name: String,
    address: StdAddr,
) -> anyhow::Result<()> {
    ctx.build
        .known_addresses
        .addresses
        .insert(address, KnownAddress { name });
    Ok(())
}

extension!(register_code in (Context) with (name: String, code: Cell) using register_code_impl);
fn register_code_impl(
    ctx: &mut Context,
    _: &mut Tuple,
    name: String,
    code: Cell,
) -> anyhow::Result<()> {
    let hash = code.repr_hash();
    ctx.build.known_code_cells.insert(*hash, name);
    Ok(())
}

extension!(account_state in (Context) with (addr: StdAddr) using account_state_impl);
fn account_state_impl(ctx: &mut Context, stk: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let account = ctx.chain.world_state.get_account(&addr);
    let optional_account = account.account.load();
    let Ok(account) = optional_account.map_err(|e| anyhow::anyhow!("Failed to load account: {e}"))
    else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    let Some(account) = account.0 else {
        stk.push(TupleItem::Null);
        return Ok(());
    };

    let mut builder = CellBuilder::new();
    builder.store_bit(true)?;
    account.store_into(&mut builder, Cell::empty_context())?;
    let cell = builder.build()?;

    stk.push(TupleItem::Cell(cell));
    Ok(())
}

extension!(register_lib in (Context) with (lib: Cell) using register_lib_impl);
fn register_lib_impl(ctx: &mut Context, _stack: &mut Tuple, lib: Cell) -> anyhow::Result<()> {
    ctx.chain.world_state.register_lib(lib);
    Ok(())
}

/// Normalize CLI-rendered addresses like "<address> (deployer)" to plain address.
pub(crate) fn normalize_address_input(input: &str) -> &str {
    let trimmed = input.trim();
    if let Some((address, suffix)) = trimmed.rsplit_once(" (")
        && suffix.ends_with(')')
    {
        // Keep only the address part and ignore the human-readable symbolic label.
        address.trim_end()
    } else {
        trimmed
    }
}

extension!(parse_address in (Context) with (address: String) using parse_address_impl);
fn parse_address_impl(_: &mut Context, stack: &mut Tuple, address: String) -> anyhow::Result<()> {
    let (addr, _) = StdAddr::from_str_ext(normalize_address_input(&address), StdAddrFormat::any())
        .with_context(|| format!("Cannot parse address: {address}"))?;
    stack.push(TupleItem::Cell(to_cell(&addr)));
    Ok(())
}

extension!(parse_cell_from_hex in (Context) with (cell_hex: String) using parse_cell_from_hex_impl);
fn parse_cell_from_hex_impl(
    _: &mut Context,
    stack: &mut Tuple,
    cell_hex: String,
) -> anyhow::Result<()> {
    let cell = Boc::decode_hex(cell_hex.trim())
        .with_context(|| format!("Failed to decode cell hex {cell_hex}"))?;
    stack.push(TupleItem::Cell(cell));
    Ok(())
}

extension!(parse_int in (Context) with (x: String) using parse_int_impl);
fn parse_int_impl(_: &mut Context, stack: &mut Tuple, x: String) -> anyhow::Result<()> {
    let value = x
        .trim()
        .parse::<BigInt>()
        .with_context(|| format!("Failed to parse integer from '{x}'"))?;
    stack.push(TupleItem::Int(value));
    Ok(())
}

extension!(load_library_by_hash in (Context) with (hash: String) using load_library_by_hash_impl);
fn load_library_by_hash_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    hash: String,
) -> anyhow::Result<()> {
    let Ok(hash) = HashBytes::from_str(&hash) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    if let Some(cell) = ctx.chain.world_state.find_lib_by_hash(&hash) {
        stack.push(TupleItem::Cell(cell));
        return Ok(());
    }

    let network = ctx.network();
    let custom_networks = ctx.env.config.custom_networks();

    if let Network::Custom(network_name) = &network
        && !custom_networks.contains_key(network_name.as_ref())
    {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let Ok(api_client) = TonApiClient::new(network, custom_networks) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    match api_client.get_library_by_hash(&hash) {
        Ok(cell) => {
            stack.push(TupleItem::Cell(cell));
        }
        Err(_) => {
            stack.push(TupleItem::Null);
        }
    }

    Ok(())
}

extension!(is_broadcasting in (Context) using is_broadcasting_impl);
fn is_broadcasting_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    stack.push_bool(ctx.is_broadcasting);
    Ok(())
}

extension!(enable_broadcast in (Context) using enable_broadcast_impl);
const fn enable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) -> anyhow::Result<()> {
    ctx.is_broadcasting = true;
    Ok(())
}

extension!(disable_broadcast in (Context) using disable_broadcast_impl);
const fn disable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) -> anyhow::Result<()> {
    ctx.is_broadcasting = false;
    Ok(())
}

extension!(get_wallet_by_name in (Context) with (name: String) using get_wallet_by_name_impl);
fn get_wallet_by_name_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    name: String,
) -> anyhow::Result<()> {
    if let Some(wallet) = ctx.env.open_wallets.get(&name) {
        let addr = wallet.address();
        stack.push(TupleItem::Cell(to_cell(&addr)));
        return Ok(());
    }

    if let Some(tonconnect) = &ctx.env.tonconnect {
        stack.push(TupleItem::Cell(to_cell(&tonconnect.wallet.address)));
        return Ok(());
    }

    stack.push(TupleItem::Null);
    Ok(())
}

extension!(get_wallet_key_pair in (Context) with (addr: StdAddr) using get_wallet_key_pair_impl);
fn get_wallet_key_pair_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let Some(wallet) = find_open_wallet_by_address(ctx, &addr) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    // Match lib/crypto.tolk: privateKey is the 32-byte Ed25519 seed.
    let private_key = BigInt::from_bytes_be(Sign::Plus, &wallet.wallet.key_pair.secret_key[..32]);
    let public_key = BigInt::from_bytes_be(Sign::Plus, &wallet.wallet.key_pair.public_key);

    let mut result = Tuple::empty();
    result.push(TupleItem::Int(private_key));
    result.push(TupleItem::Int(public_key));
    stack.push(TupleItem::Tuple(result));

    Ok(())
}

extension!(get_wallet_id in (Context) with (addr: StdAddr) using get_wallet_id_impl);
fn get_wallet_id_impl(ctx: &mut Context, stack: &mut Tuple, addr: StdAddr) -> anyhow::Result<()> {
    let Some(wallet) = find_open_wallet_by_address(ctx, &addr) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    stack.push(TupleItem::Int(BigInt::from(wallet.wallet.wallet_id)));

    Ok(())
}

fn find_open_wallet_by_address<'a>(ctx: &'a Context, addr: &StdAddr) -> Option<&'a Wallet> {
    ctx.env
        .open_wallets
        .values()
        .find(|wallet| wallet.address() == *addr)
}

const WAIT_FOR_TRANSACTION_DEFAULT_SLEEP_MS: u64 = 1000;
const WAIT_FOR_TRANSACTION_SETTLE_DELAY_MS: u64 = 1000;

fn read_broadcast_target(tx_cell: &Cell) -> anyhow::Result<(String, HashBytes)> {
    let parsed: Transaction = tx_cell
        .parse::<Transaction>()
        .context("Failed to parse pseudo broadcast tx")?;
    let in_msg = parsed
        .in_msg
        .as_ref()
        .context("Pseudo broadcast tx has no in_msg")?;
    let dest = ext_in_dest_address(in_msg)?;
    Ok((dest, parsed.prev_trans_hash))
}

extension!(wait_for_transaction in (Context) with (sleep_duration: BigInt, attempts: BigInt, quiet: bool, tx_cell: Cell) using wait_for_transaction_impl);
fn wait_for_transaction_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    sleep_duration: BigInt,
    attempts: BigInt,
    quiet: bool,
    tx_cell: Cell,
) -> anyhow::Result<()> {
    if !ctx.is_broadcasting {
        // Tolk short-circuits emulation mode before calling FFI; this branch is defensive.
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let attempts = attempts.to_u32().unwrap_or(20);
    let sleep_duration_ms = sleep_duration
        .to_u64()
        .unwrap_or(WAIT_FOR_TRANSACTION_DEFAULT_SLEEP_MS);

    if attempts == 0 {
        anyhow::bail!("Attempt number must be positive");
    }

    // Non-pseudo txs (e.g. a real emulation result from `net.send` with broadcast toggled
    // on afterwards) won't have the external-in-shaped lookup target — treat that as
    // "nothing to wait for" rather than bailing.
    let Ok((dest_address, target_hash)) = read_broadcast_target(&tx_cell) else {
        stack.push(TupleItem::Null);
        return Ok(());
    };

    if !ctx.can_broadcast_to_network() {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let network = ctx.network();
    let custom_networks = ctx.env.config.custom_networks();
    let api_client = match TonApiClient::new(network, custom_networks) {
        Ok(client) => client,
        Err(err) => {
            log::warn!("Failed to initialize toncenter client for waitForTransaction: {err:#}");
            stack.push(TupleItem::Null);
            return Ok(());
        }
    };

    for attempt in 1..=attempts {
        if !quiet {
            println!("Awaiting transaction... [Attempt {attempt}/{attempts}]");
        }

        match poll_send_result_v2(&api_client, &dest_address, &target_hash) {
            Ok(Some(polled)) => {
                // Short settle delay so descendant transactions are more likely to be visible.
                std::thread::sleep(Duration::from_millis(WAIT_FOR_TRANSACTION_SETTLE_DELAY_MS));

                if !quiet {
                    println!("Transaction successfully applied!");
                    let link = transaction_link(ctx, &dest_address, &polled);
                    println!("You can view it at {}", link.underline());
                }

                ctx.chain.world_state.invalidate_remote_cache();

                // Tolk expects a BigArray here (see the Tolk-side unwrap): a bare struct tuple
                // would be spread across multiple stack slots and cause a stack underflow.
                stack.push(TupleItem::big_array_from_items(vec![polled.send_result]));
                return Ok(());
            }
            Ok(None) => {}
            Err(err) => {
                log::debug!("waitForTransaction poll failed: {err:#}");
            }
        }

        if attempt < attempts {
            std::thread::sleep(Duration::from_millis(sleep_duration_ms));
        }
    }

    stack.push(TupleItem::Null);
    Ok(())
}

struct PolledSendResult {
    tx_hash_hex: String,
    lt: u64,
    utime: u32,
    send_result: TupleItem,
}

/// One polling step using toncenter v2 — a direct translation of the TON docs
/// `waitForTransaction` reference: fetch the destination account's recent transactions
/// (`limit: 10, archival: true`), and for each transaction whose `inMessage` is
/// external-in, locally compute the TEP-467 normalized hash and compare with the target.
///
/// Returns `Ok(Some(...))` on a hit, or `Ok(None)` when the indexer hasn't seen the
/// transaction yet. Transport errors propagate as `Err`.
fn poll_send_result_v2(
    client: &TonApiClient,
    dest_address: &str,
    target_hash: &HashBytes,
) -> anyhow::Result<Option<PolledSendResult>> {
    let txs = client.get_transactions(dest_address, Some(100), None, None)?;
    for tx in txs {
        let tx_cell = Boc::decode_base64(&tx.data)
            .context("Failed to decode transaction BoC from toncenter")?;
        let parsed_tx: Transaction = tx_cell
            .parse::<Transaction>()
            .context("Failed to parse transaction from toncenter BoC")?;
        let Some(in_msg_cell) = parsed_tx.in_msg.as_ref() else {
            continue;
        };
        // Docs: `if (transaction.inMessage?.info.type !== 'external-in') continue;`
        let parsed_in_msg = in_msg_cell
            .parse::<Message<'_>>()
            .context("Failed to parse inMessage of polled transaction")?;
        if !matches!(parsed_in_msg.info, MsgInfo::ExtIn(_)) {
            continue;
        }
        let actual_hash = compute_normalized_ext_in_hash(&parsed_in_msg)?;
        if actual_hash != *target_hash {
            continue;
        }
        let send_result = tx_cell_to_send_result_tuple(tx_cell, &parsed_tx, &[], None);
        let tx_hash_hex = base64::engine::general_purpose::STANDARD
            .decode(&tx.transaction_id.hash)
            .map_or_else(|_| tx.transaction_id.hash.clone(), hex::encode);
        return Ok(Some(PolledSendResult {
            tx_hash_hex,
            lt: parsed_tx.lt,
            utime: parsed_tx.now,
            send_result,
        }));
    }
    Ok(None)
}

fn transaction_link(ctx: &mut Context, address_str: &str, polled: &PolledSendResult) -> String {
    let network = ctx.network();
    let tx_hash_hex = polled.tx_hash_hex.as_str();
    match &network {
        Network::Localnet => {
            if let Some(url) = localnet_transaction_link(ctx, tx_hash_hex) {
                return url;
            }
        }
        Network::Custom(network_name) => {
            if let Some(url) =
                custom_network_transaction_link(ctx, network_name.as_ref(), tx_hash_hex)
            {
                return url;
            }
        }
        Network::Mainnet | Network::Testnet => {}
    }

    let network_prefix = if network.uses_testnet_address_format() {
        "testnet."
    } else {
        ""
    };
    let explorer = ctx.env.explorer.unwrap_or(Explorer::Tonscan);
    match explorer {
        Explorer::Tonscan => format!("https://{network_prefix}tonscan.org/tx/{tx_hash_hex}"),
        Explorer::Toncx => format!(
            "https://{network_prefix}ton.cx/tx/{}:{tx_hash_hex}:{address_str}",
            polled.lt
        ),
        Explorer::Dton => format!(
            "https://{network_prefix}dton.io/tx/{tx_hash_hex}?time={}",
            polled.utime
        ),
        Explorer::Tonviewer => {
            format!("https://{network_prefix}tonviewer.com/transaction/{tx_hash_hex}")
        }
    }
}

fn custom_network_transaction_link(
    ctx: &Context,
    network_name: &str,
    tx_hash_hex: &str,
) -> Option<String> {
    let custom_networks = ctx.env.config.custom_networks();
    let network_urls = custom_networks.get(network_name)?;
    configured_network_transaction_link(network_urls, tx_hash_hex)
}

fn localnet_transaction_link(ctx: &Context, tx_hash_hex: &str) -> Option<String> {
    let custom_networks = ctx.env.config.custom_networks();
    let localnet_urls = custom_networks.get("localnet")?;
    configured_network_transaction_link(localnet_urls, tx_hash_hex)
}

#[derive(Serialize)]
struct RegisterAbiPayload {
    entries: Vec<RegisterAbiEntry>,
}

#[derive(Serialize)]
struct RegisterAbiEntry {
    code_hash: String,
    #[serde(rename = "compiler_abi")]
    abi: serde_json::Value,
}

fn register_localnet_abis(
    ctx: &Context,
    custom_networks: &HashMap<String, acton_config::config::CustomNetworkUrls>,
) -> anyhow::Result<()> {
    if !matches!(ctx.network(), Network::Localnet) {
        return Ok(());
    }

    let mut entries_by_hash = HashMap::<String, serde_json::Value>::new();
    for result in ctx.build.build_cache.built.values() {
        let Some(abi) = result.abi.as_ref() else {
            continue;
        };
        let mut abi = abi.as_ref().clone();
        if abi.contract_name.is_empty() {
            abi.contract_name.clone_from(&result.name);
        }
        entries_by_hash
            .entry(result.code_hash.to_string())
            .or_insert(serde_json::to_value(&abi)?);
    }

    if entries_by_hash.is_empty() {
        return Ok(());
    }

    let url = localnet_admin_url(custom_networks, "compiler-abis")?;
    let payload = RegisterAbiPayload {
        entries: entries_by_hash
            .into_iter()
            .map(|(code_hash, abi)| RegisterAbiEntry { code_hash, abi })
            .collect(),
    };

    let client = crate::http::blocking_client_builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(5))
        .user_agent(crate::build_info::user_agent())
        .build()?;
    let response = client
        .post(&url)
        .json(&payload)
        .send()
        .with_context(|| format!("Failed to POST compiler ABI registry to {url}"))?;
    let status = response.status();
    let body = response.text().unwrap_or_default();

    anyhow::ensure!(
        status.is_success(),
        "Localnet compiler ABI registration failed with status {status}: {body}"
    );

    let response_json: serde_json::Value = serde_json::from_str(&body).with_context(|| {
        format!("Localnet compiler ABI registration returned invalid JSON: {body}")
    })?;
    anyhow::ensure!(
        response_json.get("ok").and_then(serde_json::Value::as_bool) == Some(true),
        "Localnet compiler ABI registration failed: {}",
        response_json
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(body.as_str())
    );

    Ok(())
}

fn localnet_admin_url(
    custom_networks: &HashMap<String, acton_config::config::CustomNetworkUrls>,
    endpoint: &str,
) -> anyhow::Result<String> {
    let v2_url = Network::Localnet.toncenter_v2_url(custom_networks)?;
    let mut url = reqwest::Url::parse(&v2_url)?;
    let base_path = url.path().trim_end_matches('/');
    let admin_base = base_path.strip_suffix("/api/v2").unwrap_or(base_path);
    let admin_path = if admin_base.is_empty() {
        format!("/admin/{endpoint}")
    } else {
        format!("{admin_base}/admin/{endpoint}")
    };

    url.set_path(&admin_path);
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

fn configured_network_transaction_link(
    network_urls: &acton_config::config::CustomNetworkUrls,
    tx_hash_hex: &str,
) -> Option<String> {
    if let Some(explorer_url) = network_urls.explorer_url.as_deref() {
        return explorer_transaction_link(explorer_url, tx_hash_hex);
    }

    let mut explorer_base = reqwest::Url::parse(network_urls.v2_url.as_ref()).ok()?;
    explorer_base.set_path("/explorer");
    explorer_base.set_query(None);
    explorer_base.set_fragment(None);

    explorer_transaction_link(explorer_base.as_str(), tx_hash_hex)
}

fn explorer_transaction_link(explorer_base: &str, tx_hash_hex: &str) -> Option<String> {
    let mut url = reqwest::Url::parse(explorer_base).ok()?;
    let base_path = url.path().trim_end_matches('/');
    let tx_base = if base_path.is_empty() {
        "/tx".to_string()
    } else if base_path.ends_with("/tx") {
        base_path.to_string()
    } else {
        format!("{base_path}/tx")
    };
    let path = format!("{tx_base}/{tx_hash_hex}");
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
}

extension!(get_config in (Context) using get_config_impl);
fn get_config_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    if ctx.can_broadcast_to_network() {
        let network = ctx.network();
        let custom_networks = ctx.env.config.custom_networks();
        let api_client = TonApiClient::new(network, custom_networks)?;
        let config = api_client.get_config_all()?;
        stack.push(TupleItem::Cell(config));
        return Ok(());
    }

    let config = ctx.chain.world_state.get_config_cell();
    stack.push(TupleItem::Cell(config));
    Ok(())
}

extension!(set_config in (Context) with (config: Cell) using set_config_impl);
fn set_config_impl(ctx: &mut Context, stack: &mut Tuple, config: Cell) -> anyhow::Result<()> {
    let result = ctx.chain.emulator.set_config(ctx.chain.world_state, config);

    match result {
        Ok(res) => {
            stack.push_bool(res);
        }
        Err(_) => {
            stack.push_bool(false);
        }
    }

    Ok(())
}

extension!(set_now in (Context) with (now: BigInt) using set_now_impl);
fn set_now_impl(ctx: &mut Context, _: &mut Tuple, now: BigInt) -> anyhow::Result<()> {
    let now_u32 = now.to_u32().unwrap_or(0);
    ctx.chain.world_state.set_now(now_u32);
    Ok(())
}

extension!(get_now in (Context) using get_now_impl);
fn get_now_impl(ctx: &mut Context, stack: &mut Tuple) -> anyhow::Result<()> {
    let now = ctx.chain.world_state.get_now();
    stack.push(TupleItem::Int(BigInt::from(now)));
    Ok(())
}

extension!(get_shard_account in (Context) with (addr: StdAddr) using get_shard_account_impl);
fn get_shard_account_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let shard_account = ctx.chain.world_state.get_account(&addr);
    let shard_account_cell = to_cell(&shard_account);
    stack.push(TupleItem::Cell(shard_account_cell));
    Ok(())
}

extension!(set_shard_account in (Context) with (shard_account: Option<ShardAccount>, addr: StdAddr) using set_shard_account_impl);
fn set_shard_account_impl(
    ctx: &mut Context,
    _stack: &mut Tuple,
    shard_account: Option<ShardAccount>,
    addr: StdAddr,
) -> anyhow::Result<()> {
    let shard_account = match shard_account {
        Some(shard_account) => shard_account,
        None => ShardAccount {
            account: Lazy::new(&OptionalAccount(None))
                .context("Failed to create empty shard account")?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        },
    };

    ctx.chain.world_state.update_account(&addr, &shard_account);
    Ok(())
}

extension!(call_tolk_function in (Context) with (addr: StdAddr, arg: TupleItem, function: TupleItem) using call_tolk_function_impl);
fn call_tolk_function_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    addr: StdAddr,
    args: TupleItem,
    function: TupleItem,
) -> anyhow::Result<()> {
    let TupleItem::Cont(cont) = function else {
        anyhow::bail!("Expected Cont, got {function:?}");
    };

    let TupleItem::Tuple(args_stack) = args else {
        anyhow::bail!("Expected Tuple, got {args:?}");
    };

    // Serialize the VmCont (with savelist, captured stack, code)
    let mut cont_builder = CellBuilder::new();
    tvm_ffi::serde::serialize_vm_cont(&mut cont_builder, &cont)?;
    let cont_cell = cont_builder.build()?;
    let cont_boc = Boc::encode_base64(cont_cell);

    // Serialize args as VmStack
    let args = args_stack.unwrap_empty().unwrap_tuple();
    let stack_boc = serialize_tuple(&args)
        .map(|t| Boc::encode_base64(&t))
        .context("Cannot serialize args stack")?;

    // Get account state for emulator initialization
    let world_state = &mut ctx.chain.world_state;
    let addr_str = addr.to_string();
    let shard_account = world_state.get_account(&addr);
    let account_state = shard_account
        .account
        .load()
        .context("Failed to load account")?
        .0
        .map(|s| s.state);

    let (code, data) = if let Some(AccountState::Active(state)) = account_state {
        (
            Boc::encode_base64(state.code.unwrap_or_default()),
            Boc::encode_base64(state.data.unwrap_or_default()),
        )
    } else if let Some(test_code) = &ctx.env.test_code {
        // Use the test contract's compiled code (needed for c3 / CALLDICT)
        (
            Boc::encode_base64(test_code),
            Boc::encode_base64(Cell::default()),
        )
    } else {
        (
            Boc::encode_base64(Cell::default()),
            Boc::encode_base64(Cell::default()),
        )
    };

    let libs = ctx.chain.build_libs_with_hash_owner(&addr.address);
    let libs_root = libs.into_root();

    let unixtime = resolve_get_method_unixtime(ctx.chain.world_state)?;

    let params = RunGetMethodArgs {
        code,
        data,
        verbosity: ctx.env.default_log_level,
        libs: libs_root.map(Boc::encode_base64).unwrap_or_default(),
        address: addr_str,
        unixtime,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: 0,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let executor = GetExecutor::new(&params).context("Cannot create get executor")?;
    let result = executor
        .run_continuation(&cont_boc, &stack_boc)
        .context("Cannot run continuation")?;

    match result {
        GetMethodResult::Success(result) => {
            // NOTE: Intentionally not saving this result into `emulations.get_methods`.
            // `GetMethodResultSuccess.code` is `#[serde(skip)]` and is only populated by
            // `run_get_method` (which has the contract code at hand). `run_continuation`
            // cannot fill it, so stored continuation results would have an empty `code`
            // and break downstream consumers that look it up (coverage + failed-get-method
            // exception source-map resolution in `src/formatter.rs`). Continuation
            // executions are out of scope for coverage anyway.
            let cell =
                Boc::decode_base64(result.stack.as_ref()).context("Failed to decode stack BoC")?;
            let tuple = Tuple::deserialize(&cell).context("Failed to deserialize tuple")?;

            if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
                anyhow::bail!(
                    "Continuation execution failed with exit code {}",
                    result.vm_exit_code
                );
            }

            stack.push(TupleItem::Tuple(tuple));
            Ok(())
        }
        GetMethodResult::Error(err) => {
            anyhow::bail!("Continuation execution error: {}", err.error);
        }
    }
}

extension!(save_world_state_snapshot in (Context) with (path: String) using save_world_state_snapshot_impl);
fn save_world_state_snapshot_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    path: String,
) -> anyhow::Result<()> {
    let Some(path) = ctx.resolve_project_write_path(&path) else {
        stack.push_bool(false);
        return Ok(());
    };

    let success = ctx
        .chain
        .world_state
        .snapshot()
        .and_then(|snapshot| serde_json::to_string_pretty(&snapshot).map_err(Into::into))
        .is_ok_and(|json| fs::write(path, json).is_ok());
    stack.push_bool(success);
    Ok(())
}

extension!(load_world_state_snapshot in (Context) with (path: String) using load_world_state_snapshot_impl);
fn load_world_state_snapshot_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    path: String,
) -> anyhow::Result<()> {
    let Some(path) = ctx.resolve_project_read_path(&path) else {
        stack.push_bool(false);
        return Ok(());
    };

    let success = fs::read_to_string(path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .and_then(|snapshot| ctx.chain.world_state.load_snapshot(snapshot).ok())
        .is_some();
    stack.push_bool(success);
    Ok(())
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        6 => build : 2,
        8 => run_get_method : 5,
        9 => send_message : 2,
        10 => find_transaction_by_params : 3,
        11 => is_deployed : 1,
        12 => get_deployed_code : 1,
        13 => crc16 : 1,
        14 => type_name_by_opcode : 1,
        15 => register_address : 2,
        16 => register_code : 2,
        17 => account_state : 1,
        18 => register_lib : 1,
        19 => parse_address : 1,
        20 => parse_cell_from_hex : 1,
        21 => load_library_by_hash : 1,
        23 => is_broadcasting : 0,
        24 => get_wallet_by_name : 1,
        25 => wait_for_transaction : 4,
        26 => enable_broadcast : 0,
        27 => disable_broadcast : 0,
        28 => set_now : 1,
        29 => get_now : 0,
        30 => send_single_message : 2,
        31 => get_config : 0,
        32 => set_config : 1,
        33 => get_shard_account : 1,
        34 => set_shard_account : 2,
        35 => save_trace_name : 2,
        36 => run_tick_tock : 2,
        37 => save_world_state_snapshot : 1,
        38 => load_world_state_snapshot : 1,
        39 => start_message_iter : 2,
        40 => execute_message_iter_n : 2,
        41 => execute_message_iter_till : 2,
        42 => execute_message_iter_from : 1,
        43 => is_message_iter_done : 1,
        44 => close_message_iter : 1,
        45 => get_wallet_key_pair : 1,
        46 => get_wallet_id : 1,
        47 => parse_int : 1,
        48 => find_transaction_by_predicate_params : 3,
        49 => wait_for_trace : 4,
        501 => call_tolk_function : 3,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use rustc_hash::FxHashSet;
    use std::sync::Arc;

    fn test_hash(byte: u8) -> HashBytes {
        HashBytes([byte; 32])
    }

    fn test_transaction(lt: u64) -> Transaction {
        Transaction {
            account: Default::default(),
            lt,
            prev_trans_hash: Default::default(),
            prev_trans_lt: 0,
            now: 0,
            out_msg_count: Default::default(),
            orig_status: AccountStatus::Uninit,
            end_status: AccountStatus::Uninit,
            in_msg: None,
            out_msgs: Default::default(),
            total_fees: Default::default(),
            state_update: Lazy::new(&HashUpdate {
                old: Default::default(),
                new: Default::default(),
            })
            .expect("Invalid state update"),
            info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
                credit_first: false,
                storage_phase: None,
                credit_phase: None,
                compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                    reason: ComputePhaseSkipReason::NoState,
                }),
                action_phase: None,
                aborted: false,
                bounce_phase: None,
                destroyed: false,
            }))
            .expect("Invalid transaction info"),
        }
    }

    fn test_success(lt: u64) -> SendMessageResult {
        let empty_shard_account = ShardAccount {
            account: Lazy::new(&OptionalAccount(None)).expect("empty optional account"),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        };
        SendMessageResult::Success(SendMessageResultSuccess {
            raw_transaction: Arc::from(format!("tx-{lt}")),
            transaction: test_transaction(lt),
            parent_transaction: None,
            child_transactions: vec![],
            shard_account_before: empty_shard_account.clone(),
            shard_account: empty_shard_account,
            vm_log: Arc::default(),
            executor_logs: Arc::default(),
            actions: None,
            code: None,
            externals: vec![],
            missing_libraries: FxHashSet::default(),
        })
    }

    fn test_error(message: &str) -> SendMessageResult {
        SendMessageResult::Error(ton_executor::message::RunTransactionResultError {
            error: message.to_string(),
            vm_log: None,
            vm_exit_code: None,
            executor_logs: None,
            missing_libraries: FxHashSet::default(),
        })
    }

    #[test]
    fn configured_network_transaction_link_prefers_explorer_url() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: Some(Arc::from("https://explorer.example/explorer")),
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("explorer link should be built");
        assert_eq!(url, "https://explorer.example/explorer/tx/abc123");
    }

    #[test]
    fn configured_network_transaction_link_keeps_existing_tx_suffix() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: Some(Arc::from("https://explorer.example/explorer/tx/")),
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("explorer link should be built");
        assert_eq!(url, "https://explorer.example/explorer/tx/abc123");
    }

    #[test]
    fn configured_network_transaction_link_appends_tx_for_host_only_explorer() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: Some(Arc::from("http://localhost:3006")),
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("explorer link should be built");
        assert_eq!(url, "http://localhost:3006/tx/abc123");
    }

    #[test]
    fn configured_network_transaction_link_falls_back_to_v2() {
        let urls = acton_config::config::CustomNetworkUrls {
            v2_url: Arc::from("http://localhost:3010/api/v2"),
            v3_url: None,
            explorer_url: None,
        };

        let url = configured_network_transaction_link(&urls, "abc123")
            .expect("fallback link should be built");
        assert_eq!(url, "http://localhost:3010/explorer/tx/abc123");
    }

    #[test]
    fn step_batch_keeps_later_siblings_after_error_result() {
        let mut message_iters = MessageIterState::new();
        let cursor_id = message_iters.insert_message_cursor(Cell::default(), None, test_hash(1));
        message_iters
            .advance(cursor_id)
            .expect("root step must be consumed");
        message_iters
            .push_child_message(cursor_id, Cell::default(), 100)
            .expect("first child must be queued");
        message_iters
            .push_child_message(cursor_id, Cell::default(), 100)
            .expect("second child must be queued");

        let mut calls = 0usize;
        let batch = execute_message_iter_batch_with(
            &mut message_iters,
            cursor_id,
            IterationStop::Exhausted,
            |_pending, _| {
                calls += 1;
                Ok(if calls == 1 {
                    test_error("first child error")
                } else {
                    test_success(202)
                })
            },
        )
        .expect("batch execution should succeed");

        assert!(batch.hard_error.is_none());
        assert_eq!(batch.results.len(), 2);
        assert!(matches!(batch.results[0], SendMessageResult::Error(_)));
        assert!(matches!(batch.results[1], SendMessageResult::Success(_)));
        assert!(message_iters.is_done(cursor_id));
    }

    #[test]
    fn step_batch_preserves_pending_step_on_hard_error_after_partial_progress() {
        let mut message_iters = MessageIterState::new();
        let cursor_id = message_iters.insert_message_cursor(Cell::default(), None, test_hash(2));
        message_iters
            .advance(cursor_id)
            .expect("root step must be consumed");
        let second_child = Cell::default();
        message_iters
            .push_child_message(cursor_id, Cell::default(), 200)
            .expect("first child must be queued");
        message_iters
            .push_child_message(cursor_id, second_child.clone(), 200)
            .expect("second child must be queued");

        let mut calls = 0usize;
        let batch = execute_message_iter_batch_with(
            &mut message_iters,
            cursor_id,
            IterationStop::Exhausted,
            |_pending, _| {
                calls += 1;
                if calls == 1 {
                    Ok(test_success(303))
                } else {
                    Err(anyhow!("hard step failure"))
                }
            },
        )
        .expect("partial progress should be returned instead of dropped");

        assert_eq!(batch.results.len(), 1);
        assert!(matches!(batch.results[0], SendMessageResult::Success(_)));
        assert_eq!(
            batch
                .hard_error
                .expect("hard error must be preserved")
                .to_string(),
            "hard step failure"
        );

        let (pending, owner) = message_iters
            .peek_next(cursor_id)
            .expect("failed step must stay pending for retry");
        assert_eq!(owner, test_hash(2));
        assert_eq!(pending.parent_lt, Some(200));
        assert_eq!(pending.message, second_child);
        assert!(!message_iters.is_done(cursor_id));
    }

    /// Verify that `synthesize_tx_cell_from_v3` reconstructs transactions whose cell `BoCs`
    /// match a captured snapshot, and whose `repr_hash` equals the on-chain `hash` reported
    /// by toncenter.
    ///
    /// Fixtures are live mainnet traces captured from `/api/v3/traces` into
    /// `testdata/v3_trace_fixture*.json`; the expected synthesized-BoC output is recorded
    /// alongside each response in `testdata/v3_trace_fixture*.synthesized.txt` as one
    /// `<tx_hash_b64> <boc_b64>` line per transaction. Regenerate with `UPDATE_EXPECT=1`
    /// after a deliberate synthesis change — a silent drift here would break the on-chain
    /// `res.tx.hash()` contract that `waitForTrace` users rely on.
    #[test]
    fn synthesize_tx_cell_from_v3_reproduces_on_chain_hash() {
        use expect_test::expect_file;
        use ton_api::V3Trace;

        #[derive(serde::Deserialize)]
        struct TraceEnvelope {
            traces: Vec<V3Trace>,
        }

        let fixtures = [
            (
                include_str!("testdata/v3_trace_fixture.json"),
                expect_file!["testdata/v3_trace_fixture.synthesized.txt"],
            ),
            (
                include_str!("testdata/v3_trace_fixture_multi.json"),
                expect_file!["testdata/v3_trace_fixture_multi.synthesized.txt"],
            ),
        ];

        for (idx, (fixture, expected)) in fixtures.into_iter().enumerate() {
            let envelope: TraceEnvelope = serde_json::from_str(fixture)
                .unwrap_or_else(|e| panic!("fixture {idx} must deserialize: {e:#}"));
            let trace = envelope
                .traces
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("fixture {idx} has no traces"));

            // Iterate in indexer-reported order so the snapshot is stable —
            // `V3Trace.transactions` is a HashMap and has no natural ordering.
            let mut actual = String::new();
            for tx_hash_b64 in &trace.transactions_order {
                let summary = trace
                    .transactions
                    .get(tx_hash_b64)
                    .unwrap_or_else(|| panic!("trace references unknown tx {tx_hash_b64}"));
                let on_chain = parse_hash_bytes(tx_hash_b64).unwrap_or_else(|e| {
                    panic!("bad on-chain hash in fixture {idx} {tx_hash_b64}: {e:#}")
                });
                let (cell, _parsed) = synthesize_tx_cell_from_v3(summary)
                    .unwrap_or_else(|e| panic!("synthesize failed for {tx_hash_b64}: {e:#}"));
                assert_eq!(
                    cell.repr_hash(),
                    &on_chain,
                    "synthesized repr_hash must match on-chain hash for {tx_hash_b64} \
                     (fixture {idx})",
                );
                actual.push_str(tx_hash_b64);
                actual.push(' ');
                actual.push_str(&Boc::encode_base64(cell));
                actual.push('\n');
            }
            expected.assert_eq(&actual);
        }
    }
}

extension!(wait_for_trace in (Context) with (sleep_duration: BigInt, attempts: BigInt, quiet: bool, tx_cell: Cell) using wait_for_trace_impl);
fn wait_for_trace_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    sleep_duration: BigInt,
    attempts: BigInt,
    quiet: bool,
    tx_cell: Cell,
) -> anyhow::Result<()> {
    if !ctx.is_broadcasting {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let attempts = attempts.to_u32().unwrap_or(20);
    let sleep_duration_ms = sleep_duration
        .to_u64()
        .unwrap_or(WAIT_FOR_TRANSACTION_DEFAULT_SLEEP_MS);

    if attempts == 0 {
        anyhow::bail!("Attempt number must be positive");
    }

    // Use the TEP-467 normalized hash toncenter returned from `sendBocReturnHash` (stashed
    // in the pseudo tx's `prev_trans_hash`). v3 `/traces` accepts it as `msg_hash` and
    // matches against the same value stored by its indexer.
    let Ok((_, target_hash)) = read_broadcast_target(&tx_cell) else {
        // Non-pseudo txs don't carry the external-in lookup target — nothing to wait for.
        stack.push(TupleItem::Null);
        return Ok(());
    };

    if !ctx.can_broadcast_to_network() {
        stack.push(TupleItem::Null);
        return Ok(());
    }

    let msg_hash_hex = hex::encode(target_hash.as_slice());

    let network = ctx.network();
    let custom_networks = ctx.env.config.custom_networks();
    let api_client = match TonApiClient::new(network, custom_networks) {
        Ok(client) => client,
        Err(err) => {
            warn!("Failed to initialize toncenter client for waitForTrace: {err:#}");
            stack.push(TupleItem::Null);
            return Ok(());
        }
    };

    let mut last_transport_err: Option<anyhow::Error> = None;
    for attempt in 1..=attempts {
        if !quiet {
            println!("Awaiting trace... [Attempt {attempt}/{attempts}]");
        }

        match poll_send_results_by_trace(&api_client, &msg_hash_hex) {
            Ok(TracePollOutcome::Settled(send_results)) => {
                if !quiet {
                    println!("Trace settled with {} transaction(s)", send_results.len());
                }
                ctx.chain.world_state.invalidate_remote_cache();
                stack.push(TupleItem::big_array_from_items(send_results));
                return Ok(());
            }
            Ok(TracePollOutcome::NotYet) => {
                // Successful call, trace just not indexed yet — clear any stashed transport
                // error so a transient earlier failure doesn't poison the timeout result.
                last_transport_err = None;
            }
            Ok(TracePollOutcome::Incomplete) => {
                // `is_incomplete=true` means the trace exceeds toncenter's size threshold —
                // the response is truncated and retries won't help. Bail so callers don't
                // silently consume a partial `SendResultList`.
                anyhow::bail!(
                    "waitForTrace: toncenter returned is_incomplete=true for the trace of \
                     {msg_hash_hex} — the trace exceeds the indexer's size limit and cannot \
                     be fetched in full"
                );
            }
            Err(err) => {
                // Surface the failure on each attempt, and hold on to the last one so we can
                // include it in the bail message if errors persist through timeout.
                warn!("waitForTrace poll failed on attempt {attempt}: {err:#}");
                last_transport_err = Some(err);
            }
        }

        if attempt < attempts {
            std::thread::sleep(Duration::from_millis(sleep_duration_ms));
        }
    }

    // Only bail if we ended the budget on a run of transport errors. If the last poll was a
    // clean Ok(None), fall through to the documented "null when not fetched in time" path.
    if let Some(err) = last_transport_err {
        return Err(err.context(format!(
            "waitForTrace: toncenter polling failed on every attempt ({attempts} total)"
        )));
    }

    stack.push(TupleItem::Null);
    Ok(())
}

/// Result of one polling step. `Incomplete` is split out from `Err` so the retry loop can
/// bail instead of treating it as a transient transport failure and burning the budget.
enum TracePollOutcome {
    Settled(Vec<TupleItem>),
    NotYet,
    Incomplete,
}

/// One polling step for a full trace.
///
/// Returns `NotYet` when the indexer hasn't yet built the trace or referenced txs aren't
/// resolvable yet, `Incomplete` when the indexer explicitly flagged the trace as truncated,
/// and `Err` for transport / parse failures the caller may retry on.
fn poll_send_results_by_trace(
    client: &TonApiClient,
    msg_hash_hex: &str,
) -> anyhow::Result<TracePollOutcome> {
    let traces = client.get_traces_by_msg_hash(msg_hash_hex, 1)?;
    let Some(trace) = traces.into_iter().next() else {
        return Ok(TracePollOutcome::NotYet);
    };
    if trace.is_incomplete {
        return Ok(TracePollOutcome::Incomplete);
    }
    if trace.transactions_order.is_empty() {
        return Ok(TracePollOutcome::NotYet);
    }

    let transactions = match build_v3_trace_transactions(&trace)? {
        V3TraceTransactions::Ready(transactions) => transactions,
        V3TraceTransactions::Pending { .. } => {
            // Indexer returned an id it cannot resolve — trace is still being assembled.
            return Ok(TracePollOutcome::NotYet);
        }
    };
    if has_unmatched_internal_out_messages(&transactions) {
        return Ok(TracePollOutcome::NotYet);
    }
    let results = transactions
        .iter()
        .map(V3TraceTransaction::to_send_result_tuple)
        .collect::<Vec<_>>();

    Ok(TracePollOutcome::Settled(results))
}

fn has_unmatched_internal_out_messages(transactions: &[V3TraceTransaction]) -> bool {
    let in_msg_hashes = transactions
        .iter()
        .filter_map(|tx| tx.summary.in_msg.as_ref().and_then(v3_message_hash))
        .collect::<HashSet<_>>();

    let is_std_address =
        |address: &str| StdAddr::from_str_ext(address, StdAddrFormat::any()).is_ok();
    transactions.iter().any(|tx| {
        tx.summary.out_msgs.iter().any(|message| {
            message.source.as_deref().is_some_and(is_std_address)
                && message.destination.as_deref().is_some_and(is_std_address)
                && v3_message_hash(message).is_some_and(|hash| !in_msg_hashes.contains(hash))
        })
    })
}

/// Synthesize a `Transaction` cell from a toncenter v3 trace summary.
///
/// `TonCenter` `/traces` only ships structured summary fields, not the raw `BoC`, so we
/// reconstruct a structurally valid `Transaction` from them. The synthesized cell's
/// `repr_hash` matches the on-chain hash for traces whose external-in messages carry no
/// non-standard fields; when toncenter reports a distinct `hash_norm` (i.e. the original
/// cell had a non-addr_none src, a non-zero `import_fee`, or an init), the original
/// external-in data is lost and the reconstructed tx hash won't match.
pub(crate) fn synthesize_tx_cell_from_v3(
    summary: &V3TransactionSummary,
) -> anyhow::Result<(Cell, Transaction)> {
    let account = parse_account_hash(&summary.account)
        .with_context(|| format!("Unsupported account address format: {}", summary.account))?;
    let lt: u64 = summary
        .lt
        .parse()
        .with_context(|| format!("Invalid lt '{}' in v3 tx summary", summary.lt))?;
    let prev_trans_lt: u64 = summary
        .prev_trans_lt
        .as_deref()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let prev_trans_hash = summary
        .prev_trans_hash
        .as_deref()
        .map(parse_hash_bytes)
        .transpose()?
        .unwrap_or_default();

    let in_msg = summary
        .in_msg
        .as_ref()
        .map(build_message_cell_from_v3)
        .transpose()
        .context("Failed to reconstruct inbound message cell")?;

    let mut out_msgs = Dict::<Uint15, Cell>::new();
    for (idx, m) in summary.out_msgs.iter().enumerate() {
        let key = Uint15::new(idx as u16);
        let cell = build_message_cell_from_v3(m).with_context(|| {
            format!(
                "Failed to reconstruct outbound message {idx} of tx {}",
                summary.hash
            )
        })?;
        out_msgs.set(key, cell)?;
    }
    let out_msg_count = Uint15::new(summary.out_msgs.len() as u16);

    let total_fees = CurrencyCollection {
        tokens: parse_tokens_opt(summary.total_fees.as_deref()),
        other: ExtraCurrencyCollection::new(),
    };

    let state_update_old = summary
        .account_state_before
        .as_ref()
        .and_then(|s| s.hash.as_deref())
        .map(parse_hash_bytes)
        .transpose()?
        .unwrap_or_default();
    let state_update_new = summary
        .account_state_after
        .as_ref()
        .and_then(|s| s.hash.as_deref())
        .map(parse_hash_bytes)
        .transpose()?
        .unwrap_or_default();

    let tx = Transaction {
        account,
        lt,
        prev_trans_hash,
        prev_trans_lt,
        now: summary.now,
        out_msg_count,
        orig_status: parse_account_status(summary.orig_status.as_deref()),
        end_status: parse_account_status(summary.end_status.as_deref()),
        in_msg,
        out_msgs,
        total_fees,
        state_update: Lazy::new(&HashUpdate {
            old: state_update_old,
            new: state_update_new,
        })
        .context("Failed to build synthetic state_update")?,
        info: Lazy::new(&build_tx_info_from_v3(summary.description.as_ref()))
            .context("Failed to build synthetic tx info")?,
    };

    let cell = to_cell(&tx);
    Ok((cell, tx))
}

/// Build a full `Message` cell from a v3 message summary.
///
/// TL-B leaves two Either choices — init (inline vs ref) and body (inline vs ref) — unset
/// by the summary. Toncenter doesn't report which branch the on-chain cell used, so we try
/// every combination and pick the one whose `repr_hash` matches the summary's `hash`.
/// Without this the synthesized tx's `repr_hash` can disagree with the on-chain hash on any
/// transaction whose messages happened to be laid out differently than our fixed guess.
fn build_message_cell_from_v3(m: &V3MessageSummary) -> anyhow::Result<Cell> {
    let body_cell = match m
        .message_content
        .as_ref()
        .and_then(|c| c.body.as_deref())
        .filter(|b| !b.is_empty())
    {
        Some(b) => Boc::decode_base64(b).context("Failed to decode message body BoC")?,
        None => CellBuilder::new().build().context("empty body cell")?,
    };
    let init_cell = match m
        .init_state
        .as_ref()
        .and_then(|i| i.body.as_deref())
        .filter(|b| !b.is_empty())
    {
        Some(b) => Some(Boc::decode_base64(b).context("Failed to decode message init BoC")?),
        None => None,
    };

    let info = infer_msg_info_from_v3(m)?;
    let expected_hash = m.hash.as_deref().and_then(|h| parse_hash_bytes(h).ok());

    let try_layout = |init_as_ref: bool, body_as_ref: bool| -> anyhow::Result<Cell> {
        let ctx = Cell::empty_context();
        let mut b = CellBuilder::new();
        info.store_into(&mut b, ctx)?;
        match &init_cell {
            Some(c) => {
                b.store_bit_one()?; // Maybe.Just
                if init_as_ref {
                    b.store_bit_one()?; // Either.Right (^StateInit)
                    b.store_reference(c.clone())?;
                } else {
                    b.store_bit_zero()?; // Either.Left (inline StateInit)
                    b.store_slice(c.as_slice_allow_exotic())?;
                    for r in 0..c.reference_count() {
                        b.store_reference(c.reference_cloned(r).expect("ref exists"))?;
                    }
                }
            }
            None => b.store_bit_zero()?, // Maybe.Nothing
        }
        if body_as_ref {
            b.store_bit_one()?; // Either.Right (ref)
            b.store_reference(body_cell.clone())?;
        } else {
            b.store_bit_zero()?; // Either.Left (inline)
            b.store_slice(body_cell.as_slice_allow_exotic())?;
            for r in 0..body_cell.reference_count() {
                b.store_reference(body_cell.reference_cloned(r).expect("ref exists"))?;
            }
        }
        Ok(b.build()?)
    };

    // Try (init_ref, body_ref) combinations. Prefer ref-form first since that's what
    // toncenter emits when indexing messages, and it matches the majority of on-chain layouts.
    let layouts: &[(bool, bool)] = match (init_cell.is_some(), expected_hash.is_some()) {
        (true, true) => &[(true, true), (true, false), (false, true), (false, false)],
        (false, true) => &[(true, true), (false, false), (true, false), (false, true)],
        _ => &[(true, true)],
    };

    let mut last = None;
    for &(init_as_ref, body_as_ref) in layouts {
        match try_layout(init_as_ref, body_as_ref) {
            Ok(cell) => {
                if let Some(expected) = &expected_hash
                    && cell.repr_hash() == expected
                {
                    return Ok(cell);
                }
                last = Some(cell);
            }
            Err(e) => {
                // Inline-storage can fail if the inlined cell's bits/refs don't fit alongside
                // the existing info prefix — fall through to the next layout rather than
                // bailing the whole synthesis.
                log::debug!("Message layout build failed ({init_as_ref},{body_as_ref}): {e:#}");
            }
        }
    }

    last.ok_or_else(|| anyhow!("no buildable message layout for summary hash {:?}", m.hash))
}

/// Decide which `MsgInfo` variant a v3 message summary represents and pack its fields.
///
/// Classify by the address pair: `source=None` → external-in, `destination=None` →
/// external-out, both present → internal.
fn infer_msg_info_from_v3(m: &V3MessageSummary) -> anyhow::Result<MsgInfo> {
    let src_str = m.source.as_deref().filter(|s| !s.is_empty());
    let dst_str = m.destination.as_deref().filter(|s| !s.is_empty());

    match (src_str, dst_str) {
        (None, Some(_)) => {
            let dst = parse_int_addr(dst_str)?
                .context("External-in message missing destination address")?;
            Ok(MsgInfo::ExtIn(ExtInMsgInfo {
                src: None,
                dst,
                import_fee: parse_tokens_opt(m.import_fee.as_deref()),
            }))
        }
        (Some(_), None) => {
            let src =
                parse_int_addr(src_str)?.context("External-out message missing source address")?;
            // External-out destinations are opaque ExtAddr; we leave dst = None.
            Ok(MsgInfo::ExtOut(ExtOutMsgInfo {
                src,
                dst: None,
                created_lt: m
                    .created_lt
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                created_at: m
                    .created_at
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            }))
        }
        (Some(_), Some(_)) => {
            let src = parse_int_addr(src_str)?
                .context("Internal message summary missing source address")?;
            let dst = parse_int_addr(dst_str)?
                .context("Internal message summary missing destination address")?;
            Ok(MsgInfo::Int(IntMsgInfo {
                ihr_disabled: m.ihr_disabled.unwrap_or(true),
                bounce: m.bounce.unwrap_or(false),
                bounced: m.bounced.unwrap_or(false),
                src,
                dst,
                value: CurrencyCollection {
                    tokens: parse_tokens_opt(m.value.as_deref()),
                    other: ExtraCurrencyCollection::new(),
                },
                ihr_fee: parse_tokens_opt(m.ihr_fee.as_deref()),
                fwd_fee: parse_tokens_opt(m.fwd_fee.as_deref()),
                created_lt: m
                    .created_lt
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                created_at: m
                    .created_at
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            }))
        }
        (None, None) => anyhow::bail!(
            "message summary has neither source nor destination; cannot classify variant"
        ),
    }
}

/// Translate a v3 transaction description into a minimal `TxInfo::Ordinary`. Fields we
/// don't have (`credit_phase`, `bounce_phase`, per-phase cell hashes / message sizes) are left
/// empty; storage, compute and action phases preserve their fees / success / gas /
/// exit-code / result-code values so `SearchParams { success, actionExitCode, ... }` and
/// `toHave(All)SuccessfulTx` continue to evaluate correctly on traced results.
fn build_tx_info_from_v3(desc: Option<&V3TxDescription>) -> TxInfo {
    let Some(desc) = desc else {
        return TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: false,
            storage_phase: None,
            credit_phase: None,
            compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                reason: ComputePhaseSkipReason::NoState,
            }),
            action_phase: None,
            aborted: true,
            bounce_phase: None,
            destroyed: false,
        });
    };

    let compute_phase = match desc.compute_ph.as_ref() {
        Some(cp) if cp.skipped == Some(true) => ComputePhase::Skipped(SkippedComputePhase {
            reason: parse_compute_phase_skip_reason(cp.reason.as_deref()),
        }),
        Some(cp) => ComputePhase::Executed(tycho_types::models::ExecutedComputePhase {
            success: cp.success.unwrap_or(false),
            msg_state_used: cp.msg_state_used.unwrap_or(false),
            account_activated: cp.account_activated.unwrap_or(false),
            gas_fees: parse_tokens_opt(cp.gas_fees.as_deref()),
            gas_used: cp
                .gas_used
                .as_deref()
                .and_then(|s| s.parse::<u64>().ok())
                .map(tycho_types::num::VarUint56::new)
                .unwrap_or_default(),
            gas_limit: cp
                .gas_limit
                .as_deref()
                .and_then(|s| s.parse::<u64>().ok())
                .map(tycho_types::num::VarUint56::new)
                .unwrap_or_default(),
            gas_credit: cp
                .gas_credit
                .as_deref()
                .and_then(|s| s.parse::<u32>().ok())
                .map(tycho_types::num::VarUint24::new),
            mode: cp.mode.unwrap_or(0),
            exit_code: cp.exit_code.unwrap_or(0),
            exit_arg: cp.exit_arg,
            vm_steps: cp.vm_steps.unwrap_or(0),
            vm_init_state_hash: cp
                .vm_init_state_hash
                .as_deref()
                .map(parse_hash_bytes)
                .transpose()
                .ok()
                .flatten()
                .unwrap_or_default(),
            vm_final_state_hash: cp
                .vm_final_state_hash
                .as_deref()
                .map(parse_hash_bytes)
                .transpose()
                .ok()
                .flatten()
                .unwrap_or_default(),
        }),
        None => ComputePhase::Skipped(SkippedComputePhase {
            reason: ComputePhaseSkipReason::NoState,
        }),
    };

    let storage_phase = desc.storage_ph.as_ref().map(|sp| StoragePhase {
        storage_fees_collected: parse_tokens_opt(sp.storage_fees_collected.as_deref()),
        storage_fees_due: sp
            .storage_fees_due
            .as_deref()
            .and_then(|s| s.parse::<u128>().ok())
            .map(Tokens::new),
        status_change: parse_account_status_change(sp.status_change.as_deref()),
    });

    let action_phase = desc.action.as_ref().map(|ap| ActionPhase {
        success: ap.success.unwrap_or(false),
        valid: ap.valid.unwrap_or(false),
        no_funds: ap.no_funds.unwrap_or(false),
        status_change: parse_account_status_change(ap.status_change.as_deref()),
        total_fwd_fees: ap
            .total_fwd_fees
            .as_deref()
            .and_then(|s| s.parse::<u128>().ok())
            .map(Tokens::new),
        total_action_fees: ap
            .total_action_fees
            .as_deref()
            .and_then(|s| s.parse::<u128>().ok())
            .map(Tokens::new),
        result_code: ap.result_code.unwrap_or(0),
        result_arg: ap.result_arg,
        total_actions: ap.tot_actions.unwrap_or(0),
        special_actions: ap.spec_actions.unwrap_or(0),
        skipped_actions: ap.skipped_actions.unwrap_or(0),
        messages_created: ap.msgs_created.unwrap_or(0),
        action_list_hash: ap
            .action_list_hash
            .as_deref()
            .map(parse_hash_bytes)
            .transpose()
            .ok()
            .flatten()
            .unwrap_or_default(),
        total_message_size: ap
            .tot_msg_size
            .as_ref()
            .map(|s| StorageUsedShort {
                cells: s
                    .cells
                    .as_deref()
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(tycho_types::num::VarUint56::new)
                    .unwrap_or_default(),
                bits: s
                    .bits
                    .as_deref()
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(tycho_types::num::VarUint56::new)
                    .unwrap_or_default(),
            })
            .unwrap_or_default(),
    });

    let credit_phase = desc
        .credit_ph
        .as_ref()
        .map(|cp| tycho_types::models::CreditPhase {
            due_fees_collected: cp
                .due_fees_collected
                .as_deref()
                .and_then(|s| s.parse::<u128>().ok())
                .map(Tokens::new),
            credit: CurrencyCollection {
                tokens: parse_tokens_opt(cp.credit.as_deref()),
                other: ExtraCurrencyCollection::new(),
            },
        });

    TxInfo::Ordinary(OrdinaryTxInfo {
        credit_first: desc.credit_first.unwrap_or(false),
        storage_phase,
        credit_phase,
        compute_phase,
        action_phase,
        aborted: desc.aborted.unwrap_or(false),
        bounce_phase: None,
        destroyed: desc.destroyed.unwrap_or(false),
    })
}

fn parse_compute_phase_skip_reason(s: Option<&str>) -> ComputePhaseSkipReason {
    match s {
        Some("bad_state") => ComputePhaseSkipReason::BadState,
        Some("no_gas") => ComputePhaseSkipReason::NoGas,
        Some("suspended") => ComputePhaseSkipReason::Suspended,
        _ => ComputePhaseSkipReason::NoState,
    }
}

fn parse_account_status_change(s: Option<&str>) -> AccountStatusChange {
    match s {
        Some("frozen") => AccountStatusChange::Frozen,
        Some("deleted") => AccountStatusChange::Deleted,
        _ => AccountStatusChange::Unchanged,
    }
}

fn parse_account_hash(account: &str) -> anyhow::Result<HashBytes> {
    // Expect "wc:hex" or "hex"; the `account` field in v3 is `<wc>:<hex>`.
    let hex_part = account.rsplit(':').next().unwrap_or(account);
    parse_hash_bytes(hex_part)
}

fn parse_hash_bytes(h: &str) -> anyhow::Result<HashBytes> {
    // Accept hex or base64 (std / urlsafe).
    let bytes = if h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(h).context("hex decode")?
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(h)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(h))
            .or_else(|_| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(h))
            .context("base64 decode")?
    };
    if bytes.len() != 32 {
        anyhow::bail!("expected 32-byte hash, got {}", bytes.len());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(HashBytes(out))
}

fn parse_int_addr(s: Option<&str>) -> anyhow::Result<Option<IntAddr>> {
    match s {
        None | Some("") => Ok(None),
        Some(a) => IntAddr::from_str(a)
            .map(Some)
            .map_err(|e| anyhow!("Failed to parse address '{a}': {e:?}")),
    }
}

fn parse_tokens_opt(s: Option<&str>) -> Tokens {
    s.and_then(|v| v.parse::<u128>().ok())
        .map(Tokens::new)
        .unwrap_or_default()
}

fn parse_account_status(s: Option<&str>) -> AccountStatus {
    match s {
        Some("active") => AccountStatus::Active,
        Some("frozen") => AccountStatus::Frozen,
        Some("nonexist") => AccountStatus::NotExists,
        _ => AccountStatus::Uninit,
    }
}
