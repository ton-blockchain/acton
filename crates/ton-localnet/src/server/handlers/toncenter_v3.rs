use super::toncenter_enrichment::{
    build_extra_data_for_addresses, build_metadata_for_addresses, load_address_infos,
    map_address_book_row, map_address_info,
};
use crate::api::{toncenter_emulate, toncenter_v2 as v2, toncenter_v3, toncenter_wallet};
use crate::error::LocalnetError;
use crate::localnet;
use crate::localnet::{
    Localnet, LocalnetBlock, LocalnetContractData, LocalnetJettonWalletsQuery,
    LocalnetNftItemsOrder, LocalnetNftItemsQuery, LocalnetSortOrder, LocalnetTransaction,
};
use crate::storage::{
    AccountStatus, DnsRecordMeta, JettonMasterMeta, JettonWalletMeta, NftItemMeta, NftSaleMeta,
    TraceNode,
};
use crate::types::{Addr, Hash256};
use crate::v3_events::{parse_jetton_burn, parse_jetton_transfer, parse_nft_transfer};
use axum::{
    Json,
    extract::{Query, RawQuery, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use ton_api::toncenter::v3 as v3_types;
use ton_api::toncenter::v3::requests::{
    AccountStatesQuery, AddressInformationQuery, AddressesQuery, AdjacentTransactionsQuery,
    BlocksQuery, DnsRecordsQuery, EstimateFeeRequest, JettonBurnsQuery, JettonMastersQuery,
    JettonTransfersQuery, JettonWalletsQuery, MasterchainBlockShardStateQuery,
    MasterchainBlockShardsQuery, MessagesQuery, MultisigOrdersQuery, MultisigWalletsQuery,
    NftCollectionsQuery, NftItemsQuery, NftSalesQuery, NftTransfersQuery, PendingActionsQuery,
    PendingTracesQuery, PendingTransactionsQuery, RunGetMethodRequest, SendMessageRequest,
    StackEntry, TopAccountsByBalanceQuery, TracesQuery, TransactionsByMasterchainBlockQuery,
    TransactionsByMessageQuery, TransactionsQuery, VestingQuery, WalletInformationQuery,
    WalletStatesQuery,
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

pub async fn get_pending_actions_v3(RawQuery(raw_query): RawQuery) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<PendingActionsQuery>(raw_query.as_deref()));
    parse!(validate_pending_filter(
        payload.account.as_deref(),
        &payload.ext_msg_hash,
    ));

    Json(v3_types::ActionsResponse {
        actions: Vec::new(),
        address_book: v3_types::AddressBook::new(),
        metadata: v3_types::Metadata::new(),
    })
    .into_response()
}

pub async fn get_pending_traces_v3(RawQuery(raw_query): RawQuery) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<PendingTracesQuery>(raw_query.as_deref()));
    parse!(validate_pending_filter(
        payload.account.as_deref(),
        &payload.ext_msg_hash,
    ));

    Json(v3_types::TracesResponse {
        traces: Vec::new(),
        address_book: v3_types::AddressBook::new(),
        metadata: v3_types::Metadata::new(),
    })
    .into_response()
}

