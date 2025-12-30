use crate::commands::common::error_fmt;
use crate::config::Explorer;
use crate::context::{Context, KnownAddress, Wallet};
use crate::debugger::debug_context::StepMode;
use crate::ffi::assert::process_txs_and_search_params;
use crate::formatter::FormatterContext;
use base64::Engine;
use crc::{CRC_16_XMODEM, Crc};
use emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use emulator::step_executor::StepExecutor;
use emulator::step_get_executor::StepGetExecutor;
use emulator::utils::{BaseExecutor, StoreExt};
use emulator::{AnyExecutor, extension, pop_args, register_ext_methods, remote, try_ctx};
use log::{debug, info, warn};
use num_bigint::BigInt;
use num_traits::{ToPrimitive, Zero};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::str::FromStr;
use std::time::{Duration, Instant};
use ton_api::{Network, TonApiClient, TonCenterTransaction};
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::message::{EmulationResult, RunTransactionArgs};
use tonlib_core::TonAddress;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::block::msg_address::{MsgAddrIntStd, MsgAddress};
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::serde::serialize_tuple;
use tvmffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Load, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, ComputePhaseSkipReason, HashUpdate, IntAddr,
    LibDescr, MsgInfo, OrdinaryTxInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount,
    SkippedComputePhase, Transaction, TxInfo,
};

extension!(build in (Context) with (path: String, name: String) using build_impl);
fn build_impl(ctx: &mut Context, stack: &mut Tuple, mut path: String, mut name: String) {
    debug!("Building {name}");
    let id = name.clone();
    let start_time = Instant::now();

    if path.is_empty() {
        debug!("No path provided, search in contracts");
        let found_contract = ctx.env.find_contract(name.as_str());

        if let Some(found_contract) = found_contract {
            debug!("Found contract with info: {found_contract:?}");
            name = found_contract.name; // use actual name instead of id
            path = found_contract.src.clone();
        } else {
            ctx.asserts
                .fail(error_fmt::contract_not_found(ctx.env.config, &name));
            return;
        }
    }

    if let Some(override_code) = ctx.env.build_override.get(id.as_str()) {
        debug!("Overriding code for {name}");
        stack.push(TupleItem::Cell(override_code.clone()));
        return;
    }

    // TODO: add test for this case
    if path.ends_with(".boc") {
        // For BoC source we just return it as a Cell
        let binary_data = try_ctx!(ctx, fs::read(&path), "Cannot read BoC file {}");
        let cell = try_ctx!(
            ctx,
            ArcCell::from_boc(binary_data.as_slice()),
            "Failed to decode code BoC for {}: {}",
            path
        );
        stack.push(TupleItem::Cell(cell));
        return;
    }

    if let Some(cached) = ctx.build.build_cache.built.get(&path) {
        let elapsed = start_time.elapsed();
        info!("Build {path} from memory cache in {elapsed:?}");

        let code_cell = try_ctx!(
            ctx,
            ArcCell::from_boc_b64(&cached.code_boc64),
            "Failed to decode cached code BoC for {}: {}",
            path
        );
        stack.push(TupleItem::Cell(code_cell));
        return;
    }

    if let Some(cached_entry) =
        ctx.build
            .file_build_cache
            .get(&path, ctx.build.need_debug_info, 2, "1.2".to_string())
    {
        let elapsed = start_time.elapsed();
        info!("Build {path} from file cache (.acton/cache) in {elapsed:?}");

        ctx.build.build_cache.memoize(
            &name,
            &path,
            &cached_entry.code_boc64,
            &cached_entry.code_hash_hex,
            cached_entry.source_map.clone().unwrap_or_default(),
        );

        let code_cell = try_ctx!(
            ctx,
            ArcCell::from_boc_b64(&cached_entry.code_boc64),
            "Failed to decode cached code BoC for {}: {}",
            path
        );
        stack.push(TupleItem::Cell(code_cell));
        return;
    }

    let compile_start = Instant::now();
    let result = tolkc::compile(Path::new(&path), ctx.build.need_debug_info);
    let compile_time = compile_start.elapsed();

    match result {
        tolkc::CompilerResult::Success(success) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Build {path} from source (compilation: {compile_time:?}, total: {total_elapsed:?})"
            );

            if let Err(err) = ctx.build.file_build_cache.put(
                &path,
                &success,
                ctx.build.need_debug_info,
                2,
                "1.2".to_string(),
            ) {
                warn!("Failed to build cached code BoC for {path}: {err}");
            }

            ctx.build.build_cache.memoize(
                &name,
                &path,
                &success.code_boc64,
                &success.code_hash_hex,
                success.source_map.unwrap_or(Default::default()),
            );
            let code_cell = try_ctx!(
                ctx,
                ArcCell::from_boc_b64(&success.code_boc64),
                "Failed to decode compiled code BoC for {}: {}",
                path
            );
            stack.push(TupleItem::Cell(code_cell))
        }
        tolkc::CompilerResult::Error(error) => {
            let total_elapsed = start_time.elapsed();
            info!(
                "Build {} failed after {:?}: {}",
                path, total_elapsed, error.message
            );

            ctx.asserts
                .fail(format!("Compilation failed: {}", error.message));
            stack.push(TupleItem::Null);
        }
    };
}

