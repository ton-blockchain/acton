use crate::methods::{collect_used_libraries, find_final_actions};
use crate::methods::{
    compute_final_data, find_all_transactions_between, find_shard_block_for_tx, get_block_account,
    get_block_config, tx_opcode,
};
use crate::remote::TonCenterClient;
use crate::types::{BaseTxInfo, TraceEmulatedTx, TraceInMessage, TraceResult};
use crate::{ComputeInfo, find_base_tx_by_hash, methods};
use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, LazyLock};
use tasm_core::decompile::Disassembler;
use tasm_core::types::{ArgValue, Instruction};
use ton_api::toncenter::v3;
use ton_executor::message::{EmulationResult, Executor, PrevBlocksInfo, RunTransactionArgs};
use ton_executor::{ExecutorVerbosity, MissingLibrariesContext, missing_library_callback};
use ton_networks::CustomNetworkUrls;
pub use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Store};
use tycho_types::models::{AccountState, ShardAccount, TickTock, Transaction, TxInfo};
use tycho_types::models::{BlockchainConfigParams, ConfigParam8};
use tycho_types::num::Tokens;

/// Fully reproduce (re‑trace) a TON transaction inside a local TON Sandbox
/// and return a structured report with VM logs, money flow, generated
/// actions and other data.
///
/// # Workflow (high level)
///
/// 1.  Locate the base transaction on the selected network.
/// 2.  Load its shard‑block and the enclosing master‑block; extract
///     `rand_seed`, config‑cell and the account snapshot *prior* to the block.
/// 3.  Re‑create the exact pre‑tx state by sequentially emulating all earlier
///     account transactions that happened inside the same master‑block.
/// 4.  Emulate the target transaction itself with full VM verbosity.
///     Load c7 block history when code reads it, and retry with the full history
///     if dynamically constructed code causes the first replay to diverge.
/// 5.  Parse the resulting VM log (`c5`, action list, stack trace), compare the
///     calculated state‑hash with the on‑chain one and assemble a
///     [`TraceResult`] object for the caller.
///
/// # Arguments
///
/// * `net`             — Network to use.
/// * `link`            — Hex hash that uniquely identifies the transaction to retrace.
/// * `additional_libs` — Additional libraries to use.
/// * `custom_networks` — V2/V3 endpoints for localnet and custom networks.
///   Mainnet and testnet use the shared public endpoint configuration.
///
/// # Returns
///
/// Returns a [`TraceResult`] containing:
/// 1. an integrity flag `state_update_hash_ok`
/// 2. contract address and incoming message details (absent for tick-tock)
/// 3. balance delta, gas and fees
/// 4. full emulated transaction (`emulated_tx`) with
///    compute‑phase info, `c5`, action list and raw VM log
/// 5. version of the sandbox executor used for emulation
///
/// # Errors
///
/// Returns an error if any network lookup fails; if the corresponding shard‑ /
/// master‑block cannot be found; if deterministic replay
/// fails (TVM returns non‑success). A state-hash mismatch after replay is
/// reported through [`TraceResult::state_update_hash_ok`].
///
/// # Examples
///
/// ```ignore
/// let result = retrace(Network::Mainnet, "hash", Default::default(), &Default::default()).await?;
/// if result.state_update_hash_ok {
///     println!("Retrace successful!");
/// }
/// ```
pub async fn retrace(
    net: Network,
    link: &str,
    mut additional_libs: HashMap<HashBytes, Cell>,
    custom_networks: &HashMap<String, CustomNetworkUrls>,
) -> anyhow::Result<TraceResult> {
    let base_tx = find_base_tx_by_hash(net.clone(), link, custom_networks).await?;
    let client = TonCenterClient::new(net.clone(), custom_networks)?;

    for _ in 0..5 {
        let result = retrace_base_tx(
            net.clone(),
            base_tx.clone(),
            additional_libs.clone(),
            custom_networks,
        )
        .await?;

        if matches!(
            &result.emulated_tx.compute_info,
            ComputeInfo::Success { exit_code: 9, .. }
        ) && load_missing_libraries(
            &client,
            &result.emulated_tx.missing_libraries,
            &mut additional_libs,
        )
        .await?
        {
            continue;
        }

        return Ok(result);
    }

    anyhow::bail!("retrace failed to recover exit code 9");
}

