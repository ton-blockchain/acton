use super::utils::{get_extra, parse_method_name, parse_params};
use crate::api::toncenter_v2 as v2;
use crate::litenode::LiteNode;
use crate::server::models::*;
use axum::response::{IntoResponse, Response};
use axum::{Json, extract::State, http::StatusCode};
use serde_json::Value;
use std::sync::Arc;

pub async fn json_rpc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    tracing::debug!(
        "JSON-RPC request: method={}, id={:?}",
        payload.method,
        payload.id
    );

    let id_str = match &payload.id {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        v => v.to_string(),
    };

    let result: anyhow::Result<Response> = json_rpc_router(node, payload).await;

    match result {
        Ok(resp) => resp,
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id_str,
                "ok": false,
                "error": e.to_string(),
                "code": 500,
                "@extra": get_extra()
            })),
        )
            .into_response(),
    }
}

async fn json_rpc_router(node: Arc<LiteNode>, payload: JsonRpcRequest) -> anyhow::Result<Response> {
    let params = payload.params;
    let method = payload.method.as_str();
    let id_str = match &payload.id {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Null => "null".to_string(),
        v => v.to_string(),
    };

    let res: Value = match method {
        "sendBoc" => {
            let req: SendBocRequest = parse_params(params, method)?;
            node.send_boc(req.boc)
                .await
                .map(|r| v2::map_block_transactions(&r))?
        }
        "sendBocReturnHash" => {
            let req: SendBocRequest = parse_params(params, method)?;
            node.send_boc(req.boc)
                .await
                .map(|r| v2::map_send_boc_return_hash(&r))?
        }
        "runGetMethod" => {
            let req: RunGetMethodRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method)?;
            node.run_get_method(req.address, method_str, req.stack, req.seqno)
                .await
                .map(|r| v2::map_run_get_method(&r, true))?
        }
        "runGetMethodStd" => {
            let req: RunGetMethodRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method)?;
            node.run_get_method(req.address, method_str, req.stack, req.seqno)
                .await
                .map(|r| v2::map_run_get_method(&r, false))?
        }
        "getAddressInformation" => {
            let req: GetAddressInformationRequest = parse_params(params, method)?;
            node.get_address_information(req.address, req.seqno)
                .await
                .map(|r| v2::map_account_state(&r))?
        }
        "getAddressBalance" => {
            let req: GetAddressInformationRequest = parse_params(params, method)?;
            node.get_address_balance(req.address, req.seqno)
                .await
                .map(|r| r.to_string().into())?
        }
        "getAddressState" => {
            let req: GetAddressInformationRequest = parse_params(params, method)?;
            node.get_address_state(req.address, req.seqno)
                .await
                .map(|r| r.to_string().into())?
        }
        "getExtendedAddressInformation" => {
            let req: GetAddressInformationRequest = parse_params(params, method)?;
            node.get_address_information(req.address, req.seqno)
                .await
                .map(|r| v2::map_extended_account_state(&r))?
        }
        "getTransactions" => {
            let req: GetTransactionsRequest = parse_params(params, method)?;
            node.get_transactions(req.address, req.limit, req.lt, req.hash, req.to_lt)
                .await
                .map(|r| v2::map_transactions(&r))?
        }
        "getTransactionsStd" => {
            let req: GetTransactionsRequest = parse_params(params, method)?;
            let page_limit = req.limit;
            let fetch_limit = page_limit.saturating_add(1);
            node.get_transactions(req.address, fetch_limit, req.lt, req.hash, req.to_lt)
                .await
                .map(|r| v2::map_transactions_std(&r, page_limit))?
        }
        "getBlockHeader" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_block_header(req.seqno as u32)
                .await
                .map(|r| v2::map_block_header(&r))?
        }
        "getBlockTransactions" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_block_transactions(req.seqno as u32)
                .await
                .map(|r| v2::map_block_transactions(&r))?
        }
        "getBlockTransactionsExt" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_block_transactions(req.seqno as u32)
                .await
                .map(|r| v2::map_block_transactions_ext(&r))?
        }
        "getMasterchainInfo" => node
            .get_masterchain_info()
            .await
            .map(|r| v2::map_masterchain_info(&r))?,
        "getOutMsgQueueSize" => node
            .get_masterchain_info()
            .await
            .map(|r| v2::map_out_msg_queue_sizes(&r))?,
        "shards" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_shards(req.seqno as u32)
                .await
                .map(|r| v2::map_shards(&r))?
        }
        "lookupBlock" => {
            let req: LookupBlockRequest = parse_params(params, method)?;
            node.lookup_block(
                req.workchain,
                req.shard,
                req.seqno.map(|x| x as u32),
                req.lt,
                req.unixtime,
            )
            .await
            .map(|r| v2::map_lookup_block(&r))?
        }
        _ => {
            return Ok((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id_str,
                    "ok": false,
                    "error": "Method not found",
                    "code": 404,
                    "@extra": get_extra()
                })),
            )
                .into_response());
        }
    };

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id_str,
            "ok": true,
            "result": res,
            "@extra": get_extra()
        })),
    )
        .into_response())
}
