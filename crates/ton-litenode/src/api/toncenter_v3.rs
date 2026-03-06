use crate::litenode::{
    LiteNodeAccountState, LiteNodeBlockTransactions, LiteNodeMessage, LiteNodeRunGetMethodResult,
    LiteNodeTransaction,
};
use crate::storage::{
    AccountStatus, EmulateTraceResult, JettonMasterMeta, JettonWalletMeta, MsgMeta, TraceNode,
    TransactionInfo,
};
use base64::Engine;
use serde_json::value::Value;
use std::collections::HashMap;
use tvmffi::json_stack::stack_to_json;
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;

#[allow(clippy::ptr_arg)]
pub fn map_jetton_masters(masters: &Vec<JettonMasterMeta>) -> Value {
    serde_json::json!({
        "address_book": {},
        "metadata": {},
        "jetton_masters": masters.iter().map(map_jetton_master).collect::<Vec<_>>()
    })
}

fn map_jetton_master(m: &JettonMasterMeta) -> Value {
    serde_json::json!({
        "address": m.address.to_string(),
        "admin_address": m.admin_address.to_string(),
        "code_hash": m.code_hash.to_base64(),
        "data_hash": m.data_hash.to_base64(),
        "jetton_content": m.jetton_content,
        "jetton_wallet_code_hash": m.jetton_wallet_code_hash.to_base64(),
        "last_transaction_lt": m.last_transaction_lt.to_string(),
        "mintable": m.mintable,
        "total_supply": m.total_supply.to_string(),
    })
}

#[allow(clippy::ptr_arg)]
pub fn map_jetton_wallets(wallets: &Vec<JettonWalletMeta>) -> Value {
    serde_json::json!({
        "address_book": {},
        "metadata": {},
        "jetton_wallets": wallets.iter().map(map_jetton_wallet).collect::<Vec<_>>()
    })
}

pub fn map_address_information(state: &LiteNodeAccountState) -> Value {
    serde_json::json!({
        "balance": state.balance.to_string(),
        "code": encode_optional_boc(state.code.as_ref()),
        "data": encode_optional_boc(state.data.as_ref()),
        "frozen_hash": state.frozen_hash.as_ref().map(|h| h.to_base64()).unwrap_or_default(),
        "last_transaction_hash": state.last_transaction_id.hash.to_base64(),
        "last_transaction_lt": state.last_transaction_id.lt.to_string(),
        "status": map_account_status(&state.state),
    })
}

pub fn map_send_message(bt: &LiteNodeBlockTransactions) -> Value {
    let message_hash = bt
        .msg_hash
        .as_ref()
        .map(|h| h.to_base64())
        .unwrap_or_default();
    serde_json::json!({
        "message_hash": message_hash,
        "message_hash_norm": message_hash,
    })
}

pub fn map_transactions_response(transactions: &[LiteNodeTransaction]) -> Value {
    serde_json::json!({
        "address_book": {},
        "transactions": transactions.iter().map(map_v3_transaction).collect::<Vec<_>>()
    })
}

fn map_v3_transaction(tx: &LiteNodeTransaction) -> Value {
    let in_msg = if tx.in_msg.hash.0 == [0; 32] {
        Value::Null
    } else {
        map_v3_message(&tx.in_msg, &tx.hash, tx.utime, true)
    };
    let out_msgs = tx
        .out_msgs
        .iter()
        .filter(|msg| msg.hash.0 != [0; 32])
        .map(|msg| map_v3_message(msg, &tx.hash, tx.utime, false))
        .collect::<Vec<_>>();

    serde_json::json!({
        "account": tx.address.to_string(),
        "hash": tx.hash.to_base64(),
        "lt": tx.transaction_id.lt.to_string(),
        "now": tx.utime,
        "orig_status": "active",
        "end_status": "active",
        "total_fees": tx.total_fees.to_string(),
        "total_fees_extra_currencies": {},
        "prev_trans_hash": zero_hash_base64(),
        "prev_trans_lt": "0",
        "description": {
            "type": "ord",
            "aborted": !tx.success,
            "compute_ph": {
                "skipped": false,
                "success": tx.success,
                "exit_code": tx.exit_code,
            },
            "action": {
                "success": tx.success,
                "result_code": if tx.success { 0 } else { tx.exit_code },
            }
        },
        "in_msg": in_msg,
        "out_msgs": out_msgs,
        "block_ref": {
            "workchain": 0,
            "shard": "-9223372036854775808",
            "seqno": tx.mc_block_seqno,
        },
        "mc_block_seqno": tx.mc_block_seqno,
        "emulated": false,
        "trace_id": tx.hash.to_base64(),
        "trace_external_hash": tx.hash.to_base64(),
    })
}

