//! Localnet-to-`TonCenter` v3 response adapters.
//!
//! Known `OpenAPI` deviations:
//! - address-book and metadata objects contain only data derivable by localnet and may be empty;
//! - jetton and NFT metadata is a local projection and omits fields unavailable in local state;
//! - `map_run_get_method_v3` emits the observed result shape (`gas_used`, `exit_code`, `stack`,
//!   local `vm_log`); upstream v3 `OpenAPI` 1.2.6 incorrectly declares the request type as the
//!   successful response schema;
//! - emulation responses belong to `TonCenter`'s separate emulate API, not `/api/v3/doc.json`.

use crate::localnet::{
    LocalnetAcceptedExternalMessage, LocalnetAccountState, LocalnetBlock, LocalnetMessage,
    LocalnetRunGetMethodResult, LocalnetTransaction, convert_to_message_struct,
};
use crate::storage::{
    AccountStateSnapshot, AccountStatus, EmulateTraceResult, JettonMasterMeta, JettonWalletMeta,
    MessageInfo, MsgMeta, NftItemMeta, TraceNode, TransactionInfo,
};
use crate::types::{Addr, BocBytes, Hash256};
use serde_json::value::Value;
use std::collections::HashMap;
use ton_api::toncenter::emulate::v1 as emulate;
use ton_api::toncenter::v3 as response;
use tvm_ffi::json_stack::stack_to_json;
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::cell::HashBytes;
use tycho_types::models::{
    AccountStatusChange, ActionPhase, ComputePhase, ComputePhaseSkipReason, TxInfo,
};

pub fn map_jetton_masters(masters: &[JettonMasterMeta]) -> response::JettonMastersResponse {
    let mut metadata = response::Metadata::new();
    for master in masters {
        metadata.insert(
            master.address.to_string(),
            response::AddressMetadata {
                is_indexed: Some(true),
                token_info: vec![map_jetton_master_token_info(master)],
            },
        );
    }

    response::JettonMastersResponse {
        address_book: response::AddressBook::new(),
        metadata,
        jetton_masters: masters.iter().map(map_jetton_master).collect(),
    }
}

fn map_jetton_master(m: &JettonMasterMeta) -> response::JettonMaster {
    response::JettonMaster {
        address: m.address.to_string(),
        admin_address: m.admin_address.map(|address| address.to_string()),
        code_hash: Some(m.code_hash.to_base64()),
        data_hash: Some(m.data_hash.to_base64()),
        jetton_content: object_fields(&m.jetton_content),
        jetton_wallet_code_hash: Some(m.jetton_wallet_code_hash.to_base64()),
        last_transaction_lt: Some(m.last_transaction_lt.to_string()),
        mintable: Some(m.mintable),
        total_supply: Some(m.total_supply.to_string()),
    }
}

#[must_use]
pub fn map_jetton_wallets(wallets: &[JettonWalletMeta]) -> response::JettonWalletsResponse {
    map_jetton_wallets_with_metadata(wallets, &HashMap::new())
}

pub fn map_jetton_wallets_with_metadata(
    wallets: &[JettonWalletMeta],
    masters_by_jetton: &HashMap<Addr, JettonMasterMeta>,
) -> response::JettonWalletsResponse {
    let mut token_info_by_address: HashMap<String, Vec<response::TokenInfo>> = HashMap::new();
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

    let mut metadata = response::Metadata::new();
    for (address, token_info) in token_info_by_address {
        metadata.insert(
            address,
            response::AddressMetadata {
                is_indexed: Some(true),
                token_info,
            },
        );
    }

    response::JettonWalletsResponse {
        address_book: response::AddressBook::new(),
        metadata,
        jetton_wallets: wallets.iter().map(map_jetton_wallet).collect(),
    }
}

#[must_use]
pub fn map_nft_items(items: &[NftItemMeta]) -> response::NftItemsResponse {
    map_nft_items_with_metadata(items)
}

pub fn map_nft_items_with_metadata(items: &[NftItemMeta]) -> response::NftItemsResponse {
    let mut token_info_by_address: HashMap<String, Vec<response::TokenInfo>> = HashMap::new();
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

    let mut metadata = response::Metadata::new();
    for (address, token_info) in token_info_by_address {
        metadata.insert(
            address,
            response::AddressMetadata {
                is_indexed: Some(true),
                token_info,
            },
        );
    }

    response::NftItemsResponse {
        address_book: response::AddressBook::new(),
        metadata,
        nft_items: items.iter().map(map_nft_item).collect(),
    }
}

pub struct AccountStateContext {
    pub interfaces: Vec<String>,
    pub token_info: Vec<response::TokenInfo>,
    pub user_friendly: String,
}

#[must_use]
pub fn map_account_states(
    states: &[LocalnetAccountState],
    context_by_address: &HashMap<Addr, AccountStateContext>,
    include_boc: bool,
) -> response::AccountStatesResponse {
    let mut address_book = response::AddressBook::new();
    let mut metadata = response::Metadata::new();

    for state in states {
        let default_user_friendly = state.address.to_string();
        let context = context_by_address.get(&state.address);
        let interfaces = context
            .map(|ctx| ctx.interfaces.clone())
            .unwrap_or_default();

        address_book.insert(
            state.address.to_string(),
            response::AddressBookRow {
                user_friendly: Some(
                    context.map_or(default_user_friendly, |ctx| ctx.user_friendly.clone()),
                ),
                domain: None,
                interfaces: Some(interfaces),
            },
        );

        if let Some(ctx) = context
            && !ctx.token_info.is_empty()
        {
            metadata.insert(
                state.address.to_string(),
                response::AddressMetadata {
                    is_indexed: Some(true),
                    token_info: ctx.token_info.clone(),
                },
            );
        }
    }

    response::AccountStatesResponse {
        accounts: states
            .iter()
            .map(|state| {
                map_account_state_full(state, context_by_address.get(&state.address), include_boc)
            })
            .collect(),
        address_book,
        metadata,
    }
}

