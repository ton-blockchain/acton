use super::models::*;
use crate::api::toncenter_v3;
use crate::litenode::LiteNode;
use crate::{api, node};
use axum::response::Response;
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::future::Future;
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

    let id = payload.id.clone();
    let result = json_rpc_router(node, payload).await;

    match result {
        Ok(resp) => resp,
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "ok": false,
                "error": e.to_string(),
                "code": 500
            })),
        )
            .into_response(),
    }
}

async fn json_rpc_router(node: Arc<LiteNode>, payload: JsonRpcRequest) -> anyhow::Result<Response> {
    let params = payload.params;
    let method = payload.method.as_str();

    let res: Value = match method {
        "sendBoc" => {
            let req: SendBocRequest = parse_params(params, method)?;
            node.send_boc(req.boc)
                .await
                .map(|r| api::map_block_transactions(&r))?
        }
        "sendBocReturnHash" => {
            let req: SendBocRequest = parse_params(params, method)?;
            node.send_boc(req.boc)
                .await
                .map(|r| api::map_send_boc_return_hash(&r))?
        }
        "runGetMethod" => {
            let req: RunGetMethodRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method)?;
            node.run_get_method(req.address, method_str, req.stack, req.seqno)
                .await
                .map(|r| api::map_run_get_method(&r, true))?
        }
        "runGetMethodStd" => {
            let req: RunGetMethodRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method)?;
            node.run_get_method(req.address, method_str, req.stack, req.seqno)
                .await
                .map(|r| api::map_run_get_method(&r, false))?
        }
        "getAddressInformation" => {
            let req: GetAddressInformationRequest = parse_params(params, method)?;
            node.get_address_information(req.address, req.seqno)
                .await
                .map(|r| api::map_account_state(&r))?
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
                .map(|r| format!("{:?}", r).to_lowercase().into())?
        }
        "getExtendedAddressInformation" => {
            let req: GetAddressInformationRequest = parse_params(params, method)?;
            node.get_address_information(req.address, req.seqno)
                .await
                .map(|r| api::map_extended_account_state(&r))?
        }
        "getTransactions" => {
            let req: GetTransactionsRequest = parse_params(params, method)?;
            node.get_transactions(req.address, req.limit, req.lt, req.hash, req.to_lt)
                .await
                .map(|r| api::map_transactions(&r))?
        }
        "getBlockHeader" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_block_header(req.seqno as u32)
                .await
                .map(|r| api::map_block_header(&r))?
        }
        "getBlockTransactions" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_block_transactions(req.seqno as u32)
                .await
                .map(|r| api::map_block_transactions(&r))?
        }
        "getBlockTransactionsExt" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_block_transactions(req.seqno as u32)
                .await
                .map(|r| api::map_block_transactions_ext(&r))?
        }
        "getMasterchainInfo" => node
            .get_masterchain_info()
            .await
            .map(|r| api::map_masterchain_info(&r))?,
        "shards" => {
            let req: GetBlockRequest = parse_params(params, method)?;
            node.get_shards(req.seqno as u32)
                .await
                .map(|r| api::map_shards(&r))?
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
            .map(|r| api::map_lookup_block(&r))?
        }
        _ => {
            return Ok((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": payload.id,
                    "ok": false,
                    "error": "Method not found",
                    "code": 404
                })),
            )
                .into_response());
        }
    };

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": payload.id,
            "ok": true,
            "result": res
        })),
    )
        .into_response())
}

fn parse_params<T: DeserializeOwned>(params: Value, method: &str) -> anyhow::Result<T> {
    serde_json::from_value(params).map_err(|_| anyhow::anyhow!("Invalid params for {}", method))
}

pub async fn send_boc(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), api::map_block_transactions).await
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
                "code": 400
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, payload.seqno),
        |res| api::map_run_get_method(res, true),
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
                "code": 400
            }));
        }
    };

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, payload.seqno),
        |res| api::map_run_get_method(res, false),
    )
    .await
}

pub async fn get_address_information(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        api::map_account_state,
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
        |res| format!("{:?}", res).to_lowercase().into(),
    )
    .await
}

pub async fn get_extended_address_information(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        api::map_extended_account_state,
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
        api::map_transactions,
    )
    .await
}

pub async fn get_block_header(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_header(payload.seqno as u32),
        api::map_block_header,
    )
    .await
}

pub async fn get_block_transactions_ext_post(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions_ext,
    )
    .await
}

pub async fn send_boc_return_hash(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), api::map_send_boc_return_hash).await
}

pub async fn get_block_transactions(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_ext(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        api::map_block_transactions_ext,
    )
    .await
}

pub async fn get_masterchain_info(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), api::map_masterchain_info).await
}

pub async fn get_shards(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(node.get_shards(payload.seqno as u32), api::map_shards).await
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
        api::map_lookup_block,
    )
    .await
}

pub async fn faucet(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<FaucetRequest>,
) -> Json<Value> {
    handle_result(node.faucet(payload.address, payload.amount), |res| {
        res.clone()
    })
    .await
}

pub async fn get_traces(
    State(node): State<Arc<LiteNode>>,
    Query(payload): Query<GetTracesQuery>,
) -> Json<Value> {
    handle_result(node.get_traces(payload.hash), toncenter_v3::map_traces).await
}

pub async fn get_state_source(State(node): State<Arc<LiteNode>>) -> Json<Value> {
    handle_result(node.get_state_source(), |res| {
        serde_json::to_value(res).unwrap_or(Value::Null)
    })
    .await
}

pub async fn set_state_source(
    State(node): State<Arc<LiteNode>>,
    Json(payload): Json<node::StateSource>,
) -> Json<Value> {
    handle_result(node.set_state_source(payload), |_| Value::Null).await
}

fn parse_method_name(method: &Value) -> anyhow::Result<String> {
    match method {
        Value::String(s) => Ok(s.clone()),
        Value::Number(n) => Ok(n.to_string()),
        _ => anyhow::bail!("Invalid method format"),
    }
}

async fn handle_result<T, F>(
    result: impl Future<Output = anyhow::Result<T>>,
    mapper: F,
) -> Json<Value>
where
    F: FnOnce(&T) -> Value,
{
    match result.await {
        Ok(res) => Json(serde_json::json!({
            "ok": true,
            "result": mapper(&res)
        })),
        Err(e) => Json(serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "code": 500
        })),
    }
}
