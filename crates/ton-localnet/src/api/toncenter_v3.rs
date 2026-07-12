//! Localnet-to-`TonCenter` v3 typed response mappers.
//!
//! Mapping notes:
//! - jetton and NFT metadata is a local projection and omits fields unavailable in local state;
//! - `map_run_get_method_v3` emits the observed result shape (`gas_used`, `exit_code`, `stack`,
//!   local `vm_log`); upstream v3 `OpenAPI` 1.2.6 incorrectly declares the request type as the
//!   successful response schema;

use crate::localnet::{
    LocalnetAcceptedExternalMessage, LocalnetAccountState, LocalnetBlock, LocalnetMessage,
    LocalnetRunGetMethodResult, LocalnetTransaction, convert_to_message_struct,
};
use crate::storage::{
    AccountStateSnapshot, AccountStatus, EmulateTraceResult, JettonMasterMeta, JettonWalletMeta,
    MessageInfo, MsgMeta, NftItemMeta, TraceNode, TransactionInfo,
};
use crate::types::{Addr, BocBytes, Hash256};
use num_bigint::BigInt;
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

trait AddressBookExt {
    fn insert_address(&mut self, address: Addr, interfaces: &[&str]);
    fn insert_opt_address(&mut self, address: Option<Addr>, interfaces: &[&str]);
    fn insert_message(&mut self, source: Option<Addr>, destination: Option<Addr>);
    fn insert_transaction(&mut self, transaction: &LocalnetTransaction);
    fn insert_trace(&mut self, trace: &TraceNode);
}

impl AddressBookExt for response::AddressBook {
    fn insert_address(&mut self, address: Addr, interfaces: &[&str]) {
        let row = self
            .entry(address.to_string())
            .or_insert_with(|| response::AddressBookRow {
                user_friendly: Some(address.as_user_friendly()),
                domain: None,
                interfaces: Some(Vec::new()),
            });
        let row_interfaces = row.interfaces.get_or_insert_default();
        for interface in interfaces {
            if !row_interfaces.iter().any(|value| value == interface) {
                row_interfaces.push((*interface).to_owned());
            }
        }
    }

    fn insert_opt_address(&mut self, address: Option<Addr>, interfaces: &[&str]) {
        if let Some(address) = address {
            self.insert_address(address, interfaces);
        }
    }

    fn insert_message(&mut self, source: Option<Addr>, destination: Option<Addr>) {
        self.insert_opt_address(source, &[]);
        self.insert_opt_address(destination, &[]);
    }

    fn insert_transaction(&mut self, transaction: &LocalnetTransaction) {
        self.insert_address(transaction.address, &[]);
        self.insert_message(transaction.in_msg.source, transaction.in_msg.destination);
        for message in &transaction.out_msgs {
            self.insert_message(message.source, message.destination);
        }
    }

    fn insert_trace(&mut self, trace: &TraceNode) {
        self.insert_address(trace.transaction.meta.account, &[]);
        if let Some(message) = &trace.transaction.in_msg {
            self.insert_message(message.meta.src, message.meta.dst);
        }
        for message in &trace.transaction.out_msgs {
            self.insert_message(message.meta.src, message.meta.dst);
        }
        for child in &trace.children {
            self.insert_trace(child);
        }
    }
}

pub fn map_jetton_masters(masters: &[JettonMasterMeta]) -> response::JettonMastersResponse {
    let mut address_book = response::AddressBook::new();
    let mut metadata = response::Metadata::new();

    for master in masters {
        address_book.insert_address(master.address, &["jetton_master"]);
        address_book.insert_opt_address(master.admin_address, &[]);

        metadata.insert(
            master.address.to_string(),
            response::AddressMetadata {
                is_indexed: true,
                token_info: vec![map_jetton_master_token_info(master)],
            },
        );
    }

    response::JettonMastersResponse {
        address_book,
        metadata,
        jetton_masters: masters.iter().map(map_jetton_master).collect(),
    }
}