#[must_use]
pub fn map_address_information(state: &LocalnetAccountState) -> response::V2AddressInformation {
    response::V2AddressInformation {
        balance: state.balance.to_string(),
        code: Some(
            state
                .code
                .as_ref()
                .map(BocBytes::to_base64)
                .unwrap_or_default(),
        ),
        data: Some(
            state
                .data
                .as_ref()
                .map(BocBytes::to_base64)
                .unwrap_or_default(),
        ),
        frozen_hash: Some(
            state
                .frozen_hash
                .as_ref()
                .map(Hash256::to_base64)
                .unwrap_or_default(),
        ),
        last_transaction_hash: Some(state.last_transaction_id.hash.to_base64()),
        last_transaction_lt: Some(state.last_transaction_id.lt.to_string()),
        status: map_address_information_status(&state.state).to_owned(),
    }
}

#[must_use]
pub fn map_send_message(message: &LocalnetAcceptedExternalMessage) -> response::SendMessageResult {
    response::SendMessageResult {
        message_hash: message.msg_hash.to_base64(),
        message_hash_norm: message.msg_hash_norm.to_base64(),
    }
}

pub fn map_transactions_response(
    transactions: &[LocalnetTransaction],
) -> response::TransactionsResponse {
    response::TransactionsResponse {
        address_book: response::AddressBook::new(),
        transactions: transactions.iter().map(map_v3_transaction).collect(),
    }
}

pub fn map_blocks_response(blocks: &[LocalnetBlock]) -> response::BlocksResponse {
    response::BlocksResponse {
        blocks: blocks.iter().map(map_v3_block).collect(),
    }
}

fn map_v3_block(block: &LocalnetBlock) -> response::Block {
    response::Block {
        workchain: block.workchain,
        shard: format_v3_shard_id(block.shard),
        seqno: block.seqno,
        root_hash: block.root_hash.to_base64(),
        file_hash: block.file_hash.to_base64(),
        start_lt: block.start_lt.to_string(),
        end_lt: block.end_lt.to_string(),
        gen_utime: response::StringOrNumber::String(block.gen_utime.to_string()),
        tx_count: Some(block.tx_count as i32),
        prev_blocks: block.prev_blocks.iter().map(map_v3_block_id).collect(),
        masterchain_block_ref: block.masterchain_block_ref.as_ref().map(map_v3_block_id),
        master_ref_seqno: block
            .masterchain_block_ref
            .as_ref()
            .map(|block_id| block_id.seqno as i32),
        after_merge: Some(false),
        after_split: Some(false),
        before_split: Some(false),
        created_by: Some(zero_hash_base64()),
        flags: Some(0),
        gen_catchain_seqno: Some(0),
        global_id: Some(0),
        key_block: Some(false),
        min_ref_mc_seqno: Some(0),
        prev_key_block_seqno: Some(0),
        rand_seed: Some(zero_hash_base64()),
        validator_list_hash_short: Some(0),
        version: Some(0),
        vert_seqno: Some(0),
        vert_seqno_incr: Some(false),
        want_merge: Some(false),
        want_split: Some(false),
    }
}

fn map_v3_block_id(block: &crate::localnet::LocalnetBlockId) -> response::BlockId {
    response::BlockId {
        workchain: block.workchain,
        shard: format_v3_shard_id(block.shard),
        seqno: block.seqno,
    }
}

fn format_v3_shard_id(shard: i64) -> String {
    format!("{:X}", shard as u64)
}

fn map_v3_transaction(tx: &LocalnetTransaction) -> response::Transaction {
    let tx_details = transaction_details(&tx.data);
    let trace_external_hash = tx
        .in_msg
        .hash_norm
        .as_ref()
        .unwrap_or(&tx.in_msg.hash)
        .to_base64();
    let in_msg =
        (tx.in_msg.hash.0 != [0; 32]).then(|| map_v3_message(&tx.in_msg, &tx.hash, tx.utime, true));
    let out_msgs = tx
        .out_msgs
        .iter()
        .filter(|msg| msg.hash.0 != [0; 32])
        .map(|msg| map_v3_message(msg, &tx.hash, tx.utime, false))
        .collect::<Vec<_>>();
    response::Transaction {
        account: tx.address.to_string(),
        hash: tx.hash.to_base64(),
        lt: tx.transaction_id.lt.to_string(),
        now: tx.utime,
        orig_status: Some(tx_details.orig_status.to_owned()),
        end_status: Some(tx_details.end_status.to_owned()),
        total_fees: Some(tx.total_fees.to_string()),
        total_fees_extra_currencies: HashMap::new(),
        prev_trans_hash: Some(tx_details.prev_trans_hash),
        prev_trans_lt: Some(tx_details.prev_trans_lt),
        description: Some(response::TransactionDescr {
            kind: Some("ord".to_owned()),
            aborted: Some(tx_details.aborted.unwrap_or(!tx.success)),
            destroyed: Some(tx_details.destroyed.unwrap_or(false)),
            credit_first: Some(tx_details.credit_first.unwrap_or(false)),
            is_tock: Some(false),
            installed: Some(false),
            storage_ph: tx_details.storage_phase,
            compute_ph: tx_details.compute_phase,
            action: tx_details.action_phase,
            credit_ph: None,
            bounce: None,
            split_info: None,
        }),
        in_msg,
        out_msgs,
        account_state_before: Some(map_transaction_account_state(
            None,
            &tx_details.account_state_before_hash,
            tx_details.orig_status,
        )),
        account_state_after: Some(map_transaction_account_state(
            None,
            &tx_details.account_state_after_hash,
            tx_details.end_status,
        )),
        block_ref: Some(response::BlockId {
            workchain: 0,
            shard: format_v3_shard_id(i64::MIN),
            seqno: tx.mc_block_seqno,
        }),
        mc_block_seqno: Some(tx.mc_block_seqno),
        emulated: Some(false),
        trace_id: Some(tx.hash.to_base64()),
        trace_external_hash: Some(trace_external_hash),
        finality: None,
        child_transactions: Vec::new(),
    }
}