fn map_v3_message(
    msg: &LiteNodeMessage,
    tx_hash: &crate::types::Hash256,
    tx_utime: u32,
    is_in_msg: bool,
) -> Value {
    let mut mapped = serde_json::json!({
        "hash": msg.hash.to_base64(),
        "hash_norm": msg.hash.to_base64(),
        "source": msg.source.as_ref().map(|a| a.to_string()),
        "destination": msg.destination.as_ref().map(|a| a.to_string()),
        "value": msg.value.to_string(),
        "value_extra_currencies": {},
        "fwd_fee": msg.fwd_fee.to_string(),
        "ihr_fee": msg.ihr_fee.to_string(),
        "import_fee": "0",
        "created_lt": msg.created_lt.to_string(),
        "created_at": tx_utime.to_string(),
        "bounce": false,
        "bounced": false,
        "ihr_disabled": true,
        "message_content": {
            "hash": msg.body_hash.to_base64(),
            "body": base64::engine::general_purpose::STANDARD.encode(&msg.body),
        },
    });

    if let Some(opcode) = msg.opcode
        && let Some(root) = mapped.as_object_mut()
    {
        root.insert("opcode".to_string(), Value::from(i64::from(opcode)));
    }

    if !msg.init_state.is_empty()
        && let Some(root) = mapped.as_object_mut()
    {
        root.insert(
            "init_state".to_string(),
            serde_json::json!({
                "hash": hash_boc_base64(&msg.init_state).unwrap_or_default(),
                "body": base64::engine::general_purpose::STANDARD.encode(&msg.init_state),
            }),
        );
    }

    if let Some(root) = mapped.as_object_mut() {
        if is_in_msg {
            root.insert(
                "in_msg_tx_hash".to_string(),
                Value::String(tx_hash.to_base64()),
            );
        } else {
            root.insert(
                "out_msg_tx_hash".to_string(),
                Value::String(tx_hash.to_base64()),
            );
        }
    }

    mapped
}

fn hash_boc_base64(boc: &crate::types::BocBytes) -> Option<String> {
    let cell = Boc::decode(boc).ok()?;
    Some(crate::types::Hash256(*cell.repr_hash().as_array()).to_base64())
}

fn map_jetton_wallet(w: &JettonWalletMeta) -> Value {
    serde_json::json!({
        "address": w.address.to_string(),
        "balance": w.balance.to_string(),
        "code_hash": w.code_hash.to_base64(),
        "data_hash": w.data_hash.to_base64(),
        "jetton": w.jetton_address.to_string(),
        "last_transaction_lt": w.last_transaction_lt.to_string(),
        "owner": w.owner_address.to_string(),
    })
}

pub fn map_traces(tn: &TraceNode) -> Value {
    let mut transactions = HashMap::new();
    let mut transactions_order = Vec::new();
    collect_transactions(tn, &mut transactions, &mut transactions_order);

    serde_json::json!({
        "address_book": {},
        "metadata": {},
        "traces": [
            map_trace(tn, &transactions, &transactions_order)
        ]
    })
}

pub fn map_emulate_trace_response(
    emulation: &EmulateTraceResult,
    with_actions: bool,
    include_code_data: bool,
    include_address_book: bool,
    include_metadata: bool,
) -> Value {
    let tn = &emulation.trace;
    let mapped = map_traces(tn);
    let trace_entry = mapped
        .get("traces")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    let mut response = serde_json::Map::new();
    response.insert(
        "mc_block_seqno".to_string(),
        serde_json::json!(tn.transaction.meta.block_seqno),
    );
    response.insert(
        "trace".to_string(),
        trace_entry
            .get("trace")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );
    response.insert(
        "transactions".to_string(),
        trace_entry
            .get("transactions")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    );

    if with_actions {
        response.insert(
            "actions".to_string(),
            trace_entry
                .get("actions")
                .cloned()
                .unwrap_or_else(|| serde_json::json!([])),
        );
    }

    if include_code_data {
        response.insert(
            "code_cells".to_string(),
            map_cells_by_hash_base64(&emulation.code_cells),
        );
        response.insert(
            "data_cells".to_string(),
            map_cells_by_hash_base64(&emulation.data_cells),
        );
    }

    if include_address_book {
        response.insert("address_book".to_string(), serde_json::json!({}));
    }

    if include_metadata {
        response.insert("metadata".to_string(), serde_json::json!({}));
    }

    response.insert("rand_seed".to_string(), serde_json::json!(""));
    response.insert(
        "is_incomplete".to_string(),
        trace_entry
            .get("is_incomplete")
            .cloned()
            .unwrap_or(Value::Bool(false)),
    );

    Value::Object(response)
}

