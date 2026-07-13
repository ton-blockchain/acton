//! Localnet-to-`TonCenter` v2 response adapters.
//!
//! Known `OpenAPI` deviations:
//! - `map_run_get_method` adds local `vm_log` to the legacy `RunGetMethodResult`;
//! - `map_consensus_block` and internal-message responses are Acton extensions, not v2 `OpenAPI`
//!   operations.

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
use ton_api::toncenter::v2 as response;
use tvm_ffi::json_stack::{legacy_stack_to_json, std_stack_from_tuple};
use tvm_ffi::stack::{Tuple, TupleItem};
use tycho_types::boc::Boc;
use tycho_types::cell::HashBytes as CellHashBytes;
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StdAddr};

#[must_use]
pub fn map_block_id(id: &LocalnetBlockId) -> response::TonBlockIdExt {
    response::TonBlockIdExt {
        type_field: "ton.blockIdExt".to_owned(),
        workchain: id.workchain,
        shard: id.shard.to_string(),
        seqno: u64::from(id.seqno),
        root_hash: id.root_hash.to_base64(),
        file_hash: id.file_hash.to_base64(),
    }
}

pub fn map_transactions(txs: &[LocalnetTransaction]) -> Vec<response::Transaction> {
    txs.iter().map(map_transaction).collect()
}

pub fn map_transactions_std(
    txs: &[LocalnetTransaction],
    limit: usize,
) -> response::RawTransactions {
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

    response::RawTransactions {
        type_field: "raw.transactions".to_owned(),
        transactions: txs_to_return.iter().map(map_transaction_std).collect(),
        previous_transaction_id: map_internal_transaction_id(&previous_id),
    }
}

pub fn map_transaction(tx: &LocalnetTransaction) -> response::Transaction {
    response::Transaction {
        type_field: "ext.transaction".to_owned(),
        address: map_account_address(&tx.address),
        account: tx.address.to_string(),
        utime: u64::from(tx.utime),
        data: tx.data.to_base64(),
        transaction_id: map_internal_transaction_id(&tx.transaction_id),
        fee: tx.total_fees.to_string(),
        storage_fee: tx.storage_fees.to_string(),
        other_fee: tx.other_fees.to_string(),
        in_msg: map_message(&tx.in_msg),
        out_msgs: tx.out_msgs.iter().filter_map(map_message).collect(),
    }
}

pub fn map_transaction_std(tx: &LocalnetTransaction) -> response::RawTransaction {
    response::RawTransaction {
        type_field: "raw.transaction".to_owned(),
        address: map_account_address(&tx.address),
        utime: u64::from(tx.utime),
        data: tx.data.to_base64(),
        transaction_id: map_internal_transaction_id(&tx.transaction_id),
        fee: tx.total_fees.to_string(),
        storage_fee: tx.storage_fees.to_string(),
        other_fee: tx.other_fees.to_string(),
        in_msg: map_message_std(&tx.in_msg),
        out_msgs: tx.out_msgs.iter().filter_map(map_message_std).collect(),
    }
}

pub fn map_transaction_ext(tx: &LocalnetTransaction) -> response::TransactionExt {
    response::TransactionExt {
        type_field: "raw.transactionExt".to_owned(),
        address: map_account_address(&tx.address),
        account: tx.address.to_string(),
        utime: u64::from(tx.utime),
        data: tx.data.to_base64(),
        transaction_id: map_internal_transaction_id(&tx.transaction_id),
        fee: tx.total_fees.to_string(),
        storage_fee: tx.storage_fees.to_string(),
        other_fee: tx.other_fees.to_string(),
        in_msg: map_message_std(&tx.in_msg),
        out_msgs: tx.out_msgs.iter().filter_map(map_message_std).collect(),
    }
}

#[must_use]
pub fn map_message(msg: &crate::localnet::LocalnetMessage) -> Option<response::Message> {
    if msg.hash.is_zero() {
        return None;
    }
    Some(response::Message::Full(Box::new(response::MessageFull {
        hash: msg.hash.to_base64(),
        source: msg
            .source
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        destination: msg
            .destination
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default(),
        value: msg.value.to_string(),
        fwd_fee: msg.fwd_fee.to_string(),
        ihr_fee: msg.ihr_fee.to_string(),
        created_lt: msg.created_lt.to_string(),
        body_hash: msg.body_hash.to_base64(),
        msg_data: map_message_data(msg),
        extra_currencies: Vec::new(),
    })))
}

