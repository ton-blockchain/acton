#![allow(dead_code)]

use acton_client::__private::tycho_types::cell::{CellBuilder, CellSliceParts};
use acton_client::__private::tycho_types::dict::Dict;
use acton_client::__private::tycho_types::models::{
    AccountState, AccountStatus, ComputePhase, CurrencyCollection, IntAddr, IntMsgInfo, LibDescr,
    MsgInfo, OwnedMessage, RelaxedMessage, RelaxedMsgInfo, StateInit, TxInfo,
};
use acton_client::__private::tycho_types::prelude::HashBytes;
use acton_client::{
    BigInt, Cell, ContractProvider, ContractSender, InternalMessage, StdAddr, Tuple,
};
use std::collections::HashMap;
use std::future::ready;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use ton_emulator::{
    AccountsState, Emulator, LocalAccountsState, SendMessageResult, WorldState, WorldStateSnapshot,
};
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use tvm_ffi::serde::serialize_tuple;
use tycho_types::boc::Boc;

/// A local [`ContractProvider`] backed by the real TVM get-method executor.
#[derive(Debug, Clone)]
pub(super) struct TvmGetterProvider {
    address: StdAddr,
    code: Cell,
    data: Cell,
}

impl TvmGetterProvider {
    #[must_use]
    pub(super) const fn new(address: StdAddr, code: Cell, data: Cell) -> Self {
        Self {
            address,
            code,
            data,
        }
    }

    #[allow(dead_code)]
    #[must_use]
    pub(super) const fn address(&self) -> &StdAddr {
        &self.address
    }
}

impl ContractProvider for TvmGetterProvider {
    type Error = String;

    async fn run_get_method(
        &self,
        address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> Result<Tuple, Self::Error> {
        if address != &self.address {
            return Err(format!(
                "address mismatch: expected {}, got {address}",
                self.address
            ));
        }

        let unixtime = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs()
            .try_into()
            .map_err(|error: std::num::TryFromIntError| error.to_string())?;
        let params = RunGetMethodArgs {
            code: Boc::encode_base64(&self.code),
            data: Boc::encode_base64(&self.data),
            verbosity: ExecutorVerbosity::Short,
            libs: String::new(),
            address: self.address.to_string(),
            unixtime,
            balance: "10".to_owned(),
            rand_seed: "0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            gas_limit: "0".to_owned(),
            method_id,
            debug_enabled: true,
            extra_currencies: HashMap::new(),
            prev_blocks_info: None,
        };
        let executor = GetExecutor::new(&params).map_err(|error| error.to_string())?;
        let stack = serialize_tuple(&arguments).map_err(|error| error.to_string())?;
        let stack = Boc::encode_base64(stack);
        let result = executor
            .run_get_method(&stack, &params, None)
            .map_err(|error| error.to_string())?;

        match result {
            GetMethodResult::Success(result) => {
                if result.vm_exit_code != 0 {
                    return Err(format!("exit_code: {}", result.vm_exit_code));
                }
                let stack =
                    Boc::decode_base64(result.stack.as_ref()).map_err(|error| error.to_string())?;
                Tuple::deserialize(&stack).map_err(|error| error.to_string())
            }
            GetMethodResult::Error(error) => Err(error.error.to_string()),
        }
    }
}

/// A deterministic source account used by [`TvmContractProvider`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TvmSender {
    name: String,
    address: StdAddr,
}

impl TvmSender {
    #[must_use]
    pub(super) fn new(name: impl Into<String>, address_byte: u8) -> Self {
        Self {
            name: name.into(),
            address: StdAddr::new(0, HashBytes([address_byte; 32])),
        }
    }
}

/// Stable facts extracted from a transaction executed by the real TON emulator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TvmTransaction {
    pub(super) sender: String,
    pub(super) sender_matches: bool,
    pub(super) recipient_matches: bool,
    pub(super) value: BigInt,
    pub(super) bounce: bool,
    pub(super) body: String,
    pub(super) opcode: Option<u32>,
    pub(super) deploy: bool,
    pub(super) success: bool,
    pub(super) aborted: bool,
    pub(super) exit_code: Option<i32>,
    pub(super) action_result_code: Option<i32>,
}

