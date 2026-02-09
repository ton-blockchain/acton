use crate::commands::test::{Pos, TestDescriptor};
use crate::context::{BuildCache, Emulations, KnownAddresses, to_cell};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use ton_abi::ContractAbi;
use ton_source_map::SourceMap;
use tycho_types::boc::Boc;
use tycho_types::models::IntAddr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TestTrace {
    pub name: String,
    pub pos: Pos,
    pub traces: Vec<TransactionList>,
    pub contracts: Vec<String>,
    pub wallets: BTreeMap<String, String>, // Address -> Name
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TransactionList {
    pub transactions: Vec<TransactionInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ContractInfo {
    pub name: String,
    pub code_boc64: String,
    pub source_map: SourceMap,
    pub abi: Option<ContractAbi>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub lt: String,
    pub raw_transaction: String,
    pub parent_transaction: Option<String>,
    pub child_transactions: Vec<String>,
    pub shard_account_before: String,
    pub shard_account: String,
    pub vm_log_diff: String,
    pub executor_logs: String,
    pub actions: Option<String>,
    pub dest_contract_info: Option<String>,
}

pub(super) fn dump_test_transactions(
    test: &TestDescriptor,
    build_cache: &BuildCache,
    known_addresses: &KnownAddresses,
    txs: &Emulations,
    output_dir: &str,
) -> anyhow::Result<()> {
    let traces = txs
        .messages
        .iter()
        .map(|txs| {
            let transactions = txs
                .iter()
                .map(|tx| {
                    let build = build_cache.result_for_code(&tx.code);

                    let contract_info = build.map(|(_, info)| ContractInfo {
                        name: info.name.clone(),
                        code_boc64: info.code_boc64.clone(),
                        source_map: info.source_map.clone(),
                        abi: info.abi,
                    });

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
                        vm_log_diff: vmlogs::convert_to_diff_logs(&tx.vm_log),
                        executor_logs: tx.executor_logs.clone(),
                        actions: tx.actions.clone(),
                    }
                })
                .collect::<Vec<_>>();

            TransactionList { transactions }
        })
        .collect::<Vec<_>>();

    let mut wallets = BTreeMap::new();
    for (addr, known) in &known_addresses.addresses {
        if let IntAddr::Std(addr) = addr {
            wallets.insert(
                addr.display_base64_url(true).to_string(),
                known.name.clone(),
            );
        }
    }

    let mut known_contracts = BTreeMap::new();
    for result in build_cache.built.values() {
        let info = ContractInfo {
            abi: result.abi.clone(),
            name: result.name.clone(),
            code_boc64: result.code_boc64.clone(),
            source_map: result.source_map.clone(),
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
        if !contract_file.exists() {
            let info_json = serde_json::to_string(&info)?;
            fs::write(contract_file, info_json)?;
        }
    }

    let filename = format!("{}_trace.json", test.name);
    let file_path = output_path.join(filename);
    fs::write(file_path, str)?;

    Ok(())
}