extension!(send_message in (Context) with (mode: BigInt, from: ArcCell, message: ArcCell) using send_message_impl);
fn send_message_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    _mode: BigInt,
    from: ArcCell,
    message: ArcCell,
) {
    let emulator = &ctx.chain.emulator;

    let msg_b64 = try_ctx!(
        ctx,
        message.to_boc(false),
        "Failed to encode message to BoC: {}"
    );
    let msg_cell = try_ctx!(
        ctx,
        Boc::decode(msg_b64),
        "Failed to decode message from BoC: {}"
    );

    let from_b64 = try_ctx!(
        ctx,
        from.to_boc(false),
        "Failed to encode from address to BoC: {}"
    );
    let from_cell = try_ctx!(
        ctx,
        Boc::decode(from_b64),
        "Failed to decode from address from BoC: {}"
    );
    let mut from_slice = try_ctx!(
        ctx,
        from_cell.as_slice(),
        "Failed to create slice `from` from address cell: {}"
    );
    let src_addr = IntAddr::load_from(&mut from_slice);
    let src_addr = match src_addr {
        Ok(src_addr) => src_addr,
        Err(err) => {
            ctx.asserts.fail(format!(
                "Failed to decode src address from x{{{}}} with length={}: {}",
                from_slice.display_data(),
                from_slice.size_bits(),
                err
            ));
            return;
        }
    };

    if let Some(wallet) = ctx.env.find_wallet_by_address(&src_addr) {
        let result = send_wallet_message(
            &message,
            wallet,
            &ctx.network(),
            ctx.chain.blockchain.get_api_key(),
        );
        try_ctx!(ctx, result, "Failed to send message to real network: {}");

        // Add pseudo transaction to the result list to wait on it
        let tx = Transaction {
            account: Default::default(),
            lt: 0,
            prev_trans_hash: Default::default(),
            prev_trans_lt: 0,
            now: ctx.chain.blockchain.get_now(),
            out_msg_count: Default::default(),
            orig_status: AccountStatus::Uninit,
            end_status: AccountStatus::Uninit,
            in_msg: Some(
                Boc::decode(
                    message
                        .to_boc(false)
                        .expect("Unreachable, cannot encode valid message cell"),
                )
                .expect("Unreachable, cannot decode/encode message cell"),
            ),
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

        let tx_cell = tx.to_cell();
        let tx_cell = ArcCell::from_boc_hex(&Boc::encode_hex(tx_cell))
            .expect("Unreachable, cannot decode/encode cell");

        let transaction_cells = vec![TupleItem::Tuple(Tuple(vec![
            TupleItem::Cell(tx_cell),
            TupleItem::Tuple(Tuple::empty()),
            TupleItem::Null,
            TupleItem::Cell(ArcCell::default()),
            TupleItem::Tuple(Tuple::empty()),
            TupleItem::Int(BigInt::from(0)),
            TupleItem::Tuple(Tuple::empty()),
        ]))];
        stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
        return;
    }

    let libs = ctx.chain.build_libs(&src_addr);
    let blockchain = &mut ctx.chain.blockchain;

    let emulations = if ctx.debug.is_enabled() {
        send_message_debug(ctx, &msg_cell, &libs, Some(src_addr))
    } else {
        emulator.send_message(blockchain, msg_cell, &libs, Some(src_addr))
    };

    let successful_emulations = emulations.iter().filter_map(|emulation| match emulation {
        SendMessageResult::Success(res) => Some(res),
        SendMessageResult::Error(_) => None,
    });

    let transaction_cells = successful_emulations
        .filter_map(emulation_to_send_result)
        .collect::<Vec<_>>();

    ctx.chain.emulations.results.push(emulations);
    stack.push(TupleItem::Tuple(Tuple(transaction_cells)));
}

fn emulation_to_send_result(emulation: &SendMessageResultSuccess) -> Option<TupleItem> {
    let Ok(tx) = ArcCell::from_boc_b64(&emulation.raw_transaction) else {
        return None;
    };
    let child_txs = Tuple(
        emulation
            .child_transactions
            .iter()
            .map(|lt| TupleItem::Int(BigInt::from(*lt)))
            .collect::<Vec<_>>(),
    );
    let parent_lt = match &emulation.parent_transaction {
        Some(parent_tx) => TupleItem::Int(BigInt::from(parent_tx.lt)),
        None => TupleItem::Null,
    };
    let actions = match &emulation.actions {
        Some(actions_b64) => {
            ArcCell::from_boc_b64(actions_b64).unwrap_or_else(|_| ArcCell::default())
        }
        None => ArcCell::default(),
    };

    let result = tx.to_boc(false).ok()?;
    let tx_cell = Boc::decode(&result).ok()?;
    let mut tx_slice = tx_cell.as_slice().ok()?;
    let parsed_tx = Transaction::load_from(&mut tx_slice).ok()?;

    let out_messages = Tuple(
        parsed_tx
            .iter_out_msgs()
            .filter_map(|msg| msg.ok())
            .filter_map(|msg| {
                let cell = msg.to_cell();
                let boc = Boc::encode_base64(&cell);
                ArcCell::from_boc_b64(&boc).ok()
            })
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    let gas_used = match parsed_tx.load_info() {
        Ok(TxInfo::Ordinary(info)) => match info.compute_phase {
            ComputePhase::Executed(compute) => compute.gas_used.into(),
            _ => BigInt::from(0),
        },
        _ => BigInt::from(0),
    };

    let externals_tuple = Tuple(
        emulation
            .externals
            .iter()
            .filter_map(|ext_cell| {
                let boc = Boc::encode_base64(ext_cell);
                ArcCell::from_boc_b64(&boc).ok()
            })
            .map(TupleItem::Cell)
            .collect::<Vec<_>>(),
    );

    Some(TupleItem::Tuple(Tuple(vec![
        TupleItem::Cell(tx),
        TupleItem::Tuple(child_txs),
        parent_lt,
        TupleItem::Cell(actions),
        TupleItem::Tuple(out_messages),
        TupleItem::Int(gas_used),
        TupleItem::Tuple(externals_tuple),
    ])))
}

fn send_wallet_message(
    message: &ArcCell,
    wallet: Wallet,
    network: &str,
    api_key: &Option<String>,
) -> anyhow::Result<()> {
    let expired_at_time = std::time::SystemTime::now() + Duration::from_secs(600);
    let expire_at = expired_at_time
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as u32;

    let (seqno, need_state_init) = wallet.seqno(network)?;
    let external = wallet.wallet.create_external_msg(
        expire_at,
        seqno,
        need_state_init,
        vec![message.clone()],
    )?;

    if api_key.is_none() {
        std::thread::sleep(Duration::from_millis(1000)); // rate limit
    }

    let network = Network::from_str(network)?;
    let client = TonApiClient::new(network, api_key.clone());
    client.send_boc(&external.to_boc_b64(false)?)?;

    Ok(())
}

fn send_message_debug(
    ctx: &mut Context,
    msg_cell: &Cell,
    libs: &Dict<HashBytes, LibDescr>,
    src_addr: Option<IntAddr>,
) -> Vec<SendMessageResult> {
    let mut msg_slice = try_ctx!(
        ctx,
        msg_cell.as_slice(),
        "Failed to create slice from message cell: {}"
    );
    let message_obj = try_ctx!(
        ctx,
        RelaxedMessage::load_from(&mut msg_slice),
        "Failed to load message from slice: {}"
    );

    let RelaxedMsgInfo::Int(int_message) = &message_obj.info else {
        ctx.asserts
            .fail("Emulator only supports internal messages for now".to_string());
        return vec![];
    };

    let dest_account = ctx
        .chain
        .blockchain
        .get_account(&int_message.dst.to_string());
    let code = Emulator::get_code_cell(&message_obj, &dest_account);

    let step_executor = StepExecutor::new();
    let source_map = ctx
        .build
        .build_cache
        .result_for_code(&code)
        .map(|res| res.1.source_map);

    let need_to_stop_on_entry = ctx.debug.ctx().need_to_stop_child_thread_on_start();

    ctx.debug
        .ctx()
        .begin_thread(
            2,
            AnyExecutor::Message(step_executor.clone()),
            source_map,
            "Send internal message".to_string(),
            need_to_stop_on_entry,
        )
        .expect("Cannot send response");

    let msg_cell = Emulator::patch_src_addr(msg_cell.clone(), src_addr.clone());
    let prepare_result = step_executor.prepare_transaction(
        msg_cell.clone(),
        BigInt::from(0),
        RunTransactionArgs {
            libs: libs.clone().into_root().map(Boc::encode_base64),
            shard_account: Boc::encode_base64(dest_account.to_cell()),
            now: ctx.chain.blockchain.get_now(),
            lt: ctx.chain.blockchain.get_lt(),
            random_seed: None,
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
            is_tick_tock: None,
            is_tock: None,
        },
    );
    if !prepare_result.success {
        panic!("Failed to prepare Emulator in debug mode");
    }
    if prepare_result.skipped {
        // Since compute phase is skipped, we don't need to run anything
        ctx.debug
            .ctx()
            .finish_thread(2)
            .expect("Cannot send response");
        return vec![];
    }

    // Step to update internal state
    if need_to_stop_on_entry {
        ctx.debug.ctx().step(StepMode::StepIn);
    } else {
        ctx.debug.ctx().step(StepMode::Continue);
    }

    if !ctx.debug.ctx().stepper.is_terminated() {
        // Process requests only if we have something to execute and generates a requests
        ctx.debug
            .ctx()
            .process_incoming_requests(false)
            .expect("Cannot send response");
    }

    let result = step_executor.finish_transaction();

    ctx.debug
        .ctx()
        .finish_thread(2)
        .expect("Cannot send response");

    if ctx.debug.ctx().performing_step != Some(StepMode::Continue) {
        // When we step out from nested message/get method, send stop message to client to
        // stop on a line after send/call get method
        ctx.debug.ctx().step(StepMode::StepIn);
    }

    let result = match result {
        EmulationResult::Success(result) => result,
        EmulationResult::Error(_) => {
            return vec![];
        }
    };

    let shard_account_after = &result.shard_account;
    let shard_account_cell = try_ctx!(
        ctx,
        Boc::decode_base64(shard_account_after),
        "Failed to decode shard account BoC: {}"
    );
    let mut shard_account_slice = try_ctx!(
        ctx,
        shard_account_cell.as_slice(),
        "Failed to create slice from shard account cell: {}"
    );
    let shard_account = try_ctx!(
        ctx,
        ShardAccount::load_from(&mut shard_account_slice),
        "Failed to load shard account from slice: {}"
    );

    ctx.chain
        .blockchain
        .update_account(&int_message.dst.to_string(), &shard_account);

    let tx_cell: Cell = try_ctx!(
        ctx,
        Boc::decode_base64(&result.transaction),
        "Failed to decode transaction BoC: {}"
    );
    let mut tx_slice = try_ctx!(
        ctx,
        tx_cell.as_slice(),
        "Failed to create slice from transaction cell: {}"
    );
    let transaction = try_ctx!(
        ctx,
        Transaction::load_from(&mut tx_slice),
        "Failed to load transaction from slice: {}"
    );

    let out_messages = transaction
        .iter_out_msgs()
        .filter_map(|it| it.ok())
        .map(|it| it.to_cell())
        .collect::<Vec<_>>();

    let code = Emulator::get_code_cell(&message_obj, &dest_account);

    let send_result = SendMessageResultSuccess {
        raw_transaction: result.transaction,
        transaction: transaction.clone(),
        parent_transaction: None,
        child_transactions: vec![],
        shard_account_before: dest_account.clone(),
        shard_account,
        out_messages,
        vm_log: result.vm_log,
        logs: "".to_string(),
        actions: result.actions,
        code,
        externals: vec![],
    };

    let mut externals: Vec<Cell> = vec![];

    let mut all_results = std::iter::once(SendMessageResult::Success(send_result.clone()))
        .chain(transaction.iter_out_msgs().flat_map(|msg| {
            let Ok(msg) = msg else { return vec![] };

            if let MsgInfo::ExtOut(_) = &msg.info {
                externals.push(msg.to_cell());
                return vec![];
            };

            let mut send_results = send_message_debug(ctx, &msg.to_cell(), libs, None);
            for result in &mut send_results {
                match result {
                    SendMessageResult::Success(result) => {
                        result.parent_transaction = Some(transaction.clone());
                    }
                    SendMessageResult::Error(_) => {}
                }
            }

            send_results
        }))
        .collect::<Vec<_>>();

    let child_txs = all_results
        .iter()
        .skip(1)
        .filter_map(|result| match result {
            SendMessageResult::Success(result) => Some(result.transaction.lt),
            SendMessageResult::Error(_) => None,
        })
        .collect();

    if let Some(SendMessageResult::Success(result)) = all_results.get_mut(0) {
        result.child_transactions = child_txs;
        result.externals = externals;
    }

    all_results
}

extension!(send_single_message in (Context) with (from: ArcCell, message: ArcCell) using send_single_message_impl);
fn send_single_message_impl(ctx: &mut Context, stack: &mut Tuple, from: ArcCell, message: ArcCell) {
    let emulator = &ctx.chain.emulator;

    let msg_b64 = try_ctx!(
        ctx,
        message.to_boc(false),
        "Failed to encode message to BoC: {}"
    );
    let msg_cell = try_ctx!(
        ctx,
        Boc::decode(msg_b64),
        "Failed to decode message from BoC: {}"
    );

    let from_b64 = try_ctx!(
        ctx,
        from.to_boc(false),
        "Failed to encode from address to BoC: {}"
    );
    let from_cell = try_ctx!(
        ctx,
        Boc::decode(from_b64),
        "Failed to decode from address from BoC: {}"
    );
    let mut from_slice = try_ctx!(
        ctx,
        from_cell.as_slice(),
        "Failed to create slice `from` from address cell: {}"
    );
    let src_addr = IntAddr::load_from(&mut from_slice);
    let src_addr = match src_addr {
        Ok(src_addr) => src_addr,
        Err(err) => {
            ctx.asserts.fail(format!(
                "Failed to decode src address from x{{{}}} with length={}: {}",
                from_slice.display_data(),
                from_slice.size_bits(),
                err
            ));
            return;
        }
    };

    let libs = ctx.chain.build_libs(&src_addr);
    let blockchain = &mut ctx.chain.blockchain;

    let emulation = match emulator.send_single_message(blockchain, msg_cell, &libs, Some(src_addr))
    {
        Ok(res) => res,
        Err(err) => {
            ctx.asserts
                .fail(format!("Cannot emulate transaction: {}", err));
            return;
        }
    };

    let SendMessageResult::Success(emulation) = emulation else {
        stack.push(TupleItem::Null);
        return;
    };

    let Some(send_result) = emulation_to_send_result(&emulation) else {
        stack.push(TupleItem::Null);
        return;
    };

    ctx.chain
        .emulations
        .results
        .push(vec![SendMessageResult::Success(emulation)]);

    stack.push(send_result);
}

extension!(find_transaction_by_params in (Context) with (params: Tuple, txs: Tuple) using find_transaction_by_params_impl);
fn find_transaction_by_params_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    params: Tuple,
    txs: Tuple,
) {
    if txs.0.is_empty() {
        stack.push(TupleItem::Null);
        return;
    }

    let (params, parsed_txs) = match process_txs_and_search_params(&txs, params) {
        Some(value) => value,
        None => {
            stack.push(TupleItem::Null);
            return;
        }
    };

    let found = parsed_txs.iter().filter(|tx| {
        if let Some(expected_deploy) = params.deploy
            && expected_deploy
            && (tx.orig_status != AccountStatus::NotExists
                || tx.end_status != AccountStatus::Active)
        {
            // We expect to deploy contract but we don't
            return false;
        }

        let in_msg = tx.load_in_msg();
        if let Ok(Some(in_msg)) = &in_msg
            && let MsgInfo::Int(info) = &in_msg.info
        {
            if let Some(expected_opcode) = &params.opcode {
                let mut slice = in_msg.body;
                let Ok(opcode) = slice.load_u32() else {
                    // No opcode at all
                    return false;
                };
                if *expected_opcode != opcode {
                    if params.bounced == Some(true) {
                        // if bounced, try to match opcode after 0xFFFFFFFF
                        let Ok(bounced_opcode) = slice.load_u32() else {
                            // No bounced opcode at all
                            return false;
                        };
                        if *expected_opcode != bounced_opcode {
                            // Bounced opcode mismatch
                            return false;
                        }
                    } else {
                        // Opcode mismatch
                        return false;
                    }
                }
            }

            if let Some(expected_bounced) = &params.bounced
                && *expected_bounced != info.bounced
            {
                // Bounced value mismatch
                return false;
            }

            if let Some(expected_bounce) = &params.bounce
                && *expected_bounce != info.bounce
            {
                // Bounce value mismatch
                return false;
            }

            if let Some(expected_from_addr) = &params.from
                && (*expected_from_addr) != info.src
            {
                // Source address mismatch
                return false;
            }

            if let Some(expected_to_addr) = &params.to
                && (*expected_to_addr) != info.dst
            {
                // Destination address mismatch
                return false;
            }

            if let Some(expected_body) = &params.body {
                let expected_hash = expected_body.repr_hash();
                let body_cell = in_msg.body.to_cell();
                let actual_hash = body_cell.repr_hash();
                if expected_hash != actual_hash {
                    // Message body hash mismatch
                    return false;
                }
            }
        };

        let Ok(TxInfo::Ordinary(info)) = tx.load_info() else {
            return false;
        };

        if let Some(expected_compute_skipped) = params.compute_phase_skipped {
            let is_skipped = matches!(info.compute_phase, ComputePhase::Skipped(_));
            if expected_compute_skipped != is_skipped {
                // Compute phase skipped mismatch
                return false;
            }
        }

        if let Some(expected_action_exit_code) = params.action_exit_code {
            if let Some(action_phase) = &info.action_phase {
                if action_phase.result_code != expected_action_exit_code {
                    // Action exit code mismatch
                    return false;
                }
            } else {
                // Action phase is missing but expected
                return false;
            }
        }

        if let ComputePhase::Executed(compute) = info.compute_phase
            && let Some(expected_exit_code) = params.exit_code
            && compute.exit_code != expected_exit_code as i32
        {
            // Exit code mismatch
            return false;
        }

        true
    });

    let txs = found.collect::<Vec<_>>();
    let Some(first) = txs.first() else {
        // No transaction found
        stack.push(TupleItem::Null);
        return;
    };

    let tx_base64 = Boc::encode_base64(first.to_cell());
    let tx_cell = try_ctx!(
        ctx,
        ArcCell::from_boc_b64(&tx_base64),
        "Failed to decode transaction BoC: {}"
    );

    stack.push(TupleItem::Cell(tx_cell));
}

extension!(run_get_method in (Context) with (args: Tuple, return_type_name: String, name: String, id: BigInt, code: ArcCell, address: ArcCell) using run_get_method_impl);
#[allow(clippy::too_many_arguments)]
fn run_get_method_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    args: Tuple,
    return_type_name: String,
    name: String,
    id: BigInt,
    code: ArcCell,
    address: ArcCell,
) {
    let args = args.unwrap_empty().unwrap_tuple();
    let blockchain = &mut ctx.chain.blockchain;
    let address_boc = address.to_boc_hex(false).unwrap();

    let address_std = MsgAddrIntStd::from_boc_hex(address_boc.as_str()).unwrap();
    let address_hash = address_std.address.clone();
    let dst_addr_str = format!("{}:{}", &address_std.workchain, hex::encode(&address_hash));

    let dest_address = TonAddress::from_msg_address(address_std).unwrap();

    let shard_account = blockchain.get_account(&dst_addr_str);
    let state = shard_account.account.load().unwrap().0.map(|s| s.state);

    let data = if let Some(AccountState::Active(state)) = state {
        state.data.unwrap_or(Cell::default())
    } else {
        Cell::default()
    };

    let libs = ctx
        .chain
        .build_libs_with_hash_owner(&HashBytes::from_slice(address_hash.clone().as_slice()));
    let libs_root = libs.clone().into_root();

    let method_id = id.to_i32().unwrap_or(0);
    let params = RunGetMethodArgs {
        code: code.to_boc_b64(false).unwrap(),
        data: Boc::encode_base64(data),
        verbosity: ctx.env.default_log_level,
        libs: libs_root.map(Boc::encode_base64).unwrap_or("".to_string()),
        address: dest_address.to_string(),
        unixtime: 0,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    let result = if ctx.debug.is_enabled() {
        let step_executor = StepGetExecutor::new(args.clone(), params.clone());

        let source_map = ctx
            .build
            .build_cache
            .result_for_code(&Some(Boc::decode(code.to_boc(false).unwrap()).unwrap()))
            .map(|res| res.1.source_map);

        let dbg_ctx = ctx.debug.ctx();
        dbg_ctx
            .begin_thread(
                2,
                AnyExecutor::Get(step_executor.clone()),
                source_map,
                "Send internal message".to_string(),
                dbg_ctx.need_to_stop_child_thread_on_start(),
            )
            .expect("Cannot send response");

        step_executor.prepare(method_id, args);

        // Step to update internal state
        if dbg_ctx.need_to_stop_child_thread_on_start() {
            dbg_ctx.step(StepMode::StepIn);
        } else {
            dbg_ctx.step(StepMode::Continue);
        }

        if !dbg_ctx.stepper.is_terminated() {
            dbg_ctx
                .process_incoming_requests(false)
                .expect("Cannot send response");
        }

        dbg_ctx.finish_thread(2).expect("Cannot send response");

        if dbg_ctx.performing_step != Some(StepMode::Continue) {
            // When we step out from nested message/get method, send stop message to client to
            // stop on a line after send/call get method
            dbg_ctx.step(StepMode::StepIn);
        }

        step_executor.finish(&params.code)
    } else {
        let executor = GetExecutor::new(&params).expect("Cannot create get executor");
        let args = serialize_tuple(&args)
            .map(|t| t.to_boc_b64(false))
            .expect("Cannot serialize tuple")
            .expect("Cannot serialize tuple");
        executor
            .run_get_method(&args, &params, None)
            .expect("Cannot run get method")
    };

    match result {
        GetMethodResult::Success(result) => {
            ctx.chain.emulations.get_results.push(result.clone());

            let cell = try_ctx!(
                ctx,
                ArcCell::from_boc_b64(&result.stack),
                "Failed to decode stack BoC: {}"
            );
            let tuple = try_ctx!(
                ctx,
                Tuple::deserialize(&cell),
                "Failed to deserialize tuple: {}"
            );

            if result.vm_exit_code != 0 && result.vm_exit_code != 1 {
                let get_method = ctx.env.abi.find_get_method_by_id(&id);
                let get_method_presentation = if let Some(get_method) = get_method {
                    format!("'{}' ({id})", get_method.name)
                } else {
                    format!("'{}' ({id})", name)
                };

                if result.vm_exit_code == 11 {
                    // TODO: right now get methods can not include all get methods
                    let get_methods: Vec<&str> = ctx
                        .env
                        .abi
                        .get_methods
                        .iter()
                        .map(|m| m.name.as_str())
                        .collect();
                    let suggested_name = suggest_name(&name, &get_methods);

                    if let Some(suggested_name) = suggested_name {
                        ctx.asserts.fail(format!(
                            "Cannot execute unknown get method {get_method_presentation}, did you mean '{suggested_name}'",
                        ));
                    } else {
                        ctx.asserts.fail(format!(
                            "Cannot execute unknown get method {get_method_presentation}",
                        ));
                    }
                } else if result.vm_exit_code == 2 {
                    ctx.asserts.fail(format!(
                        "Get method {get_method_presentation} failed due to stack underflow. Make sure you passed all parameters to the get method.",
                    ));
                } else {
                    ctx.asserts.fail(format!(
                        "Cannot execute get method {get_method_presentation}: exit code {}",
                        FormatterContext::format_exit_code(result.vm_exit_code)
                    ));
                }
                stack.push(TupleItem::Null);
                return;
            }

            stack.push(TupleItem::TypedTuple {
                type_name: return_type_name,
                inner: tuple,
            })
        }
        GetMethodResult::Error(result) => {
            println!("Error: {}", result.error);
        }
    };
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

extension!(is_deployed in (Context) with (address: ArcCell) using is_deployed_impl);
fn is_deployed_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let dst_addr_str = try_ctx!(
        ctx,
        cell_address_to_raw(address),
        "Failed to decode address: {}"
    );

    let is_deployed = ctx.chain.blockchain.check_deployed(&dst_addr_str);
    stack.push_bool(is_deployed);
}

extension!(get_deployed_code in (Context) with (address: ArcCell) using get_deployed_code_impl);
fn get_deployed_code_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let dst_addr_str = try_ctx!(
        ctx,
        cell_address_to_raw(address),
        "Failed to decode address: {}"
    );

    let is_deployed = ctx.chain.blockchain.check_deployed(&dst_addr_str);
    if !is_deployed {
        stack.push(TupleItem::Null);
        return;
    }

    let account = ctx.chain.blockchain.get_account(&dst_addr_str);
    let cell = match get_address_code(&account) {
        Some(value) => value,
        None => {
            stack.push(TupleItem::Null);
            return;
        }
    };

    stack.push(TupleItem::Cell(cell));
}