#[derive(Debug)]
struct TvmContractState {
    world: WorldStateSnapshot,
    transactions: Vec<TvmTransaction>,
}

/// A generated-wrapper provider backed by real TON transaction and get-method executors.
///
/// The provider stores a serializable local-world snapshot instead of `WorldState` itself.
/// `WorldState` can represent remote state backed by `Rc`, while generated providers must be
/// `Sync`; reconstructing its local form for each operation keeps this test provider thread-safe.
#[derive(Debug, Clone)]
pub(super) struct TvmContractProvider {
    address: StdAddr,
    state: Arc<Mutex<TvmContractState>>,
}

impl TvmContractProvider {
    pub(super) fn new(address: StdAddr) -> Result<Self, String> {
        let world = WorldState::new(AccountsState::Local(LocalAccountsState::new()), None)
            .and_then(|world| world.snapshot())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            address,
            state: Arc::new(Mutex::new(TvmContractState {
                world,
                transactions: Vec::new(),
            })),
        })
    }

    pub(super) fn transactions(&self) -> Result<Vec<TvmTransaction>, String> {
        self.state
            .lock()
            .map(|state| state.transactions.clone())
            .map_err(|_| "TVM contract state lock is poisoned".to_owned())
    }

    pub(super) fn is_deployed(&self) -> Result<bool, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "TVM contract state lock is poisoned".to_owned())?;
        let snapshot = state.world.clone();
        drop(state);
        let world = WorldState::from_snapshot(snapshot).map_err(|error| error.to_string())?;
        let Some(shard_account) = world.get_accounts().get(&self.address) else {
            return Ok(false);
        };
        let account = shard_account
            .account
            .load()
            .map_err(|error| error.to_string())?;
        Ok(account
            .0
            .is_some_and(|account| matches!(account.state, AccountState::Active(_))))
    }

    fn active_state(&self, address: &StdAddr) -> Result<(Cell, Cell), String> {
        if address != &self.address {
            return Err(format!(
                "address mismatch: expected {}, got {address}",
                self.address
            ));
        }
        let state = self
            .state
            .lock()
            .map_err(|_| "TVM contract state lock is poisoned".to_owned())?;
        let snapshot = state.world.clone();
        drop(state);
        let world = WorldState::from_snapshot(snapshot).map_err(|error| error.to_string())?;
        let shard_account = world
            .get_accounts()
            .get(address)
            .ok_or_else(|| format!("contract {address} does not exist"))?;
        let account = shard_account
            .account
            .load()
            .map_err(|error| error.to_string())?
            .0
            .ok_or_else(|| format!("contract {address} is not initialized"))?;
        let AccountState::Active(init) = account.state else {
            return Err(format!("contract {address} is not active"));
        };
        let code = init
            .code
            .ok_or_else(|| format!("contract {address} has no code"))?;
        let data = init.data.unwrap_or_default();
        Ok((code, data))
    }
}

impl ContractSender for TvmContractProvider {
    type Error = String;
    type Sender = TvmSender;
    type Output = TvmTransaction;