async fn load_missing_libraries(
    client: &TonCenterClient,
    missing_libraries: &[String],
    additional_libs: &mut HashMap<HashBytes, Cell>,
) -> anyhow::Result<bool> {
    let mut loaded_any = false;

    for hash_hex in missing_libraries {
        let hash = HashBytes::from_str(hash_hex)
            .with_context(|| format!("Invalid missing library hash: {hash_hex}"))?;
        if additional_libs.contains_key(&hash) {
            continue;
        }

        let hash_for_api = format!("{hash:X}");
        let code = methods::get_library_by_hash(client, &hash_for_api)
            .await
            .with_context(|| format!("Failed to load missing library {hash_hex}"))?;
        additional_libs.insert(hash, code);
        loaded_any = true;
    }

    Ok(loaded_any)
}

/// Fully reproduce (re‑trace) a TON transaction by transaction triple
/// inside a local TON Sandbox and return a structured report with VM logs,
/// money flow, generated actions and other data.
///
/// See [`crate::retrace`] for the full description of the workflow.
///
/// # Arguments
///
/// * `net`             — Network to use.
/// * `base_tx`         — Handle for locating the transaction.
/// * `additional_libs` — Additional libraries to use.
/// * `custom_networks` — V2/V3 endpoints for localnet and custom networks.
///   Mainnet and testnet use the shared public endpoint configuration.
///
/// # Examples
///
/// ```ignore
/// let base_tx = find_base_tx_by_hash(Network::Mainnet, "hash", &Default::default()).await?;
/// let result = retrace_base_tx(Network::Mainnet, base_tx, Default::default(), &Default::default()).await?;
/// ```
///
/// # Errors
///
/// Returns an error if any stage of fetching or emulation fails.
pub async fn retrace_base_tx(
    net: Network,
    base_tx: BaseTxInfo,
    additional_libs: HashMap<HashBytes, Cell>,
    custom_networks: &HashMap<String, CustomNetworkUrls>,
) -> anyhow::Result<TraceResult> {
    let client = TonCenterClient::new(net, custom_networks)?;
    let block = find_shard_block_for_tx(&client, &base_tx).await?;

    // master‑block sequence number that references our shard‑block
    let mc_seqno = block.masterchain_block_ref.seqno;
    let block_config = get_block_config(&client, mc_seqno).await?;
    let mut contexts = vec![ReplayBlockContext::new(block, block_config)?];
    let mut shard_account = get_block_account(&client, &base_tx.address, mc_seqno).await?;

    // The snapshot records the exact transaction boundary. This also covers
    // predecessors in earlier shard blocks referenced by the same master block.
    // Block 1 uses an after-block approximation because zerostate is unavailable;
    // in that case replay only the target, retaining the state-hash mismatch.
    let after_lt = if mc_seqno == 1 {
        base_tx.lt.saturating_sub(1)
    } else {
        shard_account.last_trans_lt
    };
    if after_lt >= base_tx.lt {
        anyhow::bail!(
            "Account snapshot at LT {after_lt} is not before target LT {}",
            base_tx.lt
        );
    }
    let mut prev_txs_in_block = find_all_transactions_between(&client, &base_tx, after_lt).await?;
    // order oldest → newest, and remove the base_tx itself (the one we want to retrace)
    prev_txs_in_block.reverse();
    let Some(our_tx) = prev_txs_in_block.pop() else {
        anyhow::bail!("Cannot find transaction to retrace")
    };

    let (libs, loaded_code) =
        collect_used_libraries(&client, &shard_account, &our_tx, &additional_libs).await?;

    // retrieve code cell if an account in active mode
    let Some(account_before_tx) = shard_account.load_account()? else {
        anyhow::bail!("Cannot load account")
    };
    let state = account_before_tx.state;
    let code_cell = match state {
        AccountState::Active(state) => state.code,
        _ => our_tx
            .load_in_msg()?
            .and_then(|msg| msg.init)
            .and_then(|init| init.code),
    };

    // for the first transaction (executor doesn't know about last tx)
    shard_account.last_trans_lt = 0;
    shard_account.last_trans_hash = HashBytes::ZERO;

    // first we emulate all transactions before to get a state that is equal to actual
    // state in blockchain before transaction to emulate
    let balance = account_before_tx.balance.tokens;
    let mut usage = prev_blocks_usage(
        code_cell
            .iter()
            .chain(loaded_code.iter())
            .chain(additional_libs.values()),
    );
    let mut previous_contexts = None;
    let (balance, res, executor_logs, state_update_hash_ok) = loop {
        if usage != PrevBlocksUsage::None && contexts[0].global_version >= 4 {
            if previous_contexts.is_none() {
                previous_contexts = Some(
                    load_previous_block_contexts(
                        &client,
                        &prev_txs_in_block,
                        &our_tx,
                        &base_tx,
                        &mut contexts,
                    )
                    .await?,
                );
            }
            for context in &mut contexts {
                // TVM 4–8 has only the recent blocks and key block in this tuple.
                let with_100 = usage == PrevBlocksUsage::Full && context.global_version >= 9;
                if context.global_version >= 4
                    && (context.prev_blocks_info.is_none()
                        || (with_100
                            && context
                                .prev_blocks_info
                                .as_ref()
                                .is_some_and(|info| info.last_mc_blocks_100.is_none())))
                {
                    context.prev_blocks_info = Some(
                        client
                            .get_prev_blocks_info(context.masterchain_ref_seqno()?, with_100)
                            .await?,
                    );
                }
            }
        }
        let target_context = &contexts[0];

        // Always restart from the original snapshot: a previous transaction may
        // have committed a different state without its c7 block history.
        let attempt = (|| -> anyhow::Result<_> {
            let (balance, before_target) = emulate_previous_transactions(
                &prev_txs_in_block,
                &shard_account,
                &balance,
                libs.as_ref(),
                (0..prev_txs_in_block.len()).map(|index| {
                    let context = &contexts[previous_contexts
                        .as_ref()
                        .map_or(0, |indices| indices[index])];
                    (
                        context.config.as_str(),
                        context.rand_seed,
                        context.prev_blocks_info.as_ref(),
                    )
                }),
            )?;
            let (tx_res, executor_logs) = emulate(
                &our_tx,
                &target_context.config,
                &before_target,
                libs.as_ref(),
                target_context.rand_seed,
                target_context.prev_blocks_info.as_ref(),
            )?;
            let res = match tx_res {
                EmulationResult::Success(res) => res,
                EmulationResult::Error(err) => {
                    anyhow::bail!("Emulated transaction failed: {}", err.error);
                }
            };
            let emulated_tx: Transaction = Boc::decode_base64(res.transaction.as_ref())?.parse()?;
            let matches = emulated_tx.state_update.load()?.new == our_tx.state_update.load()?.new;
            Ok((balance, res, executor_logs, matches))
        })();

        // Static inspection cannot see continuations built from data/messages.
        // Retry once with full history, including rejected external messages.
        // Missing libraries are recovered by the caller before fetching history.
        if usage == PrevBlocksUsage::Full
            || contexts.iter().all(|context| context.global_version < 4)
            || attempt
                .as_ref()
                .is_ok_and(|(_, res, _, matches)| *matches || !res.missing_libraries.is_empty())
        {
            break attempt?;
        }
        usage = PrevBlocksUsage::Full;
    };
    let mut missing_libraries = res.missing_libraries.iter().cloned().collect::<Vec<_>>();
    missing_libraries.sort_unstable();

    // extract out actions from the c5 control register
    let (final_actions, c5) = find_final_actions(&res);

    let (sender, contract, amount, money, emulated_tx, compute_info) =
        compute_final_data(&res, balance, &base_tx.address)?;

    let opcode = tx_opcode(&our_tx);

    Ok(TraceResult {
        state_update_hash_ok,
        code_cell: loaded_code.or_else(|| code_cell.clone()),
        original_code_cell: code_cell,
        in_msg: TraceInMessage {
            sender,
            contract,
            amount: amount.map(|a| u128::from(a) as u64),
            opcode,
        },
        money,
        emulated_tx: TraceEmulatedTx {
            raw: our_tx,
            utime: u64::from(emulated_tx.now),
            lt: emulated_tx.lt,
            compute_info,
            executor_logs,
            actions: final_actions,
            c5,
            vm_logs: res.vm_log,
            missing_libraries,
        },
    })
}

