use crate::localnet::{
    LocalnetAcceptedExternalMessage, LocalnetAcceptedInternalMessage, LocalnetAccountState,
    LocalnetAddressInfo, LocalnetBlockHeader, LocalnetBlockId, LocalnetBlockTransactions,
    LocalnetConsensusBlock, LocalnetLibrary, LocalnetMasterchainInfo, LocalnetRunGetMethodResult,
    LocalnetTransaction, LocalnetTransactionId,
};
use crate::storage::{AccountStatus, NftItemMeta};
use crate::types::{Addr, BocBytes, Hash256};
use base64::Engine;
use serde_json::value::Value;
use tvm_ffi::json_stack::{legacy_stack_to_json, stack_to_json};
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::HashBytes as CellHashBytes;
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
        "address": { "@type": "accountAddress", "account_address": tx.address.to_string() },
        "account": tx.address.to_string(),
        "utime": tx.utime,
        "data": tx.data.to_base64(),
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
        "data": tx.data.to_base64(),
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
        "@type": "ext.message",
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
            "body": msg.body.to_base64(),
            "init_state": msg.init_state.to_base64()
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
            "body": msg.body.to_base64(),
            "init_state": msg.init_state.to_base64()
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
        "frozen_hash": s.frozen_hash.as_ref().map(Hash256::to_base64).unwrap_or_default(),
        "sync_utime": s.sync_utime,
        "state": match s.state {
            AccountStatus::Active => "active",
            AccountStatus::Uninit | AccountStatus::Nonexist => "uninitialized",
            AccountStatus::Frozen => "frozen",
            // there is no nonexist in toncenter v2
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
                "frozen_hash": s.frozen_hash.as_ref().map(Hash256::to_base64).unwrap_or_default()
            }),
        },
        "revision": 0
    })
}

#[must_use]
pub fn wallet_type_name_from_code_hash(code_hash: Option<&Hash256>) -> Option<&'static str> {
    let code_hash = code_hash?;
    let wallet_type = ton_indexer::categorize_wallet(CellHashBytes(code_hash.0));
    match wallet_type {
        ton_indexer::WalletType::Unknown
        | ton_indexer::WalletType::WalletHighloadV1R1
        | ton_indexer::WalletType::WalletHighloadV1R2
        | ton_indexer::WalletType::WalletHighloadV2
        | ton_indexer::WalletType::WalletHighloadV2R1
        | ton_indexer::WalletType::WalletHighloadV2R2
        | ton_indexer::WalletType::WalletHighloadV3R1
        | ton_indexer::WalletType::WalletPreprocessedV2
        | ton_indexer::WalletType::WalletVesting => None,
        ton_indexer::WalletType::WalletV1R1 => Some("wallet v1 r1"),
        ton_indexer::WalletType::WalletV1R2 => Some("wallet v1 r2"),
        ton_indexer::WalletType::WalletV1R3 => Some("wallet v1 r3"),
        ton_indexer::WalletType::WalletV2R1 => Some("wallet v2 r1"),
        ton_indexer::WalletType::WalletV2R2 => Some("wallet v2 r2"),
        ton_indexer::WalletType::WalletV3R1 => Some("wallet v3 r1"),
        ton_indexer::WalletType::WalletV3R2 => Some("wallet v3 r2"),
        ton_indexer::WalletType::WalletV4R1 => Some("wallet v4 r1"),
        ton_indexer::WalletType::WalletV4R2 => Some("wallet v4 r2"),
        ton_indexer::WalletType::WalletV5Beta => Some("wallet v5 beta"),
        ton_indexer::WalletType::WalletV5R1 => Some("wallet v5 r1"),
    }
}

#[must_use]
pub fn map_wallet_seqno(result: &LocalnetRunGetMethodResult) -> Option<u32> {
    if result.exit_code != 0 {
        return None;
    }

    let stack_cell = Boc::decode(&result.stack).ok()?;
    let stack = Tuple::deserialize(&stack_cell)
        .ok()?
        .unwrap_single()
        .unwrap_tuple();
    let Some(TupleItem::Int(value)) = stack.first() else {
        return None;
    };
    value.to_str_radix(10).parse().ok()
}