fn cell_address_to_raw(address: ArcCell) -> anyhow::Result<String> {
    let address_std = TonAddress::from_msg_address(MsgAddress::from_cell(&address)?)?;
    Ok(address_std.to_hex())
}

fn get_address_code(account: &ShardAccount) -> Option<ArcCell> {
    let state = account.account.load().ok()?.0.map(|s| s.state);

    let Some(AccountState::Active(state)) = state else {
        return None;
    };

    let code = state.code?;
    let cell = ArcCell::from_boc_b64(&Boc::encode_base64(code)).ok()?;

    Some(cell)
}

extension!(crc16 in (Context) with (data: String) using crc16_impl);
fn crc16_impl(_ctx: &mut Context, stack: &mut Tuple, data: String) {
    let crc = Crc::<u16>::new(&CRC_16_XMODEM);
    let result = crc.checksum(data.as_bytes());
    stack.push(TupleItem::Int(BigInt::from(result)));
}

extension!(type_name_by_opcode in (Context) with (id: BigInt) using type_name_by_opcode_impl);
fn type_name_by_opcode_impl(ctx: &mut Context, stack: &mut Tuple, id: BigInt) {
    let type_abi = ctx.env.abi.find_type_by_opcode(&id);
    match type_abi {
        None => {
            stack.push(TupleItem::Null);
        }
        Some(type_abi) => {
            stack.push_string(&type_abi.name);
        }
    }
}