/// Block-scoped executor inputs reused across attempts and transactions.
/// Predecessors can belong to different shard blocks under the same MC block;
/// their random seed and referenced MC state must not come from the target.
struct ReplayBlockContext {
    block: v3::Block,
    config: String,
    rand_seed: [u8; 32],
    global_version: u32,
    prev_blocks_info: Option<PrevBlocksInfo>,
}

impl ReplayBlockContext {
    fn new(block: v3::Block, config: String) -> anyhow::Result<Self> {
        let rand_seed = general_purpose::STANDARD
            .decode(&block.rand_seed)?
            .try_into()
            .map_err(|bytes: Vec<u8>| {
                anyhow::anyhow!(
                    "Invalid block random seed length: expected 32 bytes, got {}",
                    bytes.len()
                )
            })?;
        let params = BlockchainConfigParams::from_raw(Boc::decode_base64(&config)?);
        let global_version = params
            .get::<ConfigParam8>()?
            .map_or(0, |version| version.version);
        Ok(Self {
            block,
            config,
            rand_seed,
            global_version,
            prev_blocks_info: None,
        })
    }

    fn masterchain_ref_seqno(&self) -> anyhow::Result<u32> {
        // The MC block including a shard block is later than the MC state used
        // for its execution. MC transactions use the preceding MC block.
        if self.block.workchain == -1 {
            self.block
                .seqno
                .checked_sub(1)
                .context("Masterchain transaction has no preceding block")
        } else {
            u32::try_from(self.block.master_ref_seqno)
                .context("Invalid masterchain reference seqno")
        }
    }
}

