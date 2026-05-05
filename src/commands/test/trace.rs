use crate::commands::test::{Pos, TestDescriptor};
use crate::context::{
    BuildCache, CompilationResult, Emulations, FailedSendMessageResult, KnownAddresses, to_cell,
};
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
    SetCode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<SourceLocation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_code: Option<i32>,
    },
    ChangeLibrary {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        location: Option<SourceLocation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        failure_code: Option<i32>,
    },
}

#[must_use]
pub(crate) fn parse_executor_actions(
    logs: &str,
    installed_actions: &InstalledActions,
    source_map: Option<&SourceMap>,
) -> Vec<ExecutorActionInfo> {
    let source_location =
        |loc_hash: &str, loc_offset| retrace::find_source_loc(source_map?, loc_hash, loc_offset);
    let action_location = |action: &ExecutedAction| {
        let installed = installed_actions
            .actions
            .iter()
            .find(|installed| installed.matches_executed_action(action))?;
        source_location(installed.loc_hash(), installed.loc_offset())
    };

    let executed = ExecutedActions::from(logs);
    executed
        .actions
        .into_iter()
        .map(|action| {
            let location = action_location(&action);
            executor_action_info(action, location)
        })
        .collect()
}

fn executor_action_info(
    action: ExecutedAction,
    location: Option<SourceLocation>,
) -> ExecutorActionInfo {
    match action {
        ExecutedAction::SendMessage {
            hash,
            remaining_balance,
            failure_reason,
            failure_code,
        } => ExecutorActionInfo::SendMessage {
            location,
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
            location,
            failure_reason: failure_reason.map(convert_failure_reason),
            failure_code,
        },
        ExecutedAction::SetCode { failure_code, .. } => ExecutorActionInfo::SetCode {
            location,
            failure_code,
        },
        ExecutedAction::ChangeLibrary { failure_code, .. } => ExecutorActionInfo::ChangeLibrary {
            location,
            failure_code,
        },
    }
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

fn contract_info(result: &CompilationResult) -> ContractInfo {
    ContractInfo {
        abi: result.abi.clone(),
        name: result.name.clone(),
        code_boc64: result.code_boc64.clone(),
        source_map: (*result.source_map).clone(),
    }
}

pub(super) fn dump_test_transactions(
    test: &TestDescriptor,
    build_cache: &BuildCache,
    known_addresses: &KnownAddresses,
    txs: &Emulations,
    output_dir: &str,
) -> anyhow::Result<()> {
    let mut known_contracts = BTreeMap::new();
    let traces = txs
        .messages
        .iter()
        .enumerate()
        .map(|(trace_index, trace_transactions)| {
            let transactions = trace_transactions
                .iter()
                .map(|tx| {
                    let build = build_cache.result_for_code(&tx.code);
                    let source_map = build.as_ref().map(|(_, info)| info.source_map.as_ref());
                    let installed_actions = retrace::find_installed_actions(&tx.vm_log);
                    let executor_actions =
                        parse_executor_actions(&tx.executor_logs, &installed_actions, source_map);

                    let dest_contract_info = build.as_ref().map(|(_, info)| info.name.clone());

                    TransactionInfo {
                        lt: tx.transaction.lt.to_string(),
                        raw_transaction: tx.raw_transaction.clone(),
                        parent_transaction: tx.parent_transaction.map(|lt| lt.to_string()),
                        dest_contract_info,
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
        known_contracts.insert(result.name.clone(), contract_info(result));
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
    use crate::retrace::InstalledAction;

    #[test]
    fn parse_executor_actions_extracts_send_error_details() {
        let logs = "[ 4][t 0][2026-02-25 11:22:27.910181][transaction.cpp:2649]\tprocess send message 6B4A9BAD9FCCCE4523A71307366AF36EC1C535F5D05EF2FF21E358903A399123
[ 3][t 0][2026-02-25 11:22:27.910192][transaction.cpp:3070]\tremaining balance 997209600ng
[ 4][t 0][2026-02-25 11:22:27.910194][transaction.cpp:2649]\tprocess send message 52B0D905B98FC395D52C1EF89AB4F9BBF869AF0B1445E18DA4691C1FD2ACC22F
[ 4][t 0][2026-02-25 11:22:27.910199][transaction.cpp:2926]\tnot enough grams to transfer with the message : remaining balance is 997209600ng, need 1000000400000 (including forwarding fees)
[ 4][t 0][2026-02-25 11:22:27.910201][transaction.cpp:2206]\tinvalid action 1 in action list: error code 37";

        let parsed = parse_executor_actions(logs, &InstalledActions::empty(), None);
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

        let parsed = parse_executor_actions(logs, &InstalledActions::empty(), None);
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

    #[test]
    fn parse_executor_actions_includes_set_code_and_change_library_metadata() {
        let vm_logs = r"
register new cell 0F: B5EE9C72010101010002000000
stack: [ C{0F} ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 10
execute SETCODE
gas remaining: 999
stack: [ C{0F} 18 ]
code cell hash: 734EFDF436945A5CB58154AAFB58A8258087B27EE31E98876254E4385F47B51D offset: 20
execute SETLIBCODE
gas remaining: 998";
        let installed_actions = retrace::find_installed_actions(vm_logs);
        let InstalledAction::SetCode(set_code) = &installed_actions.actions[0] else {
            panic!("expected set-code action");
        };
        let InstalledAction::ChangeLibrary(change_library) = &installed_actions.actions[1] else {
            panic!("expected change-library action");
        };
        let new_code_hash = set_code.new_code.repr_hash().to_string();
        let lib_hash = match &change_library.lib {
            ton_retrace::trace::InstalledLibraryRef::Cell(cell) => cell.repr_hash().to_string(),
            ton_retrace::trace::InstalledLibraryRef::Hash(hash) => hash.to_str_radix(16),
        };
        let executor_logs = format!(
            "[ 4][t 0][2026-03-03 13:38:24.650053][transaction.cpp:2269]\tprocess set code {new_code_hash}
[ 4][t 0][2026-03-03 13:38:24.650054][transaction.cpp:2312]\tprocess change library with mode 18, lib_hash={lib_hash}, lib_ref=cell
[ 4][t 0][2026-03-03 13:38:24.650055][transaction.cpp:2206]\tinvalid action 1 in action list: error code 34"
        );

        let parsed = parse_executor_actions(&executor_logs, &installed_actions, None);
        assert_eq!(parsed.len(), 2);

        assert!(matches!(
            &parsed[0],
            ExecutorActionInfo::SetCode {
                location: None,
                failure_code: None
            }
        ));
        assert!(matches!(
            &parsed[1],
            ExecutorActionInfo::ChangeLibrary {
                location: None,
                failure_code: Some(34)
            }
        ));
    }
}
