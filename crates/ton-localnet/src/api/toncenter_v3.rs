use crate::localnet::{
    LocalnetAccountState, LocalnetBlockTransactions, LocalnetMessage, LocalnetRunGetMethodResult,
    LocalnetTransaction, convert_to_message_struct,
};
use crate::storage::{
    AccountStatus, EmulateTraceResult, JettonMasterMeta, JettonWalletMeta, MessageInfo, MsgMeta,
    NftItemMeta, TraceNode, TransactionInfo,
};
use crate::types::Addr;
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
#[must_use]
pub fn map_jetton_wallets(wallets: &Vec<JettonWalletMeta>) -> Value {
    map_jetton_wallets_with_metadata(wallets, &HashMap::new())
}

pub fn map_jetton_wallets_with_metadata(
    wallets: &Vec<JettonWalletMeta>,
    masters_by_jetton: &HashMap<Addr, JettonMasterMeta>,
) -> Value {
    let mut token_info_by_address: HashMap<String, Vec<Value>> = HashMap::new();
    let mut master_info_added = std::collections::HashSet::new();

    for wallet in wallets {
        token_info_by_address
            .entry(wallet.address.to_string())
            .or_default()
            .push(map_jetton_wallet_token_info(wallet));

        if master_info_added.insert(wallet.jetton_address)
            && let Some(master) = masters_by_jetton.get(&wallet.jetton_address)
        {
            token_info_by_address
                .entry(master.address.to_string())
                .or_default()
                .push(map_jetton_master_token_info(master));
        }
    }

    let mut metadata = serde_json::Map::new();
    for (address, token_info) in token_info_by_address {
        metadata.insert(
            address,
            serde_json::json!({
                "is_indexed": true,
                "token_info": token_info,
            }),
        );
    }

    serde_json::json!({
        "address_book": {},
        "metadata": metadata,
        "jetton_wallets": wallets.iter().map(map_jetton_wallet).collect::<Vec<_>>()
    })
}

#[allow(clippy::ptr_arg)]
#[must_use]
pub fn map_nft_items(items: &Vec<NftItemMeta>) -> Value {
    map_nft_items_with_metadata(items)
}

pub fn map_nft_items_with_metadata(items: &Vec<NftItemMeta>) -> Value {
    let mut token_info_by_address: HashMap<String, Vec<Value>> = HashMap::new();
    let mut collection_info_added = std::collections::HashSet::new();

    for item in items {
        token_info_by_address
            .entry(item.address.to_string())
            .or_default()
            .push(map_nft_item_token_info(item));

        if let Some(collection_address) = item.collection_address
            && collection_info_added.insert(collection_address)
        {
            token_info_by_address
                .entry(collection_address.to_string())
                .or_default()
                .push(map_nft_collection_token_info(item));
        }
    }

    let mut metadata = serde_json::Map::new();
    for (address, token_info) in token_info_by_address {
        metadata.insert(
            address,
            serde_json::json!({
                "is_indexed": true,
                "token_info": token_info,
            }),
        );
    }

    serde_json::json!({
        "address_book": {},
        "metadata": metadata,
        "nft_items": items.iter().map(map_nft_item).collect::<Vec<_>>()
    })
}

pub struct AccountStateContext {
    pub interfaces: Vec<String>,
    pub token_info: Vec<Value>,
    pub user_friendly: String,
}

#[must_use]
pub fn map_account_states(
    states: &[LocalnetAccountState],
    context_by_address: &HashMap<Addr, AccountStateContext>,
    include_boc: bool,
) -> Value {
    let mut address_book = serde_json::Map::new();
    let mut metadata = serde_json::Map::new();

    for state in states {
        let default_user_friendly = state.address.to_string();
        let context = context_by_address.get(&state.address);
        let interfaces = context
            .map(|ctx| ctx.interfaces.clone())
            .unwrap_or_default();

        address_book.insert(
            state.address.to_string(),
            serde_json::json!({
                "user_friendly": context
                    .map_or(default_user_friendly, |ctx| ctx.user_friendly.clone()),
                "domain": Value::Null,
                "interfaces": interfaces,
            }),
        );

        if let Some(ctx) = context
            && !ctx.token_info.is_empty()
        {
            metadata.insert(
                state.address.to_string(),
                serde_json::json!({
                    "is_indexed": true,
                    "token_info": ctx.token_info.clone(),
                }),
            );
        }
    }

    serde_json::json!({
        "accounts": states
            .iter()
            .map(|state| map_account_state_full(state, context_by_address.get(&state.address), include_boc))
            .collect::<Vec<_>>(),
        "address_book": address_book,
        "metadata": metadata,
    })
}

