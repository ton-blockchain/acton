//! This module provides a high-level `Emulator` for executing TON transactions and messages.
//!
//! It builds upon the lower-level [`ton_executor`] to provide a more convenient API
//! for emulating complex message flows, including recursive processing of outgoing
//! internal messages and state management via [`WorldState`].
//!
//! # Core Components
//!
//! - [`Emulator`]: The main coordinator for emulations. It wraps a [`ton_executor::message::Executor`]
//!   and provides methods for high-level message sending.
//! - [`SendMessageResult`]: The result of a message emulation, which can be either a success
//!   (with transaction details) or an error.
//! - [`SendMessageResultSuccess`]: Detailed information about a successful transaction,
//!   including outgoing messages, state changes, and gas usage.
//!
//! # Features
//!
//! - **Recursive Emulation**: [`Emulator::send_message`] automatically processes all outgoing
//!   internal messages, building a full trace of transactions.
//! - **State Management**: Integrates with [`WorldState`] to track account states across
//!   multiple transactions.
//!
//! # Examples
//!
//! ```rust
//! # use ton_emulator::emulator::{Emulator, SendMessageResult};
//! # use ton_emulator::world_state::WorldState;
//! # use ton_executor::ExecutorVerbosity;
//! # use tycho_types::cell::Cell;
//! # use tycho_types::dict::Dict;
//! #
//! # fn example(state: &mut WorldState, msg: Cell) -> anyhow::Result<()> {
//! let emulator = Emulator::new(ExecutorVerbosity::FullLocationStackVerbose, None)?;
//! let libs = Dict::new();
//!
//! // Send a message and process all resulting internal messages
//! let results = emulator.send_message(state, msg, &libs, None)?;
//!
//! for result in results {
//!     match result {
//!         SendMessageResult::Success(tx) => println!("Tx LT: {}", tx.transaction.lt),
//!         SendMessageResult::Error(err) => eprintln!("Error: {:?}", err),
//!     }
//! }
//! # Ok(())
//! # }
//! ```

use crate::world_state::WorldState;
use anyhow::Context;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use std::time::SystemTime;
use ton_executor::message::{
    EmulationResult, Executor, RunTransactionArgs, RunTransactionResultError,
};
use ton_executor::{ExecutorVerbosity, MissingLibrariesContext, missing_library_callback};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::dict::Dict;
use tycho_types::models::config::BlockchainConfigParams;
use tycho_types::models::{
    AccountState, BaseMessage, ComputePhase, IntAddr, LibDescr, Message, MsgInfo, RelaxedMessage,
    RelaxedMsgInfo, ShardAccount, StdAddr, Transaction, TxInfo,
};
use tycho_types::num::Tokens;
use tycho_types::prelude::HashBytes;

/// Prepared input for a single transaction execution.
///
/// This captures the shared pre-processing both normal emulation and debug stepping
/// need before handing control to a concrete executor implementation.
#[derive(Clone)]
pub struct PreparedSendTransaction {
    /// Base64 encoded patched message `BoC`.
    pub message_b64: String,
    /// Fully populated executor arguments for this transaction.
    pub run_args: RunTransactionArgs,
    /// Resolved destination code cell, if known.
    pub code: Option<Cell>,
    destination: StdAddr,
    shard_account_before: ShardAccount,
}

/// A high-level emulator for TON transactions.
///
/// It manages an underlying [`Executor`] and provides a more convenient API for
/// emulating message flows and managing world state.
pub struct Emulator {
    /// The underlying low-level executor.
    pub executor: Executor,
}

impl Emulator {
    /// Creates a new `Emulator` instance.
    ///
    /// # Arguments
    ///
    /// * `verbosity` - The level of logging detail for the executor.
    /// * `config_b64` - Optional Base64-encoded global configuration `BoC`.
    pub fn new(verbosity: ExecutorVerbosity, config_b64: Option<&str>) -> anyhow::Result<Emulator> {
        let executor = Executor::new(verbosity, config_b64)?;
        Ok(Emulator { executor })
    }