extension!(register_address in (Context) with (name: String, address: ArcCell) using register_address_impl);
fn register_address_impl(ctx: &mut Context, _stack: &mut Tuple, name: String, address: ArcCell) {
    let address_boc = try_ctx!(
        ctx,
        address.to_boc(false),
        "Failed to encode address to BoC: {}"
    );
    let address_cell = try_ctx!(
        ctx,
        Boc::decode(address_boc),
        "Failed to decode address from BoC: {}"
    );

    let addr = try_ctx!(
        ctx,
        address_cell.parse::<IntAddr>(),
        "Failed to load address from slice: {}"
    );

    ctx.build
        .known_addresses
        .addresses
        .insert(addr, KnownAddress { name });
}

extension!(register_code in (Context) with (name: String, address: ArcCell) using register_code_impl);
fn register_code_impl(ctx: &mut Context, _stack: &mut Tuple, name: String, code: ArcCell) {
    let hash = try_ctx!(ctx, code.cell_hash(), "Failed to get cell hash: {}");
    ctx.build.known_code_cells.insert(hash.to_hex(), name);
}

extension!(account_state in (Context) with (address: ArcCell) using account_state_impl);
fn account_state_impl(ctx: &mut Context, stack: &mut Tuple, address: ArcCell) {
    let address_boc = try_ctx!(
        ctx,
        address.to_boc(false),
        "Failed to encode address to BoC: {}"
    );
    let address_cell = try_ctx!(
        ctx,
        Boc::decode(address_boc),
        "Failed to decode address from BoC: {}"
    );
    let addr = try_ctx!(
        ctx,
        address_cell.parse::<IntAddr>(),
        "Failed to load address from slice: {}"
    );

    let Ok(account) = ctx
        .chain
        .blockchain
        .get_account(&addr.to_string())
        .account
        .load()
    else {
        stack.push(TupleItem::Null);
        return;
    };

    let Some(account) = account.0 else {
        stack.push(TupleItem::Null);
        return;
    };

    let mut builder = CellBuilder::new();
    try_ctx!(
        ctx,
        builder.store_bit(true),
        "Failed to store bit in cell builder: {}"
    );
    try_ctx!(
        ctx,
        account.store_into(&mut builder, Cell::empty_context()),
        "Failed to store account into cell builder: {}"
    );
    let cell = try_ctx!(
        ctx,
        builder.build(),
        "Failed to build cell from builder: {}"
    );

    let Ok(cell) = ArcCell::from_boc_b64(&Boc::encode_base64(cell)) else {
        stack.push(TupleItem::Null);
        return;
    };

    stack.push(TupleItem::Cell(cell))
}

