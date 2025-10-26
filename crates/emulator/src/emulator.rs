use crate::blockchain::Blockchain;
use crate::executor::{
    EmulationResult, Executor, ExecutorVerbosity, ResultError, RunTransactionArgs, StoreExt,
};
use crate::step_by_step_trait::StepSyStepExecutor;
use crate::step_executor::StepExecutor;
use num_bigint::BigInt;
use serde::Deserialize;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, Load};
use tycho_types::models::{
    IntAddr, Message, MsgInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount, Transaction,
};

pub struct Emulator {
    pub executor: Executor,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum SendMessageResult {
    Success(SendMessageResultSuccess),
    Error(ResultError),
}

#[derive(Deserialize, Clone)]
pub struct SendMessageResultSuccess {
    pub raw_transaction: String,
    pub transaction: Transaction,
    pub parent_transaction: Option<Transaction>,
    pub shard_account: ShardAccount,
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

        let executor = StepExecutor::new();
        executor.prepare_transaction(
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

        while !executor.step() {
            // println!("{}", executor.get_code_pos());
        }

        // dbg!(executor.finish_transaction());

        let result = executor.finish_transaction();
        // let result = self.executor.run_transaction(
        //     message,
        //     BigInt::from(0),
        //     RunTransactionArgs {
        //         config: crate::config::DEFAULT_CONFIG.to_string(),
        //         libs: None,
        //         verbosity: ExecutorVerbosity::FullLocation,
        //         shard_account: dest_account,
        //         now: 0,
        //         lt: net.get_lt(),
        //         random_seed: None,
        //         ignore_chksig: false,
        //         debug_enabled: true,
        //         prev_blocks_info: None,
        //     },
        // );
        let result = match result {
            EmulationResult::Success(result) => result,
            EmulationResult::Error(err) => {
                dbg!(&err);
                return vec![SendMessageResult::Error(err)];
            }
        };

        let shard_account_after = &result.shard_account;
        let shard_account_cell = Boc::decode_base64(shard_account_after).unwrap();
        let mut shard_account_slice = shard_account_cell.as_slice().unwrap();
        let shard_account = ShardAccount::load_from(&mut shard_account_slice).unwrap();

        net.update_account(&int_message.dst.to_string(), &shard_account);

        let tx_cell: Cell = Boc::decode_base64(&result.transaction).unwrap();
        let mut tx_slice = tx_cell.as_slice().unwrap();
        let transaction = Transaction::load_from(&mut tx_slice).unwrap();

        let send_result = SendMessageResultSuccess {
            raw_transaction: result.transaction,
            transaction: transaction.clone(),
            parent_transaction: None,
            shard_account,
            vm_log: result.vm_log,
            actions: result.actions,
        };

        std::iter::once(SendMessageResult::Success(send_result))
            .chain(transaction.iter_out_msgs().flat_map(|msg| {
                let Ok(msg) = msg else { return vec![] };
                let send_results = self.send_message(net, msg.to_cell(), None);
                send_results
            }))
            .collect()
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
