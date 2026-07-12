use super::toncenter_enrichment::{
    build_extra_data_for_addresses, build_metadata_for_addresses, load_address_infos,
    map_address_book_row, map_address_info,
};
use crate::api::{toncenter_v2 as v2, toncenter_v3, toncenter_wallet};
use crate::localnet;
use crate::localnet::{Localnet, LocalnetBlock, LocalnetJettonWalletsQuery, LocalnetTransaction};
use crate::storage::{AccountStatus, JettonMasterMeta, TraceNode};
use crate::types::{Addr, Hash256};
use axum::{
    Json,
    extract::{Query, RawQuery, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::Engine;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use ton_api::toncenter::v3 as v3_types;
use ton_api::toncenter::v3::requests::{
    AccountStatesQuery, AddressInformationQuery, AddressesQuery, AdjacentTransactionsQuery,
    BlocksQuery, JettonMastersQuery, JettonWalletsQuery, MessagesQuery, NftItemsQuery,
    PendingTransactionsQuery, RunGetMethodRequest, SendMessageRequest, StackEntry, TracesQuery,
    TransactionsByMasterchainBlockQuery, TransactionsByMessageQuery, TransactionsQuery,
    WalletInformationQuery, WalletStatesQuery,
};
use toncenter_v3 as v3;

const BLOCK_WORKCHAIN: i32 = 0;
const BLOCK_SHARD: i64 = i64::MIN;

macro_rules! parse {
    ($expression:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => return v3_bad_request(error.to_string()),
        }
    };
}

pub async fn get_traces(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<TracesQuery>(raw_query.as_deref()));
    match collect_v3_traces(node.as_ref(), payload).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => v3_bad_request(e.to_string()),
    }
}

async fn collect_v3_traces(
    node: &Localnet,
    payload: TracesQuery,
) -> anyhow::Result<v3_types::TracesResponse> {
    let filter_count = usize::from(payload.account.is_some())
        + usize::from(!payload.trace_id.is_empty())
        + usize::from(!payload.tx_hash.is_empty())
        + usize::from(!payload.msg_hash.is_empty());
    if filter_count != 1 {
        anyhow::bail!("Exactly one of `account`, `trace_id`, `tx_hash`, or `msg_hash` is required");
    }

    let (limit, offset) = parse_limit_offset(payload.limit, payload.offset)?;
    let sort = parse_sort(payload.sort)?;
    let mc_seqno = parse_non_negative_u32("mc_seqno", payload.mc_seqno)?;
    let start_utime = parse_non_negative_u32("start_utime", payload.start_utime)?;
    let end_utime = parse_non_negative_u32("end_utime", payload.end_utime)?;

    let tx_hashes = if let Some(account) = payload.account {
        let account = Addr::parse(&account)?;
        node.get_all_transactions()
            .await?
            .into_iter()
            .filter(|tx| tx.address == account)
            .map(|tx| tx.hash)
            .collect::<HashSet<_>>()
    } else {
        payload
            .trace_id
            .into_iter()
            .chain(payload.tx_hash)
            .map(|hash| parse_hash_any(&hash))
            .collect::<anyhow::Result<HashSet<_>>>()?
    };
    let msg_hashes = payload
        .msg_hash
        .into_iter()
        .map(|hash| parse_hash_any(&hash))
        .collect::<anyhow::Result<HashSet<_>>>()?;

    let mut traces = Vec::new();
    let mut seen = HashSet::new();
    let mut address_book = v3_types::AddressBook::new();
    let mut metadata = v3_types::Metadata::new();
    for result in futures_for_trace_hashes(node, tx_hashes, msg_hashes).await {
        let trace = match result {
            Ok(trace) => trace,
            Err(e) if is_trace_not_found_error(&e) => continue,
            Err(e) => return Err(e),
        };
        let mapped = v3::map_traces(&trace);
        address_book.extend(mapped.address_book);
        metadata.extend(mapped.metadata);
        traces.extend(
            mapped
                .traces
                .into_iter()
                .filter(|trace| seen.insert(trace.trace_id.clone())),
        );
    }

    traces.retain(|trace| {
        mc_seqno.is_none_or(|seqno| {
            trace.mc_seqno_start == seqno.to_string() || trace.mc_seqno_end == seqno.to_string()
        }) && start_utime.is_none_or(|start| trace.start_utime >= start)
            && end_utime.is_none_or(|end| trace.end_utime.is_some_and(|value| value <= end))
            && payload.start_lt.is_none_or(|start| {
                trace
                    .start_lt
                    .parse::<u64>()
                    .ok()
                    .is_some_and(|value| value >= start)
            })
            && payload.end_lt.is_none_or(|end| {
                trace
                    .end_lt
                    .as_deref()
                    .and_then(|value| value.parse::<u64>().ok())
                    .is_some_and(|value| value <= end)
            })
    });
    traces.sort_by_key(|trace| trace.start_lt.parse::<u64>().ok().unwrap_or_default());
    if matches!(sort, SortOrder::Desc) {
        traces.reverse();
    }
    traces = traces.into_iter().skip(offset).take(limit).collect();

    Ok(v3_types::TracesResponse {
        traces,
        address_book,
        metadata,
    })
}

async fn futures_for_trace_hashes(
    node: &Localnet,
    tx_hashes: HashSet<Hash256>,
    msg_hashes: HashSet<Hash256>,
) -> Vec<anyhow::Result<TraceNode>> {
    let mut results = Vec::with_capacity(tx_hashes.len() + msg_hashes.len());
    for hash in tx_hashes {
        results.push(node.get_traces(hash).await);
    }
    for hash in msg_hashes {
        results.push(node.get_traces_by_message_hash(hash).await);
    }
    results
}

pub async fn get_address_information_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationQuery>,
) -> impl IntoResponse {
    let _use_v2 = payload.use_v2.unwrap_or(true);

    handle_v3_result(
        node.get_address_information(payload.address, None),
        toncenter_v3::map_address_information,
    )
    .await
}