extension!(register_lib in (Context) with (lib: ArcCell) using register_lib_impl);
fn register_lib_impl(ctx: &mut Context, _stack: &mut Tuple, lib: ArcCell) {
    let lib_boc = try_ctx!(ctx, lib.to_boc(false), "Failed to encode lib to BoC: {}");
    let cell = try_ctx!(
        ctx,
        Boc::decode(lib_boc),
        "Failed to decode lib from BoC: {}"
    );
    ctx.chain.blockchain.register_lib(cell)
}

extension!(convert_address in (Context) with (address: String) using convert_address_impl);
fn convert_address_impl(ctx: &mut Context, stack: &mut Tuple, address: String) {
    let addr = try_ctx!(
        ctx,
        tonlib_core::TonAddress::from_str(address.as_str()),
        "Failed to convert address from {address}: {}"
    );

    let cell = try_ctx!(
        ctx,
        addr.to_msg_address().to_cell(),
        "Failed to convert address to cell: {}"
    );

    stack.push(TupleItem::Cell(cell.to_arc()))
}

extension!(cell_from_hex in (Context) with (cell_hex: String) using cell_from_hex_impl);
fn cell_from_hex_impl(ctx: &mut Context, stack: &mut Tuple, cell_hex: String) {
    let cell = try_ctx!(
        ctx,
        ArcCell::from_boc_hex(cell_hex.as_str()),
        "Failed to decode cell hex {cell_hex}: {}"
    );

    stack.push(TupleItem::Cell(cell))
}

