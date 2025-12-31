use crate::world_state::WorldState;
use anyhow::Context;
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

pub struct Emulator {
    pub executor: Executor,
}

impl Emulator {
    pub fn new(verbosity: ExecutorVerbosity, config_b64: Option<&str>) -> anyhow::Result<Emulator> {
        let executor = Executor::new(verbosity, config_b64)?;
        Ok(Emulator { executor })
    }

    pub fn send_single_message(
        &self,
        state: &mut WorldState,
        message: Cell,
        libs: &Dict<HashBytes, LibDescr>,
        from: Option<IntAddr>,
    ) -> anyhow::Result<SendMessageResult> {
        let message = Emulator::patch_src_addr(message, from);
        let message_obj = message.parse::<Message>()?;
        let MsgInfo::Int(int_message) = &message_obj.info else {
            anyhow::bail!("message is not an internal message")
        };
        let dst_addr = int_message.dst.to_string();

        let dest_account = state.get_account(&dst_addr);
        let code = Self::get_code_cell(&message_obj, &dest_account);

        let (result, executor_logs) = self.executor.run_transaction(
            &Boc::encode_base64(message),
            RunTransactionArgs {
                libs: libs.clone().into_root().map(Boc::encode_base64),
                shard_account: Boc::encode_base64(&to_cell(&dest_account)),
                now: state.get_now(),
                lt: state.get_lt(),
                random_seed: None,
                ignore_chksig: false,
                debug_enabled: true,
                prev_blocks_info: None,
                is_tick_tock: None,
                is_tock: None,
            },
        )?;
        let result = match result {
            EmulationResult::Success(result) => result,
            EmulationResult::Error(err) => return Ok(SendMessageResult::Error(err)),
        };

        let shard_account_after = &result.shard_account;
        let shard_account_cell = Boc::decode_base64(shard_account_after)
            .context("Failed to decode shard account BoC")?;
        let shard_account = shard_account_cell
            .parse::<ShardAccount>()
            .context("Failed to load shard account from slice")?;

        state.update_account(&dst_addr, &shard_account);

        let tx_cell =
            Boc::decode_base64(&result.transaction).context("Failed to decode transaction BoC")?;
        let transaction = tx_cell
            .parse::<Transaction>()
            .context("Failed to parse transaction BoC")?;

        let out_messages = transaction
            .iter_out_msgs()
            .filter_map(|it| it.ok())
            .map(|it| to_cell(&it))
            .collect::<Vec<_>>();

        let send_result = SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction,
            parent_transaction: None,
            child_transactions: vec![],
            shard_account_before: dest_account,
            shard_account,
            out_messages,
            vm_log: result.vm_log,
            executor_logs,
            actions: result.actions,
            code,
            externals: vec![],
        };

        Ok(SendMessageResult::Success(send_result))
    }

    pub fn send_message(
        &self,
        state: &mut WorldState,
        message: Cell,
        libs: &Dict<HashBytes, LibDescr>,
        from: Option<IntAddr>,
    ) -> Vec<SendMessageResult> {
        let result = self.send_single_message(state, message, libs, from);
        let Ok(SendMessageResult::Success(send_result)) = result else {
            return vec![];
        };

        let transaction = send_result.transaction.clone();
        let mut externals: Vec<Cell> = vec![];

        let mut all_results = std::iter::once(SendMessageResult::Success(send_result.clone()))
            .chain(transaction.iter_out_msgs().flat_map(|msg| {
                let Ok(msg) = msg else { return vec![] };

                if let MsgInfo::ExtOut(_) = &msg.info {
                    externals.push(to_cell(&msg));
                    return vec![];
                };

                let mut send_results = self.send_message(state, to_cell(&msg), libs, None);
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

    /// Set custom `src` address if it is None.
    pub fn patch_src_addr(message_cell: Cell, src_addr: Option<IntAddr>) -> Cell {
        let Some(from) = src_addr else {
            return message_cell;
        };

        let mut message = message_cell
            .parse::<RelaxedMessage>()
            .expect("Failed to load message from cell");

        match &mut message.info {
            RelaxedMsgInfo::Int(info) if info.src.is_none() => info.src = Some(from),
            _ => {}
        }

        // For some reason this set to wrong value
        message.layout = None;

        to_cell(&message)
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

        let Some(code) = state.code else {
            return None;
        };

        Some(code)
    }

    pub fn get_code_cell<T, B>(
        message: &BaseMessage<T, B>,
        account: &ShardAccount,
    ) -> Option<Cell> {
        let account_code = Self::get_address_code_cell(&account);
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

#[derive(Clone, Debug)]
pub enum SendMessageResult {
    Success(SendMessageResultSuccess),
    Error(RunTransactionResultError),
}

#[derive(Clone, Debug)]
pub struct SendMessageResultSuccess {
    pub raw_transaction: String,
    pub transaction: Transaction,
    pub parent_transaction: Option<Transaction>,
    pub child_transactions: Vec<u64>,
    pub shard_account_before: ShardAccount,
    pub shard_account: ShardAccount,
    pub out_messages: Vec<Cell>,
    pub vm_log: String,
    pub executor_logs: String,
    pub actions: Option<String>,
    pub code: Option<Cell>,
    pub externals: Vec<Cell>,
}

impl SendMessageResultSuccess {
    pub fn opcode(&self) -> Option<u32> {
        let in_msg = self.transaction.in_msg.as_deref()?;
        let mut in_msg = in_msg.parse::<RelaxedMessage>().ok()?;
        let opcode = in_msg.body.load_u32().ok()?;
        if let RelaxedMsgInfo::Int(info) = &in_msg.info
            && info.bounced
        {
            let opcode = in_msg.body.load_u32().ok()?;
            return Some(opcode);
        }
        Some(opcode)
    }

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

fn to_cell<T: Store + ?Sized>(obj: &T) -> Cell {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())
        .expect("Failed to store data into cell builder");
    builder.build().expect("Failed to build cell from builder")
}
