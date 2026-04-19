use crate::localnet::{
    LocalnetAccountState, LocalnetBlockHeader, LocalnetBlockId, LocalnetBlockTransactions,
    LocalnetConsensusBlock, LocalnetLibrary, LocalnetMasterchainInfo, LocalnetRunGetMethodResult,
    LocalnetTransaction, LocalnetTransactionId,
};
use crate::storage::AccountStatus;
use crate::types::{Addr, BocBytes};
use base64::Engine;
use serde_json::value::Value;
use tvmffi::json_stack::{legacy_stack_to_json, stack_to_json};
use tvmffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StdAddr};

#[must_use]
pub fn map_block_id(id: &LocalnetBlockId) -> Value {
    serde_json::json!({
        "@type": "ton.blockIdExt",
        "workchain": id.workchain,
        "shard": id.shard.to_string(),
        "seqno": id.seqno,
        "root_hash": id.root_hash.to_base64(),
        "file_hash": id.file_hash.to_base64()
    })
}

#[allow(clippy::ptr_arg)]
pub fn map_transactions(txs: &Vec<LocalnetTransaction>) -> Value {
    txs.iter().map(map_transaction).collect::<Vec<_>>().into()
}

pub fn map_transactions_std(txs: &[LocalnetTransaction], limit: usize) -> Value {
    let (txs_to_return, previous_id) = if txs.len() > limit {
        (
            txs[..limit].to_vec(),
            txs.get(limit)
                .map(|tx| tx.transaction_id.clone())
                .unwrap_or_default(),
        )
    } else {
        (txs.to_vec(), LocalnetTransactionId::default())
    };

    serde_json::json!({
        "@type": "raw.transactions",
        "transactions": txs_to_return
            .iter()
            .map(map_transaction_std)
            .collect::<Vec<_>>(),
        "previous_transaction_id": map_internal_transaction_id(&previous_id)
    })
}

pub fn map_transaction(tx: &LocalnetTransaction) -> Value {
    serde_json::json!({
        "@type": "ext.transaction",
        "hash": tx.hash.to_base64(),
        "address": { "@type": "accountAddress", "account_address": tx.address.to_string() },
        "account": tx.address.to_string(),
        "utime": tx.utime,
        "data": base64::engine::general_purpose::STANDARD.encode(&tx.data),
        "success": tx.success,
        "exit_code": tx.exit_code,
        "transaction_id": map_internal_transaction_id(&tx.transaction_id),
        "fee": tx.total_fees.to_string(),
        "storage_fee": tx.storage_fees.to_string(),
        "other_fee": tx.other_fees.to_string(),
        "in_msg": map_message(&tx.in_msg),
        "out_msgs": tx.out_msgs.iter().map(map_message).collect::<Vec<_>>()
    })
}

pub fn map_transaction_std(tx: &LocalnetTransaction) -> Value {
    serde_json::json!({
        "@type": "raw.transaction",
        "address": map_account_address(&tx.address),
        "utime": tx.utime,
        "data": base64::engine::general_purpose::STANDARD.encode(&tx.data),
        "transaction_id": map_internal_transaction_id(&tx.transaction_id),
        "fee": tx.total_fees.to_string(),
        "storage_fee": tx.storage_fees.to_string(),
        "other_fee": tx.other_fees.to_string(),
        "in_msg": map_message_std(&tx.in_msg),
        "out_msgs": tx.out_msgs.iter().map(map_message_std).collect::<Vec<_>>()
    })
}

#[must_use]
pub fn map_message(msg: &crate::localnet::LocalnetMessage) -> Value {
    if msg.hash.0 == [0; 32] {
        return serde_json::json!({ "@type": "msg.message" });
    }
    serde_json::json!({
        "@type": "raw.message",
        "hash": msg.hash.to_base64(),
        "opcode": msg.opcode.map(|op| format!("0x{op:08x}")),
        "source": msg.source.as_ref().map(ToString::to_string).unwrap_or_default(),
        "destination": msg.destination.as_ref().map(ToString::to_string).unwrap_or_default(),
        "value": msg.value.to_string(),
        "fwd_fee": msg.fwd_fee.to_string(),
        "ihr_fee": msg.ihr_fee.to_string(),
        "created_lt": msg.created_lt.to_string(),
        "body_hash": msg.body_hash.to_base64(),
        "msg_data": {
            "@type": "msg.dataRaw",
            "body": base64::engine::general_purpose::STANDARD.encode(&msg.body),
            "init_state": base64::engine::general_purpose::STANDARD.encode(&msg.init_state)
        },
        "extra_currencies": []
    })
}