/// Resolves block ownership once, using hashes from the already validated history.
/// Contexts are shared for transactions in the same block and retained for a retry.
async fn load_previous_block_contexts(
    client: &TonCenterClient,
    previous: &[Transaction],
    target: &Transaction,
    base_tx: &BaseTxInfo,
    contexts: &mut Vec<ReplayBlockContext>,
) -> anyhow::Result<Vec<usize>> {
    let mut indices = Vec::with_capacity(previous.len());
    for (tx, next) in previous
        .iter()
        .zip(previous.iter().skip(1).chain(std::iter::once(target)))
    {
        let hash = general_purpose::STANDARD.encode(next.prev_trans_hash.as_slice());
        let response = client
            .get_transactions(&[("hash", hash.clone()), ("limit", "1".to_owned())])
            .await
            .with_context(|| {
                format!(
                    "Cannot resolve block for previous transaction at LT {}",
                    tx.lt
                )
            })?;
        let metadata = response.transactions.first().with_context(|| {
            format!("Cannot find block for previous transaction at LT {}", tx.lt)
        })?;
        anyhow::ensure!(
            metadata.lt.parse::<u64>()? == tx.lt
                && metadata.hash == hash
                && tycho_types::models::StdAddr::from_str(&metadata.account)? == base_tx.address,
            "TON Center returned unexpected previous transaction metadata at LT {}",
            tx.lt
        );
        let block_id = &metadata.block_ref;
        let existing = contexts.iter().position(|context| {
            context.block.workchain == block_id.workchain
                && context.block.shard == block_id.shard
                && context.block.seqno == block_id.seqno
        });
        let index = if let Some(index) = existing {
            index
        } else {
            let info = BaseTxInfo {
                address: base_tx.address.clone(),
                hash: next.prev_trans_hash.0,
                lt: tx.lt,
                block: block_id.clone(),
            };
            let block = find_shard_block_for_tx(client, &info).await?;
            let config = get_block_config(client, block.masterchain_block_ref.seqno).await?;
            contexts.push(ReplayBlockContext::new(block, config)?);
            contexts.len() - 1
        };
        indices.push(index);
    }
    Ok(indices)
}

/// How much c7 history can be established from statically available code.
/// Unknown code needs the full tuple; runtime-generated code is handled by replay.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PrevBlocksUsage {
    None,
    Recent,
    Full,
}