    /// Emulates a single transaction for an internal message.
    ///
    /// This method performs a single execution step and updates the provided [`WorldState`].
    /// It does **not** process outgoing messages recursively. Use [`Self::send_message`] for that.
    ///
    /// # Arguments
    ///
    /// * `state` - The world state to read from and update.
    /// * `message` - The internal message to emulate.
    /// * `libs` - Global libraries available for the execution.
    /// * `from` - Optional source address to override if missing in the message.
    ///
    /// # Returns
    ///
    /// Returns [`SendMessageResult::Success`] if the emulation ran (even if it failed with a TVM error),
    /// or an error if the emulation process itself failed.
    pub fn send_transaction(
        &self,
        state: &mut WorldState,
        message: Cell,
        libs: &Dict<HashBytes, LibDescr>,
        from: Option<IntAddr>,
    ) -> anyhow::Result<SendMessageResult> {
        let mut missing_libraries_ctx = MissingLibrariesContext::default();
        self.executor.register_missing_library_callback(
            &mut missing_libraries_ctx,
            missing_library_callback,
        )?;

        let prepared = Self::prepare_send_transaction(state, message, libs, from)?;
        let (result, executor_logs) = self
            .executor
            .run_transaction(&prepared.message_b64, &prepared.run_args)?;

        Self::finalize_send_transaction(
            state,
            prepared,
            result,
            Some(executor_logs),
            missing_libraries_ctx.into_set(),
        )
    }

    /// Emulates a message flow, recursively processing all outgoing internal messages.
    ///
    /// This is the primary method for simulating user interactions or complex contract calls.
    /// It returns a list of all transactions produced by the message flow, in the order they were executed.
    ///
    /// # Arguments
    ///
    /// * `state` - The world state to track account states across transactions.
    /// * `message` - The initial message to send.
    /// * `libs` - Global libraries.
    /// * `from` - Optional source address override.
    pub fn send_message(
        &self,
        state: &mut WorldState,
        message: Cell,
        libs: &Dict<HashBytes, LibDescr>,
        from: Option<IntAddr>,
    ) -> anyhow::Result<Vec<SendMessageResult>> {
        Self::execute_send_message_flow(message, from, &mut |message, from| {
            self.send_transaction(state, message, libs, from).map(Some)
        })
    }

    /// Prepare a single transaction input shared by normal and debug execution paths.
    pub fn prepare_send_transaction(
        state: &mut WorldState,
        message: Cell,
        libs: &Dict<HashBytes, LibDescr>,
        from: Option<IntAddr>,
    ) -> anyhow::Result<PreparedSendTransaction> {
        let msg_cell = Self::patch_message(state.get_config(), message, from)?;
        let msg_b64 = Boc::encode_base64(&msg_cell);
        let msg = msg_cell
            .parse::<Message<'_>>()
            .context("Failed to parse message")?;

        let dst = match &msg.info {
            MsgInfo::Int(addr) => addr.dst.clone(),
            MsgInfo::ExtIn(addr) => addr.dst.clone(),
            MsgInfo::ExtOut(_) => {
                anyhow::bail!("Send transaction only support internal and external-in messages")
            }
        };
        let dst = match dst {
            IntAddr::Std(dst) => dst,
            IntAddr::Var(_) => anyhow::bail!("Var addresses are not supported"),
        };

        let shard_account_before = state.get_account(&dst);
        let code = Self::get_code_cell(&msg, &shard_account_before);
        let run_args = RunTransactionArgs {
            libs: libs.clone().into_root().map(Boc::encode_base64),
            shard_account: Boc::encode_base64(&to_cell(&shard_account_before)?),
            now: state.get_now(),
            lt: state.get_lt(),
            random_seed: None,
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
            is_tick_tock: None,
            is_tock: None,
        };

        Ok(PreparedSendTransaction {
            message_b64: msg_b64,
            run_args,
            code,
            destination: dst,
            shard_account_before,
        })
    }