    fn send_internal(
        &self,
        via: &Self::Sender,
        address: &StdAddr,
        message: InternalMessage,
    ) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send {
        let result = (|| {
            if address != &self.address {
                return Err(format!(
                    "address mismatch: expected {}, got {address}",
                    self.address
                ));
            }

            let value = u128::try_from(message.value.clone())
                .map_err(|_| format!("message value does not fit uint128: {}", message.value))?;
            let deploy = message.init.is_some();
            let bounce = message.options.bounce.unwrap_or(!deploy);
            let body = format!(
                "x{{{:X}}}",
                message
                    .body
                    .as_slice()
                    .map_err(|error| error.to_string())?
                    .display_data()
            );
            let init = message.init.map(|init| StateInit {
                code: Some(init.code),
                data: Some(init.data),
                ..Default::default()
            });
            let message_cell = CellBuilder::build_from(OwnedMessage {
                info: MsgInfo::Int(IntMsgInfo {
                    ihr_disabled: true,
                    bounce,
                    bounced: false,
                    src: IntAddr::Std(via.address.clone()),
                    dst: IntAddr::Std(address.clone()),
                    value: CurrencyCollection::new(value),
                    ihr_fee: Default::default(),
                    fwd_fee: Default::default(),
                    created_at: 0,
                    created_lt: 0,
                }),
                init,
                body: CellSliceParts::from(message.body),
                layout: None,
            })
            .map_err(|error| error.to_string())?;

            let mut state = self
                .state
                .lock()
                .map_err(|_| "TVM contract state lock is poisoned".to_owned())?;
            let mut world = WorldState::from_snapshot(state.world.clone())
                .map_err(|error| error.to_string())?;
            let emulator =
                Emulator::new(ExecutorVerbosity::Off, None).map_err(|error| error.to_string())?;
            let result = emulator
                .send_transaction(
                    &mut world,
                    message_cell,
                    &Dict::<HashBytes, LibDescr>::new(),
                    None,
                )
                .map_err(|error| error.to_string())?;
            let result = match result {
                SendMessageResult::Success(result) => result,
                SendMessageResult::Error(error) => return Err(error.error),
            };

            let in_msg = result
                .transaction
                .in_msg
                .as_deref()
                .ok_or_else(|| "emulated transaction has no incoming message".to_owned())?
                .parse::<RelaxedMessage<'_>>()
                .map_err(|error| error.to_string())?;
            let RelaxedMsgInfo::Int(in_info) = in_msg.info else {
                return Err("emulated transaction incoming message is not internal".to_owned());
            };
            let sender_matches = in_info.src.as_ref() == Some(&IntAddr::Std(via.address.clone()));
            let recipient_matches = in_info.dst == IntAddr::Std(address.clone());

            let info = result
                .transaction
                .info
                .load()
                .map_err(|error| error.to_string())?;
            let TxInfo::Ordinary(info) = info else {
                return Err("emulated transaction is not ordinary".to_owned());
            };
            let aborted = info.aborted;
            let (compute_success, exit_code) = match &info.compute_phase {
                ComputePhase::Executed(phase) => (phase.success, Some(phase.exit_code)),
                ComputePhase::Skipped(_) => (false, None),
            };
            let action_result_code = info.action_phase.as_ref().map(|phase| phase.result_code);
            let action_success = info.action_phase.as_ref().is_none_or(|phase| phase.success);
            let deployed = result.transaction.orig_status != AccountStatus::Active
                && result.transaction.end_status == AccountStatus::Active;
            let transaction = TvmTransaction {
                sender: via.name.clone(),
                sender_matches,
                recipient_matches,
                value: message.value,
                bounce,
                body,
                opcode: result.opcode(),
                deploy: deployed,
                success: !aborted && compute_success && action_success,
                aborted,
                exit_code,
                action_result_code,
            };
            state.world = world.snapshot().map_err(|error| error.to_string())?;
            state.transactions.push(transaction.clone());
            drop(state);
            Ok(transaction)
        })();
        ready(result)
    }
}

impl ContractProvider for TvmContractProvider {
    type Error = String;

    fn run_get_method(
        &self,
        address: &StdAddr,
        method_id: i32,
        arguments: Tuple,
    ) -> impl Future<Output = Result<Tuple, Self::Error>> + Send {
        let address = address.clone();
        let state = self.active_state(&address);
        async move {
            let (code, data) = state?;
            TvmGetterProvider::new(address.clone(), code, data)
                .run_get_method(&address, method_id, arguments)
                .await
        }
    }
}