/// Inspects decoded instructions, including continuations and method dictionaries.
/// Literal data cells are not code; a mismatch after replay covers dynamically
/// constructed continuations without downloading block history for every contract.
fn prev_blocks_usage<'a>(roots: impl Iterator<Item = &'a Cell>) -> PrevBlocksUsage {
    static DISASSEMBLER: LazyLock<Disassembler> = LazyLock::new(Disassembler::new);

    fn inspect_arg(arg: &ArgValue) -> PrevBlocksUsage {
        match arg {
            ArgValue::Code { code, .. } => inspect(&code.instructions),
            ArgValue::CodeDictionary(dict) => dict
                .methods
                .iter()
                .map(|method| inspect(&method.instructions))
                .max()
                .unwrap_or(PrevBlocksUsage::None),
            _ => PrevBlocksUsage::None,
        }
    }

    fn inspect(instructions: &[Instruction]) -> PrevBlocksUsage {
        instructions
            .iter()
            .map(|instruction| match instruction {
                Instruction::Plain(plain) => {
                    let usage = match plain.name.as_str() {
                        "PREVMCBLOCKS" | "PREVKEYBLOCK" => PrevBlocksUsage::Recent,
                        "PREVMCBLOCKS_100" | "PREVBLOCKSINFOTUPLE" => PrevBlocksUsage::Full,
                        _ => PrevBlocksUsage::None,
                    };
                    usage.max(
                        plain
                            .args
                            .iter()
                            .map(inspect_arg)
                            .max()
                            .unwrap_or(PrevBlocksUsage::None),
                    )
                }
                Instruction::Ref(reference) => inspect_arg(&reference.code),
                // The disassembler preserves undecodable code as a slice. Treat it
                // conservatively rather than assuming it never accesses c7.
                Instruction::Slice(_) => PrevBlocksUsage::Full,
                // Library references are inspected through the resolved code roots.
                Instruction::ExoticCell(_) => PrevBlocksUsage::None,
            })
            .max()
            .unwrap_or(PrevBlocksUsage::None)
    }

    roots
        .map(|root| {
            DISASSEMBLER
                .decompile_cell(root)
                .map_or(PrevBlocksUsage::Full, |code| inspect(&code.instructions))
        })
        .max()
        .unwrap_or(PrevBlocksUsage::None)
}

/// Re-emulates all transactions that occurred in the same account within
/// the same master-block *before* the target transaction.
///
/// This is necessary because the sandbox starts with an account state from
/// the *previous* master-block. To get the exact state before our target tx,
/// we must apply all intermediate transactions in order.
fn emulate_previous_transactions<'a>(
    prev_txs_in_block: &[Transaction],
    shard_account: &ShardAccount,
    balance: &Tokens,
    libs: Option<&Cell>,
    contexts: impl Iterator<Item = (&'a str, [u8; 32], Option<&'a PrevBlocksInfo>)>,
) -> anyhow::Result<(Tokens, ShardAccount)> {
    let mut balance = *balance;
    let mut shard_account = shard_account.clone();

    for (prev_tx, (block_config, rand_seed, prev_blocks_info)) in
        prev_txs_in_block.iter().zip(contexts)
    {
        let (tx_res, _) = emulate(
            prev_tx,
            block_config,
            &shard_account,
            libs,
            rand_seed,
            prev_blocks_info,
        )?;
        let res = match tx_res {
            EmulationResult::Success(res) => res,
            EmulationResult::Error(err) => {
                anyhow::bail!("Previous transaction failed: {:?}", err.error);
            }
        };
        // since we change state at each transaction we need to save new state as current one
        shard_account = Boc::decode_base64(res.shard_account.as_ref())?.parse()?;
        balance = shard_account
            .load_account()?
            .map_or(Tokens::ZERO, |a| a.balance.tokens);
    }
    Ok((balance, shard_account))
}

