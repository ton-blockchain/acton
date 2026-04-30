use super::utils::{get_extra, parse_method_name, parse_params};
use crate::api::toncenter_v2 as v2;
use crate::api::toncenter_v2::map_detect_address;
use crate::localnet::Localnet;
use crate::server::models::{
    AddressRequest, DetectHashRequest, GetAddressInformationRequest, GetBlockRequest,
    GetConfigAllRequest, GetConfigParamRequest, GetLibrariesRequest, GetTransactionsRequest,
    JsonRpcRequest, LookupBlockRequest, RunGetMethodRequest, SendBocRequest, TryLocateTxRequest,
};
use crate::types::Hash256;
use axum::response::{IntoResponse, Response};
use axum::{Json, extract::State, http::StatusCode};
use base64::Engine;
use serde_json::Value;
use std::sync::Arc;
use tycho_types::models::{StdAddr, StdAddrFormat};

pub async fn json_rpc(
    State(node): State<Arc<Localnet>>,
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

async fn json_rpc_router(node: Arc<Localnet>, payload: JsonRpcRequest) -> anyhow::Result<Response> {
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
        "detectAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = parse_std_addr(&req.address)?;
            let given_type = detect_given_type(&req.address, flags.bounceable);
            map_detect_address(&addr, flags, given_type)
        }
        "detectHash" => {
            let req: DetectHashRequest = parse_params(params, method)?;
            let hash = parse_hash_any(&req.hash)?;
            v2::map_detect_hash(&hash)
        }
        "packAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = parse_std_addr(&req.address)?;
            v2::map_pack_address(&addr, flags.testnet)
        }
        "unpackAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, _) = parse_std_addr(&req.address)?;
            v2::map_unpack_address(&addr)
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
        "getLibraries" => {
            let req: GetLibrariesRequest = parse_params(params, method)?;
            let hashes = parse_libraries_query(&req.libraries)?;
            node.get_libraries(hashes)
                .await
                .map(|r| v2::map_libraries(&r))?
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
        "getConfigParam" => {
            let req: GetConfigParamRequest = parse_params(params, method)?;
            let param = parse_config_param(&req)?;
            let seqno = parse_seqno(req.seqno)?;
            node.get_config_param(param, seqno)
                .await
                .map(|r| v2::map_config_info(&r))?
        }
        "getConfigAll" => {
            let req: GetConfigAllRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            node.get_config_all(seqno)
                .await
                .map(|r| v2::map_config_info(&r))?
        }
        "tryLocateTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            node.try_locate_tx(req.source, req.destination, req.created_lt)
                .await
                .map(|r| v2::map_transaction(&r))?
        }
        "tryLocateResultTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            node.try_locate_result_tx(req.source, req.destination, req.created_lt)
                .await
                .map(|r| v2::map_transaction(&r))?
        }
        "tryLocateSourceTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            node.try_locate_source_tx(req.source, req.destination, req.created_lt)
                .await
                .map(|r| v2::map_transaction(&r))?
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
        "getConsensusBlock" => node
            .get_consensus_block()
            .await
            .map(|r| v2::map_consensus_block(&r))?,
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

fn parse_std_addr(
    address: &str,
) -> anyhow::Result<(StdAddr, tycho_types::models::Base64StdAddrFlags)> {
    StdAddr::from_str_ext(address, StdAddrFormat::any())
        .map_err(|e| anyhow::anyhow!("Invalid address format: {e}"))
}

fn detect_given_type(address: &str, bounceable: bool) -> &'static str {
    if address.contains(':') {
        "raw_form"
    } else if bounceable {
        "friendly_bounceable"
    } else {
        "friendly_non_bounceable"
    }
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

    anyhow::bail!("Invalid hash format")
}

fn parse_config_param(payload: &GetConfigParamRequest) -> anyhow::Result<u32> {
    let raw = payload
        .param
        .or(payload.config_id)
        .ok_or_else(|| anyhow::anyhow!("`param` is required"))?;
    if raw < 0 {
        anyhow::bail!("Config param must be a non-negative integer");
    }
    Ok(raw as u32)
}

fn parse_libraries_query(raw: &str) -> anyhow::Result<Vec<Hash256>> {
    let hashes = raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(parse_hash_any)
        .collect::<anyhow::Result<Vec<_>>>()?;

    if hashes.is_empty() {
        anyhow::bail!("`libraries` query parameter is required");
    }

    Ok(hashes)
}

fn parse_seqno(seqno: Option<i32>) -> anyhow::Result<Option<u32>> {
    match seqno {
        Some(value) if value < 0 => anyhow::bail!("`seqno` must be a non-negative integer"),
        Some(value) => Ok(Some(value as u32)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_libraries_query_rejects_empty_input() {
        let err = parse_libraries_query(" , ").expect_err("empty list must be rejected");
        assert!(
            err.to_string()
                .contains("`libraries` query parameter is required"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_libraries_query_rejects_invalid_hash() {
        let err = parse_libraries_query("bad-hash").expect_err("invalid hash must be rejected");
        assert!(
            err.to_string().contains("Invalid hash format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_libraries_query_accepts_multiple_hashes_and_skips_blanks() {
        let hash_a = "aa".repeat(32);
        let hash_b = "bb".repeat(32);

        let parsed = parse_libraries_query(&format!("{hash_a},,{hash_b}, "))
            .expect("valid list with blanks must parse");
        assert_eq!(parsed, vec![Hash256([0xAA; 32]), Hash256([0xBB; 32])]);
    }
}