fn map_v3_message(
    msg: &LocalnetMessage,
    tx_hash: &Hash256,
    tx_utime: u32,
    is_in_msg: bool,
) -> response::Message {
    response::Message {
        hash: Some(msg.hash.to_base64()),
        hash_norm: msg.hash_norm.as_ref().map(Hash256::to_base64),
        source: msg.source.as_ref().map(ToString::to_string),
        destination: msg.destination.as_ref().map(ToString::to_string),
        value: Some(msg.value.to_string()),
        value_extra_currencies: Some(HashMap::new()),
        fwd_fee: Some(msg.fwd_fee.to_string()),
        ihr_fee: Some(msg.ihr_fee.to_string()),
        import_fee: Some("0".to_owned()),
        created_lt: Some(msg.created_lt.to_string()),
        created_at: Some(tx_utime.to_string()),
        decoded_opcode: None,
        extra_flags: None,
        ihr_disabled: Some(true),
        bounce: Some(msg.bounce),
        bounced: Some(msg.bounced),
        in_msg_tx_hash: is_in_msg.then(|| tx_hash.to_base64()),
        out_msg_tx_hash: (!is_in_msg).then(|| tx_hash.to_base64()),
        opcode: msg
            .opcode
            .map(|opcode| response::StringOrNumber::Number(i64::from(opcode))),
        message_content: Some(response::MessageContent {
            hash: Some(msg.body_hash.to_base64()),
            body: Some(msg.body.to_base64()),
            decoded: None,
        }),
        init_state: (!msg.init_state.is_empty()).then(|| response::MessageContent {
            hash: hash_boc_base64(&msg.init_state),
            body: Some(msg.init_state.to_base64()),
            decoded: None,
        }),
    }
}

fn hash_boc_base64(boc: &BocBytes) -> Option<String> {
    let cell = Boc::decode(boc).ok()?;
    Some(Hash256::from(cell.repr_hash()).to_base64())
}

pub(crate) fn map_jetton_wallet(w: &JettonWalletMeta) -> response::JettonWallet {
    response::JettonWallet {
        address: w.address.to_string(),
        balance: w.balance.to_string(),
        code_hash: Some(w.code_hash.to_base64()),
        data_hash: Some(w.data_hash.to_base64()),
        jetton: w.jetton_address.to_string(),
        last_transaction_lt: w.last_transaction_lt.to_string(),
        mintless_info: None,
        owner: w.owner_address.to_string(),
    }
}

pub(crate) fn map_jetton_wallet_token_info(wallet: &JettonWalletMeta) -> response::TokenInfo {
    response::TokenInfo {
        valid: Some(true),
        kind: Some("jetton_wallets".to_owned()),
        extra: HashMap::from([
            (
                "owner".to_owned(),
                Value::String(wallet.owner_address.to_string()),
            ),
            (
                "jetton".to_owned(),
                Value::String(wallet.jetton_address.to_string()),
            ),
            (
                "balance".to_owned(),
                Value::String(wallet.balance.to_string()),
            ),
        ]),
        ..Default::default()
    }
}

pub(crate) fn map_jetton_master_token_info(master: &JettonMasterMeta) -> response::TokenInfo {
    response::TokenInfo {
        valid: Some(true),
        kind: Some("jetton_masters".to_owned()),
        name: content_string(&master.jetton_content, "name"),
        symbol: content_string(&master.jetton_content, "symbol"),
        description: content_string(&master.jetton_content, "description"),
        image: content_string(&master.jetton_content, "image"),
        extra: object_fields(&master.jetton_content),
        ..Default::default()
    }
}

fn map_nft_item(item: &NftItemMeta) -> response::NftItem {
    response::NftItem {
        address: item.address.to_string(),
        auction_contract_address: None,
        code_hash: Some(item.code_hash.to_base64()),
        collection: item
            .collection_address
            .as_ref()
            .map(|address| response::NftCollectionRef {
                address: address.to_string(),
            }),
        collection_address: item.collection_address.as_ref().map(ToString::to_string),
        content: object_fields(&item.content),
        data_hash: Some(item.data_hash.to_base64()),
        index: Some(item.index.clone()),
        init: Some(item.init),
        last_transaction_lt: Some(item.last_transaction_lt.to_string()),
        on_sale: Some(false),
        owner_address: item.owner_address.as_ref().map(ToString::to_string),
        real_owner: item.owner_address.as_ref().map(ToString::to_string),
        sale_contract_address: None,
    }
}

