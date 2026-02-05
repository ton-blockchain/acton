use crate::litenode::{
    LiteNodeAccountState, LiteNodeBlockHeader, LiteNodeBlockId, LiteNodeBlockTransactions,
    LiteNodeMasterchainInfo, LiteNodeRunGetMethodResult, LiteNodeTransaction,
};
use crate::storage::AccountStatus;
use crate::types::BocBytes;
use base64::Engine;
use serde_json::value::Value;
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;
use tvmffi::json_stack::{legacy_stack_to_json, stack_to_json};
use tvmffi::stack::Tuple;

pub fn map_block_id(id: &LiteNodeBlockId) -> Value {
    serde_json::json!({
        "@type": "ton.blockIdExt",
        "workchain": id.workchain,
        "shard": id.shard.to_string(),
        "seqno": id.seqno,
        "root_hash": id.root_hash.to_hex(),
        "file_hash": id.file_hash.to_hex()
    })
}

pub fn map_transaction(tx: &LiteNodeTransaction) -> Value {
    serde_json::json!({
        "@type": "ext.transaction",
        "hash": tx.hash.to_hex(),
        "address": { "@type": "accountAddress", "account_address": tx.address.to_string() },
        "account": tx.address.to_string(),
        "utime": tx.utime,
        "data": base64::engine::general_purpose::STANDARD.encode(&tx.data),
        "success": tx.success,
        "exit_code": tx.exit_code,
        "transaction_id": {
            "@type": "internal.transactionId",
            "lt": tx.transaction_id.lt.to_string(),
            "hash": tx.transaction_id.hash.to_hex()
        },
        "fee": "0",
        "storage_fee": "0",
        "other_fee": "0",
        "in_msg": map_message(&tx.in_msg),
        "out_msgs": tx.out_msgs.iter().map(map_message).collect::<Vec<_>>()
    })
}

pub fn map_message(msg: &crate::litenode::LiteNodeMessage) -> Value {
    if msg.hash.0 == [0; 32] {
        return serde_json::json!({ "@type": "msg.message" });
    }
    serde_json::json!({
        "@type": "raw.message",
        "hash": msg.hash.to_hex(),
        "opcode": msg.opcode.map(|op| format!("0x{:08x}", op)),
        "source": {
            "@type": "accountAddress",
            "account_address": msg.source.as_ref().map(|a| a.to_string()).unwrap_or_default()
        },
        "destination": {
            "@type": "accountAddress",
            "account_address": msg.destination.as_ref().map(|a| a.to_string()).unwrap_or_default()
        },
        "value": msg.value.to_string(),
        "fwd_fee": "0",
        "ihr_fee": "0",
        "created_lt": "0",
        "body_hash": msg.body_hash.to_hex(),
        "msg_data": {
            "@type": "msg.dataRaw",
            "body": base64::engine::general_purpose::STANDARD.encode(&msg.body),
            "init_state": base64::engine::general_purpose::STANDARD.encode(&msg.init_state)
        },
        "extra_currencies": []
    })
}

pub fn map_account_state(s: &LiteNodeAccountState) -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "raw.fullAccountState",
            "balance": s.balance.to_string(),
            "extra_currencies": [],
            "last_transaction_id": {
                "@type": "internal.transactionId",
                "lt": s.last_transaction_id.lt.to_string(),
                "hash": s.last_transaction_id.hash.to_hex()
            },
            "block_id": map_block_id(&s.block_id),
            "code": encode_optional_boc(s.code.as_ref()),
            "data": encode_optional_boc(s.data.as_ref()),
            "frozen_hash": "0000000000000000000000000000000000000000000000000000000000000000", // TODO
            "sync_utime": s.sync_utime,
            "state": match s.state {
                AccountStatus::Active => "active",
                AccountStatus::Uninit => "uninit",
                AccountStatus::Frozen => "frozen",
                AccountStatus::Nonexist => "nonexist",
            }
        }
    })
}