#[must_use]
pub fn map_address_information(state: &LocalnetAccountState) -> Value {
    serde_json::json!({
        "balance": state.balance.to_string(),
        "code": encode_optional_boc(state.code.as_ref()),
        "data": encode_optional_boc(state.data.as_ref()),
        "frozen_hash": state.frozen_hash.as_ref().map(super::super::types::Hash256::to_base64).unwrap_or_default(),
        "last_transaction_hash": state.last_transaction_id.hash.to_base64(),
        "last_transaction_lt": state.last_transaction_id.lt.to_string(),
        "status": map_address_information_status(&state.state),
    })
}

#[must_use]
pub fn map_send_message(bt: &LocalnetBlockTransactions) -> Value {
    let message_hash = bt
        .msg_hash
        .as_ref()
        .map(super::super::types::Hash256::to_base64)
        .unwrap_or_default();
    let message_hash_norm = bt
        .msg_hash_norm
        .as_ref()
        .map(super::super::types::Hash256::to_base64)
        .unwrap_or_else(|| message_hash.clone());
    serde_json::json!({
        "message_hash": message_hash,
        "message_hash_norm": message_hash_norm,
    })
}

pub fn map_transactions_response(transactions: &[LocalnetTransaction]) -> Value {
    serde_json::json!({
        "address_book": {},
        "transactions": transactions.iter().map(map_v3_transaction).collect::<Vec<_>>()
    })
}