pub(crate) fn map_nft_item_token_info(item: &NftItemMeta) -> response::TokenInfo {
    response::TokenInfo {
        valid: Some(true),
        kind: Some("nft_items".to_owned()),
        name: content_string(&item.content, "name"),
        symbol: content_string(&item.content, "symbol"),
        description: content_string(&item.content, "description"),
        image: content_string(&item.content, "image"),
        nft_index: Some(item.index.clone()),
        extra: object_fields(&item.content),
    }
}

pub(crate) fn map_nft_collection_token_info(item: &NftItemMeta) -> response::TokenInfo {
    response::TokenInfo {
        valid: Some(true),
        kind: Some("nft_collections".to_owned()),
        name: content_string(&item.content, "collection_name"),
        description: content_string(&item.content, "collection_description"),
        image: content_string(&item.content, "collection_image"),
        ..Default::default()
    }
}

fn map_account_state_full(
    state: &LocalnetAccountState,
    context: Option<&AccountStateContext>,
    include_boc: bool,
) -> response::AccountStateFull {
    response::AccountStateFull {
        address: state.address.to_string(),
        account_state_hash: Some(state.account_state_hash.to_base64()),
        balance: Some(state.balance.to_string()),
        code_boc: include_boc
            .then(|| state.code.as_ref().map(BocBytes::to_base64))
            .flatten(),
        code_hash: state.code_hash.as_ref().map(Hash256::to_base64),
        contract_methods: Vec::new(),
        data_boc: include_boc
            .then(|| state.data.as_ref().map(BocBytes::to_base64))
            .flatten(),
        data_hash: state.data_hash.as_ref().map(Hash256::to_base64),
        extra_currencies: Some(HashMap::new()),
        frozen_hash: state.frozen_hash.as_ref().map(Hash256::to_base64),
        interfaces: Some(
            context
                .map(|ctx| ctx.interfaces.clone())
                .unwrap_or_default(),
        ),
        last_transaction_hash: Some(state.last_transaction_id.hash.to_base64()),
        last_transaction_lt: Some(state.last_transaction_id.lt.to_string()),
        status: map_account_state_status(&state.state).to_owned(),
    }
}

fn object_fields(value: &Value) -> HashMap<String, Value> {
    value
        .as_object()
        .map(|fields| fields.clone().into_iter().collect())
        .unwrap_or_default()
}

