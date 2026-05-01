use super::utils::{get_extra, handle_result, parse_method_name};
use crate::api::toncenter_v2 as v2;
use crate::localnet::Localnet;
use crate::server::models::{
    AddressRequest, DetectHashRequest, GetAddressInformationRequest, GetBlockRequest,
    GetConfigAllRequest, GetConfigParamRequest, GetLibrariesRequest, GetTransactionsRequest,
    LookupBlockRequest, RunGetMethodRequest, SendBocRequest, TryLocateTxRequest,
};
use crate::types::Hash256;
use axum::{
    Json,
    extract::{Query, State},
};
use base64::Engine;
use serde_json::Value;
use std::sync::Arc;
use tycho_types::models::{StdAddr, StdAddrFormat};

pub async fn send_boc(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), v2::map_block_transactions).await
}

pub async fn run_get_method(
    State(node): State<Arc<Localnet>>,
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
    State(node): State<Arc<Localnet>>,
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
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        v2::map_account_state,
    )
    .await
}

pub async fn get_address_balance(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_balance(payload.address, payload.seqno),
        |res| res.to_string().into(),
    )
    .await
}

pub async fn get_address_state(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_state(payload.address, payload.seqno),
        |res| res.to_string().into(),
    )
    .await
}

pub async fn get_extended_address_information(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_address_information(payload.address, payload.seqno),
        v2::map_extended_account_state,
    )
    .await
}

pub async fn get_shard_account_cell(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetAddressInformationRequest>,
) -> Json<Value> {
    handle_result(
        node.get_shard_account_cell(payload.address, payload.seqno),
        v2::map_shard_account_cell,
    )
    .await
}

pub async fn get_libraries(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetLibrariesRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            let hashes = parse_libraries_query(&payload.libraries)?;
            node.get_libraries(hashes).await
        },
        |res| v2::map_libraries(res),
    )
    .await
}

pub async fn get_transactions(
    State(node): State<Arc<Localnet>>,
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
    State(node): State<Arc<Localnet>>,
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

pub async fn try_locate_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Json<Value> {
    handle_result(
        node.try_locate_tx(payload.source, payload.destination, payload.created_lt),
        v2::map_transaction,
    )
    .await
}

pub async fn try_locate_result_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Json<Value> {
    handle_result(
        node.try_locate_result_tx(payload.source, payload.destination, payload.created_lt),
        v2::map_transaction,
    )
    .await
}

pub async fn try_locate_source_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Json<Value> {
    handle_result(
        node.try_locate_source_tx(payload.source, payload.destination, payload.created_lt),
        v2::map_transaction,
    )
    .await
}

pub async fn get_config_param(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetConfigParamRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            let param = parse_config_param(&payload)?;
            let seqno = parse_seqno(payload.seqno)?;
            node.get_config_param(param, seqno).await
        },
        v2::map_config_info,
    )
    .await
}

pub async fn get_config_all(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetConfigAllRequest>,
) -> Json<Value> {
    handle_result(
        async move {
            let seqno = parse_seqno(payload.seqno)?;
            node.get_config_all(seqno).await
        },
        v2::map_config_info,
    )
    .await
}

pub async fn detect_address(Query(payload): Query<AddressRequest>) -> Json<Value> {
    handle_result(
        async move {
            let (addr, flags) = parse_std_addr(&payload.address)?;
            let given_type = detect_given_type(&payload.address, flags.bounceable);
            Ok(v2::map_detect_address(&addr, flags, given_type))
        },
        Value::clone,
    )
    .await
}

pub async fn detect_hash(Query(payload): Query<DetectHashRequest>) -> Json<Value> {
    handle_result(
        async move {
            let hash = parse_hash_any(&payload.hash)?;
            Ok(v2::map_detect_hash(&hash))
        },
        Value::clone,
    )
    .await
}

pub async fn pack_address(Query(payload): Query<AddressRequest>) -> Json<Value> {
    handle_result(
        async move {
            let (addr, flags) = parse_std_addr(&payload.address)?;
            Ok(v2::map_pack_address(&addr, flags.testnet))
        },
        Value::clone,
    )
    .await
}

pub async fn unpack_address(Query(payload): Query<AddressRequest>) -> Json<Value> {
    handle_result(
        async move {
            let (addr, _) = parse_std_addr(&payload.address)?;
            Ok(v2::map_unpack_address(&addr))
        },
        Value::clone,
    )
    .await
}

pub async fn get_block_header(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_header(payload.seqno as u32),
        v2::map_block_header,
    )
    .await
}

pub async fn get_block_transactions_ext_post(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions_ext,
    )
    .await
}

pub async fn send_boc_return_hash(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendBocRequest>,
) -> Json<Value> {
    handle_result(node.send_boc(payload.boc), v2::map_send_boc_return_hash).await
}

pub async fn get_block_transactions(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_ext(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions_ext,
    )
    .await
}

pub async fn get_masterchain_info(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), v2::map_masterchain_info).await
}

pub async fn get_consensus_block(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.get_consensus_block(), v2::map_consensus_block).await
}

pub async fn get_out_msg_queue_size(State(node): State<Arc<Localnet>>) -> Json<Value> {
    handle_result(node.get_masterchain_info(), v2::map_out_msg_queue_sizes).await
}

pub async fn get_shards(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<GetBlockRequest>,
) -> Json<Value> {
    handle_result(node.get_shards(payload.seqno as u32), v2::map_shards).await
}

pub async fn lookup_block(
    State(node): State<Arc<Localnet>>,
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
        let err = parse_libraries_query(" , , ").expect_err("empty list must be rejected");
        assert!(
            err.to_string()
                .contains("`libraries` query parameter is required"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_libraries_query_rejects_invalid_hash() {
        let err = parse_libraries_query("not-a-hash").expect_err("invalid hash must be rejected");
        assert!(
            err.to_string().contains("Invalid hash format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_libraries_query_accepts_multiple_hashes_and_skips_blanks() {
        let hash_a = "11".repeat(32);
        let hash_b = "22".repeat(32);

        let parsed = parse_libraries_query(&format!("{hash_a}, ,{hash_b},"))
            .expect("valid list with blanks must parse");
        assert_eq!(parsed, vec![Hash256([0x11; 32]), Hash256([0x22; 32])]);
    }
}