fn map_jetton_master(m: &JettonMasterMeta) -> response::JettonMaster {
    response::JettonMaster {
        address: m.address.to_string(),
        admin_address: m.admin_address.map(|address| address.to_string()),
        code_hash: m.code_hash.to_base64(),
        data_hash: m.data_hash.to_base64(),
        jetton_content: object_fields(&m.jetton_content),
        jetton_wallet_code_hash: m.jetton_wallet_code_hash.to_base64(),
        last_transaction_lt: m.last_transaction_lt.to_string(),
        mintable: m.mintable,
        total_supply: m.total_supply.to_string(),
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
    let mut address_book = response::AddressBook::new();
    let mut token_info_by_address: HashMap<String, Vec<response::TokenInfo>> = HashMap::new();
    let mut master_info_added = std::collections::HashSet::new();

    for wallet in wallets {
        address_book.insert_address(wallet.address, &["jetton_wallet"]);
        address_book.insert_address(wallet.owner_address, &[]);
        address_book.insert_address(wallet.jetton_address, &["jetton_master"]);
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
                is_indexed: true,
                token_info,
            },
        );
    }

    response::JettonWalletsResponse {
        address_book,
        metadata,
        jetton_wallets: wallets.iter().map(map_jetton_wallet).collect(),
    }
}

#[must_use]
pub fn map_nft_items(items: &[NftItemMeta]) -> response::NftItemsResponse {
    map_nft_items_with_metadata(items)
}

pub fn map_nft_items_with_metadata(items: &[NftItemMeta]) -> response::NftItemsResponse {
    let mut address_book = response::AddressBook::new();
    let mut token_info_by_address: HashMap<String, Vec<response::TokenInfo>> = HashMap::new();
    let mut collection_info_added = std::collections::HashSet::new();

    for item in items {
        address_book.insert_address(item.address, &["nft_item"]);
        address_book.insert_opt_address(item.owner_address, &[]);
        address_book.insert_opt_address(item.collection_address, &["nft_collection"]);

        token_info_by_address
            .entry(item.address.to_string())
            .or_default()
            .push(map_nft_item_token_info(item));

        let Some(collection_address) = item.collection_address else {
            continue;
        };

        if collection_info_added.insert(collection_address) {
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
                is_indexed: true,
                token_info,
            },
        );
    }

    response::NftItemsResponse {
        address_book,
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
                    is_indexed: true,
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
        code: state.code.as_ref().map(BocBytes::to_base64),
        data: state.data.as_ref().map(BocBytes::to_base64),
        frozen_hash: state.frozen_hash.as_ref().map(Hash256::to_base64),
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
    let mut address_book = response::AddressBook::new();
    for transaction in transactions {
        address_book.insert_transaction(transaction);
    }

    response::TransactionsResponse {
        address_book,
        transactions: transactions.iter().map(map_v3_transaction).collect(),
    }
}

pub fn map_blocks_response(blocks: &[LocalnetBlock]) -> response::BlocksResponse {
    response::BlocksResponse {
        blocks: blocks.iter().map(map_v3_block).collect(),
    }
}

pub fn map_masterchain_info_v3(blocks: &[LocalnetBlock]) -> Option<response::MasterchainInfo> {
    let first = blocks
        .iter()
        .filter(|block| block.workchain == -1)
        .min_by_key(|block| block.seqno)?;
    let last = blocks
        .iter()
        .filter(|block| block.workchain == -1)
        .max_by_key(|block| block.seqno)?;

    Some(response::MasterchainInfo {
        first: map_v3_block(first),
        last: map_v3_block(last),
    })
}

