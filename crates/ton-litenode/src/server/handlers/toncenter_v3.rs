use super::utils::{get_extra, handle_result, parse_method_name};
use crate::api::toncenter_v3;
use crate::litenode::{LiteNode, LiteNodeTransaction};
use crate::server::models::{
    EmulateTraceRequest, GetAddressInformationV3Request, GetJettonMastersRequest,
    GetJettonWalletsRequest, GetPendingTransactionsV3Query, GetTracesQuery,
    GetTransactionsByMessageV3Query, GetTransactionsV3Query, RunGetMethodRequest, SendBocRequest,
};
use crate::types::{Addr, Hash256};
use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use base64::Engine;
use serde_json::Value;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use toncenter_v3 as v3;
use tycho_types::models::{StdAddr, StdAddrFormat};

pub async fn get_traces(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> Json<Value> {
    handle_result(node.get_traces(payload.hash), v3::map_traces).await
}

pub async fn get_address_information_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationV3Request>,
) -> Json<Value> {
    let _use_v2 = payload.use_v2.unwrap_or(true);

    handle_result(
        node.get_address_information(payload.address, None),
        toncenter_v3::map_address_information,
    )
    .await
}

pub async fn get_transactions_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsV3Query>,
) -> Json<Value> {
    let parsed = match parse_transactions_v3_query(payload) {
        Ok(parsed) => parsed,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_result(node.get_all_transactions(), move |txs| {
        let filtered = filter_transactions_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_transactions_by_message_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsByMessageV3Query>,
) -> Json<Value> {
    let parsed = match parse_transactions_by_message_v3_query(payload) {
        Ok(parsed) => parsed,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_result(node.get_all_transactions(), move |txs| {
        let filtered = filter_transactions_by_message_v3(txs, &parsed);
        v3::map_transactions_response(&filtered)
    })
    .await
}

pub async fn get_pending_transactions_v3(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetPendingTransactionsV3Query>,
) -> Json<Value> {
    let parsed = match parse_pending_transactions_v3_query(payload) {
        Ok(parsed) => parsed,
        Err(e) => return v3_bad_request(e.to_string()),
    };

    handle_result(node.get_pending_transactions(), move |txs| {
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

    if include_address_book || include_metadata {
        return emulate_bad_request("invalid request: address book and metadata are not available");
    }

    match node
        .emulate_trace(boc, payload.ignore_chksig, payload.mc_block_seqno)
        .await
    {
        Ok(trace) => {
            let response = v3::map_emulate_trace_response(
                &trace,
                with_actions,
                include_code_data,
                include_address_book,
                include_metadata,
            );
            (StatusCode::OK, Json(response))
        }
        Err(e) => emulate_internal_error(e.to_string()),
    }
}

pub async fn get_jetton_masters(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetJettonMastersRequest>,
) -> Json<Value> {
    handle_result(
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
) -> Json<Value> {
    handle_result(
        node.get_jetton_wallets(
            payload.address,
            payload.owner_address,
            payload.jetton_address,
            payload.exclude_zero_balance,
            payload.limit,
            payload.offset,
        ),
        v3::map_jetton_wallets,
    )
    .await
}

pub async fn send_message_v3(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), toncenter_v3::map_send_message).await
}

pub async fn run_get_method_v3(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match parse_method_name(&payload.method) {
        Ok(s) => s,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": e.to_string(),
                "code": 400,
                "@extra": get_extra()
            }));
        }
    };

    let stack = match normalize_v3_stack(payload.stack) {
        Ok(stack) => stack,
        Err(e) => {
            return Json(json!({
                "ok": false,
                "error": e.to_string(),
                "code": 400,
                "@extra": get_extra()
            }));
        }
    };

    handle_result(
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
        workchain: std_addr.workchain as i32,
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

fn v3_bad_request(error: impl Into<String>) -> Json<Value> {
    Json(json!({
        "ok": false,
        "error": error.into(),
        "code": 400,
        "@extra": get_extra(),
    }))
}