fn map_v3_transaction(tx: &LocalnetTransaction) -> Value {
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
    msg: &LocalnetMessage,
    tx_hash: &crate::types::Hash256,
    tx_utime: u32,
    is_in_msg: bool,
) -> Value {
    let mut mapped = serde_json::json!({
        "hash": msg.hash.to_base64(),
        "source": msg.source.as_ref().map(ToString::to_string),
        "destination": msg.destination.as_ref().map(ToString::to_string),
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

    if let Some(hash_norm) = msg
        .hash_norm
        .as_ref()
        .map(super::super::types::Hash256::to_base64)
        && let Some(root) = mapped.as_object_mut()
    {
        root.insert("hash_norm".to_string(), Value::String(hash_norm));
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

pub(crate) fn map_jetton_wallet_token_info(wallet: &JettonWalletMeta) -> Value {
    serde_json::json!({
        "valid": true,
        "type": "jetton_wallets",
        "extra": {
            "owner": wallet.owner_address.to_string(),
            "jetton": wallet.jetton_address.to_string(),
            "balance": wallet.balance.to_string(),
        }
    })
}

pub(crate) fn map_jetton_master_token_info(master: &JettonMasterMeta) -> Value {
    let mut mapped = serde_json::Map::new();
    mapped.insert("valid".to_string(), Value::Bool(true));
    mapped.insert(
        "type".to_string(),
        Value::String("jetton_masters".to_string()),
    );

    if let Some(name) = master
        .jetton_content
        .get("name")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    {
        mapped.insert("name".to_string(), Value::String(name));
    }
    if let Some(symbol) = master
        .jetton_content
        .get("symbol")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    {
        mapped.insert("symbol".to_string(), Value::String(symbol));
    }
    if let Some(description) = master
        .jetton_content
        .get("description")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    {
        mapped.insert("description".to_string(), Value::String(description));
    }
    if let Some(image) = master
        .jetton_content
        .get("image")
        .and_then(Value::as_str)
        .map(ToString::to_string)
    {
        mapped.insert("image".to_string(), Value::String(image));
    }

    mapped.insert("extra".to_string(), master.jetton_content.clone());
    Value::Object(mapped)
}

fn map_nft_item(item: &NftItemMeta) -> Value {
    let collection = item
        .collection_address
        .as_ref()
        .map_or(Value::Null, |address| {
            serde_json::json!({
                "address": address.to_string(),
            })
        });

    serde_json::json!({
        "address": item.address.to_string(),
        "auction_contract_address": Value::Null,
        "code_hash": item.code_hash.to_base64(),
        "collection": collection,
        "collection_address": item.collection_address.as_ref().map(ToString::to_string),
        "content": item.content,
        "data_hash": item.data_hash.to_base64(),
        "index": item.index,
        "init": item.init,
        "last_transaction_lt": item.last_transaction_lt.to_string(),
        "on_sale": false,
        "owner_address": item.owner_address.as_ref().map(ToString::to_string),
        "real_owner": item.owner_address.as_ref().map(ToString::to_string),
        "sale_contract_address": Value::Null,
    })
}

pub(crate) fn map_nft_item_token_info(item: &NftItemMeta) -> Value {
    let mut mapped = serde_json::Map::new();
    mapped.insert("valid".to_string(), Value::Bool(true));
    mapped.insert("type".to_string(), Value::String("nft_items".to_string()));
    mapped.insert("nft_index".to_string(), Value::String(item.index.clone()));

    if let Some(name) = content_string(&item.content, "name") {
        mapped.insert("name".to_string(), Value::String(name));
    }
    if let Some(symbol) = content_string(&item.content, "symbol") {
        mapped.insert("symbol".to_string(), Value::String(symbol));
    }
    if let Some(description) = content_string(&item.content, "description") {
        mapped.insert("description".to_string(), Value::String(description));
    }
    if let Some(image) = content_string(&item.content, "image") {
        mapped.insert("image".to_string(), Value::String(image));
    }

    mapped.insert("extra".to_string(), item.content.clone());
    Value::Object(mapped)
}

pub(crate) fn map_nft_collection_token_info(item: &NftItemMeta) -> Value {
    let mut mapped = serde_json::Map::new();
    mapped.insert("valid".to_string(), Value::Bool(true));
    mapped.insert(
        "type".to_string(),
        Value::String("nft_collections".to_string()),
    );

    if let Some(name) = content_string(&item.content, "collection_name") {
        mapped.insert("name".to_string(), Value::String(name));
    }
    if let Some(description) = content_string(&item.content, "collection_description") {
        mapped.insert("description".to_string(), Value::String(description));
    }
    if let Some(image) = content_string(&item.content, "collection_image") {
        mapped.insert("image".to_string(), Value::String(image));
    }

    mapped.insert("extra".to_string(), serde_json::json!({}));
    Value::Object(mapped)
}

fn map_account_state_full(
    state: &LocalnetAccountState,
    context: Option<&AccountStateContext>,
    include_boc: bool,
) -> Value {
    let mut mapped = serde_json::Map::new();
    mapped.insert(
        "account_state_hash".to_string(),
        Value::String(state.account_state_hash.to_base64()),
    );
    mapped.insert(
        "address".to_string(),
        Value::String(state.address.to_string()),
    );
    mapped.insert(
        "balance".to_string(),
        Value::String(state.balance.to_string()),
    );
    mapped.insert("contract_methods".to_string(), serde_json::json!([]));
    mapped.insert("extra_currencies".to_string(), serde_json::json!({}));
    mapped.insert(
        "interfaces".to_string(),
        serde_json::json!(
            context
                .map(|ctx| ctx.interfaces.clone())
                .unwrap_or_default()
        ),
    );
    mapped.insert(
        "last_transaction_hash".to_string(),
        Value::String(state.last_transaction_id.hash.to_base64()),
    );
    mapped.insert(
        "last_transaction_lt".to_string(),
        Value::String(state.last_transaction_id.lt.to_string()),
    );
    mapped.insert(
        "status".to_string(),
        Value::String(map_account_state_status(&state.state).to_string()),
    );

    if include_boc {
        if let Some(code) = state.code.as_ref() {
            mapped.insert(
                "code_boc".to_string(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(code)),
            );
        }
        if let Some(data) = state.data.as_ref() {
            mapped.insert(
                "data_boc".to_string(),
                Value::String(base64::engine::general_purpose::STANDARD.encode(data)),
            );
        }
    }

    if let Some(code_hash) = state.code_hash.as_ref() {
        mapped.insert(
            "code_hash".to_string(),
            Value::String(code_hash.to_base64()),
        );
    }
    if let Some(data_hash) = state.data_hash.as_ref() {
        mapped.insert(
            "data_hash".to_string(),
            Value::String(data_hash.to_base64()),
        );
    }
    if let Some(frozen_hash) = state.frozen_hash.as_ref() {
        mapped.insert(
            "frozen_hash".to_string(),
            Value::String(frozen_hash.to_base64()),
        );
    }

    Value::Object(mapped)
}

fn content_string(content: &Value, key: &str) -> Option<String> {
    content
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[must_use]
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
    address_book: Option<Value>,
    metadata: Option<Value>,
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

    if let Some(address_book) = address_book {
        response.insert("address_book".to_string(), address_book);
    }

    if let Some(metadata) = metadata {
        response.insert("metadata".to_string(), metadata);
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

pub fn map_run_get_method_v3(result: &LocalnetRunGetMethodResult) -> Value {
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
        "external_hash": tn.external_hash.as_ref().map_or_else(|| tn.transaction.meta.tx_hash.to_base64(), super::super::types::Hash256::to_base64),
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
        "in_msg_hash": tn.transaction.meta.in_msg_hash.as_ref().map(super::super::types::Hash256::to_base64).unwrap_or_default(),
        "in_msg": tn.transaction.in_msg.as_ref().map(|m| {
            map_trace_message_info(m, &tn.transaction.meta.tx_hash, tn.transaction.meta.now, true)
        }),
        "transaction": map_transaction(&tn.transaction),
        "children": tn.children.iter().map(map_trace_node).collect::<Vec<_>>(),
    })
}

fn map_transaction(tx: &TransactionInfo) -> Value {
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
        "in_msg": tx.in_msg.as_ref().map(|m| {
            map_trace_message_info(m, &tx.meta.tx_hash, tx.meta.now, true)
        }),
        "out_msgs": tx.out_msgs.iter().map(|m| {
            map_trace_message_info(m, &tx.meta.tx_hash, tx.meta.now, false)
        }).collect::<Vec<_>>(),
        "block_ref": {
            "workchain": 0,
            "shard": "-9223372036854775808",
            "seqno": tx.meta.block_seqno
        },
        "mc_block_seqno": tx.meta.block_seqno,
        "child_transactions": [],
    })
}

fn map_trace_message_info(
    msg: &MessageInfo,
    tx_hash: &crate::types::Hash256,
    tx_utime: u32,
    is_in_msg: bool,
) -> Value {
    convert_to_message_struct(&msg.meta, &msg.boc)
        .map(|message| map_v3_message(&message, tx_hash, tx_utime, is_in_msg))
        .unwrap_or_else(|_| map_message(&msg.meta))
}

fn map_message(msg: &MsgMeta) -> Value {
    serde_json::json!({
        "hash": msg.msg_hash.to_base64(),
        "source": msg.src.as_ref().map(ToString::to_string),
        "destination": msg.dst.as_ref().map(ToString::to_string),
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

const fn map_address_information_status(status: &AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Uninit | AccountStatus::Nonexist => "uninitialized",
        AccountStatus::Frozen => "frozen",
    }
}

const fn map_account_state_status(status: &AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Uninit => "uninit",
        AccountStatus::Frozen => "frozen",
        AccountStatus::Nonexist => "nonexist",
    }
}

#[cfg(test)]
mod tests {
    use super::{map_nft_collection_token_info, map_nft_item_token_info};
    use crate::storage::NftItemMeta;
    use crate::types::Hash256;
    use serde_json::json;

    fn sample_nft_item() -> NftItemMeta {
        NftItemMeta {
            address: "0:1111111111111111111111111111111111111111111111111111111111111111"
                .parse()
                .expect("valid item address"),
            code_hash: Hash256([1; 32]),
            data_hash: Hash256([2; 32]),
            collection_address: Some(
                "0:2222222222222222222222222222222222222222222222222222222222222222"
                    .parse()
                    .expect("valid collection address"),
            ),
            owner_address: Some(
                "0:3333333333333333333333333333333333333333333333333333333333333333"
                    .parse()
                    .expect("valid owner address"),
            ),
            content: json!({
                "name": "Sample NFT",
                "description": "Sample NFT description",
                "image": "https://example.com/nft.png",
                "symbol": "SNFT",
                "collection_name": "Sample Collection",
                "collection_description": "Collection description",
                "collection_image": "https://example.com/collection.png",
            }),
            index: "7".to_string(),
            init: true,
            last_transaction_lt: 42,
        }
    }

    #[test]
    fn nft_item_token_info_uses_nft_items_type() {
        let token_info = map_nft_item_token_info(&sample_nft_item());

        assert_eq!(token_info["type"].as_str(), Some("nft_items"));
        assert_eq!(token_info["nft_index"].as_str(), Some("7"));
        assert_eq!(token_info["name"].as_str(), Some("Sample NFT"));
    }

    #[test]
    fn nft_collection_token_info_uses_nft_collections_type() {
        let token_info = map_nft_collection_token_info(&sample_nft_item());

        assert_eq!(token_info["type"].as_str(), Some("nft_collections"));
        assert_eq!(token_info["name"].as_str(), Some("Sample Collection"));
        assert_eq!(
            token_info["description"].as_str(),
            Some("Collection description")
        );
    }
}