extension!(load_library_by_hash in (Context) with (hash: String) using load_library_by_hash_impl);
fn load_library_by_hash_impl(ctx: &mut Context, stack: &mut Tuple, hash: String) {
    let lib = remote::get_library_by_hash(&ctx.network(), hash.as_str(), None);
    match lib {
        Ok(lib) => {
            let lib_b64 = Boc::encode_base64(lib);
            let lib_cell = try_ctx!(
                ctx,
                ArcCell::from_boc_b64(&lib_b64),
                "Failed to decode lib from BoC: {}"
            );

            stack.push(TupleItem::Cell(lib_cell))
        }
        Err(_) => stack.push(TupleItem::Null),
    }
}

extension!(is_broadcasting in (Context) using is_broadcasting_impl);
fn is_broadcasting_impl(ctx: &mut Context, stack: &mut Tuple) {
    stack.push_bool(ctx.is_broadcasting)
}

extension!(get_wallet_by_name in (Context) with (name: String) using get_wallet_by_name_impl);
fn get_wallet_by_name_impl(ctx: &mut Context, stack: &mut Tuple, name: String) {
    if let Some(wallet) = ctx.env.open_wallets.get(&name) {
        let address_cell = try_ctx!(
            ctx,
            wallet.wallet.address.to_msg_address().to_cell(),
            "Cannot build cell from wallet address: {}"
        );
        stack.push(TupleItem::Cell(address_cell.to_arc()));
        return;
    }

    stack.push(TupleItem::Null);
}