pub async fn get_wallet_information_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<WalletInformationQuery>,
) -> impl IntoResponse {
    let _use_v2 = payload.use_v2.unwrap_or(true);

    handle_v3_result(
        async move {
            let state = node
                .get_address_information(payload.address.clone(), None)
                .await?;
            let wallet_type = v2::wallet_type_name_from_code_hash(state.code_hash.as_ref());
            let parsed_wallet = toncenter_wallet::read_standard_wallet_state(&state).ok();
            let seqno = if let Some(wallet) = parsed_wallet {
                Some(wallet.seqno)
            } else if wallet_type.is_some() {
                node.run_get_method(payload.address, "seqno".to_owned(), Vec::new(), None)
                    .await
                    .ok()
                    .and_then(|result| v2::map_wallet_seqno(&result))
            } else {
                None
            };
            let wallet_id = parsed_wallet.and_then(|wallet| wallet.wallet_id);

            Ok(v3::map_wallet_information_v3(
                &state,
                wallet_type,
                seqno,
                wallet_id,
            ))
        },
        Clone::clone,
    )
    .await
}

pub async fn get_masterchain_info_v3(State(node): State<Arc<Localnet>>) -> impl IntoResponse {
    handle_v3_result(
        async move {
            let blocks = node.get_blocks().await?;
            v3::map_masterchain_info_v3(&blocks)
                .ok_or_else(|| anyhow::anyhow!("Masterchain has no blocks"))
        },
        Clone::clone,
    )
    .await
}

pub async fn get_address_book_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<AddressesQuery>(raw_query.as_deref()));
    let addresses = parse!(parse_requested_addresses(payload.address));
    let infos = match load_address_infos(
        node.as_ref(),
        addresses.iter().map(|(_, address)| *address).collect(),
    )
    .await
    {
        Ok(infos) => infos,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let address_book = addresses
        .into_iter()
        .map(|(requested, address)| {
            let info = infos.get(&address).cloned().unwrap_or_default();
            (requested, map_address_book_row(address, &info))
        })
        .collect::<v3_types::AddressBook>();

    (StatusCode::OK, Json(address_book)).into_response()
}

pub async fn get_metadata_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<AddressesQuery>(raw_query.as_deref()));
    let addresses = parse!(parse_requested_addresses(payload.address));
    let addresses = addresses.into_iter().map(|(_, address)| address).collect();
    match build_metadata_for_addresses(node.as_ref(), addresses).await {
        Ok(metadata) => (StatusCode::OK, Json(metadata)).into_response(),
        Err(e) => request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

pub async fn get_account_states_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_account_states_request(raw_query.as_deref()));
    let parsed = parse!(parse_account_states_v3_query(payload));

    let states_with_info = match node.get_account_states(parsed.addresses, None).await {
        Ok(states) => states,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut states = Vec::with_capacity(states_with_info.len());
    let mut context_by_address = HashMap::with_capacity(states_with_info.len());

    for state_with_info in states_with_info {
        let address = state_with_info.state.address;
        let info = map_address_info(state_with_info.info);
        context_by_address.insert(
            address,
            v3::AccountStateContext {
                interfaces: info.interfaces.into_iter().collect(),
                token_info: info.token_info,
                user_friendly: address.as_user_friendly(),
            },
        );
        states.push(state_with_info.state);
    }

    (
        StatusCode::OK,
        Json(v3::map_account_states(
            &states,
            &context_by_address,
            parsed.include_boc,
        )),
    )
        .into_response()
}