#[must_use]
pub fn map_message_std(msg: &crate::localnet::LocalnetMessage) -> Option<response::MessageStd> {
    if msg.hash.is_zero() {
        return None;
    }
    Some(response::MessageStd {
        type_field: "raw.message".to_owned(),
        hash: msg.hash.to_base64(),
        source: map_optional_account_address(msg.source.as_ref()),
        destination: map_optional_account_address(msg.destination.as_ref()),
        value: msg.value.to_string(),
        fwd_fee: msg.fwd_fee.to_string(),
        ihr_fee: msg.ihr_fee.to_string(),
        created_lt: msg.created_lt.to_string(),
        body_hash: msg.body_hash.to_base64(),
        msg_data: map_message_data(msg),
        extra_currencies: Vec::new(),
    })
}

#[must_use]
pub fn map_account_state(s: &LocalnetAccountState) -> response::AddressInformation {
    response::AddressInformation {
        type_field: "raw.fullAccountState".to_owned(),
        balance: response::StringOrNumber::String(s.balance.to_string()),
        extra_currencies: Vec::new(),
        last_transaction_id: map_internal_transaction_id(&s.last_transaction_id),
        block_id: map_block_id(&s.block_id),
        code: encode_optional_boc(s.code.as_ref()),
        data: encode_optional_boc(s.data.as_ref()),
        frozen_hash: s
            .frozen_hash
            .as_ref()
            .map(Hash256::to_base64)
            .unwrap_or_default(),
        sync_utime: s.sync_utime,
        state: map_account_status(&s.state).to_owned(),
        suspended: false,
    }
}

#[must_use]
pub const fn map_account_status(status: &AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Uninit | AccountStatus::Nonexist => "uninitialized",
        AccountStatus::Frozen => "frozen",
    }
}