/// Replays either a message-driven or tick-tock transaction using its original
/// execution time and kind. The same dispatch is used for preceding transactions
/// so state reconstruction can include tick-tock calls without inventing messages.
fn emulate(
    tx: &Transaction,
    block_config: &str,
    shard_account: &ShardAccount,
    libs: Option<&Cell>,
    rand_seed: [u8; 32],
    prev_blocks_info: Option<&PrevBlocksInfo>,
) -> anyhow::Result<(EmulationResult, Arc<str>)> {
    let (message, is_tock) = match tx.load_info()? {
        TxInfo::Ordinary(_) => {
            let in_msg = tx
                .in_msg
                .as_ref()
                .context("No in_message was found in transaction")?;
            (Boc::encode_base64(in_msg), None)
        }
        // The native tick-tock entrypoint ignores the message parameter.
        TxInfo::TickTock(info) => (String::new(), Some(info.kind == TickTock::Tock)),
    };

    let emulator = Executor::new(
        ExecutorVerbosity::FullLocationStackVerbose,
        Some(block_config),
    )?;
    let mut missing_libraries_ctx = MissingLibrariesContext::default();
    emulator
        .register_missing_library_callback(&mut missing_libraries_ctx, missing_library_callback)
        .context("Cannot register missing library callback")?;

    let (mut tx_res, executor_logs) = emulator.run_transaction(
        &message,
        &RunTransactionArgs {
            libs: libs.map(Boc::encode_base64),
            shard_account: Boc::encode_base64(to_cell(shard_account)),
            now: tx.now,
            lt: tx.lt,
            random_seed: Some(rand_seed),
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: prev_blocks_info.cloned(),
            is_tick_tock: is_tock.map(|_| true),
            is_tock,
        },
    )?;
    let mut missing_libraries = Some(missing_libraries_ctx.into_set());
    match &mut tx_res {
        EmulationResult::Success(result) => {
            result.missing_libraries = missing_libraries.take().unwrap_or_default();
        }
        EmulationResult::Error(error) => {
            error.missing_libraries = missing_libraries.take().unwrap_or_default();
        }
    }

    Ok((tx_res, executor_logs))
}

fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("Failed to store data into cell builder");
    builder.build().expect("Failed to build cell from builder")
}

#[cfg(test)]
mod tests {
    use super::*;
    use expect_test::expect;
    use serde_json::json;
    use std::path::Path;
    use ton_executor::DEFAULT_CONFIG;
    use ton_executor::message::RunTransactionResultSuccess;
    use tycho_types::cell::Lazy;
    use tycho_types::models::{
        Account, CurrencyCollection, IntMsgInfo, MsgInfo, OptionalAccount, OwnedMessage,
        SpecialFlags, StateInit, StdAddr,
    };

    const NOW: u32 = 1_780_000_000;
    const SEED: [u8; 32] = [0x42; 32];

    // Distinct counters make incorrect tick/tock dispatch and omitted predecessors
    // observable in storage, independently of the retracer's reported status.
    const CONTRACT: &str = r"
    struct Storage {
        ticks: uint32
        tocks: uint32
        messages: uint32
    }

    fun onRunTickTock(isTock: bool) {
        var storage = Storage.fromCell(contract.getData());
        if (isTock) {
            storage.tocks += 1;
        } else {
            storage.ticks += 1;
        }
        contract.setData(storage.toCell());
    }

    fun onInternalMessage(_: InMessage) {
        var storage = Storage.fromCell(contract.getData());
        storage.messages += 1;
        contract.setData(storage.toCell());
    }
    ";

    fn fixture() -> anyhow::Result<(StdAddr, ShardAccount)> {
        let path = Path::new("/retrace-tests/tick-tock.tolk");
        let compiler = tolk_compiler::Compiler::new(2).with_source_overrides([(path, CONTRACT)]);
        let tolk_compiler::CompilerResult::Success(compiled) = compiler.compile(path, false) else {
            anyhow::bail!("Cannot compile tick-tock test contract");
        };
        let address = StdAddr::new(-1, HashBytes([0x11; 32]));
        let account = Account {
            address: address.clone().into(),
            storage_stat: Default::default(),
            last_trans_lt: 0,
            balance: CurrencyCollection::new(10_000_000_000),
            state: AccountState::Active(StateInit {
                // Preserve both flags during replay, even when running just tick or tock.
                special: Some(SpecialFlags {
                    tick: true,
                    tock: true,
                }),
                code: Some(Boc::decode_base64(compiled.code_boc64)?),
                data: Some(to_cell(&(0u32, 0u32, 0u32))),
                ..Default::default()
            }),
        };

        Ok((
            address,
            ShardAccount {
                account: Lazy::new(&OptionalAccount(Some(account)))?,
                last_trans_hash: HashBytes::ZERO,
                last_trans_lt: 0,
            },
        ))
    }