extension!(wait_for_transaction in (Context) with (sleep_duration: BigInt, attempts: BigInt, quiet: BigInt, ext_message_hash: ArcCell, address: ArcCell) using wait_for_transaction_impl);
#[allow(clippy::too_many_arguments)]
fn wait_for_transaction_impl(
    ctx: &mut Context,
    stack: &mut Tuple,
    sleep_duration: BigInt,
    attempts: BigInt,
    quiet: BigInt,
    ext_message_hash: ArcCell,
    address: ArcCell,
) {
    if !ctx.is_broadcasting {
        return;
    }

    let quiet = !quiet.is_zero();
    let attempts = attempts.to_u32().unwrap_or(20);
    let sleep_duration_ms = sleep_duration.to_u64().unwrap_or(2000);

    if attempts == 0 {
        ctx.asserts
            .fail("Attempt number must be positive".to_owned());
        return;
    }

    let address_str = try_ctx!(
        ctx,
        cell_address_to_raw(address.clone()),
        "Failed to decode address: {}"
    );

    let network = try_ctx!(
        ctx,
        Network::from_str(&ctx.network()),
        "Failed to parse network: {}"
    );

    let api_key = ctx.chain.blockchain.get_api_key();
    let api_client = TonApiClient::new(network, api_key.clone());

    let ext_message_hash_bytes = ext_message_hash.data();

    if api_key.is_none() {
        std::thread::sleep(Duration::from_millis(1000)); // rate limit
    }

    for attempt in 1..=attempts {
        if !quiet {
            println!("Awaiting transaction... [Attempt {}/{}]", attempt, attempts);
        }

        let txs = match api_client.get_transactions(&address_str, Some(100), None, None) {
            Ok(txs) => txs,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(sleep_duration_ms));
                continue;
            }
        };

        for tx in txs {
            if let Some(in_msg) = &tx.in_msg
                && let Some(body_hash) = &in_msg.body_hash
            {
                let msg_hash_bytes = try_ctx!(
                    ctx,
                    base64::engine::general_purpose::STANDARD.decode(body_hash),
                    "Failed to decode message body hash: {}"
                );

                if msg_hash_bytes == ext_message_hash_bytes {
                    std::thread::sleep(Duration::from_millis(1000)); // wait a bit more for txs in row

                    if !quiet {
                        let hex = base64::engine::general_purpose::STANDARD
                            .decode(tx.transaction_id.hash.clone())
                            .map(hex::encode)
                            .unwrap_or(tx.transaction_id.hash.clone());
                        println!("Transaction successfully applied!");

                        let url = get_transaction_link(ctx, address_str, tx, hex);
                        println!("You can view it at {}", url.underline());
                    }
                    stack.push_bool(true);
                    return;
                }
            }
        }

        if attempt < attempts {
            std::thread::sleep(Duration::from_millis(sleep_duration_ms));
        }
    }

    ctx.asserts.fail(
        "Transaction was not applied after {} attempts. Check your wallet's transactions"
            .to_owned(),
    );
    stack.push_bool(false);
}

