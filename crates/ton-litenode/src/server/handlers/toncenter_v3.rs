use super::utils::parse_method_name;
use crate::api::toncenter_v3;
use crate::litenode::{LiteNode, LiteNodeTransaction};
use crate::server::models::{
    EmulateTraceRequest, GetAddressInformationV3Request, GetJettonMastersRequest,
    GetJettonWalletsRequest, GetNftItemsRequest, GetPendingTransactionsV3Query, GetTracesQuery,
    GetTransactionsByMessageV3Query, GetTransactionsV3Query, RunGetMethodRequest, SendBocRequest,
};
use crate::storage::{JettonMasterMeta, TraceNode};
use crate::types::{Addr, Hash256};
use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::Engine;
use serde_json::Value;
use serde_json::json;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use toncenter_v3 as v3;
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StdAddr, StdAddrFormat};
use tycho_types::prelude::HashBytes;

pub async fn get_traces(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> impl IntoResponse {
    handle_v3_result(node.get_traces(payload.hash), v3::map_traces).await
}

pub async fn get_address_information_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationV3Request>,
) -> impl IntoResponse {
    let _use_v2 = payload.use_v2.unwrap_or(true);

    handle_v3_result(
        node.get_address_information(payload.address, None),
        toncenter_v3::map_address_information,
    )
    .await
}