#[must_use]
pub fn map_wallet_information(s: &LocalnetAccountState, seqno: Option<u32>) -> Value {
    let wallet_type = wallet_type_name_from_code_hash(s.code_hash.as_ref());
    let mut mapped = serde_json::Map::new();
    mapped.insert(
        "@type".to_string(),
        Value::String("ext.accounts.walletInformation".to_string()),
    );
    mapped.insert("wallet".to_string(), Value::Bool(wallet_type.is_some()));
    mapped.insert("balance".to_string(), Value::String(s.balance.to_string()));
    mapped.insert("extra_currencies".to_string(), serde_json::json!([]));
    mapped.insert(
        "account_state".to_string(),
        Value::String(
            match s.state {
                AccountStatus::Active => "active",
                AccountStatus::Uninit | AccountStatus::Nonexist => "uninitialized",
                AccountStatus::Frozen => "frozen",
            }
            .to_string(),
        ),
    );
    mapped.insert(
        "last_transaction_id".to_string(),
        map_internal_transaction_id(&s.last_transaction_id),
    );

    if let Some(wallet_type) = wallet_type {
        mapped.insert(
            "wallet_type".to_string(),
            Value::String(wallet_type.to_string()),
        );
        if let Some(seqno) = seqno {
            mapped.insert("seqno".to_string(), serde_json::json!(seqno));
        }
    }

    Value::Object(mapped)
}

#[must_use]
pub fn map_token_data(
    info: &LocalnetAddressInfo,
    jetton_wallet_code: Option<&BocBytes>,
    collection_next_item_index: Option<&str>,
) -> Option<Value> {
    if let Some(master) = info.jetton_master.as_ref() {
        return Some(serde_json::json!({
            "@type": "ext.tokens.jettonMasterData",
            "address": master.address.to_string(),
            "contract_type": "jetton_master",
            "total_supply": master.total_supply.to_string(),
            "mintable": master.mintable,
            "admin_address": master.admin_address.as_ref().map(ToString::to_string),
            "jetton_content": map_token_content(&master.jetton_content),
            "jetton_wallet_code": jetton_wallet_code.map(BocBytes::to_base64).unwrap_or_default(),
        }));
    }

    if let Some(wallet) = info.jetton_wallet.as_ref() {
        return Some(serde_json::json!({
            "@type": "ext.tokens.jettonWalletData",
            "address": wallet.address.to_string(),
            "contract_type": "jetton_wallet",
            "balance": wallet.balance.to_string(),
            "owner": wallet.owner_address.to_string(),
            "jetton": wallet.jetton_address.to_string(),
            "jetton_wallet_code": jetton_wallet_code.map(BocBytes::to_base64).unwrap_or_default(),
        }));
    }

    if let Some(item) = info.nft_collection_item.as_ref() {
        return Some(map_nft_collection_data(
            info.address.to_string(),
            item,
            collection_next_item_index,
        ));
    }

    info.nft_item.as_ref().map(map_nft_item_data)
}

fn map_nft_collection_data(
    address: String,
    item: &NftItemMeta,
    next_item_index: Option<&str>,
) -> Value {
    serde_json::json!({
        "@type": "ext.tokens.nftCollectionData",
        "address": address,
        "contract_type": "nft_collection",
        "next_item_index": next_item_index.unwrap_or(&item.index),
        "owner_address": item.owner_address.as_ref().map(ToString::to_string),
        "collection_content": map_token_content(&map_collection_content(&item.content)),
    })
}

fn map_nft_item_data(item: &NftItemMeta) -> Value {
    serde_json::json!({
        "@type": "ext.tokens.nftItemData",
        "address": item.address.to_string(),
        "contract_type": "nft_item",
        "init": item.init,
        "index": item.index,
        "collection_address": item.collection_address.as_ref().map(ToString::to_string),
        "owner_address": item.owner_address.as_ref().map(ToString::to_string),
        "content": map_token_content(&item.content),
    })
}

fn map_collection_content(content: &Value) -> Value {
    let Some(source) = content.as_object() else {
        return content.clone();
    };

    let mut mapped = serde_json::Map::new();
    for (from, to) in [
        ("collection_uri", "uri"),
        ("collection_name", "name"),
        ("collection_description", "description"),
        ("collection_image", "image"),
    ] {
        if let Some(value) = source.get(from) {
            mapped.insert(to.to_string(), value.clone());
        }
    }

    if mapped.is_empty() {
        content.clone()
    } else {
        Value::Object(mapped)
    }
}

fn map_token_content(content: &Value) -> Value {
    let Some(map) = content.as_object() else {
        return serde_json::json!({
            "type": "onchain",
            "data": content,
        });
    };

    if map.len() == 1
        && let Some(uri) = map.get("uri").and_then(Value::as_str)
    {
        return serde_json::json!({
            "type": "offchain",
            "data": uri,
        });
    }

    serde_json::json!({
        "type": "onchain",
        "data": content,
    })
}

#[must_use]
pub fn map_shard_account_cell(boc: &BocBytes) -> Value {
    serde_json::json!({
        "@type": "tvm.cell",
        "bytes": boc.to_base64()
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

#[must_use]
pub fn map_send_boc(_: &LocalnetAcceptedExternalMessage) -> Value {
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
                    "data": data.to_base64(),
                })
            })
            .collect::<Vec<_>>()
    })
}

