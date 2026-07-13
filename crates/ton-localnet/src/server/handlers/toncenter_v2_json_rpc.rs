use super::toncenter_v2::{
    parse_config_param, parse_i32_seqno, parse_libraries_request, parse_seqno,
    parse_transactions_request, resolve_block_header, resolve_block_transactions,
    token_wallet_code_hash,
};
use super::utils::{ToncenterHttpError, error_status, get_extra, parse_method_name, parse_params};
use crate::api::toncenter_v2 as v2;
use crate::api::toncenter_v2::map_detect_address;
use crate::localnet::Localnet;
use crate::server::{ApiCallAlreadyRecorded, ApiCallFamily, ApiCallInput, ApiCallLog, ApiCallType};
use crate::types::{Addr, Hash256};
use axum::extract::OriginalUri;
use axum::response::{IntoResponse, Response};
use axum::{Json, extract::State, http::StatusCode};
use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use ton_api::toncenter::v2 as wire;
use ton_api::toncenter::v2::requests::{
    AddressInformationRequest, AddressRequest, BlockHeaderRequest, BlockTransactionsRequest,
    ConfigAllRequest, ConfigParamRequest, DetectHashRequest, JsonRpcIncomingRequest,
    LibrariesRequest, LookupBlockRequest, RunGetMethodRequest, RunGetMethodStdRequest,
    SendBocRequest, SeqnoRequest, TransactionsRequest, TryLocateTxRequest,
};
use tycho_types::models::{StdAddr, StdAddrFormat};

pub async fn json_rpc(
    State(node): State<Arc<Localnet>>,
    State(api_calls): State<ApiCallLog>,
    OriginalUri(original_uri): OriginalUri,
    Json(payload): Json<JsonRpcIncomingRequest<Value>>,
) -> impl IntoResponse {
    tracing::debug!(
        "JSON-RPC request: method={}, id={:?}",
        payload.method,
        payload.id
    );

    let start = ApiCallLog::start();
    let method = payload.method.clone();
    let call_type = classify_json_rpc_call(&method);
    let request_id = payload.id.clone().unwrap_or(Value::Null);

    let result: anyhow::Result<Response> = json_rpc_router(node, payload).await;
    let mut response = result.unwrap_or_else(|e| json_rpc_error(error_status(&e), e.to_string()));

    api_calls.record(
        ApiCallInput {
            call_type,
            api_family: ApiCallFamily::JsonRpc,
            http_method: "POST".to_owned(),
            path: original_uri.path().to_owned(),
            method,
            request_id,
            status_code: response.status().as_u16(),
        },
        start,
    );
    response.extensions_mut().insert(ApiCallAlreadyRecorded);

    response
}

fn classify_json_rpc_call(method: &str) -> ApiCallType {
    match method {
        "sendBoc" | "sendBocReturnHash" => ApiCallType::Write,
        _ => ApiCallType::Read,
    }
}

fn normalize_json_rpc_params(params: Option<Value>) -> anyhow::Result<Value> {
    match params {
        Some(params @ Value::Object(_)) => Ok(params),
        Some(Value::Array(values)) if !values.is_empty() => Err(
            ToncenterHttpError::unprocessable_entity("params must contain an object"),
        ),
        _ => Ok(Value::Object(Default::default())),
    }
}

