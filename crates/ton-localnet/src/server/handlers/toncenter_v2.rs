use super::utils::{get_extra, handle_tonlib_result as handle_result, parse_method_name};
use crate::api::toncenter_v2 as v2;
use crate::localnet::{Localnet, LocalnetAddressInfo};
use crate::server::toncenter_adapters::{BlockQueryAdapter, LibrariesRestQuery};
use crate::types::Hash256;
use axum::{
    Json,
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use base64::Engine;
use std::sync::Arc;
use ton_api::toncenter::v2::StringOrNumber;
use ton_api::toncenter::v2::requests::{
    AddressInformationRequest, AddressRequest, ConfigAllRequest, ConfigParamRequest,
    DetectHashRequest, LookupBlockRequest, RunGetMethodRequest, SendBocRequest,
    TransactionsRequest, TryLocateTxRequest,
};
use tycho_types::models::{StdAddr, StdAddrFormat};

macro_rules! parse {
    ($expression:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => return v2_bad_request(error),
        }
    };
}

pub async fn send_boc(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendBocRequest>,
) -> Response {
    handle_result(node.send_boc(payload.boc), v2::map_send_boc).await
}

pub async fn run_get_method(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Response {
    let method_str = parse!(parse_method_name(&payload.method));
    let seqno = parse!(parse_seqno(payload.seqno));

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, seqno),
        |res| v2::map_run_get_method(res, true),
    )
    .await
}

pub async fn run_get_method_std(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RunGetMethodRequest>,
) -> Response {
    let method_str = parse!(parse_method_name(&payload.method));
    let seqno = parse!(parse_seqno(payload.seqno));

    handle_result(
        node.run_get_method(payload.address, method_str, payload.stack, seqno),
        |res| v2::map_run_get_method(res, false),
    )
    .await
}

pub async fn get_address_information(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    let seqno = parse!(parse_seqno(payload.seqno));
    handle_result(
        node.get_address_information(payload.address, seqno),
        v2::map_account_state,
    )
    .await
}

pub async fn get_address_balance(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    let seqno = parse!(parse_seqno(payload.seqno));
    handle_result(
        node.get_address_balance(payload.address, seqno),
        ToString::to_string,
    )
    .await
}

pub async fn get_address_state(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    let seqno = parse!(parse_seqno(payload.seqno));
    handle_result(
        node.get_address_state(payload.address, seqno),
        ToString::to_string,
    )
    .await
}

pub async fn get_extended_address_information(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    let seqno = parse!(parse_seqno(payload.seqno));
    handle_result(
        node.get_address_information(payload.address, seqno),
        v2::map_extended_account_state,
    )
    .await
}

pub async fn get_wallet_information(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    handle_result(
        async move {
            let seqno = parse_seqno(payload.seqno)?;
            let info = node
                .get_address_information(payload.address.clone(), seqno)
                .await?;
            let seqno = if v2::wallet_type_name_from_code_hash(info.code_hash.as_ref()).is_some() {
                node.run_get_method(payload.address, "seqno".to_string(), Vec::new(), seqno)
                    .await
                    .ok()
                    .and_then(|result| v2::map_wallet_seqno(&result))
            } else {
                None
            };

            Ok(v2::map_wallet_information(&info, seqno))
        },
        Clone::clone,
    )
    .await
}

pub async fn get_token_data(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    handle_result(
        async move {
            let address = Localnet::parse_addr(&payload.address)?;
            let mut infos = node.get_address_infos(vec![address]).await?;
            let info = infos
                .pop()
                .ok_or_else(|| anyhow::anyhow!("Address information not found"))?;
            let jetton_wallet_code_hash = token_wallet_code_hash(&node, &info).await;
            let jetton_wallet_code = match jetton_wallet_code_hash {
                Some(hash) => node.get_cell_boc(hash).await?,
                None => None,
            };

            v2::map_token_data(&info, jetton_wallet_code.as_ref(), None).ok_or_else(|| {
                anyhow::anyhow!("Smart contract {} is not Jetton or NFT", payload.address)
            })
        },
        Clone::clone,
    )
    .await
}