fn content_string(content: &Value, key: &str) -> Option<String> {
    content
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

#[must_use]
pub fn map_traces(tn: &TraceNode) -> response::TracesResponse {
    map_traces_with_emulated(tn, false)
}

fn map_traces_with_emulated(tn: &TraceNode, emulated: bool) -> response::TracesResponse {
    let mut transactions = HashMap::new();
    let mut transactions_order = Vec::new();
    collect_transactions(tn, &mut transactions, &mut transactions_order, emulated);

    response::TracesResponse {
        address_book: response::AddressBook::new(),
        metadata: response::Metadata::new(),
        traces: vec![map_trace(tn, transactions, transactions_order, emulated)],
    }
}

pub fn map_emulate_trace_response(
    emulation: &EmulateTraceResult,
    with_actions: bool,
    include_code_data: bool,
    address_book: Option<response::AddressBook>,
    metadata: Option<response::Metadata>,
) -> emulate::EmulateTraceResponse {
    let tn = &emulation.trace;
    let mut transactions = HashMap::new();
    let mut transactions_order = Vec::new();
    collect_transactions(tn, &mut transactions, &mut transactions_order, true);

    emulate::EmulateTraceResponse {
        mc_block_seqno: tn.transaction.meta.block_seqno,
        trace: map_trace_node(tn, true),
        transactions,
        actions: with_actions.then(Vec::new),
        code_cells: include_code_data.then(|| map_cells_by_hash_base64(&emulation.code_cells)),
        data_cells: include_code_data.then(|| map_cells_by_hash_base64(&emulation.data_cells)),
        address_book,
        metadata,
        rand_seed: zero_hash_base64(),
        is_incomplete: false,
    }
}

fn map_cells_by_hash_base64(cells: &HashMap<Hash256, BocBytes>) -> HashMap<String, String> {
    cells
        .iter()
        .map(|(hash, boc)| (hash.to_base64(), boc.to_base64()))
        .collect()
}

pub fn map_run_get_method_v3(result: &LocalnetRunGetMethodResult) -> response::RunGetMethodResult {
    let stack_cell = Boc::decode(&result.stack).unwrap_or_default();
    let stack_tuple = Tuple::deserialize(&stack_cell).unwrap_or_default();
    let stack = stack_to_json(&stack_tuple)
        .unwrap_or_default()
        .into_iter()
        .map(map_stack_entry)
        .collect::<Vec<_>>();

    response::RunGetMethodResult {
        gas_used: response::StringOrNumber::String(result.gas_used.to_string()),
        exit_code: result.exit_code,
        stack,
        vm_log: Some(result.vm_log.to_string()),
    }
}

fn collect_transactions(
    tn: &TraceNode,
    transactions: &mut HashMap<String, response::Transaction>,
    order: &mut Vec<String>,
    emulated: bool,
) {
    let tx_hash = tn.transaction.meta.tx_hash.to_base64();
    if !transactions.contains_key(&tx_hash) {
        let mut transaction = map_transaction(&tn.transaction, emulated);
        transaction.child_transactions = tn
            .children
            .iter()
            .map(|c| c.transaction.meta.lt.to_string())
            .collect();
        transactions.insert(tx_hash.clone(), transaction);
        order.push(tx_hash);
    }
    for child in &tn.children {
        collect_transactions(child, transactions, order, emulated);
    }
}

fn map_trace(
    tn: &TraceNode,
    transactions: HashMap<String, response::Transaction>,
    transactions_order: Vec<String>,
    emulated: bool,
) -> response::Trace {
    let transaction_count = transactions.len();
    response::Trace {
        trace_id: tn.transaction.meta.tx_hash.to_base64(),
        external_hash: Some(tn.external_hash.as_ref().map_or_else(
            || tn.transaction.meta.tx_hash.to_base64(),
            Hash256::to_base64,
        )),
        mc_seqno_start: Some("0".to_owned()),
        mc_seqno_end: Some("0".to_owned()),
        start_lt: Some(tn.transaction.meta.lt.to_string()),
        start_utime: Some(tn.transaction.meta.now),
        end_lt: Some(tn.max_lt().to_string()),
        end_utime: Some(tn.max_utime()),
        is_incomplete: false,
        trace: Some(map_trace_node(tn, emulated)),
        transactions,
        transactions_order,
        actions: Vec::new(),
        trace_info: Some(response::TraceInfo {
            transactions: transaction_count,
            messages: transaction_count.saturating_sub(1) + tn.children.len(),
            pending_messages: 0,
            trace_state: "complete".to_owned(),
            classification_state: "classified".to_owned(),
        }),
        warning: None,
    }
}

fn map_trace_node(tn: &TraceNode, emulated: bool) -> response::TraceNode {
    response::TraceNode {
        tx_hash: Some(tn.transaction.meta.tx_hash.to_base64()),
        in_msg_hash: tn
            .transaction
            .meta
            .in_msg_hash
            .as_ref()
            .map(Hash256::to_base64),
        in_msg: tn.transaction.in_msg.as_ref().map(|m| {
            map_trace_message_info(
                m,
                &tn.transaction.meta.tx_hash,
                tn.transaction.meta.now,
                true,
            )
        }),
        transaction: Some(map_transaction(&tn.transaction, emulated)),
        children: tn
            .children
            .iter()
            .map(|child| map_trace_node(child, emulated))
            .collect(),
    }
}

fn map_transaction(tx: &TransactionInfo, emulated: bool) -> response::Transaction {
    let tx_details = transaction_details(&tx.tx_boc);
    let trace_external_hash = tx.meta.in_msg_hash.unwrap_or(tx.meta.tx_hash).to_base64();
    response::Transaction {
        account: tx.meta.account.to_string(),
        hash: tx.meta.tx_hash.to_base64(),
        lt: tx.meta.lt.to_string(),
        now: tx.meta.now,
        orig_status: Some(tx_details.orig_status.to_owned()),
        end_status: Some(tx_details.end_status.to_owned()),
        total_fees: Some(tx.meta.total_fees.to_string()),
        total_fees_extra_currencies: HashMap::new(),
        prev_trans_hash: Some(tx_details.prev_trans_hash),
        prev_trans_lt: Some(tx_details.prev_trans_lt),
        description: Some(response::TransactionDescr {
            kind: Some("ord".to_owned()),
            aborted: Some(tx_details.aborted.unwrap_or(!tx.meta.success)),
            destroyed: Some(tx_details.destroyed.unwrap_or(false)),
            credit_first: Some(tx_details.credit_first.unwrap_or(false)),
            is_tock: Some(false),
            installed: Some(false),
            storage_ph: tx_details.storage_phase,
            compute_ph: tx_details.compute_phase.or_else(|| {
                tx.meta
                    .compute_exit_code
                    .map(|exit_code| default_compute_phase(false, exit_code == 0, exit_code))
            }),
            action: tx_details.action_phase.or_else(|| {
                tx.meta.action_result_code.map(|result_code| {
                    default_action_phase(result_code == 0, result_code, tx.out_msgs.len())
                })
            }),
            credit_ph: None,
            bounce: None,
            split_info: None,
        }),
        in_msg: tx
            .in_msg
            .as_ref()
            .map(|m| map_trace_message_info(m, &tx.meta.tx_hash, tx.meta.now, true)),
        out_msgs: tx
            .out_msgs
            .iter()
            .map(|m| map_trace_message_info(m, &tx.meta.tx_hash, tx.meta.now, false))
            .collect(),
        account_state_before: Some(map_transaction_account_state(
            tx.account_state_before.as_ref(),
            &tx_details.account_state_before_hash,
            tx_details.orig_status,
        )),
        account_state_after: Some(map_transaction_account_state(
            tx.account_state_after.as_ref(),
            &tx_details.account_state_after_hash,
            tx_details.end_status,
        )),
        block_ref: Some(response::BlockId {
            workchain: 0,
            shard: format_v3_shard_id(i64::MIN),
            seqno: tx.meta.block_seqno,
        }),
        mc_block_seqno: Some(tx.meta.block_seqno),
        child_transactions: Vec::new(),
        emulated: Some(emulated),
        trace_id: Some(tx.meta.tx_hash.to_base64()),
        trace_external_hash: Some(trace_external_hash),
        finality: None,
    }
}

struct TransactionDetails {
    prev_trans_hash: String,
    prev_trans_lt: String,
    orig_status: &'static str,
    end_status: &'static str,
    account_state_before_hash: String,
    account_state_after_hash: String,
    aborted: Option<bool>,
    destroyed: Option<bool>,
    credit_first: Option<bool>,
    storage_phase: Option<response::StoragePhase>,
    compute_phase: Option<response::ComputePhase>,
    action_phase: Option<response::ActionPhase>,
}

impl Default for TransactionDetails {
    fn default() -> Self {
        Self {
            prev_trans_hash: zero_hash_base64(),
            prev_trans_lt: "0".to_string(),
            orig_status: "active",
            end_status: "active",
            account_state_before_hash: zero_hash_base64(),
            account_state_after_hash: zero_hash_base64(),
            aborted: None,
            destroyed: None,
            credit_first: None,
            storage_phase: None,
            compute_phase: None,
            action_phase: None,
        }
    }
}

fn transaction_details(tx_boc: &BocBytes) -> TransactionDetails {
    let Some(transaction) = Boc::decode(tx_boc)
        .ok()
        .and_then(|cell| cell.parse::<tycho_types::models::Transaction>().ok())
    else {
        return TransactionDetails::default();
    };

    let state_update = transaction.state_update.load().ok();
    let tx_info = transaction.info.load().ok();
    let ordinary_info = match tx_info {
        Some(TxInfo::Ordinary(info)) => Some(info),
        _ => None,
    };

    TransactionDetails {
        prev_trans_hash: hash_bytes_base64(&transaction.prev_trans_hash),
        prev_trans_lt: transaction.prev_trans_lt.to_string(),
        orig_status: map_tycho_account_status(transaction.orig_status),
        end_status: map_tycho_account_status(transaction.end_status),
        account_state_before_hash: state_update
            .as_ref()
            .map_or_else(zero_hash_base64, |update| hash_bytes_base64(&update.old)),
        account_state_after_hash: state_update
            .as_ref()
            .map_or_else(zero_hash_base64, |update| hash_bytes_base64(&update.new)),
        aborted: ordinary_info.as_ref().map(|info| info.aborted),
        destroyed: ordinary_info.as_ref().map(|info| info.destroyed),
        credit_first: ordinary_info.as_ref().map(|info| info.credit_first),
        storage_phase: ordinary_info
            .as_ref()
            .and_then(|info| info.storage_phase.as_ref())
            .map(map_storage_phase),
        compute_phase: ordinary_info
            .as_ref()
            .map(|info| map_compute_phase(&info.compute_phase)),
        action_phase: ordinary_info
            .as_ref()
            .and_then(|info| info.action_phase.as_ref())
            .map(map_action_phase),
    }
}

fn map_storage_phase(phase: &tycho_types::models::StoragePhase) -> response::StoragePhase {
    response::StoragePhase {
        storage_fees_collected: Some(u128::from(phase.storage_fees_collected).to_string()),
        storage_fees_due: phase
            .storage_fees_due
            .map(|value| u128::from(value).to_string()),
        status_change: Some(map_account_status_change(phase.status_change).to_owned()),
    }
}

fn default_compute_phase(skipped: bool, success: bool, exit_code: i32) -> response::ComputePhase {
    response::ComputePhase {
        skipped: Some(skipped),
        success: Some(success),
        msg_state_used: Some(false),
        account_activated: Some(false),
        gas_fees: Some("0".to_owned()),
        gas_used: Some("0".to_owned()),
        gas_limit: Some("0".to_owned()),
        gas_credit: None,
        mode: Some(0),
        exit_code: Some(exit_code),
        exit_arg: None,
        vm_steps: Some(0),
        vm_init_state_hash: Some(zero_hash_base64()),
        vm_final_state_hash: Some(zero_hash_base64()),
        reason: None,
    }
}

fn map_compute_phase(phase: &ComputePhase) -> response::ComputePhase {
    match phase {
        ComputePhase::Skipped(phase) => response::ComputePhase {
            skipped: Some(true),
            success: Some(false),
            exit_code: Some(0),
            reason: Some(map_compute_skip_reason(phase.reason).to_owned()),
            ..default_compute_phase(true, false, 0)
        },
        ComputePhase::Executed(phase) => response::ComputePhase {
            skipped: Some(false),
            success: Some(phase.success),
            msg_state_used: Some(phase.msg_state_used),
            account_activated: Some(phase.account_activated),
            gas_fees: Some(u128::from(phase.gas_fees).to_string()),
            gas_used: Some(u64::from(phase.gas_used).to_string()),
            gas_limit: Some(u64::from(phase.gas_limit).to_string()),
            gas_credit: phase.gas_credit.map(|credit| u32::from(credit).to_string()),
            mode: Some(phase.mode),
            exit_code: Some(phase.exit_code),
            exit_arg: phase.exit_arg,
            vm_steps: Some(phase.vm_steps),
            vm_init_state_hash: Some(hash_bytes_base64(&phase.vm_init_state_hash)),
            vm_final_state_hash: Some(hash_bytes_base64(&phase.vm_final_state_hash)),
            reason: None,
        },
    }
}

fn default_action_phase(
    success: bool,
    result_code: i32,
    out_msgs_len: usize,
) -> response::ActionPhase {
    let out_msgs_len =
        u32::try_from(out_msgs_len).expect("TON transaction message count must fit into u32");
    response::ActionPhase {
        success: Some(success),
        valid: Some(true),
        no_funds: Some(false),
        status_change: Some("unchanged".to_owned()),
        result_code: Some(result_code),
        result_arg: None,
        tot_actions: Some(out_msgs_len),
        spec_actions: Some(0),
        skipped_actions: Some(0),
        msgs_created: Some(out_msgs_len),
        total_fwd_fees: None,
        total_action_fees: None,
        action_list_hash: Some(zero_hash_base64()),
        tot_msg_size: Some(response::MsgSize {
            cells: Some("0".to_owned()),
            bits: Some("0".to_owned()),
        }),
    }
}

fn map_action_phase(phase: &ActionPhase) -> response::ActionPhase {
    response::ActionPhase {
        success: Some(phase.success),
        valid: Some(phase.valid),
        no_funds: Some(phase.no_funds),
        status_change: Some(map_account_status_change(phase.status_change).to_owned()),
        result_code: Some(phase.result_code),
        result_arg: phase.result_arg,
        tot_actions: Some(u32::from(phase.total_actions)),
        spec_actions: Some(u32::from(phase.special_actions)),
        skipped_actions: Some(u32::from(phase.skipped_actions)),
        msgs_created: Some(u32::from(phase.messages_created)),
        total_fwd_fees: phase
            .total_fwd_fees
            .map(|value| u128::from(value).to_string()),
        total_action_fees: phase
            .total_action_fees
            .map(|value| u128::from(value).to_string()),
        action_list_hash: Some(hash_bytes_base64(&phase.action_list_hash)),
        tot_msg_size: Some(response::MsgSize {
            cells: Some(u64::from(phase.total_message_size.cells).to_string()),
            bits: Some(u64::from(phase.total_message_size.bits).to_string()),
        }),
    }
}

const fn map_account_status_change(change: AccountStatusChange) -> &'static str {
    match change {
        AccountStatusChange::Unchanged => "unchanged",
        AccountStatusChange::Frozen => "frozen",
        AccountStatusChange::Deleted => "deleted",
    }
}