fn map_cells_by_hash_base64(
    cells: &HashMap<crate::types::Hash256, crate::types::BocBytes>,
) -> Value {
    let mut entries = cells
        .iter()
        .map(|(hash, boc)| (hash.to_base64(), boc.to_base64()))
        .collect::<Vec<_>>();
    entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));

    let mut mapped = serde_json::Map::new();
    for (hash, boc) in entries {
        mapped.insert(hash, Value::String(boc));
    }

    Value::Object(mapped)
}

pub fn map_run_get_method_v3(result: &LiteNodeRunGetMethodResult) -> Value {
    let stack_cell = Boc::decode(&result.stack).unwrap_or_default();
    let stack_tuple = Tuple::deserialize(&stack_cell).unwrap_or_default();
    let stack = stack_to_json(&stack_tuple)
        .unwrap_or_default()
        .into_iter()
        .map(map_stack_entry)
        .collect::<Vec<_>>();

    serde_json::json!({
        "gas_used": result.gas_used,
        "exit_code": result.exit_code,
        "stack": stack,
        "vm_log": result.vm_log,
    })
}

fn collect_transactions(
    tn: &TraceNode,
    transactions: &mut HashMap<String, Value>,
    order: &mut Vec<String>,
) {
    let tx_hash = tn.transaction.meta.tx_hash.to_base64();
    if !transactions.contains_key(&tx_hash) {
        let mut tx_val = map_transaction(&tn.transaction);

        let child_lts: Vec<String> = tn
            .children
            .iter()
            .map(|c| c.transaction.meta.lt.to_string())
            .collect();

        if let Some(obj) = tx_val.as_object_mut() {
            obj.insert(
                "child_transactions".to_string(),
                serde_json::json!(child_lts),
            );
        }

        transactions.insert(tx_hash.clone(), tx_val);
        order.push(tx_hash);
    }
    for child in &tn.children {
        collect_transactions(child, transactions, order);
    }
}

fn map_trace(
    tn: &TraceNode,
    transactions: &HashMap<String, Value>,
    transactions_order: &[String],
) -> Value {
    serde_json::json!({
        "trace_id": tn.transaction.meta.tx_hash.to_base64(),
        "external_hash": tn.external_hash.as_ref().map(|h| h.to_base64()).unwrap_or_else(|| tn.transaction.meta.tx_hash.to_base64()),
        "mc_seqno_start": "0",
        "mc_seqno_end": "0",
        "start_lt": tn.transaction.meta.lt.to_string(),
        "start_utime": tn.transaction.meta.now,
        "end_lt": tn.max_lt().to_string(),
        "end_utime": tn.max_utime(),
        "is_incomplete": false,
        "trace": map_trace_node(tn),
        "transactions": transactions,
        "transactions_order": transactions_order,
        "actions": [],
        "trace_info": {
            "transactions": transactions.len(),
            "messages": transactions.len().saturating_sub(1) + tn.children.len(), // Approximation
            "pending_messages": 0,
            "trace_state": "complete",
            "classification_state": "classified"
        }
    })
}

fn map_trace_node(tn: &TraceNode) -> Value {
    serde_json::json!({
        "tx_hash": tn.transaction.meta.tx_hash.to_base64(),
        "in_msg_hash": tn.transaction.meta.in_msg_hash.as_ref().map(|h| h.to_base64()).unwrap_or_default(),
        "in_msg": tn.transaction.in_msg.as_ref().map(|m| map_message(&m.meta)),
        "transaction": map_transaction(&tn.transaction),
        "children": tn.children.iter().map(map_trace_node).collect::<Vec<_>>(),
    })
}

