use crate::blockchain::Blockchain;
use crate::executor::{
    EmulationResult, Executor, ExecutorVerbosity, ResultError, RunTransactionArgs, StoreExt,
};
use num_bigint::BigInt;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    IntAddr, Message, MsgInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount, Transaction,
};

pub struct Emulator {
    pub executor: Executor,
}

#[derive(Clone, Debug)]
pub enum SendMessageResult {
    Success(SendMessageResultSuccess),
    Error(ResultError),
}

impl SendMessageResult {
    pub fn vm_logs(&self) -> String {
        match self {
            SendMessageResult::Success(res) => res.vm_log.clone(),
            SendMessageResult::Error(res) => res.vm_log.clone().unwrap_or("".to_string()),
        }
    }

    pub fn debug_logs(&self) -> String {
        match self {
            SendMessageResult::Success(res) => res.debug_logs.clone(),
            SendMessageResult::Error(res) => "".to_string(),
        }
    }

    pub fn executor_logs(&self) -> String {
        match self {
            SendMessageResult::Success(res) => res.logs.clone(),
            SendMessageResult::Error(res) => "".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SendMessageResultSuccess {
    pub raw_transaction: String,
    pub transaction: Transaction,
    pub parent_transaction: Option<Transaction>,
    pub child_transactions: Vec<u64>,
    pub shard_account: ShardAccount,
    pub out_messages: Vec<Cell>,
    pub vm_log: String,
    pub logs: String,
    pub debug_logs: String,
    pub actions: Option<String>,
    pub code: Option<Cell>,
    pub externals: Vec<Cell>,
}

impl Emulator {
    pub fn new() -> Self {
        let executor = Executor::new();
        Self { executor }
    }

    pub fn send_message(
        &self,
        net: &mut Blockchain,
        message: Cell,
        src_addr: Option<IntAddr>,
    ) -> Vec<SendMessageResult> {
        let message = Emulator::patch_src_addr(message, src_addr);
        let message_obj = Message::load_from(&mut message.parse().unwrap()).unwrap();
        let MsgInfo::Int(int_message) = &message_obj.info else {
            return vec![];
        };

        let dest_account = net.get_account(&int_message.dst.to_string());
        let (result, logs, debug_logs) = self.executor.run_transaction(
            message.clone(),
            BigInt::from(0),
            RunTransactionArgs {
                config: crate::config::DEFAULT_CONFIG.to_string(),
                libs: None,
                verbosity: ExecutorVerbosity::FullLocation,
                shard_account: dest_account.clone(),
                now: 0,
                lt: net.get_lt(),
                random_seed: None,
                ignore_chksig: false,
                debug_enabled: true,
                prev_blocks_info: None,
            },
        );
        let result = match result {
            EmulationResult::Success(result) => result,
            EmulationResult::Error(err) => return vec![SendMessageResult::Error(err)],
        };

        let shard_account_after = &result.shard_account;
        let shard_account_cell = Boc::decode_base64(shard_account_after).unwrap();
        let mut shard_account_slice = shard_account_cell.as_slice().unwrap();
        let shard_account = ShardAccount::load_from(&mut shard_account_slice).unwrap();

        net.update_account(&int_message.dst.to_string(), &shard_account);

        let tx_cell: Cell = Boc::decode_base64(&result.transaction).unwrap();
        let mut tx_slice = tx_cell.as_slice().unwrap();
        let transaction = Transaction::load_from(&mut tx_slice).unwrap();

        let out_messages = transaction
            .iter_out_msgs()
            .filter_map(|it| it.ok())
            .map(|it| it.to_cell())
            .collect::<Vec<_>>();

        let code = Executor::get_code_cell(&message_obj, &dest_account);

        let send_result = SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction: transaction.clone(),
            parent_transaction: None,
            child_transactions: vec![],
            shard_account,
            out_messages,
            vm_log: result.vm_log,
            logs,
            debug_logs,
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

                let mut send_results = self.send_message(net, msg.to_cell(), None);
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
    pub fn patch_src_addr(message: Cell, src_addr: Option<IntAddr>) -> Cell {
        let Some(from) = src_addr else { return message };

        let mut slice = message.as_slice().unwrap();
        let mut message_obj = RelaxedMessage::load_from(&mut slice).unwrap();

        match &mut message_obj.info {
            RelaxedMsgInfo::Int(info) if info.src.is_none() => info.src = Some(from),
            _ => {}
        }

        // For some reason this set to wrong value
        message_obj.layout = None;

        message_obj.to_cell()
    }
}
