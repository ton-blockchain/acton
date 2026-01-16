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
//! # use emulator::emulator::{Emulator, SendMessageResult};
//! # use emulator::world_state::WorldState;
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
use std::time::SystemTime;
use ton_executor::ExecutorVerbosity;
use ton_executor::message::{
    EmulationResult, Executor, RunTransactionArgs, RunTransactionResultError,
};
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::dict::Dict;
use tycho_types::models::{
    AccountState, BaseMessage, ComputePhase, IntAddr, LibDescr, Message, MsgInfo, RelaxedMessage,
    RelaxedMsgInfo, ShardAccount, Transaction, TxInfo,
};
use tycho_types::prelude::HashBytes;

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
        let msg_cell = Self::patch_message(message, from)?;
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
        let dst_addr = dst.to_string();

        let shard_account_before = state.get_account(&dst_addr);
        let code = Self::get_code_cell(&msg, &shard_account_before);

        let args = RunTransactionArgs {
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

        let (result, executor_logs) = self.executor.run_transaction(&msg_b64, &args)?;

        let result = match result {
            EmulationResult::Success(result) => result,
            EmulationResult::Error(err) => return Ok(SendMessageResult::Error(err)),
        };

        let shard_account_after = Boc::decode_base64(&result.shard_account)?
            .parse::<ShardAccount>()
            .context("Failed to parse shard account")?;

        // Since state was updated, we need to update it in world state too.
        state.update_account(&dst_addr, &shard_account_after);

        let transaction = Boc::decode_base64(&result.transaction)?
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
            executor_logs,
            actions: result.actions,
            code,
            externals: vec![],
        }))
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
        let mut results = Vec::new();

        // 1. Process the initial message
        let initial_res = self.send_transaction(state, message, libs, from)?;

        results.push(initial_res.clone());

        // If the initial transaction failed, or we didn't get a success, stop here
        let SendMessageResult::Success(main_res) = initial_res else {
            return Ok(vec![initial_res]);
        };

        let mut externals = Vec::new();
        let mut child_lts = Vec::new();
        let main_tx = main_res.transaction.clone();

        // 2. Recursively process outgoing messages
        for out_msg_cell in main_res.out_messages {
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
                        res.parent_transaction = Some(main_tx.lt);
                        child_lts.push(res.transaction.lt);
                    }
                    results.extend(sub_results);
                }
                MsgInfo::ExtIn(_) => {}
            }
        }

        // 3. Finalize the main result with gathered information
        if let Some(SendMessageResult::Success(res)) = results.get_mut(0) {
            res.externals = externals;
            res.child_transactions = child_lts;
        }

        Ok(results)
    }

    pub fn patch_message(message_cell: Cell, src_addr: Option<IntAddr>) -> anyhow::Result<Cell> {
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
            }

            // For some reason this set to wrong value
            message.layout = None;

            return to_cell(&message);
        }

        Ok(message_cell)
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
            Ok(res) => match res {
                true => {
                    let mut config_slice = config
                        .as_slice()
                        .ok()
                        .context("Failed to parse config cell to slice")?;
                    let config_dict = Dict::<u32, Cell>::load_from_root_ext(
                        &mut config_slice,
                        Cell::empty_context(),
                    )
                    .context("Failed to load config dict from cell")?;

                    state.set_config(config_dict);
                    Ok(true)
                }
                false => Ok(false),
            },
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
    pub raw_transaction: String,
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
    pub vm_log: String,
    /// High-level executor logs.
    pub executor_logs: String,
    /// Base64-encoded outgoing actions `BoC`.
    pub actions: Option<String>,
    /// The code cell used for this transaction.
    pub code: Option<Cell>,
    /// External outgoing messages produced by this transaction.
    pub externals: Vec<Cell>,
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