#[must_use]
pub fn map_message_std(msg: &crate::localnet::LocalnetMessage) -> Value {
    if msg.hash.0 == [0; 32] {
        return serde_json::json!({ "@type": "msg.message" });
    }
    serde_json::json!({
        "@type": "raw.message",
        "hash": msg.hash.to_base64(),
        "source": map_optional_account_address(msg.source.as_ref()),
        "destination": map_optional_account_address(msg.destination.as_ref()),
        "value": msg.value.to_string(),
        "fwd_fee": msg.fwd_fee.to_string(),
        "ihr_fee": msg.ihr_fee.to_string(),
        "created_lt": msg.created_lt.to_string(),
        "body_hash": msg.body_hash.to_base64(),
        "msg_data": {
            "@type": "msg.dataRaw",
            "body": base64::engine::general_purpose::STANDARD.encode(&msg.body),
            "init_state": base64::engine::general_purpose::STANDARD.encode(&msg.init_state)
        },
        "extra_currencies": []
    })
}

#[must_use]
pub fn map_account_state(s: &LocalnetAccountState) -> Value {
    serde_json::json!({
        "@type": "raw.fullAccountState",
        "balance": s.balance.to_string(),
        "extra_currencies": [],
        "last_transaction_id": map_internal_transaction_id(&s.last_transaction_id),
        "block_id": map_block_id(&s.block_id),
        "code": encode_optional_boc(s.code.as_ref()),
        "data": encode_optional_boc(s.data.as_ref()),
        "frozen_hash": s.frozen_hash.as_ref().map(super::super::types::Hash256::to_base64).unwrap_or_default(),
        "sync_utime": s.sync_utime,
        "state": match s.state {
            AccountStatus::Active => "active",
            AccountStatus::Uninit => "uninitialized",
            AccountStatus::Frozen => "frozen",
            AccountStatus::Nonexist => "uninitialized", // there is no nonexist in toncenter v2
        }
    })
}

#[must_use]
pub fn map_extended_account_state(s: &LocalnetAccountState) -> Value {
    serde_json::json!({
        "@type": "fullAccountState",
        "address": { "@type": "accountAddress", "account_address": s.address.to_string() },
        "balance": s.balance.to_string(),
        "extra_currencies": [],
        "last_transaction_id": map_internal_transaction_id(&s.last_transaction_id),
        "block_id": map_block_id(&s.block_id),
        "sync_utime": s.sync_utime,
        "account_state": match s.state {
            AccountStatus::Nonexist => serde_json::json!({
                "@type": "uninited.accountState",
                "frozen_hash": ""
            }),
            _ => serde_json::json!({
                "@type": "raw.accountState",
                "code": encode_optional_boc(s.code.as_ref()),
                "data": encode_optional_boc(s.data.as_ref()),
                "frozen_hash": s.frozen_hash.as_ref().map(super::super::types::Hash256::to_base64).unwrap_or_default()
            }),
        },
        "revision": 0
    })
}

#[must_use]
pub fn map_run_get_method(r: &LocalnetRunGetMethodResult, is_legacy: bool) -> Value {
    let stack_cell = Boc::decode(&r.stack).unwrap_or_default();
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
        "@type": "smc.runResult",
        "gas_used": r.gas_used,
        "stack": stack,
        "exit_code": r.exit_code,
        "vm_log": r.vm_log,
        "block_id": map_block_id(&r.block_id),
        "last_transaction_id": map_internal_transaction_id(&r.last_transaction_id),
    })
}

#[must_use]
pub fn map_block_transactions(_: &LocalnetBlockTransactions) -> Value {
    serde_json::json!({
      "@type": "ok",
    })
}

pub fn map_block_transactions_ext(bt: &LocalnetBlockTransactions) -> Value {
    serde_json::json!({
        "@type": "blocks.transactionsExt",
        "id": map_block_id(&bt.id),
        "req_count": bt.transactions.len(),
        "incomplete": false,
        "transactions": bt.transactions.iter().map(map_transaction).collect::<Vec<_>>()
    })
}

#[must_use]
pub fn map_masterchain_info(mi: &LocalnetMasterchainInfo) -> Value {
    serde_json::json!({
        "@type": "blocks.masterchainInfo",
        "last": map_block_id(&mi.last),
        "state_root_hash": mi.state_root_hash.to_base64(),
        "init": map_block_id(&mi.init)
    })
}

#[must_use]
pub fn map_consensus_block(cb: &LocalnetConsensusBlock) -> Value {
    serde_json::json!({
        "@type": "ext.blocks.consensusBlock",
        "consensus_block": cb.consensus_block,
        "timestamp": cb.timestamp
    })
}