#[must_use]
pub fn map_extended_account_state(
    s: &LocalnetAccountState,
) -> response::ExtendedAddressInformation {
    response::ExtendedAddressInformation {
        type_field: "fullAccountState".to_owned(),
        address: map_account_address(&s.address),
        balance: s.balance.to_string(),
        extra_currencies: Vec::new(),
        last_transaction_id: map_internal_transaction_id(&s.last_transaction_id),
        block_id: map_block_id(&s.block_id),
        sync_utime: s.sync_utime,
        account_state: match s.state {
            AccountStatus::Nonexist => response::AccountStateKind::Uninited {
                frozen_hash: String::new(),
            },
            _ => response::AccountStateKind::Raw {
                code: encode_optional_boc(s.code.as_ref()),
                data: encode_optional_boc(s.data.as_ref()),
                frozen_hash: s
                    .frozen_hash
                    .as_ref()
                    .map(Hash256::to_base64)
                    .unwrap_or_default(),
            },
        },
        revision: 0,
    }
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
pub fn map_wallet_information(
    s: &LocalnetAccountState,
    seqno: Option<u32>,
) -> response::WalletInformation {
    let wallet_type = wallet_type_name_from_code_hash(s.code_hash.as_ref());
    response::WalletInformation {
        type_field: "ext.accounts.walletInformation".to_owned(),
        wallet: wallet_type.is_some(),
        balance: s.balance.to_string(),
        extra_currencies: Vec::new(),
        account_state: map_account_status(&s.state).to_owned(),
        last_transaction_id: map_internal_transaction_id(&s.last_transaction_id),
        wallet_type: wallet_type.map(ToOwned::to_owned),
        seqno: wallet_type.and(seqno),
    }
}

#[must_use]
pub fn map_token_data(
    info: &LocalnetAddressInfo,
    jetton_wallet_code: Option<&BocBytes>,
    collection_next_item_index: Option<&str>,
) -> Option<response::TokenData> {
    if let Some(master) = info.jetton_master.as_ref() {
        return Some(response::TokenData::JettonMaster {
            address: master.address.to_string(),
            contract_type: "jetton_master".to_owned(),
            total_supply: master.total_supply.to_string(),
            mintable: master.mintable,
            admin_address: master.admin_address.as_ref().map(ToString::to_string),
            jetton_content: map_token_content(&master.jetton_content),
            jetton_wallet_code: jetton_wallet_code
                .map(BocBytes::to_base64)
                .unwrap_or_default(),
        });
    }

    if let Some(wallet) = info.jetton_wallet.as_ref() {
        return Some(response::TokenData::JettonWallet {
            address: wallet.address.to_string(),
            contract_type: "jetton_wallet".to_owned(),
            balance: wallet.balance.to_string(),
            owner: wallet.owner_address.to_string(),
            jetton: wallet.jetton_address.to_string(),
            jetton_wallet_code: jetton_wallet_code
                .map(BocBytes::to_base64)
                .unwrap_or_default(),
        });
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
) -> response::TokenData {
    response::TokenData::NftCollection {
        address,
        contract_type: "nft_collection".to_owned(),
        next_item_index: next_item_index.unwrap_or(&item.index).to_owned(),
        owner_address: item.owner_address.as_ref().map(ToString::to_string),
        collection_content: map_token_content(&map_collection_content(&item.content)),
    }
}

fn map_nft_item_data(item: &NftItemMeta) -> response::TokenData {
    response::TokenData::NftItem {
        address: item.address.to_string(),
        contract_type: "nft_item".to_owned(),
        init: item.init,
        index: item.index.clone(),
        collection_address: item.collection_address.as_ref().map(ToString::to_string),
        owner_address: item.owner_address.as_ref().map(ToString::to_string),
        content: map_token_content(&item.content),
    }
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

fn map_token_content(content: &Value) -> response::TokenContent {
    let Some(map) = content.as_object() else {
        return response::TokenContent {
            kind: "onchain".to_owned(),
            data: content.clone(),
        };
    };

    if map.len() == 1
        && let Some(uri) = map.get("uri").and_then(Value::as_str)
    {
        return response::TokenContent {
            kind: "offchain".to_owned(),
            data: Value::String(uri.to_owned()),
        };
    }

    response::TokenContent {
        kind: "onchain".to_owned(),
        data: content.clone(),
    }
}

#[must_use]
pub fn map_shard_account_cell(boc: &BocBytes) -> response::TvmCell {
    response::TvmCell {
        type_field: "tvm.cell".to_owned(),
        bytes: boc.to_base64(),
    }
}

pub const MAX_RUN_GET_METHOD_STACK_DEPTH: usize = 100;

#[derive(Debug, thiserror::Error)]
#[error("Result stack depth >= {MAX_RUN_GET_METHOD_STACK_DEPTH}")]
pub struct RunGetMethodStackDepthError;

pub fn map_run_get_method(
    r: &LocalnetRunGetMethodResult,
) -> anyhow::Result<response::RunGetMethodResult> {
    let stack = decode_run_get_method_stack(r)?;
    ensure_legacy_stack_depth(&stack)?;

    Ok(response::RunGetMethodResult {
        type_field: "smc.runResult".to_owned(),
        gas_used: response::StringOrNumber::Unsigned(r.gas_used),
        stack: legacy_stack_to_json(&stack)?,
        exit_code: r.exit_code,
        block_id: map_block_id(&r.block_id),
        last_transaction_id: map_internal_transaction_id(&r.last_transaction_id),
        vm_log: Some(r.vm_log.to_string()),
    })
}

pub fn map_run_get_method_std(
    r: &LocalnetRunGetMethodResult,
) -> anyhow::Result<response::RunGetMethodStdResult> {
    let stack = decode_run_get_method_stack(r)?;
    ensure_std_stack_depth(&stack)?;

    Ok(response::RunGetMethodStdResult {
        type_field: "smc.runResult".to_owned(),
        gas_used: i64::try_from(r.gas_used)?,
        stack: std_stack_from_tuple(&stack),
        exit_code: r.exit_code,
    })
}

fn decode_run_get_method_stack(r: &LocalnetRunGetMethodResult) -> anyhow::Result<Tuple> {
    let stack_cell = Boc::decode(&r.stack)?;
    Tuple::deserialize(&stack_cell)
}

fn ensure_std_stack_depth(stack: &Tuple) -> anyhow::Result<()> {
    ensure_stack_depth(stack.0.iter().map(|item| (item, 1)))
}

fn ensure_legacy_stack_depth(stack: &Tuple) -> anyhow::Result<()> {
    ensure_stack_depth(stack.0.iter().flat_map(|item| match item {
        TupleItem::Tuple(tuple) => tuple.0.iter().map(|item| (item, 1)).collect(),
        _ => Vec::new(),
    }))
}

fn ensure_stack_depth<'a>(
    entries: impl IntoIterator<Item = (&'a TupleItem, usize)>,
) -> anyhow::Result<()> {
    let mut pending = entries.into_iter().collect::<Vec<_>>();
    while let Some((entry, depth)) = pending.pop() {
        if depth >= MAX_RUN_GET_METHOD_STACK_DEPTH {
            return Err(RunGetMethodStackDepthError.into());
        }
        if let TupleItem::Tuple(tuple) = entry {
            pending.extend(tuple.0.iter().map(|item| (item, depth + 1)));
        }
    }
    Ok(())
}

#[must_use]
pub fn map_block_transactions(block: &LocalnetBlockTransactions) -> response::BlockTransactions {
    response::BlockTransactions {
        type_field: "blocks.transactions".to_owned(),
        id: map_block_id(&block.id),
        req_count: block.requested_count,
        incomplete: block.incomplete,
        transactions: block
            .transactions
            .iter()
            .map(|transaction| response::ShortTxId {
                type_field: "blocks.shortTxId".to_owned(),
                mode: 7,
                account: transaction.address.to_string(),
                lt: transaction.transaction_id.lt.to_string(),
                hash: transaction.hash.to_base64(),
            })
            .collect(),
    }
}

#[must_use]
pub fn map_send_boc(_: &LocalnetAcceptedExternalMessage) -> response::ResultOk {
    response::ResultOk {
        type_field: "ok".to_owned(),
    }
}

pub fn map_block_transactions_ext(
    bt: &LocalnetBlockTransactions,
) -> response::BlockTransactionsExt {
    response::BlockTransactionsExt {
        type_field: "blocks.transactionsExt".to_owned(),
        id: map_block_id(&bt.id),
        req_count: bt.requested_count,
        incomplete: bt.incomplete,
        transactions: bt.transactions.iter().map(map_transaction_ext).collect(),
    }
}

#[must_use]
pub fn map_masterchain_info(mi: &LocalnetMasterchainInfo) -> response::MasterchainInfo {
    response::MasterchainInfo {
        type_field: "blocks.masterchainInfo".to_owned(),
        last: map_block_id(&mi.last),
        state_root_hash: mi.state_root_hash.to_base64(),
        init: map_block_id(&mi.init),
    }
}

#[must_use]
pub fn map_consensus_block(cb: &LocalnetConsensusBlock) -> response::ConsensusBlock {
    response::ConsensusBlock {
        type_field: "ext.blocks.consensusBlock".to_owned(),
        consensus_block: cb.consensus_block,
        timestamp: cb.timestamp,
    }
}

#[must_use]
pub fn map_libraries(libs: &[LocalnetLibrary]) -> response::LibraryResult {
    response::LibraryResult {
        type_field: "smc.libraryResult".to_owned(),
        result: libs
            .iter()
            .filter_map(|lib| lib.data.as_ref().map(|data| (lib, data)))
            .map(|(lib, data)| response::LibraryEntry {
                type_field: "smc.libraryEntry".to_owned(),
                hash: lib.hash.to_base64(),
                data: data.to_base64(),
            })
            .collect(),
    }
}

#[must_use]
pub fn map_send_boc_return_hash(
    message: &LocalnetAcceptedExternalMessage,
) -> response::ExtMessageInfo {
    response::ExtMessageInfo {
        type_field: "raw.extMessageInfo".to_owned(),
        hash: message.msg_hash.to_base64(),
        hash_norm: message.msg_hash_norm.to_base64(),
    }
}

#[must_use]
pub fn map_send_internal_message(
    message: &LocalnetAcceptedInternalMessage,
) -> response::InternalMessageInfo {
    response::InternalMessageInfo {
        type_field: "ok".to_owned(),
        hash: message.msg_hash.to_base64(),
    }
}

#[must_use]
pub fn map_block_header(bh: &LocalnetBlockHeader) -> response::BlockHeader {
    response::BlockHeader {
        type_field: "blocks.header".to_owned(),
        id: map_block_id(&bh.id),
        global_id: 0,
        version: 0,
        after_merge: false,
        after_split: false,
        before_split: false,
        want_merge: false,
        want_split: false,
        validator_list_hash_short: 0,
        catchain_seqno: 0,
        min_ref_mc_seqno: 0,
        is_key_block: false,
        prev_key_block_seqno: bh.prev_seqno.unwrap_or_default() as i32,
        start_lt: bh.start_lt.to_string(),
        end_lt: bh.end_lt.to_string(),
        gen_utime: bh.gen_utime,
        prev_blocks: Vec::new(),
    }
}

pub fn map_shards(shards: &[LocalnetBlockId]) -> response::Shards {
    response::Shards {
        type_field: "blocks.shards".to_owned(),
        shards: shards.iter().map(map_block_id).collect(),
    }
}

#[must_use]
pub fn map_lookup_block(id: &LocalnetBlockId) -> response::TonBlockIdExt {
    map_block_id(id)
}

#[must_use]
pub fn map_config_info(config: &BocBytes) -> response::ConfigInfo {
    response::ConfigInfo {
        type_field: "configInfo".to_owned(),
        config: response::TvmCell {
            type_field: "tvm.cell".to_owned(),
            bytes: config.to_base64(),
        },
    }
}

#[must_use]
pub fn map_out_msg_queue_sizes(mi: &LocalnetMasterchainInfo) -> response::OutMsgQueueSizes {
    response::OutMsgQueueSizes {
        type_field: "blocks.outMsgQueueSizes".to_owned(),
        shards: vec![response::OutMsgQueueSize {
            type_field: "blocks.outMsgQueueSize".to_owned(),
            id: map_block_id(&mi.last),
            size: 0,
        }],
        ext_msg_queue_size_limit: 0,
    }
}

#[must_use]
pub fn map_detect_address(
    addr: &StdAddr,
    flags: Base64StdAddrFlags,
    given_type: &str,
) -> response::DetectAddress {
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

    response::DetectAddress {
        type_field: "ext.utils.detectedAddress".to_owned(),
        raw_form: addr.to_string(),
        bounceable: response::DetectAddressBase64Variant {
            type_field: "ext.utils.detectedAddressVariant".to_owned(),
            b64: bounceable_b64,
            b64url: bounceable_b64url,
        },
        non_bounceable: response::DetectAddressBase64Variant {
            type_field: "ext.utils.detectedAddressVariant".to_owned(),
            b64: non_bounceable_b64,
            b64url: non_bounceable_b64url,
        },
        given_type: given_type.to_owned(),
        test_only: flags.testnet,
    }
}

#[must_use]
pub fn map_detect_hash(hash: &Hash256) -> response::DetectHash {
    response::DetectHash {
        type_field: "ext.utils.detectedHash".to_owned(),
        b64: hash.to_base64(),
        b64url: base64::engine::general_purpose::URL_SAFE.encode(hash.0),
        hex: hash.to_hex(),
    }
}

#[must_use]
pub fn map_pack_address(addr: &StdAddr, test_only: bool) -> String {
    DisplayBase64StdAddr {
        addr,
        flags: Base64StdAddrFlags {
            testnet: test_only,
            base64_url: true,
            bounceable: true,
        },
    }
    .to_string()
}

#[must_use]
pub fn map_unpack_address(addr: &StdAddr) -> String {
    addr.to_string()
}

fn encode_optional_boc(data: Option<&BocBytes>) -> String {
    data.map(BocBytes::to_base64).unwrap_or_default()
}

fn map_internal_transaction_id(id: &LocalnetTransactionId) -> response::InternalTransactionId {
    response::InternalTransactionId {
        type_field: "internal.transactionId".to_owned(),
        lt: id.lt.to_string(),
        hash: id.hash.to_base64(),
    }
}

fn map_account_address(addr: &Addr) -> response::AccountAddress {
    response::AccountAddress {
        type_field: "accountAddress".to_owned(),
        account_address: addr.to_string(),
    }
}

fn map_optional_account_address(addr: Option<&Addr>) -> response::AccountAddress {
    response::AccountAddress {
        type_field: "accountAddress".to_owned(),
        account_address: addr.map(ToString::to_string).unwrap_or_default(),
    }
}

fn map_message_data(msg: &crate::localnet::LocalnetMessage) -> response::MessageData {
    response::MessageData::Raw {
        body: msg.body.to_base64(),
        init_state: msg.init_state.to_base64(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::JettonMasterMeta;
    use serde_json::json;
    use tycho_types::cell::Cell;

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

    fn run_get_method_result(stack: Tuple) -> LocalnetRunGetMethodResult {
        let stack = stack.serialize().expect("stack must serialize");
        LocalnetRunGetMethodResult {
            gas_used: 17,
            stack: BocBytes::from(Boc::encode(stack)),
            exit_code: 0,
            vm_log: "vm log".into(),
            block_id: LocalnetBlockId::first(),
            last_transaction_id: LocalnetTransactionId::default(),
        }
    }

    fn nested_tuple_item(depth: usize) -> TupleItem {
        if depth == 1 {
            TupleItem::Int(1.into())
        } else {
            TupleItem::Tuple(Tuple(vec![nested_tuple_item(depth - 1)]))
        }
    }

    #[test]
    fn wallet_information_maps_known_wallet_code_hash() {
        let wallet_v4r2_hash = Hash256::from_base64("/rX/aCDi/w2Ug+fg1iyBfYRniftK5YDIeIZtlZ2r1cA=")
            .expect("valid wallet hash");
        let mapped = map_wallet_information(&account_state(Some(wallet_v4r2_hash)), Some(7));

        assert_eq!(mapped.type_field, "ext.accounts.walletInformation");
        assert!(mapped.wallet);
        assert_eq!(mapped.wallet_type.as_deref(), Some("wallet v4 r2"));
        assert_eq!(mapped.seqno, Some(7));
        assert_eq!(mapped.balance, "123");
        assert_eq!(mapped.account_state, "active");
    }

    #[test]
    fn wallet_information_maps_unknown_wallet_code_hash() {
        let mapped = map_wallet_information(&account_state(Some(Hash256([0x44; 32]))), None);

        assert!(!mapped.wallet);
        assert!(mapped.wallet_type.is_none());
        assert!(mapped.seqno.is_none());
    }

    #[test]
    fn wallet_seqno_parses_success_stack() {
        let result = run_get_method_result(Tuple(vec![TupleItem::Int(9.into())]));

        assert_eq!(map_wallet_seqno(&result), Some(9));
    }

    #[test]
    fn run_get_method_std_maps_canonical_response() {
        let cell = Cell::default();
        let result = run_get_method_result(Tuple(vec![
            TupleItem::Int((-7).into()),
            TupleItem::Cell(cell.clone()),
            TupleItem::Slice(cell),
            TupleItem::Tuple(Tuple(vec![TupleItem::Int(9.into())])),
        ]));

        let mapped = map_run_get_method_std(&result).unwrap();

        assert_eq!(mapped.gas_used, 17);
        assert_eq!(mapped.exit_code, 0);
        assert_eq!(mapped.stack.len(), 4);
        assert_eq!(
            serde_json::to_value(&mapped.stack[0]).unwrap(),
            json!({
                "@type": "tvm.stackEntryNumber",
                "number": {"@type": "tvm.numberDecimal", "number": "-7"}
            })
        );
        assert!(
            serde_json::to_value(mapped)
                .unwrap()
                .get("block_id")
                .is_none()
        );
    }

    #[test]
    fn run_get_method_mappers_reject_invalid_stack_boc() {
        let mut result = run_get_method_result(Tuple::default());
        result.stack = BocBytes(vec![1, 2, 3]);

        assert!(map_run_get_method(&result).is_err());
        assert!(map_run_get_method_std(&result).is_err());
    }

    #[test]
    fn run_get_method_mappers_enforce_upstream_depth_limit() {
        let std_allowed = run_get_method_result(Tuple(vec![nested_tuple_item(99)]));
        let std_rejected = run_get_method_result(Tuple(vec![nested_tuple_item(100)]));
        let legacy_allowed = run_get_method_result(Tuple(vec![nested_tuple_item(100)]));
        let legacy_rejected = run_get_method_result(Tuple(vec![nested_tuple_item(101)]));

        assert!(map_run_get_method_std(&std_allowed).is_ok());
        assert!(
            map_run_get_method_std(&std_rejected)
                .unwrap_err()
                .downcast_ref::<RunGetMethodStackDepthError>()
                .is_some()
        );
        assert!(map_run_get_method(&legacy_allowed).is_ok());
        assert!(
            map_run_get_method(&legacy_rejected)
                .unwrap_err()
                .downcast_ref::<RunGetMethodStackDepthError>()
                .is_some()
        );
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

        let response::TokenData::JettonMaster {
            contract_type,
            total_supply,
            jetton_wallet_code,
            jetton_content,
            ..
        } = mapped
        else {
            panic!("expected jetton master token data");
        };
        assert_eq!(contract_type, "jetton_master");
        assert_eq!(total_supply, "1000");
        assert_eq!(jetton_wallet_code, "AQID");
        assert_eq!(jetton_content.kind, "onchain");
        assert_eq!(jetton_content.data["symbol"].as_str(), Some("LOC"));
    }
}