fn map_transaction(tx: &TransactionInfo) -> Value {
    let b64 = base64::engine::general_purpose::STANDARD;
    let raw_transaction = b64.encode(&tx.tx_boc);

    serde_json::json!({
        "account": tx.meta.account.to_string(),
        "hash": tx.meta.tx_hash.to_base64(),
        "lt": tx.meta.lt.to_string(),
        "now": tx.meta.now,
        "orig_status": "active",
        "end_status": "active",
        "total_fees": tx.meta.total_fees.unwrap_or(0).to_string(),
        "prev_trans_hash": zero_hash_base64(),
        "prev_trans_lt": "0",
        "description": {
            "type": "ord",
            "aborted": !tx.meta.success,
            "compute_ph": {
                "skipped": tx.meta.compute_exit_code.is_none(),
                "success": tx.meta.compute_exit_code == Some(0),
                "exit_code": tx.meta.compute_exit_code.unwrap_or(0),
            },
            "action": {
                "success": tx.meta.action_result_code == Some(0),
                "result_code": tx.meta.action_result_code.unwrap_or(0),
            }
        },
        "in_msg": tx.in_msg.as_ref().map(|m| map_message(&m.meta)),
        "out_msgs": tx.out_msgs.iter().map(|m| map_message(&m.meta)).collect::<Vec<_>>(),
        "block_ref": {
            "workchain": 0,
            "shard": "-9223372036854775808",
            "seqno": tx.meta.block_seqno
        },
        "mc_block_seqno": tx.meta.block_seqno,
        "raw_transaction": raw_transaction,
        "child_transactions": [],
    })
}

fn map_message(msg: &MsgMeta) -> Value {
    serde_json::json!({
        "hash": msg.msg_hash.to_base64(),
        "source": msg.src.as_ref().map(|a| a.to_string()),
        "destination": msg.dst.as_ref().map(|a| a.to_string()),
        "value": msg.value.unwrap_or(0).to_string(),
        "fwd_fee": "0",
        "ihr_fee": "0",
        "import_fee": "0",
        "created_lt": msg.created_lt.unwrap_or(0).to_string(),
        "created_at": msg.created_at.unwrap_or(0).to_string(),
        "bounce": msg.bounce.unwrap_or(false),
        "bounced": false,
        "message_content": {
            "hash": msg.msg_boc_hash.to_base64(),
            "body": "", // We don't have BOC here easily
        }
    })
}

fn map_stack_entry(entry: Value) -> Value {
    let Some(entry_type) = entry.get("@type").and_then(Value::as_str) else {
        return entry;
    };

    match entry_type {
        "tvm.stackEntryNull" => serde_json::json!({
            "type": "null",
            "value": Value::Null
        }),
        "tvm.stackEntryNumber" => serde_json::json!({
            "type": "num",
            "value": entry
                .pointer("/number/number")
                .cloned()
                .unwrap_or(Value::Null)
        }),
        "tvm.stackEntryCell" => serde_json::json!({
            "type": "cell",
            "value": entry.get("cell").cloned().unwrap_or(Value::Null)
        }),
        "tvm.stackEntrySlice" => serde_json::json!({
            "type": "slice",
            "value": entry.get("slice").cloned().unwrap_or(Value::Null)
        }),
        "tvm.stackEntryBuilder" => serde_json::json!({
            "type": "builder",
            "value": entry.get("builder").cloned().unwrap_or(Value::Null)
        }),
        "tvm.stackEntryTuple" => {
            let elements = entry
                .pointer("/tuple/elements")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .cloned()
                        .map(map_stack_entry)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            serde_json::json!({
                "type": "tuple",
                "value": elements
            })
        }
        _ => entry,
    }
}

fn encode_optional_boc(data: Option<&crate::types::BocBytes>) -> String {
    data.map(|c| base64::engine::general_purpose::STANDARD.encode(c))
        .unwrap_or_default()
}

fn zero_hash_base64() -> String {
    crate::types::Hash256([0; 32]).to_base64()
}

const fn map_account_status(status: &AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Uninit => "uninitialized",
        AccountStatus::Frozen => "frozen",
        AccountStatus::Nonexist => "uninitialized",
    }
}