fn get_transaction_link(
    ctx: &mut Context,
    address_str: String,
    tx: TonCenterTransaction,
    hex: String,
) -> String {
    let network_prefix = if ctx.network() == "testnet" {
        "testnet."
    } else {
        ""
    };
    let explorer = ctx.env.explorer.unwrap_or(Explorer::Tonviewer);
    match explorer {
        Explorer::Tonscan => {
            format!("https://{}tonscan.org/tx/{}", network_prefix, hex)
        }
        Explorer::Toncx => format!(
            "https://{}ton.cx/tx/{}:{}:{}",
            network_prefix, tx.transaction_id.lt, hex, address_str
        ),
        Explorer::Dton => format!(
            "https://{}dton.io/tx/{}?time={}",
            network_prefix, hex, tx.utime
        ),
        Explorer::Tonviewer => format!(
            "https://{}tonviewer.com/transaction/{}",
            network_prefix, hex
        ),
    }
}

extension!(enable_broadcast in (Context) using enable_broadcast_impl);
fn enable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) {
    ctx.is_broadcasting = true;
}

extension!(disable_broadcast in (Context) using disable_broadcast_impl);
fn disable_broadcast_impl(ctx: &mut Context, _stack: &mut Tuple) {
    ctx.is_broadcasting = false;
}

extension!(set_now in (Context) with (now: BigInt) using set_now_impl);
fn set_now_impl(ctx: &mut Context, _stack: &mut Tuple, now: BigInt) {
    let now_u32 = now.to_u32().unwrap_or(0);
    ctx.chain.blockchain.set_now(now_u32);
}

extension!(get_now in (Context) using get_now_impl);
fn get_now_impl(ctx: &mut Context, stack: &mut Tuple) {
    let now = ctx.chain.blockchain.get_now();
    stack.push(TupleItem::Int(BigInt::from(now)));
}

pub fn register_extensions<T: BaseExecutor>(executor: &mut T, ctx: &mut Context) {
    register_ext_methods!(executor, ctx, {
        6 => build,
        8 => run_get_method,
        9 => send_message,
        10 => find_transaction_by_params,
        11 => is_deployed,
        12 => get_deployed_code,
        13 => crc16,
        14 => type_name_by_opcode,
        15 => register_address,
        16 => register_code,
        17 => account_state,
        18 => register_lib,
        19 => convert_address,
        20 => cell_from_hex,
        21 => load_library_by_hash,
        23 => is_broadcasting,
        24 => get_wallet_by_name,
        25 => wait_for_transaction,
        26 => enable_broadcast,
        27 => disable_broadcast,
        28 => set_now,
        29 => get_now,
        30 => send_single_message,
    });
}