pub async fn get_transactions_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<TransactionsQuery>(raw_query.as_deref()));
    let parsed = parse!(parse_transactions_v3_query(payload));

    match transactions_fast_path(&parsed) {
        Some(TransactionsFastPath::Empty) => {
            return (StatusCode::OK, Json(v3::map_transactions_response(&[]))).into_response();
        }
        Some(TransactionsFastPath::Block { seqno }) => {
            let descending = matches!(parsed.sort, SortOrder::Desc);
            return handle_v3_result(
                node.get_block_transactions_page(seqno, parsed.limit, parsed.offset, descending),
                |txs| v3::map_transactions_response(txs),
            )
            .await;
        }
        Some(TransactionsFastPath::Recent) => {
            let descending = matches!(parsed.sort, SortOrder::Desc);
            return handle_v3_result(
                node.get_all_transactions_page(parsed.limit, parsed.offset, descending),
                |txs| v3::map_transactions_response(txs),
            )
            .await;
        }
        None => {}
    }

    handle_v3_result(node.get_all_transactions(), move |txs| {
        let filtered = filter_transactions_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_blocks_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<BlocksQuery>,
) -> impl IntoResponse {
    let parsed = parse!(parse_blocks_v3_query(payload));

    handle_v3_result(node.get_blocks(), move |blocks| {
        let filtered = filter_blocks_v3(blocks, &parsed);
        v3::map_blocks_response(&filtered)
    })
    .await
}

pub async fn get_transactions_by_message_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TransactionsByMessageQuery>,
) -> impl IntoResponse {
    let parsed = parse!(parse_transactions_by_message_v3_query(payload));

    handle_v3_result(node.get_all_transactions(), move |txs| {
        let filtered = filter_transactions_by_message_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_transactions_by_masterchain_block_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TransactionsByMasterchainBlockQuery>,
) -> impl IntoResponse {
    let seqno = parse!(parse_required_non_negative_u32("seqno", payload.seqno));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let sort = parse!(parse_sort(payload.sort));

    handle_v3_result(
        node.get_block_transactions_page(seqno, limit, offset, matches!(sort, SortOrder::Desc)),
        |txs| v3::map_transactions_response(txs),
    )
    .await
}

pub async fn get_messages_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<MessagesQuery>(raw_query.as_deref()));
    let parsed = parse!(parse_messages_v3_query(payload));
    let transactions = match node.get_all_transactions().await {
        Ok(transactions) => transactions,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let (messages, addresses) = collect_messages_v3(&transactions, &parsed);
    let (address_book, metadata) =
        match build_extra_data_for_addresses(node.as_ref(), addresses, true, true).await {
            Ok(extra) => extra,
            Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

    (
        StatusCode::OK,
        Json(v3_types::MessagesResponse {
            messages,
            address_book: address_book.unwrap_or_default(),
            metadata: metadata.unwrap_or_default(),
        }),
    )
        .into_response()
}

pub async fn get_adjacent_transactions_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AdjacentTransactionsQuery>,
) -> impl IntoResponse {
    let hash = parse!(parse_hash_any(&payload.hash));
    let direction = parse!(parse_message_direction(payload.direction.as_deref()));
    let transactions = match node.get_all_transactions().await {
        Ok(transactions) => transactions,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let adjacent = collect_adjacent_transactions_v3(&transactions, hash, direction);
    if adjacent.is_empty() {
        return request_error(StatusCode::NOT_FOUND, "adjacent transactions not found");
    }

    (
        StatusCode::OK,
        Json(v3::map_transactions_response(&adjacent)),
    )
        .into_response()
}

pub async fn get_wallet_states_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<WalletStatesQuery>(raw_query.as_deref()));
    let addresses = parse!(parse_required_addresses(payload.address));
    let states = match node.get_account_states(addresses, None).await {
        Ok(states) => states,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let mut wallet_states = Vec::with_capacity(states.len());
    let mut existing_addresses = Vec::with_capacity(states.len());
    for item in states {
        if item.state.state == AccountStatus::Nonexist {
            continue;
        }
        let wallet_type = v2::wallet_type_name_from_code_hash(item.state.code_hash.as_ref());
        let wallet = toncenter_wallet::read_standard_wallet_state(&item.state).ok();
        existing_addresses.push(item.state.address);
        wallet_states.push(v3::map_wallet_state_v3(
            &item.state,
            wallet_type,
            wallet.as_ref(),
        ));
    }

    let (address_book, metadata) =
        match build_extra_data_for_addresses(node.as_ref(), existing_addresses, true, true).await {
            Ok(extra) => extra,
            Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

    (
        StatusCode::OK,
        Json(v3_types::WalletStatesResponse {
            wallets: wallet_states,
            address_book: address_book.unwrap_or_default(),
            metadata: metadata.unwrap_or_default(),
        }),
    )
        .into_response()
}

pub async fn get_pending_transactions_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<PendingTransactionsQuery>(
        raw_query.as_deref()
    ));
    let parsed = parse!(parse_pending_transactions_v3_query(payload));

    handle_v3_result(node.get_pending_transactions(), move |txs| {
        let filtered = filter_pending_transactions_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_jetton_masters(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<JettonMastersQuery>(raw_query.as_deref()));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    handle_v3_result(
        node.get_jetton_masters(
            payload.address,
            payload.admin_address,
            Some(limit),
            Some(offset),
        ),
        |masters| v3::map_jetton_masters(masters),
    )
    .await
}

pub async fn get_jetton_wallets(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<JettonWalletsQuery>(raw_query.as_deref()));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let sort = parse!(parse_sort(payload.sort));
    let wallets = match node
        .get_jetton_wallets(LocalnetJettonWalletsQuery {
            addresses: payload.address,
            owner_addresses: payload.owner_address,
            jetton_addresses: payload.jetton_address,
            exclude_zero_balance: payload.exclude_zero_balance,
            descending: matches!(sort, SortOrder::Desc),
            limit: Some(limit),
            offset: Some(offset),
        })
        .await
    {
        Ok(wallets) => wallets,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    let mut masters_by_jetton: HashMap<Addr, JettonMasterMeta> = HashMap::new();
    let unique_jettons: BTreeSet<Addr> =
        wallets.iter().map(|wallet| wallet.jetton_address).collect();
    for jetton_address in unique_jettons {
        let lookup_result = node
            .get_jetton_masters(
                vec![jetton_address.to_string()],
                Vec::new(),
                Some(1),
                Some(0),
            )
            .await;
        if let Ok(mut masters) = lookup_result
            && let Some(master) = masters.pop()
        {
            masters_by_jetton.insert(jetton_address, master);
        }
    }

    (
        StatusCode::OK,
        Json(v3::map_jetton_wallets_with_metadata(
            &wallets,
            &masters_by_jetton,
        )),
    )
        .into_response()
}

pub async fn get_nft_items(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<NftItemsQuery>(raw_query.as_deref()));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));

    handle_v3_result(
        node.get_nft_items(
            payload.address,
            payload.owner_address,
            payload.collection_address,
            payload.index,
            payload.sort_by_last_transaction_lt,
            Some(limit),
            Some(offset),
        ),
        |items| v3::map_nft_items(items),
    )
    .await
}

pub async fn send_message_v3(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendMessageRequest>,
) -> impl IntoResponse {
    handle_v3_result(node.send_boc(payload.boc), toncenter_v3::map_send_message).await
}