pub async fn get_shard_account_cell(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    let seqno = parse!(parse_seqno(payload.seqno));
    handle_result(
        node.get_shard_account_cell(payload.address, seqno),
        v2::map_shard_account_cell,
    )
    .await
}

pub(super) async fn token_wallet_code_hash(
    node: &Localnet,
    info: &LocalnetAddressInfo,
) -> Option<Hash256> {
    if let Some(master) = info.jetton_master.as_ref() {
        return Some(master.jetton_wallet_code_hash);
    }

    let wallet = info.jetton_wallet.as_ref()?;
    node.get_jetton_masters(
        vec![wallet.jetton_address.to_string()],
        Vec::new(),
        Some(1),
        Some(0),
    )
    .await
    .ok()
    .and_then(|mut masters| masters.pop())
    .map(|master| master.jetton_wallet_code_hash)
}

pub async fn get_libraries(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<LibrariesRestQuery>,
) -> Response {
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
    Query(payload): Query<TransactionsRequest>,
) -> Response {
    let (limit, lt, to_lt) = parse!(parse_transactions_request(&payload));
    handle_result(
        node.get_transactions(payload.address, limit, lt, payload.hash, to_lt),
        |transactions| v2::map_transactions(transactions),
    )
    .await
}

pub async fn get_transactions_std(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TransactionsRequest>,
) -> Response {
    let (page_limit, lt, to_lt) = parse!(parse_transactions_request(&payload));
    let fetch_limit = page_limit.saturating_add(1);
    handle_result(
        node.get_transactions(payload.address, fetch_limit, lt, payload.hash, to_lt),
        |res| v2::map_transactions_std(res, page_limit),
    )
    .await
}

pub async fn try_locate_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Response {
    let created_lt = parse!(payload.created_lt.to_u64());
    handle_result(
        node.try_locate_tx(payload.source, payload.destination, created_lt),
        v2::map_transaction,
    )
    .await
}

pub async fn try_locate_result_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Response {
    let created_lt = parse!(payload.created_lt.to_u64());
    handle_result(
        node.try_locate_result_tx(payload.source, payload.destination, created_lt),
        v2::map_transaction,
    )
    .await
}

pub async fn try_locate_source_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Response {
    let created_lt = parse!(payload.created_lt.to_u64());
    handle_result(
        node.try_locate_source_tx(payload.source, payload.destination, created_lt),
        v2::map_transaction,
    )
    .await
}

pub async fn get_config_param(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<ConfigParamRequest>,
) -> Response {
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
    Query(payload): Query<ConfigAllRequest>,
) -> Response {
    handle_result(
        async move {
            let seqno = parse_seqno(payload.seqno)?;
            node.get_config_all(seqno).await
        },
        v2::map_config_info,
    )
    .await
}

pub async fn detect_address(Query(payload): Query<AddressRequest>) -> Response {
    handle_result(
        async move {
            let (addr, flags) = parse_std_addr(&payload.address)?;
            let given_type = detect_given_type(&payload.address, flags.bounceable);
            Ok(v2::map_detect_address(&addr, flags, given_type))
        },
        Clone::clone,
    )
    .await
}

pub async fn detect_hash(Query(payload): Query<DetectHashRequest>) -> Response {
    handle_result(
        async move {
            let hash = parse_hash_any(&payload.hash)?;
            Ok(v2::map_detect_hash(&hash))
        },
        Clone::clone,
    )
    .await
}

pub async fn pack_address(Query(payload): Query<AddressRequest>) -> Response {
    handle_result(
        async move {
            let (addr, flags) = parse_std_addr(&payload.address)?;
            Ok(v2::map_pack_address(&addr, flags.testnet))
        },
        Clone::clone,
    )
    .await
}

pub async fn unpack_address(Query(payload): Query<AddressRequest>) -> Response {
    handle_result(
        async move {
            let (addr, _) = parse_std_addr(&payload.address)?;
            Ok(v2::map_unpack_address(&addr))
        },
        Clone::clone,
    )
    .await
}

pub async fn get_block_header(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<BlockQueryAdapter>,
) -> Response {
    handle_result(
        node.get_block_header(payload.seqno as u32),
        v2::map_block_header,
    )
    .await
}

