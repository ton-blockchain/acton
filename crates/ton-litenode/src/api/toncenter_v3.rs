use crate::storage::{MsgMeta, TraceNode, TransactionInfo};
use base64::Engine;
use serde_json::value::Value;
use std::collections::HashMap;

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

fn collect_transactions(
    tn: &TraceNode,
    transactions: &mut HashMap<String, Value>,
    order: &mut Vec<String>,
) {
    let tx_hash = tn.transaction.meta.tx_hash.to_hex();
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
        "trace_id": tn.transaction.meta.tx_hash.to_hex(),
        "external_hash": tn.external_hash.as_ref().map(|h| h.to_hex()).unwrap_or_else(|| tn.transaction.meta.tx_hash.to_hex()),
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
        "tx_hash": tn.transaction.meta.tx_hash.to_hex(),
        "in_msg_hash": tn.transaction.meta.in_msg_hash.as_ref().map(|h| h.to_hex()).unwrap_or_default(),
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
        "hash": tx.meta.tx_hash.to_hex(),
        "lt": tx.meta.lt.to_string(),
        "now": tx.meta.now,
        "orig_status": "active",
        "end_status": "active",
        "total_fees": tx.meta.total_fees.unwrap_or(0).to_string(),
        "prev_trans_hash": "0000000000000000000000000000000000000000000000000000000000000000",
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
        "hash": msg.msg_hash.to_hex(),
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
            "hash": msg.msg_boc_hash.to_hex(),
            "body": "", // We don't have BOC here easily
        }
    })
}