const fn map_compute_skip_reason(reason: ComputePhaseSkipReason) -> &'static str {
    match reason {
        ComputePhaseSkipReason::NoState => "no_state",
        ComputePhaseSkipReason::BadState => "bad_state",
        ComputePhaseSkipReason::NoGas => "no_gas",
        ComputePhaseSkipReason::Suspended => "suspended",
    }
}

fn map_transaction_account_state(
    snapshot: Option<&AccountStateSnapshot>,
    fallback_hash: &str,
    fallback_status: &str,
) -> response::AccountState {
    if let Some(snapshot) = snapshot {
        let data_hash = snapshot.data_hash();
        let code_hash = snapshot.code_hash();
        return response::AccountState {
            hash: Some(fallback_hash.to_owned()),
            account_status: Some(map_account_state_status(&snapshot.status).to_owned()),
            balance: Some(snapshot.balance.to_string()),
            code_boc: snapshot.code.as_ref().map(Boc::encode_base64),
            code_hash: code_hash.as_ref().map(Hash256::to_base64),
            data_boc: snapshot.data.as_ref().map(Boc::encode_base64),
            data_hash: data_hash.as_ref().map(Hash256::to_base64),
            extra_currencies: Some(HashMap::new()),
            frozen_hash: snapshot.frozen_hash.as_ref().map(Hash256::to_base64),
        };
    }

    map_emulation_account_state(fallback_hash, "0", fallback_status, None, None, None)
}