    // Produce reference transactions directly with the executor, without using
    // retrace's transaction-kind dispatch or state reconstruction.
    fn reference_transaction(
        address: &StdAddr,
        account: &ShardAccount,
        kind: Option<TickTock>,
        lt: u64,
    ) -> anyhow::Result<(Transaction, ShardAccount)> {
        let message = if kind.is_some() {
            String::new()
        } else {
            Boc::encode_base64(to_cell(&OwnedMessage {
                info: MsgInfo::Int(IntMsgInfo {
                    src: StdAddr::new(-1, HashBytes([0x22; 32])).into(),
                    dst: address.clone().into(),
                    value: CurrencyCollection::new(1_000_000_000),
                    ..Default::default()
                }),
                init: None,
                body: Cell::empty_cell().into(),
                layout: None,
            }))
        };
        let executor = Executor::new(
            ExecutorVerbosity::FullLocationStackVerbose,
            Some(DEFAULT_CONFIG),
        )?;
        let (result, _) = executor.run_transaction(
            &message,
            &RunTransactionArgs {
                shard_account: Boc::encode_base64(to_cell(account)),
                now: NOW,
                lt,
                random_seed: Some(SEED),
                is_tick_tock: kind.map(|_| true),
                is_tock: kind.map(|kind| kind == TickTock::Tock),
                ..Default::default()
            },
        )?;
        let result = success(result)?;

        Ok((
            Boc::decode_base64(result.transaction.as_ref())?.parse()?,
            Boc::decode_base64(result.shard_account.as_ref())?.parse()?,
        ))
    }

    fn success(result: EmulationResult) -> anyhow::Result<RunTransactionResultSuccess> {
        match result {
            EmulationResult::Success(result) => Ok(result),
            EmulationResult::Error(error) => anyhow::bail!("Emulation failed: {}", error.error),
        }
    }

    fn counters(account: &ShardAccount) -> anyhow::Result<(u32, u32, u32)> {
        let account = account.load_account()?.context("Missing account")?;
        let AccountState::Active(state) = account.state else {
            anyhow::bail!("Account is not active");
        };
        Ok(state.data.context("Missing storage")?.parse()?)
    }