async fn json_rpc_router(
    node: Arc<Localnet>,
    payload: JsonRpcIncomingRequest<Value>,
) -> anyhow::Result<Response> {
    let params = normalize_json_rpc_params(payload.params)?;
    let method = payload.method.as_str();

    let result: wire::JsonRpcResult = match method {
        "sendBoc" => {
            let req: SendBocRequest = parse_params(params, method)?;
            wire::JsonRpcResult::Ok(Box::new(
                node.send_boc(req.boc).await.map(|r| v2::map_send_boc(&r))?,
            ))
        }
        "sendBocReturnHash" => {
            let req: SendBocRequest = parse_params(params, method)?;
            wire::JsonRpcResult::ExternalMessage(Box::new(
                node.send_boc(req.boc)
                    .await
                    .map(|r| v2::map_send_boc_return_hash(&r))?,
            ))
        }
        "runGetMethod" => {
            let req: RunGetMethodRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method)?;
            let seqno = parse_seqno(req.seqno)?;
            let result = node
                .run_get_method(req.address, method_str, req.stack, seqno)
                .await?;
            wire::JsonRpcResult::RunGetMethod(Box::new(v2::map_run_get_method(&result)?))
        }
        "runGetMethodStd" => {
            let req: RunGetMethodStdRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method)?;
            let seqno = parse_i32_seqno(req.seqno)?;
            let result = node
                .run_get_method_std(req.address, method_str, req.stack, seqno)
                .await?;
            wire::JsonRpcResult::RunGetMethodStd(Box::new(v2::map_run_get_method_std(&result)?))
        }
        "detectAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = parse_std_addr(&req.address)?;
            let given_type = detect_given_type(&req.address, flags.bounceable);
            wire::JsonRpcResult::DetectAddress(Box::new(map_detect_address(
                &addr, flags, given_type,
            )))
        }
        "detectHash" => {
            let req: DetectHashRequest = parse_params(params, method)?;
            let hash = parse_hash_any(&req.hash)?;
            wire::JsonRpcResult::DetectHash(Box::new(v2::map_detect_hash(&hash)))
        }
        "packAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = parse_std_addr(&req.address)?;
            wire::JsonRpcResult::String(v2::map_pack_address(&addr, flags.testnet))
        }
        "unpackAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, _) = parse_std_addr(&req.address)?;
            wire::JsonRpcResult::String(v2::map_unpack_address(&addr))
        }
        "getAddressInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::AddressInformation(Box::new(
                node.get_address_information(req.address, seqno)
                    .await
                    .map(|r| v2::map_account_state(&r))?,
            ))
        }
        "getShardAccountCell" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::ShardAccountCell(Box::new(
                node.get_shard_account_cell(req.address, seqno)
                    .await
                    .map(|r| v2::map_shard_account_cell(&r))?,
            ))
        }
        "getAddressBalance" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::String(
                node.get_address_balance(req.address, seqno)
                    .await?
                    .to_string(),
            )
        }
        "getAddressState" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::String(
                node.get_address_state(req.address, seqno)
                    .await?
                    .to_string(),
            )
        }
        "getLibraries" => {
            let req: LibrariesRequest = parse_params(params, method)?;
            let hashes = parse_libraries_request(&req.libraries)?;
            wire::JsonRpcResult::Libraries(Box::new(
                node.get_libraries(hashes)
                    .await
                    .map(|r| v2::map_libraries(&r))?,
            ))
        }
        "getExtendedAddressInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::ExtendedAddressInformation(Box::new(
                node.get_address_information(req.address, seqno)
                    .await
                    .map(|r| v2::map_extended_account_state(&r))?,
            ))
        }
        "getWalletInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let request_seqno = parse_seqno(req.seqno)?;
            let info = node
                .get_address_information(req.address.clone(), request_seqno)
                .await?;
            let seqno = if v2::wallet_type_name_from_code_hash(info.code_hash.as_ref()).is_some() {
                node.run_get_method(req.address, "seqno".to_string(), Vec::new(), request_seqno)
                    .await
                    .ok()
                    .and_then(|result| v2::map_wallet_seqno(&result))
            } else {
                None
            };
            wire::JsonRpcResult::WalletInformation(Box::new(v2::map_wallet_information(
                &info, seqno,
            )))
        }
        "getTokenData" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let address = Addr::parse(&req.address)?;
            let mut infos = node.get_address_infos(vec![address]).await?;
            let info = infos
                .pop()
                .ok_or_else(|| anyhow::anyhow!("Address information not found"))?;
            let jetton_wallet_code_hash = token_wallet_code_hash(node.as_ref(), &info).await;
            let jetton_wallet_code = match jetton_wallet_code_hash {
                Some(hash) => node.get_cell_boc(hash).await?,
                None => None,
            };

            wire::JsonRpcResult::TokenData(Box::new(
                v2::map_token_data(&info, jetton_wallet_code.as_ref(), None).ok_or_else(|| {
                    anyhow::anyhow!("Smart contract {} is not Jetton or NFT", req.address)
                })?,
            ))
        }
        "getTransactions" => {
            let req: TransactionsRequest = parse_params(params, method)?;
            let (limit, lt, to_lt) = parse_transactions_request(&req)?;
            wire::JsonRpcResult::Transactions(
                node.get_transactions(req.address, limit, lt, req.hash, to_lt)
                    .await
                    .map(|r| v2::map_transactions(&r))?,
            )
        }
        "getTransactionsStd" => {
            let req: TransactionsRequest = parse_params(params, method)?;
            let (page_limit, lt, to_lt) = parse_transactions_request(&req)?;
            let fetch_limit = page_limit.saturating_add(1);
            wire::JsonRpcResult::RawTransactions(Box::new(
                node.get_transactions(req.address, fetch_limit, lt, req.hash, to_lt)
                    .await
                    .map(|r| v2::map_transactions_std(&r, page_limit))?,
            ))
        }
        "getConfigParam" => {
            let req: ConfigParamRequest = parse_params(params, method)?;
            let param = parse_config_param(&req)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::ConfigInfo(Box::new(
                node.get_config_param(param, seqno)
                    .await
                    .map(|r| v2::map_config_info(&r))?,
            ))
        }
        "getConfigAll" => {
            let req: ConfigAllRequest = parse_params(params, method)?;
            let seqno = parse_seqno(req.seqno)?;
            wire::JsonRpcResult::ConfigInfo(Box::new(
                node.get_config_all(seqno)
                    .await
                    .map(|r| v2::map_config_info(&r))?,
            ))
        }
        "tryLocateTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let created_lt = req.created_lt.to_u64()?;
            wire::JsonRpcResult::Transaction(Box::new(
                node.try_locate_tx(req.source, req.destination, created_lt)
                    .await
                    .map(|r| v2::map_transaction(&r))?,
            ))
        }
        "tryLocateResultTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let created_lt = req.created_lt.to_u64()?;
            wire::JsonRpcResult::Transaction(Box::new(
                node.try_locate_result_tx(req.source, req.destination, created_lt)
                    .await
                    .map(|r| v2::map_transaction(&r))?,
            ))
        }
        "tryLocateSourceTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let created_lt = req.created_lt.to_u64()?;
            wire::JsonRpcResult::Transaction(Box::new(
                node.try_locate_source_tx(req.source, req.destination, created_lt)
                    .await
                    .map(|r| v2::map_transaction(&r))?,
            ))
        }
        "getBlockHeader" => {
            let req: BlockHeaderRequest = parse_params(params, method)?;
            wire::JsonRpcResult::BlockHeader(Box::new(
                resolve_block_header(&node, &req)
                    .await
                    .map(|r| v2::map_block_header(&r))?,
            ))
        }
        "getBlockTransactions" => {
            let req: BlockTransactionsRequest = parse_params(params, method)?;
            wire::JsonRpcResult::BlockTransactions(Box::new(
                resolve_block_transactions(&node, &req)
                    .await
                    .map(|r| v2::map_block_transactions(&r))?,
            ))
        }
        "getBlockTransactionsExt" => {
            let req: BlockTransactionsRequest = parse_params(params, method)?;
            wire::JsonRpcResult::BlockTransactionsExt(Box::new(
                resolve_block_transactions(&node, &req)
                    .await
                    .map(|r| v2::map_block_transactions_ext(&r))?,
            ))
        }
        "getMasterchainInfo" => wire::JsonRpcResult::MasterchainInfo(Box::new(
            node.get_masterchain_info()
                .await
                .map(|r| v2::map_masterchain_info(&r))?,
        )),
        "getConsensusBlock" => wire::JsonRpcResult::ConsensusBlock(Box::new(
            node.get_consensus_block()
                .await
                .map(|r| v2::map_consensus_block(&r))?,
        )),
        "getOutMsgQueueSize" => wire::JsonRpcResult::OutMsgQueueSizes(Box::new(
            node.get_masterchain_info()
                .await
                .map(|r| v2::map_out_msg_queue_sizes(&r))?,
        )),
        "getShards" => {
            let req: SeqnoRequest = parse_params(params, method)?;
            let seqno = req.seqno.to_u32()?;
            anyhow::ensure!(seqno > 0, "`seqno` must be positive");
            wire::JsonRpcResult::Shards(Box::new(
                node.get_shards(seqno).await.map(|r| v2::map_shards(&r))?,
            ))
        }
        "lookupBlock" => {
            let req: LookupBlockRequest = parse_params(params, method)?;
            let workchain = req.workchain.to_i32()?;
            let shard = req.shard.to_i64()?.to_string();
            let seqno = parse_seqno(req.seqno)?;
            let lt = req.lt.map(|value| value.to_u64()).transpose()?;
            let unixtime = req.unixtime.map(|value| value.to_u32()).transpose()?;
            wire::JsonRpcResult::BlockId(Box::new(
                node.lookup_block(workchain, shard, seqno, lt, unixtime)
                    .await
                    .map(|r| v2::map_lookup_block(&r))?,
            ))
        }
        _ => {
            return Ok(json_rpc_error(StatusCode::NOT_FOUND, "Method not found"));
        }
    };

    Ok(json_rpc_success(result))
}

fn json_rpc_success<T: Serialize>(result: T) -> Response {
    (
        StatusCode::OK,
        Json(wire::JsonRpcResponse {
            jsonrpc: None,
            id: None,
            response: wire::TonlibResponse {
                ok: true,
                result,
                extra: get_extra(),
            },
        }),
    )
        .into_response()
}

fn json_rpc_error(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(wire::TonlibErrorResponse {
            ok: false,
            error: error.into(),
            code: i32::from(status.as_u16()),
            extra: get_extra(),
            jsonrpc: None,
            id: None,
        }),
    )
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