#[must_use]
pub fn map_send_boc_return_hash(message: &LocalnetAcceptedExternalMessage) -> Value {
    serde_json::json!({
        "@type": "ok",
        "hash": message.msg_hash.to_base64(),
        "hash_norm": message.msg_hash_norm.to_base64(),
    })
}

#[must_use]
pub fn map_send_internal_message(message: &LocalnetAcceptedInternalMessage) -> Value {
    serde_json::json!({
        "@type": "ok",
        "hash": message.msg_hash.to_base64()
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
            "bytes": config.to_base64(),
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
pub fn map_detect_hash(hash: &Hash256) -> Value {
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
    data.map(BocBytes::to_base64).unwrap_or_default()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::JettonMasterMeta;
    use serde_json::json;

    fn addr(hex_byte: u8) -> Addr {
        format!("0:{}", format!("{hex_byte:02x}").repeat(32))
            .parse()
            .expect("valid address")
    }

    fn account_state(code_hash: Option<Hash256>) -> LocalnetAccountState {
        LocalnetAccountState {
            address: addr(0x11),
            account_state_hash: Hash256([0x22; 32]),
            balance: 123,
            code: None,
            code_hash,
            data: None,
            data_hash: None,
            last_transaction_id: LocalnetTransactionId {
                lt: 42,
                hash: Hash256([0x33; 32]),
            },
            block_id: LocalnetBlockId::first(),
            state: AccountStatus::Active,
            sync_utime: 0,
            frozen_hash: None,
        }
    }

    #[test]
    fn wallet_information_maps_known_wallet_code_hash() {
        let wallet_v4r2_hash = Hash256::from_base64("/rX/aCDi/w2Ug+fg1iyBfYRniftK5YDIeIZtlZ2r1cA=")
            .expect("valid wallet hash");
        let mapped = map_wallet_information(&account_state(Some(wallet_v4r2_hash)), Some(7));

        assert_eq!(
            mapped["@type"].as_str(),
            Some("ext.accounts.walletInformation")
        );
        assert_eq!(mapped["wallet"].as_bool(), Some(true));
        assert_eq!(mapped["wallet_type"].as_str(), Some("wallet v4 r2"));
        assert_eq!(mapped["seqno"].as_u64(), Some(7));
        assert_eq!(mapped["balance"].as_str(), Some("123"));
        assert_eq!(mapped["account_state"].as_str(), Some("active"));
    }

    #[test]
    fn wallet_information_maps_unknown_wallet_code_hash() {
        let mapped = map_wallet_information(&account_state(Some(Hash256([0x44; 32]))), None);

        assert_eq!(mapped["wallet"].as_bool(), Some(false));
        assert!(mapped.get("wallet_type").is_none());
        assert!(mapped.get("seqno").is_none());
    }

    #[test]
    fn wallet_seqno_parses_success_stack() {
        let stack = Tuple(vec![TupleItem::Int(9.into())])
            .serialize()
            .expect("stack must serialize");
        let result = LocalnetRunGetMethodResult {
            gas_used: 0,
            stack: BocBytes::from(Boc::encode(stack)),
            exit_code: 0,
            vm_log: "".into(),
            block_id: LocalnetBlockId::first(),
            last_transaction_id: LocalnetTransactionId::default(),
        };

        assert_eq!(map_wallet_seqno(&result), Some(9));
    }

    #[test]
    fn token_data_maps_jetton_master_with_wallet_code() {
        let wallet_code = BocBytes(vec![1, 2, 3]);
        let master = JettonMasterMeta {
            address: addr(0xaa),
            admin_address: Some(addr(0xbb)),
            code_hash: Hash256([1; 32]),
            data_hash: Hash256([2; 32]),
            jetton_content: json!({
                "name": "Local Token",
                "symbol": "LOC",
                "decimals": "9",
            }),
            jetton_wallet_code_hash: Hash256([3; 32]),
            last_transaction_lt: 4,
            mintable: true,
            total_supply: 1000,
        };
        let info = LocalnetAddressInfo {
            address: master.address,
            code_hash: Some(master.code_hash),
            jetton_wallet: None,
            jetton_master: Some(master),
            nft_item: None,
            nft_collection_item: None,
        };

        let mapped = map_token_data(&info, Some(&wallet_code), None).expect("jetton data must map");

        assert_eq!(
            mapped["@type"].as_str(),
            Some("ext.tokens.jettonMasterData")
        );
        assert_eq!(mapped["contract_type"].as_str(), Some("jetton_master"));
        assert_eq!(mapped["total_supply"].as_str(), Some("1000"));
        assert_eq!(mapped["jetton_wallet_code"].as_str(), Some("AQID"));
        assert_eq!(mapped["jetton_content"]["type"].as_str(), Some("onchain"));
        assert_eq!(
            mapped["jetton_content"]["data"]["symbol"].as_str(),
            Some("LOC")
        );
    }
}