    #[test]
    fn replay_tick_and_tock_targets() -> anyhow::Result<()> {
        let (address, account) = fixture()?;
        let balance = account.load_account()?.unwrap().balance.tokens;
        let mut results = Vec::new();

        for kind in [TickTock::Tick, TickTock::Tock] {
            let (original, expected_account) =
                reference_transaction(&address, &account, Some(kind), 1_000_000)?;
            let (replayed, _) = emulate(&original, DEFAULT_CONFIG, &account, None, SEED, None)?;
            let replayed = success(replayed)?;
            let (sender, contract, amount, money, transaction, compute) =
                compute_final_data(&replayed, balance, &address)?;
            let replayed_account: ShardAccount =
                Boc::decode_base64(replayed.shard_account.as_ref())?.parse()?;
            let (actions, _) = find_final_actions(&replayed);

            results.push(json!({
                "kind": format!("{kind:?}"),
                "sender": sender.map(|value| value.to_string()),
                "contract": contract.to_string(),
                "amount": amount.map(u128::from),
                "opcode": tx_opcode(&transaction),
                "in_message": transaction.in_msg.is_some(),
                "compute": compute,
                "sent_total": money.sent_total,
                "actions": actions.len(),
                "balance_after_matches": money.balance_after == u128::from(expected_account.load_account()?.unwrap().balance.tokens) as u64,
                "state_hash_matches": transaction.state_update.load()?.new == original.state_update.load()?.new,
                "account_matches": replayed_account == expected_account,
                "storage": counters(&replayed_account)?,
            }));
        }

        expect![[r#"
            [
              {
                "account_matches": true,
                "actions": 0,
                "amount": null,
                "balance_after_matches": true,
                "compute": {
                  "exitCode": 0,
                  "gasFees": 13200000,
                  "gasUsed": 1320,
                  "success": true,
                  "type": "success",
                  "vmSteps": 25
                },
                "contract": "-1:1111111111111111111111111111111111111111111111111111111111111111",
                "in_message": false,
                "kind": "Tick",
                "opcode": null,
                "sender": null,
                "sent_total": 0,
                "state_hash_matches": true,
                "storage": [
                  1,
                  0,
                  0
                ]
              },
              {
                "account_matches": true,
                "actions": 0,
                "amount": null,
                "balance_after_matches": true,
                "compute": {
                  "exitCode": 0,
                  "gasFees": 12840000,
                  "gasUsed": 1284,
                  "success": true,
                  "type": "success",
                  "vmSteps": 23
                },
                "contract": "-1:1111111111111111111111111111111111111111111111111111111111111111",
                "in_message": false,
                "kind": "Tock",
                "opcode": null,
                "sender": null,
                "sent_total": 0,
                "state_hash_matches": true,
                "storage": [
                  0,
                  1,
                  0
                ]
              }
            ]"#]]
        .assert_eq(&serde_json::to_string_pretty(&results)?);
        Ok(())
    }

    #[test]
    fn replay_tick_tock_predecessors_before_ordinary_target() -> anyhow::Result<()> {
        let (address, account) = fixture()?;
        let balance = account.load_account()?.unwrap().balance.tokens;
        let (tick, after_tick) =
            reference_transaction(&address, &account, Some(TickTock::Tick), 1_000_000)?;
        let (tock, after_tock) =
            reference_transaction(&address, &after_tick, Some(TickTock::Tock), 2_000_000)?;
        let (target, expected_account) =
            reference_transaction(&address, &after_tock, None, 3_000_000)?;

        let (balance_before, before_target) = emulate_previous_transactions(
            &[tick, tock],
            &account,
            &balance,
            None,
            std::iter::repeat((DEFAULT_CONFIG, SEED, None)),
        )?;
        let (replayed, _) = emulate(&target, DEFAULT_CONFIG, &before_target, None, SEED, None)?;
        let replayed = success(replayed)?;
        let (sender, contract, amount, _, transaction, compute) =
            compute_final_data(&replayed, balance_before, &address)?;
        let replayed_account: ShardAccount =
            Boc::decode_base64(replayed.shard_account.as_ref())?.parse()?;

        expect![[r#"
            {
              "account_matches": true,
              "amount": 1000000000,
              "compute": {
                "exitCode": 0,
                "gasFees": 12770000,
                "gasUsed": 1277,
                "success": true,
                "type": "success",
                "vmSteps": 21
              },
              "contract": "-1:1111111111111111111111111111111111111111111111111111111111111111",
              "predecessors_match": true,
              "sender": "-1:2222222222222222222222222222222222222222222222222222222222222222",
              "state_hash_matches": true,
              "storage_after": [
                1,
                1,
                1
              ],
              "storage_before": [
                1,
                1,
                0
              ]
            }"#]].assert_eq(&serde_json::to_string_pretty(&json!({
            "predecessors_match": before_target == after_tock,
            "storage_before": counters(&before_target)?,
            "storage_after": counters(&replayed_account)?,
            "state_hash_matches": transaction.state_update.load()?.new == target.state_update.load()?.new,
            "account_matches": replayed_account == expected_account,
            "sender": sender.map(|value| value.to_string()),
            "contract": contract.to_string(),
            "amount": amount.map(u128::from),
            "compute": compute,
        }))?);
        Ok(())
    }

    #[test]
    fn ordinary_transaction_still_requires_incoming_message() -> anyhow::Result<()> {
        let (address, account) = fixture()?;
        let (mut transaction, _) = reference_transaction(&address, &account, None, 1_000_000)?;
        transaction.in_msg = None;

        let error = emulate(&transaction, DEFAULT_CONFIG, &account, None, SEED, None).unwrap_err();
        expect![["No in_message was found in transaction"]].assert_eq(&error.to_string());
        Ok(())
    }
}
