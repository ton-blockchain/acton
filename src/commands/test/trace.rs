use crate::commands::test::{Pos, TestDescriptor};
use crate::context::BuildCache;
use emulator::emulator::SendMessageResult;
use emulator::executor::StoreExt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tolkc::source_map::SourceMap;
use tycho_types::boc::Boc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestTrace {
    pub name: String,
    pub pos: Pos,
    pub txs: TransactionList,
    pub contracts: Vec<ContractInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionList {
    pub transactions: Vec<TransactionInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractInfo {
    pub name: String,
    pub code_boc64: String,
    pub source_map: SourceMap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub raw_transaction: String,
    pub parent_transaction: Option<u64>,
    pub child_transactions: Vec<u64>,
    pub shard_account_before: String,
    pub shard_account: String,
    pub vm_log_diff: String,
    pub logs: String,
    pub actions: Option<String>,
    pub dest_contract_info: Option<String>,
}

pub fn dump_test_transactions(
    test: &TestDescriptor,
    build_cache: &BuildCache,
    txs: &[Vec<SendMessageResult>],
    output_dir: &str,
) -> anyhow::Result<()> {
    let mut known_contracts = BTreeMap::new();

    let txs = txs
        .iter()
        .flatten()
        .flat_map(|tx| {
            let SendMessageResult::Success(tx) = tx else {
                return None;
            };

            let build = build_cache.result_for_code(&tx.code);

            let contract_info = build.map(|(_, info)| ContractInfo {
                name: info.name.clone(),
                code_boc64: info.code_boc64.clone(),
                source_map: info.source_map.clone(),
            });

            if let Some(contract_info) = &contract_info {
                known_contracts.insert(contract_info.name.clone(), contract_info.clone());
            }

            Some(TransactionInfo {
                raw_transaction: tx.raw_transaction.clone(),
                parent_transaction: tx.parent_transaction.as_ref().map(|tx| tx.lt),
                dest_contract_info: contract_info.as_ref().map(|info| info.name.clone()),
                child_transactions: tx.child_transactions.clone(),
                shard_account_before: Boc::encode_hex(tx.shard_account_before.to_cell()),
                shard_account: Boc::encode_hex(tx.shard_account.to_cell()),
                vm_log_diff: vmlogs::convert_to_diff_logs(&tx.vm_log),
                logs: tx.logs.clone(),
                actions: tx.actions.clone(),
            })
        })
        .collect::<Vec<_>>();

    let list = TransactionList { transactions: txs };
    let test_info = TestTrace {
        name: test.name.clone(),
        pos: test.pos.clone(),
        txs: list,
        contracts: known_contracts.values().cloned().collect(),
    };

    let str = serde_json::to_string(&test_info)?;

    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)?;
    }

    let filename = format!("{}_trace.json", test.name);
    let file_path = output_path.join(filename);
    fs::write(file_path, str)?;

    Ok(())
}
