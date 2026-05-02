use crate::commands::test::{Pos, TestDescriptor};
use crate::context::{
    BuildCache, CompilationResult, Context, Emulations, FailedSendMessageResult, to_cell,
};
use crate::ffi::emulation::compilation_result_for_code;
use crate::retrace::{self, InstalledActions};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use tolk_compiler::SourceMap;
use tolk_compiler::abi::ContractABI;
use ton_retrace::trace::{ExecutedAction, ExecutedActionFailureReason, ExecutedActions};
use ton_source_map::SourceLocation;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{AccountState, IntAddr, MsgInfo, ShardAccount, StdAddr, Transaction};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TestTrace {
    pub name: Arc<str>,
    pub pos: Pos,
    pub traces: Vec<TransactionList>,
    pub contracts: Vec<String>,
    pub wallets: BTreeMap<String, String>, // Address -> Name
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TransactionList {
    pub name: String,
    pub transactions: Vec<TransactionInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_messages: Vec<FailedMessageInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ContractInfo {
    pub name: String,
    pub code_boc64: String,
    pub source_map: SourceMap,
    pub abi: Option<Arc<ContractABI>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub lt: String,
    pub raw_transaction: Arc<str>,
    pub parent_transaction: Option<String>,
    pub child_transactions: Vec<String>,
    pub shard_account_before: String,
    pub shard_account: String,
    pub vm_log_diff: String,
    pub executor_logs: Arc<str>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executor_actions: Vec<ExecutorActionInfo>,
    pub actions: Option<Arc<str>>,
    pub dest_contract_info: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FailedMessageInfo {
    pub error: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_log_diff: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm_exit_code: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_logs: Option<Arc<str>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_libraries: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutorActionFailureReasonInfo {
    NotEnoughToncoinToSend {
        remaining_balance: String,
        required: String,
    },
    CannotReserveToncoin {
        requested: String,
        available: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutorActionInfo {
    SendMessage {
        hash: String,
        remaining_balance: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<SourceLocation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_reason: Option<ExecutorActionFailureReasonInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_code: Option<i32>,
    },
    ReserveCurrency {
        mode: i32,
        reserve: String,
        balance: String,
        original_balance: String,
        changed_remaining_balance: String,
        changed_reserved_balance: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<SourceLocation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_reason: Option<ExecutorActionFailureReasonInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_code: Option<i32>,
    },
}

#[must_use]
pub(crate) fn parse_executor_actions(
    logs: &str,
    installed_actions: &InstalledActions,
    source_maps: &[&SourceMap],
) -> Vec<ExecutorActionInfo> {
    let executed = ExecutedActions::from(logs);
    executed
        .actions
        .into_iter()
        .map(|action| match action {
            ExecutedAction::SendMessage {
                hash,
                remaining_balance,
                failure_reason,
                failure_code,
            } => ExecutorActionInfo::SendMessage {
                location: installed_actions.find_message(&hash).and_then(|action| {
                    action_source_location(source_maps, &action.loc_hash, action.loc_offset)
                }),
                hash,
                remaining_balance: remaining_balance.to_string(),
                failure_reason: failure_reason.map(convert_failure_reason),
                failure_code,
            },
            ExecutedAction::ReserveCurrency {
                mode,
                reserve,
                balance,
                original_balance,
                changed_remaining_balance,
                changed_reserved_balance,
                failure_reason,
                failure_code,
            } => ExecutorActionInfo::ReserveCurrency {
                mode,
                reserve: reserve.to_string(),
                balance: balance.to_string(),
                original_balance: original_balance.to_string(),
                changed_remaining_balance: changed_remaining_balance.to_string(),
                changed_reserved_balance: changed_reserved_balance.to_string(),
                location: installed_actions
                    .find_reserve(mode, &reserve)
                    .and_then(|action| {
                        action_source_location(source_maps, &action.loc_hash, action.loc_offset)
                    }),
                failure_reason: failure_reason.map(convert_failure_reason),
                failure_code,
            },
        })
        .collect()
}

#[must_use]
fn action_source_location(
    source_maps: &[&SourceMap],
    loc_hash: &str,
    loc_offset: u16,
) -> Option<SourceLocation> {
    source_maps
        .iter()
        .find_map(|source_map| retrace::find_source_loc(source_map, loc_hash, loc_offset))
}

#[must_use]
fn convert_failure_reason(reason: ExecutedActionFailureReason) -> ExecutorActionFailureReasonInfo {
    match reason {
        ExecutedActionFailureReason::NotEnoughToncoinToSend {
            remaining_balance,
            required,
        } => ExecutorActionFailureReasonInfo::NotEnoughToncoinToSend {
            remaining_balance: remaining_balance.to_string(),
            required: required.to_string(),
        },
        ExecutedActionFailureReason::CannotReserveToncoin {
            requested,
            available,
        } => ExecutorActionFailureReasonInfo::CannotReserveToncoin {
            requested: requested.to_string(),
            available: available.to_string(),
        },
    }
}

#[must_use]
fn build_result_for_transaction(
    ctx: &Context<'_>,
    tx_code: Option<&Cell>,
    shard_account: &ShardAccount,
    tx: &Transaction,
) -> Option<(std::path::PathBuf, CompilationResult)> {
    resolve_build_result(ctx, tx_code.cloned())
        .or_else(|| resolve_build_result(ctx, shard_account_code(shard_account)))
        .or_else(|| {
            let addr = transaction_destination(tx).unwrap_or_else(|| StdAddr::new(0, tx.account));
            resolve_build_result(ctx, account_code(ctx, &addr))
        })
}

#[must_use]
fn shard_account_code(shard_account: &ShardAccount) -> Option<Cell> {
    let state = shard_account.account.load().ok()?.0?.state;
    match state {
        AccountState::Active(state) => state.code,
        AccountState::Uninit | AccountState::Frozen(_) => None,
    }
}

#[must_use]
fn account_code(ctx: &Context<'_>, addr: &StdAddr) -> Option<Cell> {
    ctx.chain
        .world_state
        .get_accounts()
        .get(addr)
        .and_then(shard_account_code)
}

#[must_use]
fn resolve_build_result(
    ctx: &Context<'_>,
    code: Option<Cell>,
) -> Option<(std::path::PathBuf, CompilationResult)> {
    let code = code?;
    ctx.build
        .build_cache
        .result_for_code(&Some(code.clone()))
        .or_else(|| compilation_result_for_code(ctx, Some(&code), true))
}

#[must_use]
fn transaction_destination(tx: &Transaction) -> Option<StdAddr> {
    match &tx.load_in_msg().ok()??.info {
        MsgInfo::Int(info) => match &info.dst {
            IntAddr::Std(addr) => Some(addr.clone()),
            IntAddr::Var(_) => None,
        },
        MsgInfo::ExtIn(info) => match &info.dst {
            IntAddr::Std(addr) => Some(addr.clone()),
            IntAddr::Var(_) => None,
        },
        MsgInfo::ExtOut(_) => None,
    }
}

pub(super) fn dump_test_transactions(
    test: &TestDescriptor,
    ctx: &Context<'_>,
    txs: &Emulations,
    output_dir: &str,
) -> anyhow::Result<()> {
    let build_cache: &BuildCache = ctx.build.build_cache;
    let known_addresses = &*ctx.build.known_addresses;
    let mut known_contracts = BTreeMap::new();
    let traces = txs
        .messages
        .iter()
        .enumerate()
        .map(|(trace_index, trace_transactions)| {
            let transactions = trace_transactions
                .iter()
                .map(|tx| {
                    let build = build_result_for_transaction(
                        ctx,
                        tx.code.as_ref(),
                        &tx.shard_account,
                        &tx.transaction,
                    );
                    let mut source_maps = Vec::new();
                    if let Some((_, info)) = &build {
                        source_maps.push(info.source_map.as_ref());
                    }
                    for result in build_cache.built.values() {
                        source_maps.push(result.source_map.as_ref());
                    }
                    let installed_actions = retrace::find_installed_actions(&tx.vm_log);
                    let executor_actions =
                        parse_executor_actions(&tx.executor_logs, &installed_actions, &source_maps);

                    let contract_info = build.map(|(_, info)| ContractInfo {
                        name: info.name.clone(),
                        code_boc64: info.code_boc64.clone(),
                        source_map: (*info.source_map).clone(),
                        abi: info.abi,
                    });
                    if let Some(info) = &contract_info {
                        known_contracts.insert(info.name.clone(), info.clone());
                    }

                    TransactionInfo {
                        lt: tx.transaction.lt.to_string(),
                        raw_transaction: tx.raw_transaction.clone(),
                        parent_transaction: tx.parent_transaction.map(|lt| lt.to_string()),
                        dest_contract_info: contract_info.as_ref().map(|info| info.name.clone()),
                        child_transactions: tx
                            .child_transactions
                            .iter()
                            .map(ToString::to_string)
                            .collect(),
                        shard_account_before: Boc::encode_base64(to_cell(&tx.shard_account_before)),
                        shard_account: Boc::encode_base64(to_cell(&tx.shard_account)),
                        vm_log_diff: tvm_logs::convert_to_diff_logs(&tx.vm_log),
                        executor_logs: tx.executor_logs.clone(),
                        executor_actions,
                        actions: tx.actions.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let failed_messages = txs.failed_messages.get(trace_index).map_or_else(
                Vec::new,
                |trace_failed_messages| {
                    trace_failed_messages
                        .iter()
                        .map(failed_message_info)
                        .collect::<Vec<_>>()
                },
            );

            let name = txs
                .trace_name(trace_transactions)
                .map_or_else(|| format!("Trace {}", trace_index + 1), ToString::to_string);

            TransactionList {
                name,
                transactions,
                failed_messages,
            }
        })
        .collect::<Vec<_>>();

    let mut wallets = BTreeMap::new();
    for (addr, known) in &known_addresses.addresses {
        wallets.insert(
            addr.display_base64_url(true).to_string(),
            known.name.clone(),
        );
    }

    for result in build_cache.built.values() {
        let info = ContractInfo {
            abi: result.abi.clone(),
            name: result.name.clone(),
            code_boc64: result.code_boc64.clone(),
            source_map: (*result.source_map).clone(),
        };

        known_contracts.insert(result.name.clone(), info);
    }

    let test_info = TestTrace {
        name: test.name.clone(),
        pos: test.pos.clone(),
        traces,
        contracts: known_contracts.keys().cloned().collect(),
        wallets,
    };

    let str = serde_json::to_string(&test_info)?;

    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)?;
    }

    // Save contracts separately
    let contracts_dir = output_path.join("contracts");
    if !contracts_dir.exists() {
        fs::create_dir_all(&contracts_dir)?;
    }

    for (name, info) in known_contracts {
        let contract_file = contracts_dir.join(format!("{name}.json"));
        let info_json = serde_json::to_string(&info)?;
        fs::write(contract_file, info_json)?;
    }

    let filename = format!("{}_trace.json", test.name);
    let file_path = output_path.join(filename);
    fs::write(file_path, str)?;

    Ok(())
}

#[must_use]
fn failed_message_info(message: &FailedSendMessageResult) -> FailedMessageInfo {
    let mut missing_libraries = message
        .missing_libraries
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    missing_libraries.sort_unstable();

    FailedMessageInfo {
        error: message.error.clone(),
        vm_log_diff: message
            .vm_log
            .as_deref()
            .map(tvm_logs::convert_to_diff_logs),
        vm_exit_code: message.vm_exit_code,
        executor_logs: message.executor_logs.clone(),
        missing_libraries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_executor_actions_extracts_send_error_details() {
        let logs = "[ 4][t 0][2026-02-25 11:22:27.910181][transaction.cpp:2649]\tprocess send message 6B4A9BAD9FCCCE4523A71307366AF36EC1C535F5D05EF2FF21E358903A399123
[ 3][t 0][2026-02-25 11:22:27.910192][transaction.cpp:3070]\tremaining balance 997209600ng
[ 4][t 0][2026-02-25 11:22:27.910194][transaction.cpp:2649]\tprocess send message 52B0D905B98FC395D52C1EF89AB4F9BBF869AF0B1445E18DA4691C1FD2ACC22F
[ 4][t 0][2026-02-25 11:22:27.910199][transaction.cpp:2926]\tnot enough grams to transfer with the message : remaining balance is 997209600ng, need 1000000400000 (including forwarding fees)
[ 4][t 0][2026-02-25 11:22:27.910201][transaction.cpp:2206]\tinvalid action 1 in action list: error code 37";

        let parsed = parse_executor_actions(logs, &InstalledActions::empty(), &[]);
        assert_eq!(parsed.len(), 2);

        assert!(matches!(
            &parsed[0],
            ExecutorActionInfo::SendMessage {
                failure_code: None,
                failure_reason: None,
                location: None,
                ..
            }
        ));

        assert!(matches!(
            &parsed[1],
            ExecutorActionInfo::SendMessage {
                failure_code: Some(37),
                failure_reason: Some(
                    ExecutorActionFailureReasonInfo::NotEnoughToncoinToSend { .. }
                ),
                location: None,
                ..
            }
        ));
    }

    #[test]
    fn parse_executor_actions_extracts_reserve_error_details() {
        let logs = "[ 4][t 0][2026-02-25 11:24:46.612154][transaction.cpp:3089]\tprocess raw reserve with mode 0
[ 4][t 0][2026-02-25 11:24:46.612156][transaction.cpp:3108]\taction_reserve_currency: mode=0, reserve=10000000ng, balance=1098500000ng, original balance=999742800ng
[ 3][t 0][2026-02-25 11:24:46.612158][transaction.cpp:3168]\tchanged remaining balance to 1088500000ng, reserved balance to 10000000ng
[ 4][t 0][2026-02-25 11:24:46.612160][transaction.cpp:3089]\tprocess raw reserve with mode 0
[ 4][t 0][2026-02-25 11:24:46.612161][transaction.cpp:3108]\taction_reserve_currency: mode=0, reserve=1000000000000ng, balance=1088500000ng, original balance=999742800ng
[ 4][t 0][2026-02-25 11:24:46.612163][transaction.cpp:3143]\tcannot reserve 1000000000000 nanograms : only 1088500000 available
[ 4][t 0][2026-02-25 11:24:46.612164][transaction.cpp:2206]\tinvalid action 1 in action list: error code 37";

        let parsed = parse_executor_actions(logs, &InstalledActions::empty(), &[]);
        assert_eq!(parsed.len(), 2);

        assert!(matches!(
            &parsed[0],
            ExecutorActionInfo::ReserveCurrency {
                failure_code: None,
                failure_reason: None,
                location: None,
                ..
            }
        ));

        assert!(matches!(
            &parsed[1],
            ExecutorActionInfo::ReserveCurrency {
                failure_code: Some(37),
                failure_reason: Some(ExecutorActionFailureReasonInfo::CannotReserveToncoin { .. }),
                location: None,
                ..
            }
        ));
    }
}