pub(crate) fn map_wallet_information_v3(
    state: &LocalnetAccountState,
    wallet_type: Option<&str>,
    seqno: Option<u32>,
    wallet_id: Option<i32>,
) -> response::V2WalletInformation {
    response::V2WalletInformation {
        balance: state.balance.to_string(),
        wallet_type: wallet_type.map(ToOwned::to_owned),
        seqno: wallet_type.and(seqno),
        wallet_id: wallet_type.and(wallet_id),
        last_transaction_lt: state.last_transaction_id.lt.to_string(),
        last_transaction_hash: state.last_transaction_id.hash.to_base64(),
        status: map_wallet_information_status(&state.state).to_owned(),
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
        tx_count: block.tx_count as i64,
        prev_blocks: block.prev_blocks.iter().map(map_v3_block_id).collect(),
        masterchain_block_ref: block.masterchain_block_ref.as_ref().map_or_else(
            || response::BlockId {
                workchain: block.workchain,
                shard: format_v3_shard_id(block.shard),
                seqno: block.seqno,
            },
            map_v3_block_id,
        ),
        master_ref_seqno: block
            .masterchain_block_ref
            .as_ref()
            .map_or(0, |block_id| block_id.seqno as i32),
        after_merge: false,
        after_split: false,
        before_split: false,
        created_by: zero_hash_base64(),
        flags: 0,
        gen_catchain_seqno: 0,
        global_id: 0,
        key_block: false,
        min_ref_mc_seqno: 0,
        prev_key_block_seqno: 0,
        rand_seed: zero_hash_base64(),
        validator_list_hash_short: 0,
        version: 0,
        vert_seqno: 0,
        vert_seqno_incr: false,
        want_merge: false,
        want_split: false,
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
        orig_status: tx_details.orig_status.to_owned(),
        end_status: tx_details.end_status.to_owned(),
        total_fees: tx.total_fees.to_string(),
        total_fees_extra_currencies: HashMap::new(),
        prev_trans_hash: tx_details.prev_trans_hash,
        prev_trans_lt: tx_details.prev_trans_lt,
        description: response::TransactionDescr {
            kind: "ord".to_owned(),
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
        },
        in_msg,
        out_msgs,
        account_state_before: map_transaction_account_state(
            None,
            &tx_details.account_state_before_hash,
        ),
        account_state_after: map_transaction_account_state(
            None,
            &tx_details.account_state_after_hash,
        ),
        block_ref: response::BlockId {
            workchain: 0,
            shard: format_v3_shard_id(i64::MIN),
            seqno: tx.mc_block_seqno,
        },
        mc_block_seqno: tx.mc_block_seqno,
        emulated: false,
        trace_id: Some(tx.hash.to_base64()),
        trace_external_hash: Some(trace_external_hash),
        finality: "finalized".to_owned(),
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
        hash: msg.hash.to_base64(),
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
        code_hash: item.code_hash.to_base64(),
        collection: item
            .collection_address
            .as_ref()
            .map(|address| response::NftCollectionRef {
                address: address.to_string(),
            }),
        collection_address: item.collection_address.as_ref().map(ToString::to_string),
        content: object_fields(&item.content),
        data_hash: item.data_hash.to_base64(),
        index: item.index.clone(),
        init: item.init,
        last_transaction_lt: item.last_transaction_lt.to_string(),
        on_sale: false,
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
        ..Default::default()
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
        account_state_hash: state.account_state_hash.to_base64(),
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
        extra_currencies: HashMap::new(),
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
    let mut address_book = response::AddressBook::new();
    collect_transactions(tn, &mut transactions, &mut transactions_order, emulated);
    address_book.insert_trace(tn);

    response::TracesResponse {
        address_book,
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
        gas_used: response::StringOrNumber::Unsigned(result.gas_used),
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
        external_hash: tn.external_hash.as_ref().map(Hash256::to_base64),
        mc_seqno_start: tn.transaction.meta.block_seqno.to_string(),
        mc_seqno_end: tn.transaction.meta.block_seqno.to_string(),
        start_lt: tn.transaction.meta.lt.to_string(),
        start_utime: tn.transaction.meta.now,
        end_lt: Some(tn.max_lt().to_string()),
        end_utime: Some(tn.max_utime()),
        is_incomplete: false,
        trace: Some(map_trace_node(tn, emulated)),
        transactions,
        transactions_order,
        actions: Vec::new(),
        trace_info: response::TraceInfo {
            transactions: transaction_count,
            messages: transaction_count.saturating_sub(1) + tn.children.len(),
            pending_messages: 0,
            trace_state: "complete".to_owned(),
            classification_state: "classified".to_owned(),
        },
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
        orig_status: tx_details.orig_status.to_owned(),
        end_status: tx_details.end_status.to_owned(),
        total_fees: tx.meta.total_fees.to_string(),
        total_fees_extra_currencies: HashMap::new(),
        prev_trans_hash: tx_details.prev_trans_hash,
        prev_trans_lt: tx_details.prev_trans_lt,
        description: response::TransactionDescr {
            kind: "ord".to_owned(),
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
        },
        in_msg: tx
            .in_msg
            .as_ref()
            .map(|m| map_trace_message_info(m, &tx.meta.tx_hash, tx.meta.now, true)),
        out_msgs: tx
            .out_msgs
            .iter()
            .map(|m| map_trace_message_info(m, &tx.meta.tx_hash, tx.meta.now, false))
            .collect(),
        account_state_before: map_transaction_account_state(
            tx.account_state_before.as_ref(),
            &tx_details.account_state_before_hash,
        ),
        account_state_after: map_transaction_account_state(
            tx.account_state_after.as_ref(),
            &tx_details.account_state_after_hash,
        ),
        block_ref: response::BlockId {
            workchain: 0,
            shard: format_v3_shard_id(i64::MIN),
            seqno: tx.meta.block_seqno,
        },
        mc_block_seqno: tx.meta.block_seqno,
        child_transactions: Vec::new(),
        emulated,
        trace_id: Some(tx.meta.tx_hash.to_base64()),
        trace_external_hash: Some(trace_external_hash),
        finality: if emulated { "pending" } else { "finalized" }.to_owned(),
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
) -> response::AccountState {
    if let Some(snapshot) = snapshot {
        let data_hash = snapshot.data_hash();
        let code_hash = snapshot.code_hash();
        return response::AccountState {
            hash: snapshot.hash.to_base64(),
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

    response::AccountState {
        hash: fallback_hash.to_owned(),
        account_status: None,
        balance: None,
        code_boc: None,
        code_hash: None,
        data_boc: None,
        data_hash: None,
        extra_currencies: None,
        frozen_hash: None,
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
        hash: msg.msg_hash.to_base64(),
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
            kind: "list".to_owned(),
            value: response::StackValue::Entries(Vec::new()),
        },
        "tvm.stackEntryNumber" => response::StackEntity {
            kind: "num".to_owned(),
            value: response::StackValue::Json(map_v3_stack_number(entry.pointer("/number/number"))),
        },
        "tvm.stackEntryCell" => response::StackEntity {
            kind: "cell".to_owned(),
            value: response::StackValue::Json(
                entry.pointer("/cell/bytes").cloned().unwrap_or(Value::Null),
            ),
        },
        "tvm.stackEntrySlice" => response::StackEntity {
            kind: "slice".to_owned(),
            value: response::StackValue::Json(
                entry
                    .pointer("/slice/bytes")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        },
        "tvm.stackEntryBuilder" => response::StackEntity {
            kind: "builder".to_owned(),
            value: response::StackValue::Json(
                entry
                    .pointer("/builder/bytes")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
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
        "tvm.stackEntryList" => {
            let elements = entry
                .pointer("/list/elements")
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
                kind: "list".to_owned(),
                value: response::StackValue::Entries(elements),
            }
        }
        _ => response::StackEntity {
            kind: entry_type.to_owned(),
            value: response::StackValue::Json(entry),
        },
    }
}

fn map_v3_stack_number(value: Option<&Value>) -> Value {
    let Some(value) = value.and_then(Value::as_str) else {
        return Value::Null;
    };
    if value.starts_with("0x") {
        return Value::String(value.to_owned());
    }

    BigInt::parse_bytes(value.as_bytes(), 10).map_or(Value::Null, |value| {
        Value::String(format!("0x{}", value.to_str_radix(16)))
    })
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

const fn map_wallet_information_status(status: &AccountStatus) -> &'static str {
    match status {
        AccountStatus::Active => "active",
        AccountStatus::Uninit | AccountStatus::Nonexist => "uninit",
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
        format_v3_shard_id, map_address_information, map_blocks_response, map_jetton_masters,
        map_jetton_wallets, map_nft_collection_token_info, map_nft_item_token_info, map_nft_items,
        map_transaction_account_state,
    };
    use crate::localnet::{LocalnetAccountState, LocalnetBlock, LocalnetBlockId};
    use crate::storage::{JettonMasterMeta, JettonWalletMeta, NftItemMeta};
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
        let admin_address = master.admin_address.expect("admin address").to_string();
        let mapped = map_jetton_masters(&[master]);
        let metadata = mapped.metadata.get(&address).expect("master metadata");
        let token_info = &metadata.token_info[0];

        let master_row = mapped
            .address_book
            .get(&address)
            .expect("master address row");
        assert!(master_row.user_friendly.is_some());
        assert_eq!(
            master_row.interfaces.as_deref(),
            Some(["jetton_master".to_owned()].as_slice())
        );
        assert!(mapped.address_book.contains_key(&admin_address));

        assert!(metadata.is_indexed);
        assert_eq!(token_info.kind.as_deref(), Some("jetton_masters"));
        assert_eq!(token_info.name.as_deref(), Some("UTYA"));
        assert_eq!(token_info.symbol.as_deref(), Some("UTYA"));
        assert_eq!(token_info.description.as_deref(), Some("Duck token"));
        assert_eq!(token_info.extra["decimals"].as_str(), Some("9"));
    }

    #[test]
    fn nft_item_token_info_uses_nft_items_type() {
        let item = sample_nft_item();
        let item_address = item.address.to_string();
        let owner_address = item.owner_address.expect("owner address").to_string();
        let collection_address = item
            .collection_address
            .expect("collection address")
            .to_string();
        let token_info = map_nft_item_token_info(&item);
        let mapped = map_nft_items(&[item]);

        assert_eq!(token_info.kind.as_deref(), Some("nft_items"));
        assert_eq!(token_info.nft_index.as_deref(), Some("7"));
        assert_eq!(token_info.name.as_deref(), Some("Sample NFT"));
        assert_eq!(mapped.address_book.len(), 3);
        assert_eq!(
            mapped.address_book[&item_address].interfaces.as_deref(),
            Some(["nft_item".to_owned()].as_slice())
        );
        assert!(mapped.address_book.contains_key(&owner_address));
        assert_eq!(
            mapped.address_book[&collection_address]
                .interfaces
                .as_deref(),
            Some(["nft_collection".to_owned()].as_slice())
        );
    }

    #[test]
    fn jetton_wallets_response_includes_wallet_owner_and_master_addresses() {
        let wallet = JettonWalletMeta {
            address: "0:4444444444444444444444444444444444444444444444444444444444444444"
                .parse()
                .expect("valid wallet address"),
            balance: 10,
            code_hash: Hash256([4; 32]),
            data_hash: Hash256([5; 32]),
            jetton_address: sample_jetton_master().address,
            last_transaction_lt: 43,
            owner_address: "0:5555555555555555555555555555555555555555555555555555555555555555"
                .parse()
                .expect("valid owner address"),
        };
        let wallet_address = wallet.address.to_string();
        let owner_address = wallet.owner_address.to_string();
        let master_address = wallet.jetton_address.to_string();
        let mapped = map_jetton_wallets(&[wallet]);

        assert_eq!(mapped.address_book.len(), 3);
        assert_eq!(
            mapped.address_book[&wallet_address].interfaces.as_deref(),
            Some(["jetton_wallet".to_owned()].as_slice())
        );
        assert!(mapped.address_book.contains_key(&owner_address));
        assert_eq!(
            mapped.address_book[&master_address].interfaces.as_deref(),
            Some(["jetton_master".to_owned()].as_slice())
        );
    }

    #[test]
    fn transaction_account_state_without_snapshot_contains_only_hash() {
        let mapped = map_transaction_account_state(None, "state-hash");

        assert_eq!(mapped.hash, "state-hash");
        assert!(mapped.balance.is_none());
        assert!(mapped.account_status.is_none());
        assert!(mapped.extra_currencies.is_none());
        assert!(mapped.code_boc.is_none());
        assert!(mapped.data_boc.is_none());
    }

    #[test]
    fn missing_address_information_omits_optional_state_fields() {
        let address = "0:6666666666666666666666666666666666666666666666666666666666666666"
            .parse()
            .expect("valid address");
        let mapped = map_address_information(&LocalnetAccountState::empty(
            address,
            LocalnetBlockId {
                workchain: 0,
                shard: i64::MIN,
                seqno: 0,
                root_hash: Hash256([0; 32]),
                file_hash: Hash256([0; 32]),
            },
            0,
        ));

        assert!(mapped.code.is_none());
        assert!(mapped.data.is_none());
        assert!(mapped.frozen_hash.is_none());
        assert_eq!(mapped.last_transaction_lt.as_deref(), Some("0"));
        assert_eq!(mapped.status, "uninitialized");
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
        assert_eq!(block.masterchain_block_ref.shard, "8000000000000000");
    }
}