pub async fn get_block_transactions_ext_post(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<BlockQueryAdapter>,
) -> Response {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions_ext,
    )
    .await
}

pub async fn send_boc_return_hash(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<SendBocRequest>,
) -> Response {
    handle_result(node.send_boc(payload.boc), v2::map_send_boc_return_hash).await
}

pub async fn get_block_transactions(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<BlockQueryAdapter>,
) -> Response {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_ext(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<BlockQueryAdapter>,
) -> Response {
    handle_result(
        node.get_block_transactions(payload.seqno as u32),
        v2::map_block_transactions_ext,
    )
    .await
}

pub async fn get_masterchain_info(State(node): State<Arc<Localnet>>) -> Response {
    handle_result(node.get_masterchain_info(), v2::map_masterchain_info).await
}

pub async fn get_consensus_block(State(node): State<Arc<Localnet>>) -> Response {
    handle_result(node.get_consensus_block(), v2::map_consensus_block).await
}

pub async fn get_out_msg_queue_size(State(node): State<Arc<Localnet>>) -> Response {
    handle_result(node.get_masterchain_info(), v2::map_out_msg_queue_sizes).await
}

pub async fn get_shards(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<BlockQueryAdapter>,
) -> Response {
    handle_result(node.get_shards(payload.seqno as u32), |shards| {
        v2::map_shards(shards)
    })
    .await
}

pub async fn lookup_block(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<LookupBlockRequest>,
) -> Response {
    let workchain = parse!(payload.workchain.to_i32());
    let shard = parse!(payload.shard.to_i64());
    let seqno = parse!(parse_seqno(payload.seqno));
    let lt = parse!(payload.lt.map(|value| value.to_u64()).transpose());
    let unixtime = parse!(payload.unixtime.map(|value| value.to_u32()).transpose());
    handle_result(
        node.lookup_block(workchain, shard.to_string(), seqno, lt, unixtime),
        v2::map_lookup_block,
    )
    .await
}

fn v2_bad_request(error: impl std::fmt::Display) -> Response {
    Json(ton_api::toncenter::v2::TonlibErrorResponse {
        ok: false,
        error: error.to_string(),
        code: 400,
        extra: Some(get_extra()),
        jsonrpc: None,
        id: None,
    })
    .into_response()
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

pub(super) fn parse_config_param(payload: &ConfigParamRequest) -> anyhow::Result<u32> {
    let raw = payload
        .param
        .as_ref()
        .or(payload.config_id.as_ref())
        .ok_or_else(|| anyhow::anyhow!("`param` is required"))?;
    raw.to_u32()
        .map_err(|_| anyhow::anyhow!("Config param must be a non-negative 32-bit integer"))
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

pub(super) fn parse_seqno(seqno: Option<StringOrNumber>) -> anyhow::Result<Option<u32>> {
    seqno
        .map(|value| value.to_u32())
        .transpose()
        .map_err(|_| anyhow::anyhow!("`seqno` must be a non-negative 32-bit integer"))
}

pub(super) fn parse_transactions_request(
    payload: &TransactionsRequest,
) -> anyhow::Result<(usize, Option<u64>, Option<u64>)> {
    let limit = payload
        .limit
        .as_ref()
        .map(StringOrNumber::to_usize)
        .transpose()?
        .unwrap_or(10);
    if !(1..=1000).contains(&limit) {
        anyhow::bail!("`limit` must be between 1 and 1000");
    }
    let lt = payload
        .lt
        .as_ref()
        .map(StringOrNumber::to_u64)
        .transpose()?;
    let to_lt = payload
        .to_lt
        .as_ref()
        .map(StringOrNumber::to_u64)
        .transpose()?;
    let has_lt = lt.is_some_and(|value| value != 0);
    let has_hash = payload.hash.as_ref().is_some_and(|hash| !hash.is_empty());
    if has_lt != has_hash {
        anyhow::bail!("`lt` and `hash` must be used together");
    }
    Ok((limit, lt.filter(|value| *value != 0), to_lt))
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