fn map_emulation_account_state(
    hash: &str,
    balance: &str,
    account_status: &str,
    frozen_hash: Option<&Hash256>,
    data_hash: Option<&Hash256>,
    code_hash: Option<&Hash256>,
) -> response::AccountState {
    response::AccountState {
        hash: Some(hash.to_owned()),
        account_status: Some(account_status.to_owned()),
        balance: Some(balance.to_owned()),
        code_boc: None,
        code_hash: code_hash.map(Hash256::to_base64),
        data_boc: None,
        data_hash: data_hash.map(Hash256::to_base64),
        extra_currencies: Some(HashMap::new()),
        frozen_hash: frozen_hash.map(Hash256::to_base64),
    }
}

fn hash_bytes_base64(hash: &HashBytes) -> String {
    Hash256::from(hash).to_base64()
}

const fn map_tycho_account_status(status: tycho_types::models::AccountStatus) -> &'static str {
    match status {
        tycho_types::models::AccountStatus::Uninit => "uninit",
        tycho_types::models::AccountStatus::Frozen => "frozen",
        tycho_types::models::AccountStatus::Active => "active",
        tycho_types::models::AccountStatus::NotExists => "nonexist",
    }
}

fn map_trace_message_info(
    msg: &MessageInfo,
    tx_hash: &Hash256,
    tx_utime: u32,
    is_in_msg: bool,
) -> response::Message {
    convert_to_message_struct(&msg.meta, &msg.boc).map_or_else(
        |_| map_message(&msg.meta),
        |message| map_v3_message(&message, tx_hash, tx_utime, is_in_msg),
    )
}

fn map_message(msg: &MsgMeta) -> response::Message {
    response::Message {
        hash: Some(msg.msg_hash.to_base64()),
        hash_norm: None,
        source: msg.src.as_ref().map(ToString::to_string),
        destination: msg.dst.as_ref().map(ToString::to_string),
        value: Some(msg.value.unwrap_or(0).to_string()),
        value_extra_currencies: Some(HashMap::new()),
        fwd_fee: Some("0".to_owned()),
        ihr_fee: Some("0".to_owned()),
        created_lt: Some(msg.created_lt.unwrap_or(0).to_string()),
        created_at: Some(msg.created_at.unwrap_or(0).to_string()),
        decoded_opcode: None,
        extra_flags: None,
        ihr_disabled: None,
        bounce: Some(msg.bounce.unwrap_or(false)),
        bounced: Some(false),
        import_fee: Some("0".to_owned()),
        in_msg_tx_hash: None,
        opcode: None,
        out_msg_tx_hash: None,
        message_content: Some(response::MessageContent {
            hash: Some(msg.msg_boc_hash.to_base64()),
            body: Some(String::new()),
            decoded: None,
        }),
        init_state: None,
    }
}