#[must_use]
pub fn map_libraries(libs: &[LocalnetLibrary]) -> Value {
    serde_json::json!({
        "@type": "smc.libraryResult",
        "result": libs
            .iter()
            .filter_map(|lib| lib.data.as_ref().map(|data| (lib, data)))
            .map(|(lib, data)| {
                serde_json::json!({
                    "@type": "smc.libraryEntry",
                    "hash": lib.hash.to_base64(),
                    "data": base64::engine::general_purpose::STANDARD.encode(data),
                })
            })
            .collect::<Vec<_>>()
    })
}

#[must_use]
pub fn map_send_boc_return_hash(bt: &LocalnetBlockTransactions) -> Value {
    let msg_hash = bt
        .msg_hash
        .as_ref()
        .map(super::super::types::Hash256::to_base64)
        .unwrap_or_default();
    serde_json::json!({
        "@type": "ok",
        "hash": msg_hash
    })
}

#[must_use]
pub fn map_block_header(bh: &LocalnetBlockHeader) -> Value {
    serde_json::json!({
        "@type": "ton.blockHeader",
        "id": map_block_id(&bh.id),
        "gen_utime": bh.gen_utime,
        "start_lt": bh.start_lt.to_string(),
        "end_lt": bh.end_lt.to_string(),
        "prev_seqno": bh.prev_seqno
    })
}

#[allow(clippy::ptr_arg)]
pub fn map_shards(shards: &Vec<LocalnetBlockId>) -> Value {
    serde_json::json!({
        "@type": "blocks.shards",
        "shards": shards.iter().map(map_block_id).collect::<Vec<_>>()
    })
}

#[must_use]
pub fn map_lookup_block(id: &LocalnetBlockId) -> Value {
    map_block_id(id)
}

#[must_use]
pub fn map_config_info(config: &BocBytes) -> Value {
    serde_json::json!({
        "@type": "configInfo",
        "config": {
            "@type": "tvm.cell",
            "bytes": base64::engine::general_purpose::STANDARD.encode(config),
        }
    })
}

#[must_use]
pub fn map_out_msg_queue_sizes(mi: &LocalnetMasterchainInfo) -> Value {
    serde_json::json!({
        "@type": "blocks.outMsgQueueSizes",
        "shards": [{
            "@type": "blocks.outMsgQueueSize",
            "id": map_block_id(&mi.last),
            "size": 0
        }],
        "ext_msg_queue_size_limit": 0
    })
}

#[must_use]
pub fn map_detect_address(addr: &StdAddr, flags: Base64StdAddrFlags, given_type: &str) -> Value {
    let bounceable_b64 = DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: flags.testnet,
            base64_url: false,
            bounceable: true,
        },
    }
    .to_string();
    let bounceable_b64url = DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: flags.testnet,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string();

    let non_bounceable_b64 = DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: flags.testnet,
            base64_url: false,
            bounceable: false,
        },
    }
    .to_string();
    let non_bounceable_b64url = DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: flags.testnet,
            base64_url: true,
            bounceable: false,
        },
    }
    .to_string();

    serde_json::json!({
        "@type": "ext.utils.detectedAddress",
        "raw_form": addr.to_string(),
        "bounceable": {
            "@type": "ext.utils.detectedAddressVariant",
            "b64": bounceable_b64,
            "b64url": bounceable_b64url,
        },
        "non_bounceable": {
            "@type": "ext.utils.detectedAddressVariant",
            "b64": non_bounceable_b64,
            "b64url": non_bounceable_b64url,
        },
        "given_type": given_type,
        "test_only": flags.testnet
    })
}

#[must_use]
pub fn map_detect_hash(hash: &crate::types::Hash256) -> Value {
    serde_json::json!({
        "@type": "ext.utils.detectedHash",
        "b64": hash.to_base64(),
        "b64url": base64::engine::general_purpose::URL_SAFE.encode(hash.0),
        "hex": hash.to_hex(),
    })
}

#[must_use]
pub fn map_pack_address(addr: &StdAddr, test_only: bool) -> Value {
    DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: test_only,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string()
    .into()
}

#[must_use]
pub fn map_unpack_address(addr: &StdAddr) -> Value {
    addr.to_string().into()
}

fn encode_optional_boc(data: Option<&BocBytes>) -> String {
    data.map(|c| base64::engine::general_purpose::STANDARD.encode(c))
        .unwrap_or_default()
}

fn map_internal_transaction_id(id: &LocalnetTransactionId) -> Value {
    serde_json::json!({
        "@type": "internal.transactionId",
        "lt": id.lt.to_string(),
        "hash": id.hash.to_base64()
    })
}

fn map_account_address(addr: &Addr) -> Value {
    serde_json::json!({
        "@type": "accountAddress",
        "account_address": addr.to_string()
    })
}

fn map_optional_account_address(addr: Option<&Addr>) -> Value {
    serde_json::json!({
        "@type": "accountAddress",
        "account_address": addr.map(ToString::to_string).unwrap_or_default()
    })
}