fn validate_pending_filter(
    account: Option<&str>,
    external_hashes: &[String],
) -> anyhow::Result<()> {
    if account.is_none() && external_hashes.is_empty() {
        anyhow::bail!("account or ext_msg_hash should be specified");
    }
    if let Some(account) = account {
        Addr::parse(account)?;
    }
    for hash in external_hashes {
        parse_hash_any(hash)?;
    }
    Ok(())
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

    let state = match node
        .get_address_information(payload.address.clone(), None)
        .await
    {
        Ok(state) => state,
        Err(error) => {
            return request_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };
    let wallet_type = v2::wallet_type_name_from_code_hash(state.code_hash.as_ref());
    if state.state == AccountStatus::Active && wallet_type.is_none() {
        return request_error(StatusCode::CONFLICT, "not a wallet");
    }

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

    (
        StatusCode::OK,
        Json(v3::map_wallet_information_v3(
            &state,
            wallet_type,
            seqno,
            wallet_id,
        )),
    )
        .into_response()
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

pub async fn get_masterchain_block_shard_state_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<MasterchainBlockShardStateQuery>(
        raw_query.as_deref()
    ));
    let seqno = parse!(parse_required_non_negative_u32("seqno", payload.seqno));
    let shard_ids = match node.get_shards(seqno).await {
        Ok(shards) => shards,
        Err(error) => {
            if matches!(
                error.downcast_ref::<LocalnetError>(),
                Some(LocalnetError::BlockNotFound { .. })
            ) {
                return request_error(StatusCode::NOT_FOUND, "blocks not found");
            }
            return request_error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };
    let blocks = match node.get_blocks().await {
        Ok(blocks) => blocks,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let shard_ids = shard_ids
        .into_iter()
        .map(|block| (block.workchain, block.shard, block.seqno))
        .collect::<HashSet<_>>();
    let mut blocks = blocks
        .into_iter()
        .filter(|block| shard_ids.contains(&(block.workchain, block.shard, block.seqno)))
        .collect::<Vec<_>>();
    blocks.sort_by_key(|block| (block.workchain, block.shard, block.seqno));
    if blocks.is_empty() {
        return request_error(StatusCode::NOT_FOUND, "blocks not found");
    }

    (StatusCode::OK, Json(v3::map_blocks_response(&blocks))).into_response()
}

pub async fn get_masterchain_block_shards_v3(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<MasterchainBlockShardsQuery>(
        raw_query.as_deref()
    ));
    let seqno = parse!(parse_required_non_negative_u32("seqno", payload.seqno));
    let (limit, offset) = parse!(parse_limit_offset_with(
        payload.limit,
        payload.offset,
        10,
        1000,
    ));
    let blocks = match node.get_blocks().await {
        Ok(blocks) => blocks,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut blocks = blocks
        .into_iter()
        .filter(|block| {
            block
                .masterchain_block_ref
                .as_ref()
                .is_some_and(|masterchain| masterchain.seqno == seqno)
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|block| (block.workchain, block.shard, block.seqno));
    let blocks = paginate(blocks, limit, offset);
    if blocks.is_empty() {
        return request_error(StatusCode::NOT_FOUND, "blocks not found");
    }

    (StatusCode::OK, Json(v3::map_blocks_response(&blocks))).into_response()
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
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<TransactionsByMessageQuery>(
        raw_query.as_deref()
    ));
    let parsed = parse!(parse_transactions_by_message_v3_query(payload));
    if parsed.msg_hashes.is_none() && parsed.body_hash.is_none() && parsed.opcode.is_none() {
        return v3_unprocessable_entity(
            "at least one of msg_hash, body_hash, opcode should be specified",
        );
    }

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
    if parsed.account.is_none() {
        return v3_unprocessable_entity("at least 1 account address required");
    }

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
    let sort = match payload.sort {
        Some(sort) => Some(parse!(parse_sort(Some(sort)))),
        None => None,
    };
    let wallets = match node
        .get_jetton_wallets(LocalnetJettonWalletsQuery {
            addresses: payload.address,
            owner_addresses: payload.owner_address,
            jetton_addresses: payload.jetton_address,
            exclude_zero_balance: payload.exclude_zero_balance,
            sort: sort.map(|sort| match sort {
                SortOrder::Asc => LocalnetSortOrder::Asc,
                SortOrder::Desc => LocalnetSortOrder::Desc,
            }),
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
    if !payload.index.is_empty() && payload.collection_address.is_empty() {
        return request_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "index parameter is not allowed without the collection_address".to_owned(),
        );
    }
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let include_on_sale = payload.include_on_sale.unwrap_or(false);
    let order = if payload.sort_by_last_transaction_lt.unwrap_or(false) {
        LocalnetNftItemsOrder::LastTransactionLtDesc
    } else if payload.collection_address.len() == 1 {
        LocalnetNftItemsOrder::CollectionIndex
    } else if !payload.owner_address.is_empty() {
        LocalnetNftItemsOrder::OwnerCollectionIndex
    } else {
        LocalnetNftItemsOrder::Insertion
    };
    let real_owner_filter = if include_on_sale && !payload.owner_address.is_empty() {
        parse!(parse_address_set(&payload.owner_address))
    } else {
        HashSet::new()
    };
    let query_owner = if real_owner_filter.is_empty() {
        payload.owner_address
    } else {
        Vec::new()
    };
    let mut items = match node
        .get_nft_items(LocalnetNftItemsQuery {
            addresses: payload.address,
            owner_addresses: query_owner,
            collection_addresses: payload.collection_address,
            indexes: payload.index,
            order,
            limit: Some(if real_owner_filter.is_empty() {
                limit
            } else {
                usize::MAX
            }),
            offset: Some(if real_owner_filter.is_empty() {
                offset
            } else {
                0
            }),
        })
        .await
    {
        Ok(items) => items,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let sales = match load_nft_sales_for_items(node.as_ref(), &items).await {
        Ok(sales) => sales,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };

    if !real_owner_filter.is_empty() {
        let sale_owners = sales
            .iter()
            .filter_map(|sale| {
                sale.nft_owner_address
                    .map(|owner| (sale.nft_address, owner))
            })
            .collect::<HashMap<_, _>>();
        items.retain(|item| {
            item.owner_address
                .is_some_and(|owner| real_owner_filter.contains(&owner))
                || sale_owners
                    .get(&item.address)
                    .is_some_and(|owner| real_owner_filter.contains(owner))
        });
        items = paginate(items, limit, offset);
    }

    (StatusCode::OK, Json(v3::map_nft_items(&items, &sales))).into_response()
}

pub async fn get_dns_records(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<DnsRecordsQuery>(raw_query.as_deref()));
    let wallet_value = payload.wallet.as_deref().filter(|value| !value.is_empty());
    let domain_value = payload.domain.as_deref().filter(|value| !value.is_empty());
    if wallet_value.is_some() == domain_value.is_some() {
        return v3_bad_request("Exactly one of `wallet` or `domain` is required");
    }
    let wallet = parse!(wallet_value.map(Addr::parse).transpose());
    let (limit, offset) = parse!(parse_limit_offset_with(
        payload.limit,
        payload.offset,
        100,
        1000,
    ));
    let data = match discover_contract_data(node.as_ref(), &[], true).await {
        Ok(data) => data,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let records = filter_dns_records(
        data.into_iter().filter_map(|data| data.dns),
        wallet,
        domain_value,
        limit,
        offset,
    );
    (StatusCode::OK, Json(v3::map_dns_records(&records))).into_response()
}

fn filter_dns_records(
    records: impl IntoIterator<Item = DnsRecordMeta>,
    wallet: Option<Addr>,
    domain: Option<&str>,
    limit: usize,
    offset: usize,
) -> Vec<DnsRecordMeta> {
    let mut records = records
        .into_iter()
        .filter(|record| {
            wallet.is_none_or(|address| record.wallet == Some(address))
                && domain.is_none_or(|domain| record.domain == domain)
        })
        .collect::<Vec<_>>();
    sort_dns_records(&mut records);
    paginate(records, limit, offset)
}

fn sort_dns_records(records: &mut [DnsRecordMeta]) {
    records.sort_unstable_by(|left, right| {
        left.domain
            .chars()
            .count()
            .cmp(&right.domain.chars().count())
            .then_with(|| left.domain.cmp(&right.domain))
    });
}

pub async fn get_jetton_transfers(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<JettonTransfersQuery>(raw_query.as_deref()));
    parse!(validate_filter_len(
        "owner_address",
        payload.owner_address.len(),
        1000,
    ));
    parse!(validate_filter_len(
        "jetton_wallet",
        payload.jetton_wallet.len(),
        1000,
    ));
    let owners = parse!(parse_address_set(&payload.owner_address));
    let wallet_filter = parse!(parse_address_set(&payload.jetton_wallet));
    let master = parse!(
        payload
            .jetton_master
            .as_deref()
            .map(Addr::parse)
            .transpose()
    );
    let direction = parse!(parse_message_direction(payload.direction.as_deref()));
    let bounds = parse!(parse_event_bounds(
        payload.start_utime,
        payload.end_utime,
        payload.start_lt,
        payload.end_lt,
    ));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let sort = parse!(parse_sort(payload.sort));
    let transactions = match node.get_all_transactions().await {
        Ok(transactions) => transactions,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let (wallets, masters) = match load_jetton_event_context(node.as_ref(), &transactions).await {
        Ok(context) => context,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let wallets_by_address = wallets
        .iter()
        .map(|wallet| (wallet.address, wallet))
        .collect::<HashMap<_, _>>();
    let mut events = transactions
        .iter()
        .filter(|transaction| !transaction.aborted)
        .filter_map(|transaction| {
            let wallet = wallets_by_address.get(&transaction.address)?;
            parse_jetton_transfer(transaction, wallet).ok().flatten()
        })
        .filter(|event| wallet_filter.is_empty() || wallet_filter.contains(&event.source_wallet))
        .filter(|event| master.is_none_or(|address| event.jetton_master == address))
        .filter(|event| match direction {
            Some(MessageDirection::In) => owners.is_empty() || owners.contains(&event.destination),
            Some(MessageDirection::Out) => owners.is_empty() || owners.contains(&event.source),
            None => {
                owners.is_empty()
                    || owners.contains(&event.source)
                    || owners.contains(&event.destination)
            }
        })
        .filter(|event| bounds.contains(event.transaction_now, event.transaction_lt))
        .collect::<Vec<_>>();
    sort_transaction_events(
        &mut events,
        sort,
        bounds,
        |event| event.transaction_now,
        |event| event.transaction_lt,
    );
    let events = paginate(events, limit, offset);
    let selected_wallets = events
        .iter()
        .map(|event| event.source_wallet)
        .collect::<HashSet<_>>();
    let wallets = wallets
        .into_iter()
        .filter(|wallet| selected_wallets.contains(&wallet.address))
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(v3::map_jetton_transfers(&events, &wallets, &masters)),
    )
        .into_response()
}

pub async fn get_jetton_burns(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<JettonBurnsQuery>(raw_query.as_deref()));
    parse!(validate_filter_len("address", payload.address.len(), 1000));
    parse!(validate_filter_len(
        "jetton_wallet",
        payload.jetton_wallet.len(),
        1000,
    ));
    let owners = parse!(parse_address_set(&payload.address));
    let wallet_filter = parse!(parse_address_set(&payload.jetton_wallet));
    let master = parse!(
        payload
            .jetton_master
            .as_deref()
            .map(Addr::parse)
            .transpose()
    );
    let bounds = parse!(parse_event_bounds(
        payload.start_utime,
        payload.end_utime,
        payload.start_lt,
        payload.end_lt,
    ));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let sort = parse!(parse_sort(payload.sort));
    let transactions = match node.get_all_transactions().await {
        Ok(transactions) => transactions,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let (wallets, masters) = match load_jetton_event_context(node.as_ref(), &transactions).await {
        Ok(context) => context,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let wallets_by_address = wallets
        .iter()
        .map(|wallet| (wallet.address, wallet))
        .collect::<HashMap<_, _>>();
    let mut events = transactions
        .iter()
        .filter_map(|transaction| {
            let wallet = wallets_by_address.get(&transaction.address)?;
            parse_jetton_burn(transaction, wallet).ok().flatten()
        })
        .filter(|event| owners.is_empty() || owners.contains(&event.owner))
        .filter(|event| wallet_filter.is_empty() || wallet_filter.contains(&event.jetton_wallet))
        .filter(|event| master.is_none_or(|address| event.jetton_master == address))
        .filter(|event| bounds.contains(event.transaction_now, event.transaction_lt))
        .collect::<Vec<_>>();
    sort_transaction_events(
        &mut events,
        sort,
        bounds,
        |event| event.transaction_now,
        |event| event.transaction_lt,
    );
    let events = paginate(events, limit, offset);
    let selected_wallets = events
        .iter()
        .map(|event| event.jetton_wallet)
        .collect::<HashSet<_>>();
    let wallets = wallets
        .into_iter()
        .filter(|wallet| selected_wallets.contains(&wallet.address))
        .collect::<Vec<_>>();
    (
        StatusCode::OK,
        Json(v3::map_jetton_burns(&events, &wallets, &masters)),
    )
        .into_response()
}

pub async fn get_nft_collections(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<NftCollectionsQuery>(raw_query.as_deref()));
    parse!(validate_filter_len(
        "collection_address",
        payload.collection_address.len(),
        1000,
    ));
    parse!(validate_filter_len(
        "owner_address",
        payload.owner_address.len(),
        1000,
    ));
    let collection_filter = parse!(parse_address_set(&payload.collection_address));
    let owner_filter = parse!(parse_address_set(&payload.owner_address));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let scan_all = collection_filter.is_empty() || !owner_filter.is_empty();
    let explicit = collection_filter.iter().copied().collect::<Vec<_>>();
    let data = match discover_contract_data(node.as_ref(), &explicit, scan_all).await {
        Ok(data) => data,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut collections = data
        .into_iter()
        .filter_map(|data| data.nft_collection)
        .filter(|collection| {
            (collection_filter.is_empty() || collection_filter.contains(&collection.address))
                && (owner_filter.is_empty()
                    || collection
                        .owner_address
                        .is_some_and(|owner| owner_filter.contains(&owner)))
        })
        .collect::<Vec<_>>();
    collections.sort_by_key(|collection| collection.address.to_string());
    let collections = paginate(collections, limit, offset);
    (StatusCode::OK, Json(v3::map_nft_collections(&collections))).into_response()
}

pub async fn get_nft_sales(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<NftSalesQuery>(raw_query.as_deref()));
    if payload.address.is_empty() {
        return v3_bad_request("At least one `address` should be specified");
    }
    parse!(validate_filter_len("address", payload.address.len(), 1000));
    let addresses = parse!(parse_address_set(&payload.address));
    let data = match discover_contract_data(
        node.as_ref(),
        &addresses.iter().copied().collect::<Vec<_>>(),
        false,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let sales = data
        .into_iter()
        .filter_map(|data| data.nft_sale)
        .collect::<Vec<_>>();
    let items = match load_nft_items(
        node.as_ref(),
        sales.iter().map(|sale| sale.nft_address).collect(),
    )
    .await
    {
        Ok(items) => items,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    (StatusCode::OK, Json(v3::map_nft_sales(&sales, &items))).into_response()
}

pub async fn get_nft_transfers(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<NftTransfersQuery>(raw_query.as_deref()));
    parse!(validate_filter_len(
        "owner_address",
        payload.owner_address.len(),
        1000,
    ));
    parse!(validate_filter_len(
        "item_address",
        payload.item_address.len(),
        1000,
    ));
    let owners = parse!(parse_address_set(&payload.owner_address));
    let item_filter = parse!(parse_address_set(&payload.item_address));
    let collection = parse!(
        payload
            .collection_address
            .as_deref()
            .map(Addr::parse)
            .transpose()
    );
    let direction = parse!(parse_message_direction(payload.direction.as_deref()));
    let bounds = parse!(parse_event_bounds(
        payload.start_utime,
        payload.end_utime,
        payload.start_lt,
        payload.end_lt,
    ));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let sort = parse!(parse_sort(payload.sort));
    let transactions = match node.get_all_transactions().await {
        Ok(transactions) => transactions,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let items = match load_nft_items(
        node.as_ref(),
        transactions
            .iter()
            .map(|transaction| transaction.address)
            .collect(),
    )
    .await
    {
        Ok(items) => items,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let items_by_address = items
        .iter()
        .map(|item| (item.address, item))
        .collect::<HashMap<_, _>>();
    let mut events = transactions
        .iter()
        .filter_map(|transaction| {
            let item = items_by_address.get(&transaction.address)?;
            parse_nft_transfer(transaction, item).ok().flatten()
        })
        .filter(|event| item_filter.is_empty() || item_filter.contains(&event.nft_address))
        .filter(|event| collection.is_none_or(|address| event.nft_collection == address))
        .filter(|event| match direction {
            Some(MessageDirection::In) => owners.is_empty() || owners.contains(&event.new_owner),
            Some(MessageDirection::Out) => owners.is_empty() || owners.contains(&event.old_owner),
            None => {
                owners.is_empty()
                    || owners.contains(&event.old_owner)
                    || owners.contains(&event.new_owner)
            }
        })
        .filter(|event| bounds.contains(event.transaction_now, event.transaction_lt))
        .collect::<Vec<_>>();
    sort_transaction_events(
        &mut events,
        sort,
        bounds,
        |event| event.transaction_now,
        |event| event.transaction_lt,
    );
    let events = paginate(events, limit, offset);
    let selected_items = events
        .iter()
        .map(|event| event.nft_address)
        .collect::<HashSet<_>>();
    let items = items
        .into_iter()
        .filter(|item| selected_items.contains(&item.address))
        .collect::<Vec<_>>();
    (StatusCode::OK, Json(v3::map_nft_transfers(&events, &items))).into_response()
}

pub async fn get_multisig_orders(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<MultisigOrdersQuery>(raw_query.as_deref()));
    if payload.address.is_empty() && payload.multisig_address.is_empty() {
        return v3_bad_request(
            "At least one of `address` or `multisig_address` should be specified",
        );
    }
    parse!(validate_filter_len("address", payload.address.len(), 1024));
    parse!(validate_filter_len(
        "multisig_address",
        payload.multisig_address.len(),
        1024,
    ));
    let addresses = parse!(parse_address_set(&payload.address));
    let multisig_addresses = parse!(parse_address_set(&payload.multisig_address));
    let parse_actions = payload.parse_actions.unwrap_or(false);
    let (limit, offset) = parse!(parse_limit_offset_with(
        payload.limit,
        payload.offset,
        10,
        1024,
    ));
    let sort = parse!(parse_sort(payload.sort));
    let data = match discover_contract_data(
        node.as_ref(),
        &addresses.iter().copied().collect::<Vec<_>>(),
        !multisig_addresses.is_empty(),
    )
    .await
    {
        Ok(data) => data,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut orders = data
        .into_iter()
        .filter_map(|data| data.multisig_order)
        .filter(|order| {
            (addresses.is_empty() || addresses.contains(&order.address))
                && (multisig_addresses.is_empty()
                    || multisig_addresses.contains(&order.multisig_address))
        })
        .collect::<Vec<_>>();
    sort_events(&mut orders, sort, |order| order.last_transaction_lt);
    let orders = paginate(orders, limit, offset);
    (
        StatusCode::OK,
        Json(v3::map_multisig_orders(&orders, parse_actions)),
    )
        .into_response()
}

pub async fn get_multisig_wallets(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<MultisigWalletsQuery>(raw_query.as_deref()));
    if payload.address.is_empty() && payload.wallet_address.is_empty() {
        return v3_bad_request("At least one of `address` or `wallet_address` should be specified");
    }
    parse!(validate_filter_len("address", payload.address.len(), 1024));
    parse!(validate_filter_len(
        "wallet_address",
        payload.wallet_address.len(),
        1024,
    ));
    let addresses = parse!(parse_address_set(&payload.address));
    let wallet_addresses = parse!(parse_address_set(&payload.wallet_address));
    let (limit, offset) = parse!(parse_limit_offset_with(
        payload.limit,
        payload.offset,
        10,
        1024,
    ));
    let sort = parse!(parse_sort(payload.sort));
    let include_orders = payload.include_orders.unwrap_or(true);
    let data = match discover_contract_data(
        node.as_ref(),
        &addresses.iter().copied().collect::<Vec<_>>(),
        !wallet_addresses.is_empty() || include_orders,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut multisigs = Vec::new();
    let mut orders = Vec::new();
    for data in data {
        if let Some(multisig) = data.multisig
            && (addresses.is_empty() || addresses.contains(&multisig.address))
            && (wallet_addresses.is_empty()
                || multisig
                    .signers
                    .iter()
                    .chain(&multisig.proposers)
                    .any(|address| wallet_addresses.contains(address)))
        {
            multisigs.push(multisig);
        }
        if include_orders && let Some(order) = data.multisig_order {
            orders.push(order);
        }
    }
    sort_events(&mut multisigs, sort, |multisig| {
        multisig.last_transaction_lt
    });
    let multisigs = paginate(multisigs, limit, offset);
    let selected = multisigs
        .iter()
        .map(|multisig| multisig.address)
        .collect::<HashSet<_>>();
    orders.retain(|order| selected.contains(&order.multisig_address));
    (StatusCode::OK, Json(v3::map_multisigs(&multisigs, &orders))).into_response()
}

pub async fn get_vesting(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> impl IntoResponse {
    let payload = parse!(parse_v3_query::<VestingQuery>(raw_query.as_deref()));
    parse!(validate_filter_len(
        "contract_address",
        payload.contract_address.len(),
        1000,
    ));
    parse!(validate_filter_len(
        "wallet_address",
        payload.wallet_address.len(),
        1000,
    ));
    let contracts = parse!(parse_address_set(&payload.contract_address));
    let wallets = parse!(parse_address_set(&payload.wallet_address));
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    let include_whitelist = payload.check_whitelist.unwrap_or(false);
    let data = match discover_contract_data(
        node.as_ref(),
        &contracts.iter().copied().collect::<Vec<_>>(),
        contracts.is_empty(),
    )
    .await
    {
        Ok(data) => data,
        Err(e) => return request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    };
    let mut vesting = data
        .into_iter()
        .filter_map(|data| data.vesting)
        .filter(|vesting| {
            (contracts.is_empty() || contracts.contains(&vesting.address))
                && (wallets.is_empty()
                    || wallets.contains(&vesting.owner_address)
                    || wallets.contains(&vesting.sender_address)
                    || (include_whitelist
                        && vesting
                            .whitelist
                            .iter()
                            .any(|address| wallets.contains(address))))
        })
        .collect::<Vec<_>>();
    vesting.sort_by_key(|vesting| (vesting.first_transaction_lt, vesting.address));
    let vesting = vesting
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    (StatusCode::OK, Json(v3::map_vesting_contracts(&vesting))).into_response()
}

pub async fn send_message_v3(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendMessageRequest>,
) -> impl IntoResponse {
    handle_v3_result(node.send_boc(payload.boc), toncenter_v3::map_send_message).await
}

pub async fn get_top_accounts_by_balance_v3(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TopAccountsByBalanceQuery>,
) -> impl IntoResponse {
    let (limit, offset) = parse!(parse_limit_offset(payload.limit, payload.offset));
    handle_v3_result(node.get_top_account_balances(limit, offset), |accounts| {
        v3::map_account_balances(accounts)
    })
    .await
}

pub async fn estimate_fee_v3(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<EstimateFeeRequest>,
) -> impl IntoResponse {
    let boc = parse!(toncenter_emulate::compose_estimate_fee_message(&payload));
    handle_v3_result(
        node.estimate_fees(boc, payload.ignore_chksig.unwrap_or(true)),
        v3::map_estimate_fee,
    )
    .await
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
        async move {
            let result = node
                .run_get_method(payload.address, payload.method, stack, None)
                .await?;
            toncenter_v3::map_run_get_method_v3(&result)
        },
        Clone::clone,
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

async fn discover_contract_data(
    node: &Localnet,
    explicit: &[Addr],
    scan_all: bool,
) -> anyhow::Result<Vec<LocalnetContractData>> {
    let mut addresses = explicit.iter().copied().collect::<HashSet<_>>();
    if scan_all {
        addresses.extend(
            node.get_top_account_balances(usize::MAX, 0)
                .await?
                .into_iter()
                .map(|account| account.account),
        );
    }

    let mut result = Vec::with_capacity(addresses.len());
    for address in addresses {
        result.push(node.detect_contract_data(address.to_string()).await?);
    }
    Ok(result)
}

async fn load_jetton_event_context(
    node: &Localnet,
    transactions: &[LocalnetTransaction],
) -> anyhow::Result<(Vec<JettonWalletMeta>, HashMap<Addr, JettonMasterMeta>)> {
    let addresses = transactions
        .iter()
        .map(|transaction| transaction.address.to_string())
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let wallets = node
        .get_jetton_wallets(LocalnetJettonWalletsQuery {
            addresses,
            owner_addresses: Vec::new(),
            jetton_addresses: Vec::new(),
            exclude_zero_balance: None,
            sort: None,
            limit: Some(usize::MAX),
            offset: Some(0),
        })
        .await?;
    let mut masters = HashMap::new();
    let jettons = wallets
        .iter()
        .map(|wallet| wallet.jetton_address)
        .collect::<HashSet<_>>();
    for jetton in jettons {
        if let Some(master) = node
            .get_jetton_masters(vec![jetton.to_string()], Vec::new(), Some(1), Some(0))
            .await?
            .pop()
        {
            masters.insert(jetton, master);
        }
    }
    Ok((wallets, masters))
}

async fn load_nft_items(node: &Localnet, addresses: Vec<Addr>) -> anyhow::Result<Vec<NftItemMeta>> {
    node.get_nft_items(LocalnetNftItemsQuery {
        addresses: addresses
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .map(|address| address.to_string())
            .collect(),
        owner_addresses: Vec::new(),
        collection_addresses: Vec::new(),
        indexes: Vec::new(),
        order: LocalnetNftItemsOrder::Insertion,
        limit: Some(usize::MAX),
        offset: Some(0),
    })
    .await
}

async fn load_nft_sales_for_items(
    node: &Localnet,
    items: &[NftItemMeta],
) -> anyhow::Result<Vec<NftSaleMeta>> {
    let owners_by_nft = items
        .iter()
        .filter_map(|item| item.owner_address.map(|owner| (item.address, owner)))
        .collect::<HashMap<_, _>>();
    let owner_addresses = owners_by_nft.values().copied().collect::<HashSet<_>>();
    if owner_addresses.is_empty() {
        return Ok(Vec::new());
    }

    Ok(discover_contract_data(
        node,
        &owner_addresses.into_iter().collect::<Vec<_>>(),
        false,
    )
    .await?
    .into_iter()
    .filter_map(|data| data.nft_sale)
    .filter(|sale| owners_by_nft.get(&sale.nft_address) == Some(&sale.address))
    .collect())
}

fn parse_address_set(values: &[String]) -> anyhow::Result<HashSet<Addr>> {
    values.iter().map(|value| Addr::parse(value)).collect()
}

fn validate_filter_len(name: &str, len: usize, max: usize) -> anyhow::Result<()> {
    if len > max {
        anyhow::bail!("Maximum {max} `{name}` values allowed");
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct EventBounds {
    start_utime: Option<u32>,
    end_utime: Option<u32>,
    start_lt: Option<u64>,
    end_lt: Option<u64>,
}

impl EventBounds {
    fn contains(self, utime: u32, lt: u64) -> bool {
        self.start_utime.is_none_or(|start| utime >= start)
            && self.end_utime.is_none_or(|end| utime <= end)
            && self.start_lt.is_none_or(|start| lt >= start)
            && self.end_lt.is_none_or(|end| lt <= end)
    }

    const fn has_time_bound(self) -> bool {
        self.start_utime.is_some() || self.end_utime.is_some()
    }
}

fn parse_event_bounds(
    start_utime: Option<i32>,
    end_utime: Option<i32>,
    start_lt: Option<u64>,
    end_lt: Option<u64>,
) -> anyhow::Result<EventBounds> {
    let start_utime = parse_non_negative_u32("start_utime", start_utime)?;
    let end_utime = parse_non_negative_u32("end_utime", end_utime)?;
    if start_utime
        .zip(end_utime)
        .is_some_and(|(start, end)| start > end)
    {
        anyhow::bail!("`start_utime` must not be greater than `end_utime`");
    }
    if start_lt.zip(end_lt).is_some_and(|(start, end)| start > end) {
        anyhow::bail!("`start_lt` must not be greater than `end_lt`");
    }
    Ok(EventBounds {
        start_utime,
        end_utime,
        start_lt,
        end_lt,
    })
}

fn sort_events<T, K: Ord>(items: &mut [T], sort: SortOrder, key: impl Fn(&T) -> K) {
    match sort {
        SortOrder::Asc => items.sort_by_key(key),
        SortOrder::Desc => items.sort_by_key(|item| std::cmp::Reverse(key(item))),
    }
}

fn sort_transaction_events<T>(
    items: &mut [T],
    sort: SortOrder,
    bounds: EventBounds,
    transaction_now: impl Fn(&T) -> u32,
    transaction_lt: impl Fn(&T) -> u64,
) {
    if bounds.has_time_bound() {
        sort_events(items, sort, transaction_now);
    } else {
        sort_events(items, sort, transaction_lt);
    }
}

fn paginate<T>(items: Vec<T>, limit: usize, offset: usize) -> Vec<T> {
    items.into_iter().skip(offset).take(limit).collect()
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
    hashes: Option<HashSet<Hash256>>,
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
    msg_hashes: Option<HashSet<Hash256>>,
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
        hashes: parse_hashes(payload.hash)?,
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
        msg_hashes: parse_hashes(payload.msg_hash)?,
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
            if let Some(hashes) = &query.hashes
                && !hashes.contains(&tx.hash)
            {
                return false;
            }
            if let Some(lt) = query.lt
                && tx.transaction_id.lt != lt
            {
                return false;
            }
            if let Some(start_utime) = query.start_utime
                && tx.utime < start_utime
            {
                return false;
            }
            if let Some(end_utime) = query.end_utime
                && tx.utime > end_utime
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

    if query.start_utime.is_some() || query.end_utime.is_some() {
        sort_transactions_by_time(&mut filtered, query.sort);
    } else {
        sort_transactions(&mut filtered, query.sort);
    }
    filtered
        .into_iter()
        .skip(query.offset)
        .take(query.limit)
        .collect()
}

const fn transactions_fast_path(query: &ParsedTransactionsV3Query) -> Option<TransactionsFastPath> {
    let has_expensive_filters = query.account.is_some()
        || query.exclude_account.is_some()
        || query.hashes.is_some()
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
    let mut filtered = txs
        .iter()
        .filter(|tx| {
            let mut messages = Vec::new();
            if query.opcode.is_some() {
                if query.direction != Some(MessageDirection::Out) {
                    messages.push(&tx.in_msg);
                }
            } else {
                match query.direction {
                    Some(MessageDirection::In) => messages.push(&tx.in_msg),
                    Some(MessageDirection::Out) => messages.extend(tx.out_msgs.iter()),
                    None => {
                        messages.push(&tx.in_msg);
                        messages.extend(tx.out_msgs.iter());
                    }
                }
            }

            messages
                .into_iter()
                .filter(|msg| !msg.hash.is_zero())
                .any(|msg| {
                    if let Some(msg_hashes) = &query.msg_hashes
                        && !msg_hashes.contains(&msg.hash)
                        && msg.hash_norm.is_none_or(|hash| !msg_hashes.contains(&hash))
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
                    .then_with(|| a.address.cmp(&b.address))
                    .then_with(|| a.hash.cmp(&b.hash))
            });
        }
        SortOrder::Desc => {
            transactions.sort_by(|a, b| {
                b.transaction_id
                    .lt
                    .cmp(&a.transaction_id.lt)
                    .then_with(|| a.address.cmp(&b.address))
                    .then_with(|| a.hash.cmp(&b.hash))
            });
        }
    }
}

fn sort_transactions_by_time(transactions: &mut [LocalnetTransaction], order: SortOrder) {
    match order {
        SortOrder::Asc => transactions.sort_by(|a, b| {
            a.utime
                .cmp(&b.utime)
                .then_with(|| a.transaction_id.lt.cmp(&b.transaction_id.lt))
                .then_with(|| a.address.cmp(&b.address))
                .then_with(|| a.hash.cmp(&b.hash))
        }),
        SortOrder::Desc => transactions.sort_by(|a, b| {
            b.utime
                .cmp(&a.utime)
                .then_with(|| b.transaction_id.lt.cmp(&a.transaction_id.lt))
                .then_with(|| a.address.cmp(&b.address))
                .then_with(|| a.hash.cmp(&b.hash))
        }),
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
    parse_limit_offset_with(limit, offset, 10, 1000)
}

fn parse_limit_offset_with(
    limit: Option<i32>,
    offset: Option<i32>,
    default_limit: i32,
    max_limit: i32,
) -> anyhow::Result<(usize, usize)> {
    let limit = limit.unwrap_or(default_limit);
    if !(1..=max_limit).contains(&limit) {
        anyhow::bail!("`limit` must be between 1 and {max_limit}");
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
    hash.parse()
        .map_err(|_| anyhow::anyhow!("Invalid hash format: {hash}"))
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

fn v3_unprocessable_entity(error: impl Into<String>) -> Response {
    request_error(StatusCode::UNPROCESSABLE_ENTITY, error)
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
    use crate::localnet::{LocalnetMessage, LocalnetTransactionId};

    fn test_addr(byte: u8) -> Addr {
        Addr {
            workchain: 0,
            addr: [byte; 32],
        }
    }

    fn dns_record(domain: &str, address_byte: u8, wallet: Option<Addr>) -> DnsRecordMeta {
        DnsRecordMeta {
            nft_item_address: test_addr(address_byte),
            nft_item_owner: None,
            domain: domain.to_owned(),
            next_resolver: None,
            wallet,
            site_adnl: None,
            storage_bag_id: None,
        }
    }

    fn message(hash_byte: u8, opcode: u32) -> LocalnetMessage {
        LocalnetMessage {
            hash: Hash256([hash_byte; 32]),
            hash_norm: None,
            source: None,
            destination: None,
            bounce: false,
            bounced: false,
            value: 0,
            body_hash: Hash256([hash_byte.wrapping_add(1); 32]),
            body: Default::default(),
            init_state: Default::default(),
            opcode: Some(opcode),
            fwd_fee: 0,
            ihr_fee: 0,
            created_lt: 0,
            extra_currencies: Vec::new(),
        }
    }

    fn transaction_with_message_opcodes(in_opcode: u32, out_opcode: u32) -> LocalnetTransaction {
        LocalnetTransaction {
            hash: Hash256([3; 32]),
            address: Addr::default(),
            mc_block_seqno: 1,
            utime: 1,
            data: Default::default(),
            aborted: false,
            exit_code: 0,
            transaction_id: LocalnetTransactionId {
                lt: 1,
                hash: Hash256([3; 32]),
            },
            in_msg: message(1, in_opcode),
            out_msgs: vec![message(2, out_opcode)],
            total_fees: 0,
            storage_fees: 0,
            other_fees: 0,
        }
    }

    fn transaction_at(hash_byte: u8, address_byte: u8, lt: u64, utime: u32) -> LocalnetTransaction {
        let mut transaction = transaction_with_message_opcodes(7, 9);
        transaction.hash = Hash256([hash_byte; 32]);
        transaction.address.addr = [address_byte; 32];
        transaction.utime = utime;
        transaction.transaction_id = LocalnetTransactionId {
            lt,
            hash: transaction.hash,
        };
        transaction
    }

    #[test]
    fn dns_records_follow_upstream_length_then_domain_order() {
        let mut records = vec![
            dns_record("longer.ton", 1, None),
            dns_record("\u{e9}.ton", 2, None),
            dns_record("b.ton", 3, None),
            dns_record("aa.ton", 4, None),
            dns_record("a.ton", 5, None),
        ];

        sort_dns_records(&mut records);

        assert_eq!(
            records
                .iter()
                .map(|record| record.domain.as_str())
                .collect::<Vec<_>>(),
            ["a.ton", "b.ton", "\u{e9}.ton", "aa.ton", "longer.ton"]
        );
    }

    #[test]
    fn dns_record_filters_are_applied_before_ordering_and_pagination() {
        let wallet = test_addr(9);
        let records = vec![
            dns_record("longer.ton", 1, Some(wallet)),
            dns_record("b.ton", 2, Some(wallet)),
            dns_record("a.ton", 3, Some(wallet)),
            dns_record("aa.ton", 4, Some(test_addr(8))),
        ];

        let page = filter_dns_records(records, Some(wallet), None, 1, 1);

        assert_eq!(page.len(), 1);
        assert_eq!(page[0].domain, "b.ton");

        let exact = filter_dns_records(
            [
                dns_record("a.ton", 1, Some(wallet)),
                dns_record("b.ton", 2, Some(wallet)),
            ],
            None,
            Some("b.ton"),
            100,
            0,
        );
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].domain, "b.ton");
    }

    fn transactions_query() -> ParsedTransactionsV3Query {
        ParsedTransactionsV3Query {
            workchain: None,
            shard: None,
            seqno: None,
            mc_seqno: None,
            account: None,
            exclude_account: None,
            hashes: None,
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
    fn event_bounds_choose_upstream_sort_column() {
        let events = [(30_u64, 10_u32), (20, 30), (10, 20)];
        let cases = [
            (
                EventBounds {
                    start_utime: None,
                    end_utime: None,
                    start_lt: Some(0),
                    end_lt: Some(u64::MAX),
                },
                SortOrder::Asc,
                [(10, 20), (20, 30), (30, 10)],
            ),
            (
                EventBounds {
                    start_utime: Some(0),
                    end_utime: None,
                    start_lt: None,
                    end_lt: None,
                },
                SortOrder::Asc,
                [(30, 10), (10, 20), (20, 30)],
            ),
            (
                EventBounds {
                    start_utime: None,
                    end_utime: Some(u32::MAX),
                    start_lt: Some(0),
                    end_lt: None,
                },
                SortOrder::Desc,
                [(20, 30), (10, 20), (30, 10)],
            ),
        ];

        for (bounds, sort, expected) in cases {
            let mut actual = events;
            sort_transaction_events(
                &mut actual,
                sort,
                bounds,
                |(_, utime)| *utime,
                |(lt, _)| *lt,
            );
            assert_eq!(actual, expected);
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

    #[test]
    fn transactions_by_message_filters_match_upstream_semantics() {
        let transactions = [transaction_with_message_opcodes(7, 9)];
        let mut query = ParsedTransactionsByMessageV3Query {
            msg_hashes: None,
            body_hash: None,
            opcode: Some(9),
            direction: None,
            limit: 10,
            offset: 0,
        };

        assert!(filter_transactions_by_message_v3(&transactions, &query).is_empty());

        query.opcode = Some(7);
        assert_eq!(
            filter_transactions_by_message_v3(&transactions, &query).len(),
            1
        );

        query.direction = Some(MessageDirection::Out);
        assert!(filter_transactions_by_message_v3(&transactions, &query).is_empty());

        query.direction = None;
        query.opcode = None;
        query.msg_hashes = Some(HashSet::from([Hash256([2; 32]), Hash256([8; 32])]));
        assert_eq!(
            filter_transactions_by_message_v3(&transactions, &query).len(),
            1
        );
    }

    #[test]
    fn transaction_time_ranges_are_inclusive_and_control_ordering() {
        let transactions = [
            transaction_at(1, 2, 30, 10),
            transaction_at(2, 2, 20, 20),
            transaction_at(3, 1, 20, 20),
        ];
        let mut query = transactions_query();
        query.start_utime = Some(10);
        query.end_utime = Some(20);
        query.sort = SortOrder::Asc;
        query.limit = 10;

        let asc = filter_transactions_v3(&transactions, &query)
            .into_iter()
            .map(|transaction| {
                (
                    transaction.utime,
                    transaction.transaction_id.lt,
                    transaction.address.addr[0],
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(asc, [(10, 30, 2), (20, 20, 1), (20, 20, 2)]);

        query.sort = SortOrder::Desc;
        let desc = filter_transactions_v3(&transactions, &query)
            .into_iter()
            .map(|transaction| {
                (
                    transaction.utime,
                    transaction.transaction_id.lt,
                    transaction.address.addr[0],
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(desc, [(20, 20, 1), (20, 20, 2), (10, 30, 2)]);
    }
}