pub async fn get_transactions_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsV3Query>,
) -> impl IntoResponse {
    let parsed = match parse_transactions_v3_query(payload) {
        Ok(parsed) => parsed,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_v3_result(node.get_all_transactions(), move |txs| {
        let filtered = filter_transactions_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_transactions_by_message_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsByMessageV3Query>,
) -> impl IntoResponse {
    let parsed = match parse_transactions_by_message_v3_query(payload) {
        Ok(parsed) => parsed,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_v3_result(node.get_all_transactions(), move |txs| {
        let filtered = filter_transactions_by_message_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_pending_transactions_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetPendingTransactionsV3Query>,
) -> impl IntoResponse {
    let parsed = match parse_pending_transactions_v3_query(payload) {
        Ok(parsed) => parsed,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_v3_result(node.get_pending_transactions(), move |txs| {
        let filtered = filter_pending_transactions_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn emulate_trace_v1(State(node): State<Arc<LiteNode>>, body: Bytes) -> impl IntoResponse {
    let payload: EmulateTraceRequest = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(e) => return emulate_bad_request(format!("invalid request: {e}")),
    };

    let boc = payload.boc.unwrap_or_default();
    if boc.is_empty() {
        return emulate_bad_request("invalid request: boc is required");
    }

    if let Err(e) = base64::engine::general_purpose::STANDARD.decode(&boc) {
        return emulate_bad_request(format!("invalid request: invalid boc: {e}"));
    }

    let include_code_data = payload.include_code_data.unwrap_or(false);
    let include_address_book = payload.include_address_book.unwrap_or(false);
    let include_metadata = payload.include_metadata.unwrap_or(false);
    let with_actions = payload.with_actions.unwrap_or(false);

    match node
        .emulate_trace(boc, payload.ignore_chksig, payload.mc_block_seqno)
        .await
    {
        Ok(trace) => {
            let (address_book, metadata) = match build_emulate_v1_extra_data(
                node.as_ref(),
                &trace.trace,
                include_address_book,
                include_metadata,
            )
            .await
            {
                Ok(extra) => extra,
                Err(e) => return emulate_internal_error(e.to_string()),
            };

            let response = v3::map_emulate_trace_response(
                &trace,
                with_actions,
                include_code_data,
                address_book,
                metadata,
            );
            (StatusCode::OK, Json(response))
        }
        Err(e) => emulate_internal_error(e.to_string()),
    }
}

pub async fn get_jetton_masters(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonMastersRequest>,
) -> impl IntoResponse {
    handle_v3_result(
        node.get_jetton_masters(
            payload.address,
            payload.admin_address,
            payload.limit,
            payload.offset,
        ),
        v3::map_jetton_masters,
    )
    .await
}

pub async fn get_jetton_wallets(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonWalletsRequest>,
) -> impl IntoResponse {
    let wallets = match node
        .get_jetton_wallets(
            payload.address,
            payload.owner_address,
            payload.jetton_address,
            payload.exclude_zero_balance,
            payload.limit,
            payload.offset,
        )
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
            .get_jetton_masters(Some(jetton_address.to_string()), None, Some(1), Some(0))
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
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetNftItemsRequest>,
) -> impl IntoResponse {
    handle_v3_result(
        node.get_nft_items(
            payload.address,
            payload.owner_address,
            payload.collection_address,
            payload.index,
            payload.sort_by_last_transaction_lt,
            payload.limit,
            payload.offset,
        ),
        v3::map_nft_items,
    )
    .await
}

pub async fn send_message_v3(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> impl IntoResponse {
    handle_v3_result(node.send_boc(payload.boc), toncenter_v3::map_send_message).await
}

pub async fn run_get_method_v3(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> impl IntoResponse {
    let method_str = match parse_method_name(&payload.method) {
        Ok(s) => s,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    let stack = match normalize_v3_stack(payload.stack) {
        Ok(stack) => stack,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_v3_result(
        node.run_get_method(payload.address, method_str, stack, payload.seqno),
        toncenter_v3::map_run_get_method_v3,
    )
    .await
}

fn normalize_v3_stack(stack: Vec<Value>) -> anyhow::Result<Vec<Value>> {
    stack.into_iter().map(normalize_v3_stack_item).collect()
}

fn normalize_v3_stack_item(item: Value) -> anyhow::Result<Value> {
    if item.is_array() {
        return Ok(item);
    }

    let stack_type = item
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("v3 stack entry must contain string `type`"))?;
    let value = item.get("value").cloned().unwrap_or(Value::Null);

    match stack_type {
        "null" => Ok(json!(["null", Value::Null])),
        "num" => Ok(json!(["num", value])),
        "cell" | "slice" | "builder" => {
            let bytes = extract_stack_bytes(&value, stack_type)?;
            Ok(json!([stack_type, { "bytes": bytes }]))
        }
        "tuple" | "list" => {
            let elements = value
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("{stack_type} stack value must be an array"))?
                .iter()
                .cloned()
                .map(normalize_v3_stack_item)
                .collect::<anyhow::Result<Vec<_>>>()?;
            Ok(json!([stack_type, { "elements": elements }]))
        }
        _ => anyhow::bail!("Unsupported v3 stack entry type: {stack_type}"),
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

#[derive(Clone, Copy)]
enum MessageDirection {
    In,
    Out,
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

struct ParsedTransactionsByMessageV3Query {
    msg_hash: Option<Hash256>,
    body_hash: Option<Hash256>,
    opcode: Option<u32>,
    direction: Option<MessageDirection>,
    limit: usize,
    offset: usize,
}

struct ParsedPendingTransactionsV3Query {
    account: Option<HashSet<Addr>>,
    trace_ids: Option<HashSet<Hash256>>,
}

fn parse_transactions_v3_query(
    payload: GetTransactionsV3Query,
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
        seqno: payload.seqno,
        mc_seqno: payload.mc_seqno,
        account: parse_optional_address(payload.account)?,
        exclude_account: parse_optional_address(payload.exclude_account)?,
        hash: payload.hash.as_deref().map(parse_hash_any).transpose()?,
        lt: payload.lt,
        start_utime: payload.start_utime,
        end_utime: payload.end_utime,
        start_lt: payload.start_lt,
        end_lt: payload.end_lt,
        limit,
        offset,
        sort,
    })
}

fn parse_transactions_by_message_v3_query(
    payload: GetTransactionsByMessageV3Query,
) -> anyhow::Result<ParsedTransactionsByMessageV3Query> {
    let (limit, offset) = parse_limit_offset(payload.limit, payload.offset)?;
    let direction = match payload.direction.as_deref() {
        None => None,
        Some("in") => Some(MessageDirection::In),
        Some("out") => Some(MessageDirection::Out),
        Some(other) => anyhow::bail!("Invalid `direction`: {other}. Supported values: in, out"),
    };

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

fn parse_pending_transactions_v3_query(
    payload: GetPendingTransactionsV3Query,
) -> anyhow::Result<ParsedPendingTransactionsV3Query> {
    Ok(ParsedPendingTransactionsV3Query {
        account: parse_optional_address(payload.account)?,
        trace_ids: parse_optional_hash(payload.trace_id)?,
    })
}

fn filter_transactions_v3(
    txs: &[LiteNodeTransaction],
    query: &ParsedTransactionsV3Query,
) -> Vec<LiteNodeTransaction> {
    const BLOCK_WORKCHAIN: i32 = 0;
    const BLOCK_SHARD: i64 = i64::MIN;

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

fn filter_transactions_by_message_v3(
    txs: &[LiteNodeTransaction],
    query: &ParsedTransactionsByMessageV3Query,
) -> Vec<LiteNodeTransaction> {
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
                .filter(|msg| msg.hash.0 != [0; 32])
                .any(|msg| {
                    if let Some(msg_hash) = query.msg_hash
                        && msg.hash != msg_hash
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

fn filter_pending_transactions_v3(
    txs: &[LiteNodeTransaction],
    query: &ParsedPendingTransactionsV3Query,
) -> Vec<LiteNodeTransaction> {
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

fn sort_transactions(transactions: &mut [LiteNodeTransaction], order: SortOrder) {
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

fn parse_limit_offset(
    limit: Option<usize>,
    offset: Option<usize>,
) -> anyhow::Result<(usize, usize)> {
    let limit = limit.unwrap_or(10);
    if !(1..=1000).contains(&limit) {
        anyhow::bail!("`limit` must be between 1 and 1000");
    }
    Ok((limit, offset.unwrap_or(0)))
}

fn parse_sort(sort: Option<String>) -> anyhow::Result<SortOrder> {
    match sort.as_deref().unwrap_or("desc") {
        "asc" => Ok(SortOrder::Asc),
        "desc" => Ok(SortOrder::Desc),
        other => anyhow::bail!("Invalid `sort`: {other}. Supported values: asc, desc"),
    }
}

#[derive(Default)]
struct EmulateAddressInfo {
    interfaces: BTreeSet<String>,
    token_info: Vec<Value>,
    extra_jetton_masters: BTreeSet<Addr>,
}

async fn build_emulate_v1_extra_data(
    node: &LiteNode,
    trace: &TraceNode,
    include_address_book: bool,
    include_metadata: bool,
) -> anyhow::Result<(Option<Value>, Option<Value>)> {
    if !include_address_book && !include_metadata {
        return Ok((None, None));
    }

    let mut addresses = BTreeSet::new();
    collect_trace_addresses(trace, &mut addresses);

    let mut address_book = serde_json::Map::new();
    let mut metadata = serde_json::Map::new();
    let mut pending_jetton_masters = BTreeSet::new();

    for address in &addresses {
        let info = collect_emulate_address_info(node, *address).await?;
        pending_jetton_masters.extend(info.extra_jetton_masters.iter().copied());

        if include_address_book {
            address_book.insert(
                address.to_string(),
                json!({
                    "user_friendly": as_user_friendly(*address),
                    "domain": Value::Null,
                    "interfaces": info.interfaces.into_iter().collect::<Vec<_>>(),
                }),
            );
        }

        if include_metadata && !info.token_info.is_empty() {
            metadata.insert(
                address.to_string(),
                json!({
                    "is_indexed": true,
                    "token_info": info.token_info,
                }),
            );
        }
    }

    if include_metadata {
        for master_address in pending_jetton_masters {
            let key = master_address.to_string();
            if metadata.contains_key(&key) {
                continue;
            }
            let info = collect_emulate_address_info(node, master_address).await?;
            if info.token_info.is_empty() {
                continue;
            }
            metadata.insert(
                key,
                json!({
                    "is_indexed": true,
                    "token_info": info.token_info,
                }),
            );
        }
    }

    let address_book = include_address_book.then_some(Value::Object(address_book));
    let metadata = include_metadata.then_some(Value::Object(metadata));

    Ok((address_book, metadata))
}

fn collect_trace_addresses(trace: &TraceNode, out: &mut BTreeSet<Addr>) {
    out.insert(trace.transaction.meta.account);
    if let Some(in_msg) = &trace.transaction.in_msg {
        if let Some(src) = in_msg.meta.src {
            out.insert(src);
        }
        if let Some(dst) = in_msg.meta.dst {
            out.insert(dst);
        }
    }
    for out_msg in &trace.transaction.out_msgs {
        if let Some(src) = out_msg.meta.src {
            out.insert(src);
        }
        if let Some(dst) = out_msg.meta.dst {
            out.insert(dst);
        }
    }
    for child in &trace.children {
        collect_trace_addresses(child, out);
    }
}

async fn collect_emulate_address_info(
    node: &LiteNode,
    address: Addr,
) -> anyhow::Result<EmulateAddressInfo> {
    let mut out = EmulateAddressInfo::default();
    let address_str = address.to_string();

    let wallets = node
        .get_jetton_wallets(
            Some(address_str.clone()),
            None,
            None,
            Some(false),
            Some(1),
            Some(0),
        )
        .await?;
    if let Some(wallet) = wallets.first() {
        out.interfaces.insert("jetton_wallet".to_string());
        out.token_info.push(json!({
            "valid": true,
            "type": "jetton_wallets",
            "extra": {
                "owner": wallet.owner_address.to_string(),
                "jetton": wallet.jetton_address.to_string(),
                "balance": wallet.balance.to_string(),
            },
        }));
        out.extra_jetton_masters.insert(wallet.jetton_address);
    }

    let masters = node
        .get_jetton_masters(Some(address_str.clone()), None, Some(1), Some(0))
        .await?;
    if let Some(master) = masters.first() {
        out.interfaces.insert("jetton_master".to_string());
        out.token_info.push(map_emulate_jetton_master_token_info(
            master.jetton_content.clone(),
        ));
    }

    let items = node
        .get_nft_items(
            Some(address_str),
            None,
            None,
            None,
            Some(false),
            Some(1),
            Some(0),
        )
        .await?;
    if let Some(item) = items.first() {
        out.interfaces.insert("nft_item".to_string());
        out.token_info.push(map_emulate_nft_item_token_info(
            item.index.clone(),
            item.content.clone(),
        ));
    }

    Ok(out)
}

fn map_emulate_jetton_master_token_info(content: Value) -> Value {
    let mut mapped = serde_json::Map::new();
    mapped.insert("valid".to_string(), Value::Bool(true));
    mapped.insert(
        "type".to_string(),
        Value::String("jetton_masters".to_string()),
    );

    if let Some(name) = token_content_string(&content, "name") {
        mapped.insert("name".to_string(), Value::String(name));
    }
    if let Some(symbol) = token_content_string(&content, "symbol") {
        mapped.insert("symbol".to_string(), Value::String(symbol));
    }
    if let Some(description) = token_content_string(&content, "description") {
        mapped.insert("description".to_string(), Value::String(description));
    }
    if let Some(image) = token_content_string(&content, "image") {
        mapped.insert("image".to_string(), Value::String(image));
    }

    mapped.insert("extra".to_string(), content);
    Value::Object(mapped)
}

fn map_emulate_nft_item_token_info(index: String, content: Value) -> Value {
    let mut mapped = serde_json::Map::new();
    mapped.insert("valid".to_string(), Value::Bool(true));
    mapped.insert("type".to_string(), Value::String("nft_items".to_string()));
    mapped.insert("nft_index".to_string(), Value::String(index));

    if let Some(name) = token_content_string(&content, "name") {
        mapped.insert("name".to_string(), Value::String(name));
    }
    if let Some(symbol) = token_content_string(&content, "symbol") {
        mapped.insert("symbol".to_string(), Value::String(symbol));
    }
    if let Some(description) = token_content_string(&content, "description") {
        mapped.insert("description".to_string(), Value::String(description));
    }
    if let Some(image) = token_content_string(&content, "image") {
        mapped.insert("image".to_string(), Value::String(image));
    }

    mapped.insert("extra".to_string(), content);
    Value::Object(mapped)
}

fn token_content_string(content: &Value, key: &str) -> Option<String> {
    content
        .as_object()
        .and_then(|map| map.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn as_user_friendly(address: Addr) -> String {
    let workchain = i8::try_from(address.workchain).ok().unwrap_or_default();
    let std_addr = StdAddr::new(workchain, HashBytes(address.addr));
    DisplayBase64StdAddr {
        addr: &std_addr,
        flags: Base64StdAddrFlags {
            testnet: false,
            base64_url: true,
            bounceable: false,
        },
    }
    .to_string()
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

fn parse_std_addr(address: &str) -> anyhow::Result<Addr> {
    let (std_addr, _) = StdAddr::from_str_ext(address, StdAddrFormat::any())
        .map_err(|e| anyhow::anyhow!("Invalid address format `{address}`: {e}"))?;
    Ok(Addr {
        workchain: i32::from(std_addr.workchain),
        addr: std_addr.address.0,
    })
}

fn parse_optional_address(value: Option<String>) -> anyhow::Result<Option<HashSet<Addr>>> {
    let Some(address) = value else {
        return Ok(None);
    };
    let mut parsed = HashSet::new();
    parsed.insert(parse_std_addr(&address)?);
    Ok(Some(parsed))
}

fn parse_optional_hash(value: Option<String>) -> anyhow::Result<Option<HashSet<Hash256>>> {
    let Some(hash) = value else {
        return Ok(None);
    };
    let mut parsed = HashSet::new();
    parsed.insert(parse_hash_any(&hash)?);
    Ok(Some(parsed))
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

fn emulate_bad_request(error: impl Into<String>) -> (StatusCode, Json<Value>) {
    emulate_error_response(StatusCode::BAD_REQUEST, error)
}

fn emulate_internal_error(error: impl Into<String>) -> (StatusCode, Json<Value>) {
    emulate_error_response(StatusCode::INTERNAL_SERVER_ERROR, error)
}

fn emulate_error_response(
    status: StatusCode,
    error: impl Into<String>,
) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": error.into() })))
}

async fn handle_v3_result<T, F>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Response
where
    F: FnOnce(&T) -> Value,
{
    match result.await {
        Ok(res) => (StatusCode::OK, Json(mapper(&res))).into_response(),
        Err(e) => request_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}

fn v3_bad_request(error: impl Into<String>) -> Response {
    request_error(StatusCode::BAD_REQUEST, error)
}

fn request_error(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(json!({
            "error": error.into(),
            "code": status.as_u16(),
        })),
    )
        .into_response()
}
