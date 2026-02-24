use super::utils::{get_extra, handle_result, parse_method_name};
use crate::api::toncenter_v2 as v2;
use crate::litenode::LiteNode;
use crate::server::models::*;
use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::Value;
use std::sync::Arc;

pub async fn send_boc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), v2::map_block_transactions).await
}

pub async fn run_get_method(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match parse_method_name(&payload.method) {
        Ok(s) => s,
        Err(e) => {
            return Json(serde_json::json!({
                "ok": false,
                "error": e.to_string(),
                "code": 400,
                "@extra": get_extra()
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, payload.seqno),
        |res| v2::map_run_get_method(res, true),
    )
    .await
}

pub async fn run_get_method_std(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Json<Value> {
    let method_str = match parse_method_name(&payload.method) {
        Ok(s) => s,
        Err(e) => {
            return Json(serde_json::json!({
                "ok": false,
                "error": e.to_string(),
                "code": 400,
                "@extra": get_extra()
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, payload.seqno),
        |res| v2::map_run_get_method(res, false),
    )
    .await
}

pub async fn get_address_information(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        v2::map_account_state,
    )
    .await
}

pub async fn get_address_balance(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_balance(payload.address, payload.seqno),
        |res| res.to_string().into(),
    )
    .await
}

pub async fn get_address_state(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_state(payload.address, payload.seqno),
        |res| res.to_string().into(),
    )
    .await
}

pub async fn get_extended_address_information(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        v2::map_extended_account_state,
    )
    .await
}

pub async fn get_transactions(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsRequest>,
) -> Json<Value> {
    handle_result(
        node.get_transactions(
            payload.address,
            payload.limit,
            payload.lt,
            payload.hash,
            payload.to_lt,
        ),
        v2::map_transactions,
    )
    .await
}

pub async fn get_transactions_std(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTransactionsRequest>,
) -> Json<Value> {
    let page_limit = payload.limit;
    let fetch_limit = page_limit.saturating_add(1);
    handle_result(
        node.get_transactions(
            payload.address,
            fetch_limit,
            payload.lt,
            payload.hash,
            payload.to_lt,
        ),
        |res| v2::map_transactions_std(res, page_limit),
    )
    .await
}

pub async fn get_block_header(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_header(payload.seqno as u32),
        v2::map_block_header,
    )
    .await
}

pub async fn get_block_transactions_ext_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions_ext,
    )
    .await
}

pub async fn send_boc_return_hash(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), v2::map_send_boc_return_hash).await
}

pub async fn get_block_transactions(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_ext(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions_ext,
    )
    .await
}

pub async fn get_masterchain_info(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), v2::map_masterchain_info).await
}

pub async fn get_out_msg_queue_size(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), v2::map_out_msg_queue_sizes).await
}

pub async fn get_shards(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(node.get_shards(payload.seqno as u32), v2::map_shards).await
}

pub async fn lookup_block(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<LookupBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.lookup_block(
            payload.workchain,
            payload.shard,
            payload.seqno.map(|x| x as u32),
            payload.lt,
            payload.unixtime,
        ),
        v2::map_lookup_block,
    )
    .await
}
