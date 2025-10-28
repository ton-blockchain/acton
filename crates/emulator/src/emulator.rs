use crate::blockchain::Blockchain;
use crate::executor::{
    EmulationResult, Executor, ExecutorVerbosity, ResultError, RunTransactionArgs, StoreExt,
};
use num_bigint::BigInt;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load, Store};
use tycho_types::models::{
    IntAddr, Message, MsgInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount, Transaction,
};

pub struct Emulator {
    pub executor: Executor,
}

#[derive(Clone)]
pub enum SendMessageResult {
    Success(SendMessageResultSuccess),
    Error(ResultError),
}

#[derive(Clone)]
pub struct SendMessageResultSuccess {
    pub raw_transaction: String,
    pub transaction: Transaction,
    pub parent_transaction: Option<Transaction>,
    pub child_transactions: Vec<u64>,
    pub shard_account: ShardAccount,
    pub out_messages: Vec<Cell>,
    pub vm_log: String,
    pub actions: Option<String>,
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
        let MsgInfo::Int(int_message) = message_obj.info else {
            panic!("Emulator only supports internal messages for now");
        };

        let dest_account = net.get_account(&int_message.dst.to_string());
        let result = self.executor.run_transaction(
            message,
            BigInt::from(0),
            RunTransactionArgs {
                config: crate::config::DEFAULT_CONFIG.to_string(),
                libs: None,
                verbosity: ExecutorVerbosity::FullLocation,
                shard_account: dest_account,
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

        let send_result = SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction: transaction.clone(),
            parent_transaction: None,
            child_transactions: vec![],
            shard_account,
            out_messages,
            vm_log: result.vm_log,
            actions: result.actions,
        };

        let mut all_results = std::iter::once(SendMessageResult::Success(send_result.clone()))
            .chain(transaction.iter_out_msgs().flat_map(|msg| {
                let Ok(msg) = msg else { return vec![] };
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