fn map_stack_entry(entry: Value) -> response::StackEntity {
    let Some(entry_type) = entry.get("@type").and_then(Value::as_str) else {
        return response::StackEntity {
            kind: "unknown".to_owned(),
            value: response::StackValue::Json(entry),
        };
    };

    match entry_type {
        "tvm.stackEntryNull" => response::StackEntity {
            kind: "null".to_owned(),
            value: response::StackValue::Json(Value::Null),
        },
        "tvm.stackEntryNumber" => response::StackEntity {
            kind: "num".to_owned(),
            value: response::StackValue::Json(
                entry
                    .pointer("/number/number")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        },
        "tvm.stackEntryCell" => response::StackEntity {
            kind: "cell".to_owned(),
            value: response::StackValue::Json(entry.get("cell").cloned().unwrap_or(Value::Null)),
        },
        "tvm.stackEntrySlice" => response::StackEntity {
            kind: "slice".to_owned(),
            value: response::StackValue::Json(entry.get("slice").cloned().unwrap_or(Value::Null)),
        },
        "tvm.stackEntryBuilder" => response::StackEntity {
            kind: "builder".to_owned(),
            value: response::StackValue::Json(entry.get("builder").cloned().unwrap_or(Value::Null)),
        },
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
            response::StackEntity {
                kind: "tuple".to_owned(),
                value: response::StackValue::Entries(elements),
            }
        }
        _ => response::StackEntity {
            kind: entry_type.to_owned(),
            value: response::StackValue::Json(entry),
        },
    }
}

fn zero_hash_base64() -> String {
    Hash256([0; 32]).to_base64()
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
    use super::{
        format_v3_shard_id, map_blocks_response, map_jetton_masters, map_nft_collection_token_info,
        map_nft_item_token_info,
    };
    use crate::localnet::{LocalnetBlock, LocalnetBlockId};
    use crate::storage::{JettonMasterMeta, NftItemMeta};
    use crate::types::Hash256;
    use serde_json::json;

    fn sample_jetton_master() -> JettonMasterMeta {
        JettonMasterMeta {
            address: "0:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .parse()
                .expect("valid master address"),
            admin_address: Some(
                "0:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .parse()
                    .expect("valid admin address"),
            ),
            code_hash: Hash256([1; 32]),
            data_hash: Hash256([2; 32]),
            jetton_content: json!({
                "name": "UTYA",
                "symbol": "UTYA",
                "description": "Duck token",
                "image": "https://example.com/utya.png",
                "decimals": "9",
            }),
            jetton_wallet_code_hash: Hash256([3; 32]),
            last_transaction_lt: 42,
            mintable: true,
            total_supply: 1_000_000,
        }
    }

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
    fn jetton_masters_response_includes_token_metadata() {
        let master = sample_jetton_master();
        let address = master.address.to_string();
        let mapped = map_jetton_masters(&[master]);
        let metadata = mapped.metadata.get(&address).expect("master metadata");
        let token_info = &metadata.token_info[0];

        assert_eq!(metadata.is_indexed, Some(true));
        assert_eq!(token_info.kind.as_deref(), Some("jetton_masters"));
        assert_eq!(token_info.name.as_deref(), Some("UTYA"));
        assert_eq!(token_info.symbol.as_deref(), Some("UTYA"));
        assert_eq!(token_info.description.as_deref(), Some("Duck token"));
        assert_eq!(token_info.extra["decimals"].as_str(), Some("9"));
    }

    #[test]
    fn nft_item_token_info_uses_nft_items_type() {
        let token_info = map_nft_item_token_info(&sample_nft_item());

        assert_eq!(token_info.kind.as_deref(), Some("nft_items"));
        assert_eq!(token_info.nft_index.as_deref(), Some("7"));
        assert_eq!(token_info.name.as_deref(), Some("Sample NFT"));
    }

    #[test]
    fn nft_collection_token_info_uses_nft_collections_type() {
        let token_info = map_nft_collection_token_info(&sample_nft_item());

        assert_eq!(token_info.kind.as_deref(), Some("nft_collections"));
        assert_eq!(token_info.name.as_deref(), Some("Sample Collection"));
        assert_eq!(
            token_info.description.as_deref(),
            Some("Collection description")
        );
    }

    #[test]
    fn blocks_response_formats_shards_as_toncenter_v3_hex_strings() {
        let block_id = LocalnetBlockId {
            workchain: 0,
            shard: i64::MIN,
            seqno: 41,
            root_hash: Hash256([1; 32]),
            file_hash: Hash256([2; 32]),
        };
        let masterchain_block_ref = LocalnetBlockId {
            workchain: -1,
            shard: i64::MIN,
            seqno: 42,
            root_hash: Hash256([3; 32]),
            file_hash: Hash256([4; 32]),
        };
        let response = map_blocks_response(&[LocalnetBlock {
            workchain: 0,
            shard: i64::MIN,
            seqno: 43,
            root_hash: Hash256([5; 32]),
            file_hash: Hash256([6; 32]),
            gen_utime: 123,
            start_lt: 1,
            end_lt: 2,
            tx_count: 1,
            prev_blocks: vec![block_id],
            masterchain_block_ref: Some(masterchain_block_ref),
        }]);

        let block = &response.blocks[0];
        assert_eq!(format_v3_shard_id(i64::MIN), "8000000000000000");
        assert_eq!(block.shard, "8000000000000000");
        assert_eq!(block.prev_blocks[0].shard, "8000000000000000");
        assert_eq!(
            block
                .masterchain_block_ref
                .as_ref()
                .expect("masterchain block ref")
                .shard,
            "8000000000000000"
        );
    }
}