pub fn map_extended_account_state(s: &LiteNodeAccountState) -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "fullAccountState",
            "address": { "@type": "accountAddress", "account_address": s.address.to_string() },
            "balance": s.balance.to_string(),
            "extra_currencies": [],
            "last_transaction_id": {
                "@type": "internal.transactionId",
                "lt": s.last_transaction_id.lt.to_string(),
                "hash": s.last_transaction_id.hash.to_hex()
            },
            "block_id": map_block_id(&s.block_id),
            "sync_utime": s.sync_utime,
            "account_state": match s.state {
                AccountStatus::Nonexist => serde_json::json!({
                    "@type": "uninited.accountState",
                    "frozen_hash": "0000000000000000000000000000000000000000000000000000000000000000"
                }),
                _ => serde_json::json!({
                    "@type": "raw.accountState",
                    "code": encode_optional_boc(s.code.as_ref()),
                    "data": encode_optional_boc(s.data.as_ref()),
                    "frozen_hash": "0000000000000000000000000000000000000000000000000000000000000000"
                }),
            },
            "revision": 0
        }
    })
}

pub fn map_run_get_method(r: &LiteNodeRunGetMethodResult, is_legacy: bool) -> Value {
    let stack_cell = ArcCell::from_boc(&r.stack).unwrap_or_default();
    let stack_tuple = Tuple::deserialize(&stack_cell).unwrap_or_default();
    let stack_json: Value = if is_legacy {
        Value::Array(legacy_stack_to_json(&stack_tuple).unwrap_or_default())
    } else {
        Value::Array(stack_to_json(&stack_tuple).unwrap_or_default())
    };

    let stack = match stack_json {
        Value::Array(a) => a,
        v => vec![v],
    };

    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "smc.runResult",
            "gas_used": r.gas_used,
            "stack": stack,
            "exit_code": r.exit_code,
            "vm_log": r.vm_log,
            "block_id": map_block_id(&r.block_id),
            "last_transaction_id": {
                "@type": "internal.transactionId",
                "lt": r.last_transaction_id.lt.to_string(),
                "hash": r.last_transaction_id.hash.to_hex()
            },
        }
    })
}

pub fn map_block_transactions(bt: &LiteNodeBlockTransactions) -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "blocks.transactions",
            "id": map_block_id(&bt.id),
            "req_count": bt.transactions.len(),
            "incomplete": false,
            "transactions": bt.transactions.iter().map(|tx| {
                serde_json::json!({
                    "@type": "blocks.shortTxId",
                    "mode": 0,
                    "account": tx.address.to_string(),
                    "lt": tx.transaction_id.lt.to_string(),
                    "hash": tx.transaction_id.hash.to_base64()
                })
            }).collect::<Vec<_>>()
        }
    })
}

pub fn map_block_transactions_ext(bt: &LiteNodeBlockTransactions) -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "blocks.transactionsExt",
            "id": map_block_id(&bt.id),
            "req_count": bt.transactions.len(),
            "incomplete": false,
            "transactions": bt.transactions.iter().map(map_transaction).collect::<Vec<_>>()
        }
    })
}

pub fn map_masterchain_info(mi: &LiteNodeMasterchainInfo) -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "blocks.masterchainInfo",
            "last": map_block_id(&mi.last),
            "state_root_hash": mi.state_root_hash.to_hex(),
            "init": map_block_id(&mi.init)
        }
    })
}

pub fn map_send_boc_return_hash(bt: &LiteNodeBlockTransactions) -> Value {
    let hash = bt
        .transactions
        .first()
        .map(|tx| tx.hash.to_base64())
        .unwrap_or_default();
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "ok",
            "hash": hash
        }
    })
}

pub fn map_block_header(bh: &LiteNodeBlockHeader) -> Value {
    serde_json::json!({
        "ok": true,
        "result": {
            "@type": "ton.blockHeader",
            "id": map_block_id(&bh.id),
            "gen_utime": bh.gen_utime,
            "start_lt": bh.start_lt.to_string(),
            "end_lt": bh.end_lt.to_string(),
            "prev_seqno": bh.prev_seqno
        }
    })
}

fn encode_optional_boc(data: Option<&BocBytes>) -> String {
    data.map(|c| base64::engine::general_purpose::STANDARD.encode(c))
        .unwrap_or_default()
}