pub async fn run_get_method_v3(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> impl IntoResponse {
    let stack = match normalize_v3_stack(payload.stack) {
        Ok(stack) => stack,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_v3_result(
        node.run_get_method(payload.address, payload.method, stack, None),
        toncenter_v3::map_run_get_method_v3,
    )
    .await
}

fn normalize_v3_stack(stack: Vec<StackEntry>) -> anyhow::Result<Vec<Value>> {
    stack.into_iter().map(normalize_v3_stack_item).collect()
}

fn normalize_v3_stack_item(item: StackEntry) -> anyhow::Result<Value> {
    match item.kind.as_str() {
        "null" => Ok(json!(["null", Value::Null])),
        "num" => Ok(json!(["num", item.value])),
        "cell" | "slice" | "builder" => {
            let bytes = extract_stack_bytes(&item.value, &item.kind)?;
            Ok(json!([item.kind, { "bytes": bytes }]))
        }
        "tuple" | "list" => {
            let elements = item
                .value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("{} stack value must be an array", item.kind))?
                .iter()
                .cloned()
                .map(serde_json::from_value::<StackEntry>)
                .map(|item| {
                    item.map_err(anyhow::Error::from)
                        .and_then(normalize_v3_stack_item)
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(json!([item.kind, { "elements": elements }]))
        }
        _ => anyhow::bail!("Unsupported v3 stack entry type: {}", item.kind),
    }
}

fn extract_stack_bytes(value: &Value, stack_type: &str) -> anyhow::Result<String> {
    if let Some(b64) = value.as_str() {
        return Ok(b64.to_owned());
    }
    if let Some(b64) = value.get("bytes").and_then(Value::as_str) {
        return Ok(b64.to_owned());
    }
    anyhow::bail!("{stack_type} stack value must be a base64 string or an object with `bytes`")
}

#[derive(Clone, Copy)]
enum SortOrder {
    Asc,
    Desc,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageDirection {
    In,
    Out,
}

#[derive(Debug, PartialEq, Eq)]
enum TransactionsFastPath {
    Empty,
    Block { seqno: u32 },
    Recent,
}

struct ParsedTransactionsV3Query {
    workchain: Option<i32>,
    shard: Option<i64>,
    seqno: Option<u32>,
    mc_seqno: Option<u32>,
    account: Option<HashSet<Addr>>,
    exclude_account: Option<HashSet<Addr>>,
    hash: Option<Hash256>,
    lt: Option<u64>,
    start_utime: Option<u32>,
    end_utime: Option<u32>,
    start_lt: Option<u64>,
    end_lt: Option<u64>,
    limit: usize,
    offset: usize,
    sort: SortOrder,
}

struct ParsedBlocksV3Query {
    workchain: Option<i32>,
    shard: Option<i64>,
    seqno: Option<u32>,
    root_hash: Option<Hash256>,
    file_hash: Option<Hash256>,
    mc_seqno: Option<u32>,
    start_utime: Option<u32>,
    end_utime: Option<u32>,
    start_lt: Option<u64>,
    end_lt: Option<u64>,
    limit: usize,
    offset: usize,
    sort: SortOrder,
}

struct ParsedTransactionsByMessageV3Query {
    msg_hash: Option<Hash256>,
    body_hash: Option<Hash256>,
    opcode: Option<u32>,
    direction: Option<MessageDirection>,
    limit: usize,
    offset: usize,
}

struct ParsedMessagesV3Query {
    msg_hashes: Option<HashSet<Hash256>>,
    body_hash: Option<Hash256>,
    source: Option<NullableAddressFilter>,
    destination: Option<NullableAddressFilter>,
    opcode: Option<u32>,
    start_utime: Option<u32>,
    end_utime: Option<u32>,
    start_lt: Option<u64>,
    end_lt: Option<u64>,
    direction: Option<MessageDirection>,
    exclude_externals: bool,
    only_externals: bool,
    limit: usize,
    offset: usize,
    sort: SortOrder,
}

#[derive(Clone, Copy)]
enum NullableAddressFilter {
    Null,
    Address(Addr),
}

struct CollectedMessage {
    message: v3_types::Message,
    source: Option<Addr>,
    destination: Option<Addr>,
    created_lt: u64,
    hash: Hash256,
}

struct ParsedPendingTransactionsV3Query {
    account: Option<HashSet<Addr>>,
    trace_ids: Option<HashSet<Hash256>>,
}

struct ParsedAccountStatesV3Query {
    addresses: Vec<Addr>,
    include_boc: bool,
}

fn parse_transactions_v3_query(
    payload: TransactionsQuery,
) -> anyhow::Result<ParsedTransactionsV3Query> {
    if payload.shard.is_some() && payload.workchain.is_none() {
        anyhow::bail!("`shard` requires `workchain`");
    }
    if payload.seqno.is_some() && (payload.workchain.is_none() || payload.shard.is_none()) {
        anyhow::bail!("`seqno` requires both `workchain` and `shard`");
    }

    let (limit, offset) = parse_limit_offset(payload.limit, payload.offset)?;
    let sort = parse_sort(payload.sort)?;

    Ok(ParsedTransactionsV3Query {
        workchain: payload.workchain,
        shard: payload
            .shard
            .as_deref()
            .map(parse_shard_query)
            .transpose()?,
        seqno: parse_non_negative_u32("seqno", payload.seqno)?,
        mc_seqno: parse_non_negative_u32("mc_seqno", payload.mc_seqno)?,
        account: parse_addresses(payload.account)?,
        exclude_account: parse_addresses(payload.exclude_account)?,
        hash: payload.hash.as_deref().map(parse_hash_any).transpose()?,
        lt: payload.lt,
        start_utime: parse_non_negative_u32("start_utime", payload.start_utime)?,
        end_utime: parse_non_negative_u32("end_utime", payload.end_utime)?,
        start_lt: payload.start_lt,
        end_lt: payload.end_lt,
        limit,
        offset,
        sort,
    })
}

fn parse_blocks_v3_query(payload: BlocksQuery) -> anyhow::Result<ParsedBlocksV3Query> {
    if payload.shard.is_some() && payload.workchain.is_none() {
        anyhow::bail!("`shard` requires `workchain`");
    }
    if payload.seqno.is_some() && (payload.workchain.is_none() || payload.shard.is_none()) {
        anyhow::bail!("`seqno` requires both `workchain` and `shard`");
    }

    let (limit, offset) = parse_limit_offset(payload.limit, payload.offset)?;
    let sort = parse_sort(payload.sort)?;

    Ok(ParsedBlocksV3Query {
        workchain: payload.workchain,
        shard: payload
            .shard
            .as_deref()
            .map(parse_shard_query)
            .transpose()?,
        seqno: parse_non_negative_u32("seqno", payload.seqno)?,
        root_hash: payload
            .root_hash
            .as_deref()
            .map(parse_hash_any)
            .transpose()?,
        file_hash: payload
            .file_hash
            .as_deref()
            .map(parse_hash_any)
            .transpose()?,
        mc_seqno: parse_non_negative_u32("mc_seqno", payload.mc_seqno)?,
        start_utime: parse_non_negative_u32("start_utime", payload.start_utime)?,
        end_utime: parse_non_negative_u32("end_utime", payload.end_utime)?,
        start_lt: payload.start_lt,
        end_lt: payload.end_lt,
        limit,
        offset,
        sort,
    })
}

fn parse_transactions_by_message_v3_query(
    payload: TransactionsByMessageQuery,
) -> anyhow::Result<ParsedTransactionsByMessageV3Query> {
    let (limit, offset) = parse_limit_offset(payload.limit, payload.offset)?;
    let direction = parse_message_direction(payload.direction.as_deref())?;

    Ok(ParsedTransactionsByMessageV3Query {
        msg_hash: payload
            .msg_hash
            .as_deref()
            .map(parse_hash_any)
            .transpose()?,
        body_hash: payload
            .body_hash
            .as_deref()
            .map(parse_hash_any)
            .transpose()?,
        opcode: payload.opcode.as_deref().map(parse_opcode).transpose()?,
        direction,
        limit,
        offset,
    })
}

fn parse_messages_v3_query(payload: MessagesQuery) -> anyhow::Result<ParsedMessagesV3Query> {
    let (limit, offset) = parse_limit_offset(payload.limit, payload.offset)?;
    Ok(ParsedMessagesV3Query {
        msg_hashes: parse_hashes(payload.msg_hash)?,
        body_hash: payload
            .body_hash
            .as_deref()
            .map(parse_hash_any)
            .transpose()?,
        source: payload
            .source
            .as_deref()
            .map(parse_nullable_address_filter)
            .transpose()?,
        destination: payload
            .destination
            .as_deref()
            .map(parse_nullable_address_filter)
            .transpose()?,
        opcode: payload.opcode.as_deref().map(parse_opcode).transpose()?,
        start_utime: parse_non_negative_u32("start_utime", payload.start_utime)?,
        end_utime: parse_non_negative_u32("end_utime", payload.end_utime)?,
        start_lt: payload.start_lt,
        end_lt: payload.end_lt,
        direction: parse_message_direction(payload.direction.as_deref())?,
        exclude_externals: payload.exclude_externals.unwrap_or(false),
        only_externals: payload.only_externals.unwrap_or(false),
        limit,
        offset,
        sort: parse_sort(payload.sort)?,
    })
}

fn parse_pending_transactions_v3_query(
    payload: PendingTransactionsQuery,
) -> anyhow::Result<ParsedPendingTransactionsV3Query> {
    Ok(ParsedPendingTransactionsV3Query {
        account: parse_addresses(payload.account)?,
        trace_ids: parse_hashes(payload.trace_id)?,
    })
}

fn parse_account_states_v3_query(
    payload: AccountStatesQuery,
) -> anyhow::Result<ParsedAccountStatesV3Query> {
    let addresses = payload.address;
    if addresses.is_empty() {
        anyhow::bail!("`address` must not be empty");
    }
    if addresses.len() > 1000 {
        anyhow::bail!("Maximum 1000 addresses allowed");
    }

    Ok(ParsedAccountStatesV3Query {
        addresses: addresses
            .into_iter()
            .map(|address| Addr::parse(&address))
            .collect::<anyhow::Result<Vec<_>>>()?,
        include_boc: payload.include_boc.unwrap_or(true),
    })
}

fn parse_account_states_request(raw_query: Option<&str>) -> anyhow::Result<AccountStatesQuery> {
    parse_v3_query(raw_query)
}

fn filter_transactions_v3(
    txs: &[LocalnetTransaction],
    query: &ParsedTransactionsV3Query,
) -> Vec<LocalnetTransaction> {
    let mut filtered = txs
        .iter()
        .filter(|tx| {
            if let Some(workchain) = query.workchain
                && workchain != BLOCK_WORKCHAIN
            {
                return false;
            }
            if let Some(shard) = query.shard
                && shard != BLOCK_SHARD
            {
                return false;
            }
            if let Some(seqno) = query.seqno
                && tx.mc_block_seqno != seqno
            {
                return false;
            }
            if let Some(mc_seqno) = query.mc_seqno
                && tx.mc_block_seqno != mc_seqno
            {
                return false;
            }
            if let Some(accounts) = &query.account
                && !accounts.contains(&tx.address)
            {
                return false;
            }
            if let Some(excluded) = &query.exclude_account
                && excluded.contains(&tx.address)
            {
                return false;
            }
            if let Some(hash) = query.hash
                && tx.hash != hash
            {
                return false;
            }
            if let Some(lt) = query.lt
                && tx.transaction_id.lt != lt
            {
                return false;
            }
            if let Some(start_utime) = query.start_utime
                && tx.utime <= start_utime
            {
                return false;
            }
            if let Some(end_utime) = query.end_utime
                && tx.utime >= end_utime
            {
                return false;
            }
            if let Some(start_lt) = query.start_lt
                && tx.transaction_id.lt < start_lt
            {
                return false;
            }
            if let Some(end_lt) = query.end_lt
                && tx.transaction_id.lt > end_lt
            {
                return false;
            }
            true
        })
        .cloned()
        .collect::<Vec<_>>();

    sort_transactions(&mut filtered, query.sort);
    filtered
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect()
}

const fn transactions_fast_path(query: &ParsedTransactionsV3Query) -> Option<TransactionsFastPath> {
    let has_expensive_filters = query.account.is_some()
        || query.exclude_account.is_some()
        || query.hash.is_some()
        || query.lt.is_some()
        || query.start_utime.is_some()
        || query.end_utime.is_some()
        || query.start_lt.is_some()
        || query.end_lt.is_some();
    if has_expensive_filters {
        return None;
    }

    if let Some(workchain) = query.workchain
        && workchain != BLOCK_WORKCHAIN
    {
        return Some(TransactionsFastPath::Empty);
    }
    if let Some(shard) = query.shard
        && shard != BLOCK_SHARD
    {
        return Some(TransactionsFastPath::Empty);
    }

    match (query.seqno, query.mc_seqno) {
        (Some(seqno), Some(mc_seqno)) if seqno == mc_seqno => {
            Some(TransactionsFastPath::Block { seqno })
        }
        (Some(_), Some(_)) => Some(TransactionsFastPath::Empty),
        (Some(seqno), None) | (None, Some(seqno)) => Some(TransactionsFastPath::Block { seqno }),
        (None, None) => Some(TransactionsFastPath::Recent),
    }
}

fn filter_blocks_v3(blocks: &[LocalnetBlock], query: &ParsedBlocksV3Query) -> Vec<LocalnetBlock> {
    let mut filtered = blocks
        .iter()
        .filter(|block| {
            if let Some(workchain) = query.workchain
                && block.workchain != workchain
            {
                return false;
            }
            if let Some(shard) = query.shard
                && block.shard != shard
            {
                return false;
            }
            if let Some(seqno) = query.seqno
                && block.seqno != seqno
            {
                return false;
            }
            if let Some(root_hash) = query.root_hash
                && block.root_hash != root_hash
            {
                return false;
            }
            if let Some(file_hash) = query.file_hash
                && block.file_hash != file_hash
            {
                return false;
            }
            if let Some(mc_seqno) = query.mc_seqno
                && block.workchain != -1
                && block
                    .masterchain_block_ref
                    .as_ref()
                    .map(|ref_block| ref_block.seqno)
                    != Some(mc_seqno)
            {
                return false;
            }
            if let Some(mc_seqno) = query.mc_seqno
                && block.workchain == -1
                && block.seqno != mc_seqno
            {
                return false;
            }
            if let Some(start_utime) = query.start_utime
                && block.gen_utime < start_utime
            {
                return false;
            }
            if let Some(end_utime) = query.end_utime
                && block.gen_utime > end_utime
            {
                return false;
            }
            if let Some(start_lt) = query.start_lt
                && block.start_lt < start_lt
            {
                return false;
            }
            if let Some(end_lt) = query.end_lt
                && block.start_lt > end_lt
            {
                return false;
            }
            true
        })
        .cloned()
        .collect::<Vec<_>>();

    sort_blocks(&mut filtered, query.sort);
    filtered
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect()
}

fn filter_transactions_by_message_v3(
    txs: &[LocalnetTransaction],
    query: &ParsedTransactionsByMessageV3Query,
) -> Vec<LocalnetTransaction> {
    let has_message_filter =
        query.msg_hash.is_some() || query.body_hash.is_some() || query.opcode.is_some();
    let mut filtered = txs
        .iter()
        .filter(|tx| {
            if !has_message_filter && query.direction.is_none() {
                return true;
            }

            let mut messages = Vec::new();
            match query.direction {
                Some(MessageDirection::In) => messages.push(&tx.in_msg),
                Some(MessageDirection::Out) => messages.extend(tx.out_msgs.iter()),
                None => {
                    messages.push(&tx.in_msg);
                    messages.extend(tx.out_msgs.iter());
                }
            }

            messages
                .into_iter()
                .filter(|msg| !msg.hash.is_zero())
                .any(|msg| {
                    if let Some(msg_hash) = query.msg_hash
                        && msg.hash != msg_hash
                        && msg.hash_norm != Some(msg_hash)
                    {
                        return false;
                    }
                    if let Some(body_hash) = query.body_hash
                        && msg.body_hash != body_hash
                    {
                        return false;
                    }
                    if let Some(opcode) = query.opcode
                        && msg.opcode != Some(opcode)
                    {
                        return false;
                    }
                    true
                })
        })
        .cloned()
        .collect::<Vec<_>>();

    sort_transactions(&mut filtered, SortOrder::Desc);
    filtered
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect()
}

fn collect_messages_v3(
    transactions: &[LocalnetTransaction],
    query: &ParsedMessagesV3Query,
) -> (Vec<v3_types::Message>, Vec<Addr>) {
    let mut collected = Vec::<CollectedMessage>::new();
    let mut indexes = HashMap::<Hash256, usize>::new();

    for transaction in transactions {
        let messages = std::iter::once((&transaction.in_msg, MessageDirection::In)).chain(
            transaction
                .out_msgs
                .iter()
                .map(|message| (message, MessageDirection::Out)),
        );
        for (message, direction) in messages {
            if message.hash.is_zero()
                || !message_matches_v3(message, transaction.utime, direction, query)
            {
                continue;
            }

            if let Some(index) = indexes.get(&message.hash).copied() {
                let transaction_hash = transaction.hash.to_base64();
                match direction {
                    MessageDirection::In => {
                        collected[index].message.in_msg_tx_hash = Some(transaction_hash);
                    }
                    MessageDirection::Out => {
                        collected[index].message.out_msg_tx_hash = Some(transaction_hash);
                    }
                }
                continue;
            }

            let index = collected.len();
            indexes.insert(message.hash, index);
            collected.push(CollectedMessage {
                message: v3::map_v3_message(
                    message,
                    &transaction.hash,
                    transaction.utime,
                    matches!(direction, MessageDirection::In),
                ),
                source: message.source,
                destination: message.destination,
                created_lt: message.created_lt,
                hash: message.hash,
            });
        }
    }

    collected.sort_by(|a, b| {
        a.created_lt
            .cmp(&b.created_lt)
            .then_with(|| a.hash.cmp(&b.hash))
    });
    if matches!(query.sort, SortOrder::Desc) {
        collected.reverse();
    }

    let selected = collected
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect::<Vec<_>>();
    let addresses = selected
        .iter()
        .flat_map(|item| [item.source, item.destination])
        .flatten()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let messages = selected.into_iter().map(|item| item.message).collect();
    (messages, addresses)
}

fn message_matches_v3(
    message: &localnet::LocalnetMessage,
    utime: u32,
    direction: MessageDirection,
    query: &ParsedMessagesV3Query,
) -> bool {
    if query
        .direction
        .is_some_and(|expected| expected != direction)
    {
        return false;
    }
    if let Some(hashes) = &query.msg_hashes
        && !hashes.contains(&message.hash)
        && message.hash_norm.is_none_or(|hash| !hashes.contains(&hash))
    {
        return false;
    }
    if query
        .body_hash
        .is_some_and(|hash| hash != message.body_hash)
    {
        return false;
    }
    if !nullable_address_matches(query.source, message.source)
        || !nullable_address_matches(query.destination, message.destination)
    {
        return false;
    }
    if query
        .opcode
        .is_some_and(|opcode| message.opcode != Some(opcode))
    {
        return false;
    }
    if query.start_utime.is_some_and(|start| utime < start)
        || query.end_utime.is_some_and(|end| utime > end)
    {
        return false;
    }

    let is_external = message.source.is_none();
    if (query.exclude_externals && is_external) || (query.only_externals && !is_external) {
        return false;
    }
    if (query.start_lt.is_some() || query.end_lt.is_some()) && is_external {
        return false;
    }
    if query
        .start_lt
        .is_some_and(|start| message.created_lt < start)
        || query.end_lt.is_some_and(|end| message.created_lt > end)
    {
        return false;
    }
    true
}

fn nullable_address_matches(filter: Option<NullableAddressFilter>, address: Option<Addr>) -> bool {
    match filter {
        None => true,
        Some(NullableAddressFilter::Null) => address.is_none(),
        Some(NullableAddressFilter::Address(expected)) => address == Some(expected),
    }
}

fn collect_adjacent_transactions_v3(
    transactions: &[LocalnetTransaction],
    hash: Hash256,
    direction: Option<MessageDirection>,
) -> Vec<LocalnetTransaction> {
    let Some(transaction) = transactions
        .iter()
        .find(|transaction| transaction.hash == hash)
    else {
        return Vec::new();
    };

    let mut adjacent_hashes = HashSet::new();
    if !matches!(direction, Some(MessageDirection::Out)) && !transaction.in_msg.hash.is_zero() {
        for candidate in transactions {
            if candidate
                .out_msgs
                .iter()
                .any(|message| message.hash == transaction.in_msg.hash)
            {
                adjacent_hashes.insert(candidate.hash);
            }
        }
    }

    if !matches!(direction, Some(MessageDirection::In)) {
        let out_hashes = transaction
            .out_msgs
            .iter()
            .map(|message| message.hash)
            .collect::<HashSet<_>>();
        for candidate in transactions {
            if out_hashes.contains(&candidate.in_msg.hash) {
                adjacent_hashes.insert(candidate.hash);
            }
        }
    }

    let mut adjacent = transactions
        .iter()
        .filter(|transaction| adjacent_hashes.contains(&transaction.hash))
        .cloned()
        .collect::<Vec<_>>();
    sort_transactions(&mut adjacent, SortOrder::Asc);
    adjacent
}

fn filter_pending_transactions_v3(
    txs: &[LocalnetTransaction],
    query: &ParsedPendingTransactionsV3Query,
) -> Vec<LocalnetTransaction> {
    txs.iter()
        .filter(|tx| {
            if let Some(accounts) = &query.account
                && !accounts.contains(&tx.address)
            {
                return false;
            }
            if let Some(trace_ids) = &query.trace_ids
                && !trace_ids.contains(&tx.hash)
            {
                return false;
            }
            true
        })
        .cloned()
        .collect()
}

fn sort_transactions(transactions: &mut [LocalnetTransaction], order: SortOrder) {
    match order {
        SortOrder::Asc => {
            transactions.sort_by(|a, b| {
                a.transaction_id
                    .lt
                    .cmp(&b.transaction_id.lt)
                    .then_with(|| a.hash.cmp(&b.hash))
            });
        }
        SortOrder::Desc => {
            transactions.sort_by(|a, b| {
                b.transaction_id
                    .lt
                    .cmp(&a.transaction_id.lt)
                    .then_with(|| b.hash.cmp(&a.hash))
            });
        }
    }
}

fn sort_blocks(blocks: &mut [LocalnetBlock], order: SortOrder) {
    match order {
        SortOrder::Asc => {
            blocks.sort_by(|a, b| {
                a.gen_utime
                    .cmp(&b.gen_utime)
                    .then_with(|| a.seqno.cmp(&b.seqno))
                    .then_with(|| a.workchain.cmp(&b.workchain))
            });
        }
        SortOrder::Desc => {
            blocks.sort_by(|a, b| {
                b.gen_utime
                    .cmp(&a.gen_utime)
                    .then_with(|| b.seqno.cmp(&a.seqno))
                    .then_with(|| b.workchain.cmp(&a.workchain))
            });
        }
    }
}

fn parse_limit_offset(limit: Option<i32>, offset: Option<i32>) -> anyhow::Result<(usize, usize)> {
    let limit = limit.unwrap_or(10);
    if !(1..=1000).contains(&limit) {
        anyhow::bail!("`limit` must be between 1 and 1000");
    }
    let offset = offset.unwrap_or(0);
    if offset < 0 {
        anyhow::bail!("`offset` must not be negative");
    }
    Ok((limit as usize, offset as usize))
}

fn parse_sort(sort: Option<String>) -> anyhow::Result<SortOrder> {
    match sort.as_deref().unwrap_or("desc") {
        "asc" => Ok(SortOrder::Asc),
        "desc" => Ok(SortOrder::Desc),
        other => anyhow::bail!("Invalid `sort`: {other}. Supported values: asc, desc"),
    }
}

fn parse_message_direction(value: Option<&str>) -> anyhow::Result<Option<MessageDirection>> {
    match value {
        None => Ok(None),
        Some("in") => Ok(Some(MessageDirection::In)),
        Some("out") => Ok(Some(MessageDirection::Out)),
        Some(other) => anyhow::bail!("Invalid `direction`: {other}. Supported values: in, out"),
    }
}

fn parse_nullable_address_filter(value: &str) -> anyhow::Result<NullableAddressFilter> {
    if value == "null" {
        Ok(NullableAddressFilter::Null)
    } else {
        Addr::parse(value).map(NullableAddressFilter::Address)
    }
}

fn parse_required_addresses(values: Vec<String>) -> anyhow::Result<Vec<Addr>> {
    if values.is_empty() {
        anyhow::bail!("address of account is required");
    }
    if values.len() > 1000 {
        anyhow::bail!("Maximum 1000 addresses allowed");
    }
    values
        .into_iter()
        .map(|address| Addr::parse(&address))
        .collect()
}

fn parse_requested_addresses(values: Vec<String>) -> anyhow::Result<Vec<(String, Addr)>> {
    if values.is_empty() {
        anyhow::bail!("at least 1 address required");
    }
    values
        .into_iter()
        .map(|value| Addr::parse(&value).map(|address| (value, address)))
        .collect()
}

fn parse_opcode(opcode: &str) -> anyhow::Result<u32> {
    let opcode = opcode.trim();
    if opcode.is_empty() {
        anyhow::bail!("`opcode` must not be empty");
    }
    if let Some(hex) = opcode
        .strip_prefix("0x")
        .or_else(|| opcode.strip_prefix("0X"))
    {
        return u32::from_str_radix(hex, 16).map_err(|e| anyhow::anyhow!("Invalid `opcode`: {e}"));
    }
    let signed = opcode
        .parse::<i32>()
        .map_err(|e| anyhow::anyhow!("Invalid `opcode`: {e}"))?;
    Ok(signed as u32)
}

fn parse_addresses(values: Vec<String>) -> anyhow::Result<Option<HashSet<Addr>>> {
    if values.is_empty() {
        return Ok(None);
    }
    values
        .into_iter()
        .map(|address| Addr::parse(&address))
        .collect::<anyhow::Result<HashSet<_>>>()
        .map(Some)
}

fn parse_hashes(values: Vec<String>) -> anyhow::Result<Option<HashSet<Hash256>>> {
    if values.is_empty() {
        return Ok(None);
    }
    values
        .into_iter()
        .map(|hash| parse_hash_any(&hash))
        .collect::<anyhow::Result<HashSet<_>>>()
        .map(Some)
}

fn parse_non_negative_u32(name: &str, value: Option<i32>) -> anyhow::Result<Option<u32>> {
    value
        .map(|value| {
            u32::try_from(value).map_err(|_| anyhow::anyhow!("`{name}` must not be negative"))
        })
        .transpose()
}

fn parse_required_non_negative_u32(name: &str, value: i32) -> anyhow::Result<u32> {
    u32::try_from(value).map_err(|_| anyhow::anyhow!("`{name}` must not be negative"))
}

fn parse_v3_query<T: DeserializeOwned>(raw_query: Option<&str>) -> anyhow::Result<T> {
    serde_html_form::from_str(raw_query.unwrap_or_default())
        .map_err(|e| anyhow::anyhow!("Invalid query: {e}"))
}

fn parse_hash_any(hash: &str) -> anyhow::Result<Hash256> {
    if let Ok(parsed) = Hash256::from_hex(hash) {
        return Ok(parsed);
    }
    if let Ok(parsed) = Hash256::from_base64(hash) {
        return Ok(parsed);
    }

    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE.decode(hash)
        && bytes.len() == 32
    {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(Hash256(arr));
    }

    if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(hash)
        && bytes.len() == 32
    {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(Hash256(arr));
    }

    anyhow::bail!("Invalid hash format: {hash}")
}

fn parse_shard_query(shard: &str) -> anyhow::Result<i64> {
    let shard = shard.trim();
    if shard.is_empty() {
        anyhow::bail!("`shard` must not be empty");
    }
    if shard.starts_with('-') {
        return Ok(shard.parse::<i64>()?);
    }

    let hex = shard
        .strip_prefix("0x")
        .or_else(|| shard.strip_prefix("0X"))
        .unwrap_or(shard);
    if !hex.is_empty() && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        let unsigned = u64::from_str_radix(hex, 16)?;
        return Ok(unsigned as i64);
    }

    if let Ok(value) = shard.parse::<i64>() {
        return Ok(value);
    }

    anyhow::bail!("Invalid shard format: {shard}")
}

async fn handle_v3_result<T, F, M>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Response
where
    F: FnOnce(&T) -> M,
    M: Serialize,
{
    match result.await {
        Ok(res) => (StatusCode::OK, Json(mapper(&res))).into_response(),
        Err(e) => request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

fn is_trace_not_found_error(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    message.starts_with("Trace not found for message ") || message == "Root transaction not found"
}

fn v3_bad_request(error: impl Into<String>) -> Response {
    request_error(StatusCode::BAD_REQUEST, error)
}

fn request_error(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(v3_types::RequestError {
            error: error.into(),
            code: Some(i32::from(status.as_u16())),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transactions_query() -> ParsedTransactionsV3Query {
        ParsedTransactionsV3Query {
            workchain: None,
            shard: None,
            seqno: None,
            mc_seqno: None,
            account: None,
            exclude_account: None,
            hash: None,
            lt: None,
            start_utime: None,
            end_utime: None,
            start_lt: None,
            end_lt: None,
            limit: 5,
            offset: 0,
            sort: SortOrder::Desc,
        }
    }

    #[test]
    fn transactions_fast_path_uses_block_page_for_simple_block_query() {
        let mut query = transactions_query();
        query.workchain = Some(BLOCK_WORKCHAIN);
        query.shard = Some(BLOCK_SHARD);
        query.seqno = Some(42);

        assert_eq!(
            transactions_fast_path(&query),
            Some(TransactionsFastPath::Block { seqno: 42 })
        );
    }

    #[test]
    fn transactions_fast_path_keeps_account_filters_on_general_path() {
        let mut query = transactions_query();
        query.workchain = Some(BLOCK_WORKCHAIN);
        query.shard = Some(BLOCK_SHARD);
        query.seqno = Some(42);
        query.account = Some(HashSet::new());

        assert_eq!(transactions_fast_path(&query), None);
    }

    #[test]
    fn transactions_fast_path_returns_empty_for_non_localnet_block_shard() {
        let mut query = transactions_query();
        query.workchain = Some(BLOCK_WORKCHAIN);
        query.shard = Some(123);
        query.seqno = Some(42);

        assert_eq!(
            transactions_fast_path(&query),
            Some(TransactionsFastPath::Empty)
        );
    }

    #[test]
    fn transactions_fast_path_uses_recent_page_for_simple_recent_query() {
        let query = transactions_query();

        assert_eq!(
            transactions_fast_path(&query),
            Some(TransactionsFastPath::Recent)
        );
    }
}