    /// Finalize a single transaction result and apply the resulting state update.
    pub fn finalize_send_transaction(
        state: &mut WorldState,
        prepared: PreparedSendTransaction,
        result: EmulationResult,
        executor_logs: Option<Arc<str>>,
        missing_libraries: FxHashSet<String>,
    ) -> anyhow::Result<SendMessageResult> {
        let PreparedSendTransaction {
            code,
            destination,
            shard_account_before,
            ..
        } = prepared;
        let mut missing_libraries = Some(missing_libraries);

        let result = match result {
            EmulationResult::Success(mut result) => {
                result.missing_libraries = missing_libraries.take().unwrap_or_default();
                result
            }
            EmulationResult::Error(mut err) => {
                err.missing_libraries = missing_libraries.take().unwrap_or_default();
                return Ok(SendMessageResult::Error(RunTransactionResultError {
                    error: err.error,
                    vm_log: err.vm_log,
                    vm_exit_code: err.vm_exit_code,
                    executor_logs,
                    missing_libraries: err.missing_libraries,
                }));
            }
        };

        let shard_account_after = Boc::decode_base64(result.shard_account.as_ref())?
            .parse::<ShardAccount>()
            .context("Failed to parse shard account")?;

        state.update_account(&destination, &shard_account_after);

        let transaction = Boc::decode_base64(result.transaction.as_ref())?
            .parse::<Transaction>()
            .context("Failed to parse transaction")?;

        let out_messages = transaction
            .iter_out_msgs()
            .filter_map(Result::ok)
            .map(|it| to_cell(&it))
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(SendMessageResult::Success(SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction,
            parent_transaction: None,
            child_transactions: vec![],
            shard_account_before,
            shard_account: shard_account_after,
            out_messages,
            vm_log: result.vm_log,
            executor_logs: executor_logs.unwrap_or_default(),
            actions: result.actions,
            code,
            externals: vec![],
            missing_libraries: result.missing_libraries,
        }))
    }

    /// Run the recursive message-flow traversal using a caller-provided single-tx runner.
    ///
    /// The hook is responsible only for executing one message. Recursion over emitted
    /// internal messages, plus bookkeeping for externals and parent/child links, stays here.
    pub fn execute_send_message_flow<Run>(
        message: Cell,
        from: Option<IntAddr>,
        run_transaction: &mut Run,
    ) -> anyhow::Result<Vec<SendMessageResult>>
    where
        Run: FnMut(Cell, Option<IntAddr>) -> anyhow::Result<Option<SendMessageResult>>,
    {
        let Some(initial_res) = run_transaction(message, from)? else {
            return Ok(Vec::new());
        };

        let mut results = vec![initial_res.clone()];
        let SendMessageResult::Success(main_res) = initial_res else {
            return Ok(results);
        };

        let mut externals = Vec::new();
        let mut child_lts = Vec::new();
        let main_tx = main_res.transaction.clone();

        for out_msg_cell in main_res.out_messages {
            let Ok(out_msg) = out_msg_cell.parse::<Message<'_>>() else {
                continue;
            };

            match out_msg.info {
                MsgInfo::ExtOut(_) => {
                    externals.push(out_msg_cell);
                }
                MsgInfo::Int(_) => {
                    let mut sub_results =
                        Self::execute_send_message_flow(out_msg_cell, None, run_transaction)?;
                    if let Some(SendMessageResult::Success(res)) = sub_results.get_mut(0) {
                        res.parent_transaction = Some(main_tx.lt);
                        child_lts.push(res.transaction.lt);
                    }
                    results.extend(sub_results);
                }
                MsgInfo::ExtIn(_) => {}
            }
        }

        if let Some(SendMessageResult::Success(res)) = results.get_mut(0) {
            res.externals = externals;
            res.child_transactions = child_lts;
        }

        Ok(results)
    }

    /// Emulates a tick-tock transaction on the given account, then recursively
    /// processes all outgoing internal messages (reusing [`Self::send_message`]).
    pub fn run_tick_tock(
        &self,
        state: &mut WorldState,
        addr: &StdAddr,
        is_tock: bool,
        libs: &Dict<HashBytes, LibDescr>,
    ) -> anyhow::Result<Vec<SendMessageResult>> {
        let mut missing_libraries_ctx = MissingLibrariesContext::default();
        self.executor.register_missing_library_callback(
            &mut missing_libraries_ctx,
            missing_library_callback,
        )?;

        let shard_account_before = state.get_account(addr);
        let code = Self::get_address_code_cell(&shard_account_before);

        let args = RunTransactionArgs {
            libs: libs.clone().into_root().map(Boc::encode_base64),
            shard_account: Boc::encode_base64(&to_cell(&shard_account_before)?),
            now: state.get_now(),
            lt: state.get_lt(),
            random_seed: None,
            ignore_chksig: false,
            debug_enabled: true,
            prev_blocks_info: None,
            is_tick_tock: Some(true),
            is_tock: Some(is_tock),
        };

        // Tick-tock has no incoming message; the C++ emulator ignores this parameter
        // when is_tick_tock is set.
        let (result, executor_logs) = self.executor.run_transaction("", &args)?;
        let mut missing_libraries = Some(missing_libraries_ctx.into_set());

        let result = match result {
            EmulationResult::Success(mut result) => {
                result.missing_libraries = missing_libraries.take().unwrap_or_default();
                result
            }
            EmulationResult::Error(mut err) => {
                err.missing_libraries = missing_libraries.take().unwrap_or_default();
                return Ok(vec![SendMessageResult::Error(RunTransactionResultError {
                    error: err.error,
                    vm_log: err.vm_log,
                    vm_exit_code: err.vm_exit_code,
                    executor_logs: Some(executor_logs),
                    missing_libraries: err.missing_libraries,
                })]);
            }
        };

        let shard_account_after = Boc::decode_base64(result.shard_account.as_ref())?
            .parse::<ShardAccount>()
            .context("Failed to parse shard account")?;

        state.update_account(addr, &shard_account_after);

        let transaction = Boc::decode_base64(result.transaction.as_ref())?
            .parse::<Transaction>()
            .context("Failed to parse transaction")?;

        let out_messages = transaction
            .iter_out_msgs()
            .filter_map(Result::ok)
            .map(|it| to_cell(&it))
            .collect::<anyhow::Result<Vec<_>>>()?;

        let main_res = SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction: transaction.clone(),
            parent_transaction: None,
            child_transactions: vec![],
            shard_account_before,
            shard_account: shard_account_after,
            out_messages: out_messages.clone(),
            vm_log: result.vm_log,
            executor_logs,
            actions: result.actions,
            code,
            externals: vec![],
            missing_libraries: result.missing_libraries,
        };

        let mut results = vec![SendMessageResult::Success(main_res)];
        let mut externals = Vec::new();
        let mut child_lts = Vec::new();

        // Recursively process outgoing internal messages via send_message
        for out_msg_cell in out_messages {
            let Ok(out_msg) = out_msg_cell.parse::<Message<'_>>() else {
                continue;
            };

            match out_msg.info {
                MsgInfo::ExtOut(_) => {
                    externals.push(out_msg_cell);
                }
                MsgInfo::Int(_) => {
                    let mut sub_results = self.send_message(state, out_msg_cell, libs, None)?;
                    if let Some(SendMessageResult::Success(res)) = sub_results.get_mut(0) {
                        res.parent_transaction = Some(transaction.lt);
                        child_lts.push(res.transaction.lt);
                    }
                    results.extend(sub_results);
                }
                MsgInfo::ExtIn(_) => {}
            }
        }

        if let Some(SendMessageResult::Success(res)) = results.get_mut(0) {
            res.externals = externals;
            res.child_transactions = child_lts;
        }

        Ok(results)
    }

    pub fn patch_message(
        config: Arc<Dict<u32, Cell>>,
        message_cell: Cell,
        src_addr: Option<IntAddr>,
    ) -> anyhow::Result<Cell> {
        let Some(from) = src_addr else {
            return Ok(message_cell);
        };

        if let Ok(mut message) = message_cell.parse::<RelaxedMessage<'_>>() {
            if let RelaxedMsgInfo::Int(info) = &mut message.info {
                // Set src address as Node does
                if info.src.is_none() {
                    info.src = Some(from);
                }

                // Set create_at as Node does
                info.created_at = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as u32;

                // This value is rewritten below after we compute forwarding fees.
                info.fwd_fee = Tokens::ZERO;
            }

            // For some reason this set to wrong value
            message.layout = None;

            if let RelaxedMsgInfo::Int(info) = &message.info {
                // Forwarding prices are selected by destination workchain.
                let is_masterchain = info.dst.is_masterchain();
                let fwd_fee = Self::compute_in_msg_fwd_fee(config, &message, is_masterchain)?;
                if let RelaxedMsgInfo::Int(info) = &mut message.info {
                    info.fwd_fee = fwd_fee;
                }
            }

            return to_cell(&message);
        }

        Ok(message_cell)
    }

    pub(crate) fn compute_in_msg_fwd_fee(
        config: Arc<Dict<u32, Cell>>,
        message: &RelaxedMessage<'_>,
        is_masterchain: bool,
    ) -> anyhow::Result<Tokens> {
        let message_cell = to_cell(message)?;
        let root_bits = u64::from(message_cell.bit_len());
        let mut stats = message_cell
            .as_slice_allow_exotic()
            .compute_unique_stats(usize::MAX)
            .context("Failed to compute message stats for forwarding fee calculation")?;

        // Real node excludes bits from the root message cell (but keeps referenced cells).
        stats.bit_count = stats.bit_count.saturating_sub(root_bits);

        let config_root = config
            .root()
            .clone()
            .context("Blockchain config is empty: missing config root dictionary")?;
        let config_params = BlockchainConfigParams::from_raw(config_root);
        let prices = config_params
            .get_msg_forward_prices(is_masterchain)
            .context("Failed to get msg forward prices from blockchain config (params 24/25)")?;

        // INMSG_FWDFEE for inbound params is expected to hold the remaining part.
        // Then GETORIGINALFWDFEE restores total from that value.
        let total = prices.compute_fwd_fee(stats);
        let first_part = prices.get_first_part(total);
        Ok(total.saturating_sub(first_part))
    }

    fn get_address_code_cell(account: &ShardAccount) -> Option<Cell> {
        let state = account
            .account
            .load()
            .ok()
            .and_then(|loaded| loaded.0)
            .map(|s| s.state);

        let Some(AccountState::Active(state)) = state else {
            return None;
        };

        let code = state.code?;
        Some(code)
    }

    /// Sets the global blockchain configuration.
    ///
    /// Updates both the executor and the world state.
    ///
    /// Returns `Ok(true)` if the configuration was successfully updated,
    /// `Ok(false)` if the executor rejected the new configuration,
    /// or an error if the operation failed.
    pub fn set_config(&self, state: &mut WorldState, config: Cell) -> anyhow::Result<bool> {
        let config_boc = Boc::encode_base64(&config);

        match self.executor.set_config(&config_boc) {
            Ok(res) => {
                if res {
                    let mut config_slice = config.as_slice_allow_exotic();

                    let config_dict = Dict::<u32, Cell>::load_from_root_ext(
                        &mut config_slice,
                        Cell::empty_context(),
                    )
                    .context("Failed to load config dict from cell")?;

                    state.set_config(config_dict);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Resolves the code cell for a message and its destination account.
    ///
    /// It first tries to get the code from the account itself. If the account is
    /// uninitialized, it tries to get the code from the message's `init` field.
    pub fn get_code_cell<T, B>(
        message: &BaseMessage<T, B>,
        account: &ShardAccount,
    ) -> Option<Cell> {
        let account_code = Self::get_address_code_cell(account);
        match account_code {
            Some(code) => Some(code),
            None => {
                if let Some(init) = &message.init
                    && let Some(code) = &init.code
                {
                    Some(code.clone())
                } else {
                    None
                }
            }
        }
    }
}

/// The result of a message emulation.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum SendMessageResult {
    /// The transaction was executed successfully (though it might have failed in TVM).
    Success(SendMessageResultSuccess),
    /// An error occurred during the emulation process.
    Error(RunTransactionResultError),
}

/// Detailed information about a successful transaction emulation.
#[derive(Clone, Debug)]
pub struct SendMessageResultSuccess {
    /// Base64-encoded transaction `BoC`.
    pub raw_transaction: Arc<str>,
    /// The parsed transaction object.
    pub transaction: Transaction,
    /// Logical time of the parent transaction, if any.
    pub parent_transaction: Option<u64>,
    /// Logical times of child transactions produced by this transaction.
    pub child_transactions: Vec<u64>,
    /// State of the account before the transaction.
    pub shard_account_before: ShardAccount,
    /// State of the account after the transaction.
    pub shard_account: ShardAccount,
    /// Cells of outgoing messages produced by this transaction.
    pub out_messages: Vec<Cell>,
    /// VM execution log.
    pub vm_log: Arc<str>,
    /// High-level executor logs.
    pub executor_logs: Arc<str>,
    /// Base64-encoded outgoing actions `BoC`.
    pub actions: Option<Arc<str>>,
    /// The code cell used for this transaction.
    pub code: Option<Cell>,
    /// External outgoing messages produced by this transaction.
    pub externals: Vec<Cell>,
    /// Hashes of missing libraries observed during this transaction emulation.
    pub missing_libraries: FxHashSet<String>,
}

impl SendMessageResultSuccess {
    /// Extracts the opcode from the incoming message body.
    ///
    /// If the message is a bounced message, it tries to extract the opcode
    /// following the initial 32-bit `0xffffffff` prefix.
    #[must_use]
    pub fn opcode(&self) -> Option<u32> {
        let in_msg = self.transaction.in_msg.as_deref()?;
        let mut in_msg = in_msg.parse::<RelaxedMessage<'_>>().ok()?;
        let opcode = in_msg.body.load_u32().ok()?;
        if let RelaxedMsgInfo::Int(info) = &in_msg.info
            && info.bounced
        {
            let opcode = in_msg.body.load_u32().ok()?;
            return Some(opcode);
        }
        Some(opcode)
    }

    /// Returns the amount of gas used during the computation phase.
    #[must_use]
    pub fn used_gas(&self) -> Option<u64> {
        let info = self.transaction.info.load().ok()?;
        let TxInfo::Ordinary(info) = info else {
            return None;
        };
        let ComputePhase::Executed(info) = info.compute_phase else {
            return None;
        };
        Some(info.gas_used.into())
    }
}

fn to_cell<T: Store + ?Sized>(obj: &T) -> anyhow::Result<Cell> {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())?;
    Ok(builder.build()?)
}
