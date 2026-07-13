use super::toncenter_v2::{
    parse_block_header_request, parse_block_transactions_request, parse_config_param,
    parse_i32_seqno, parse_libraries_request, parse_lookup_block_request, parse_required_seqno,
    parse_seqno, parse_transactions_request, parse_transactions_std_request,
    parse_try_locate_tx_request, resolve_block_header, resolve_block_transactions,
    resolve_token_data, resolve_wallet_information,
};
use super::utils::{ToncenterHttpError, error_status, get_extra, parse_method_name, parse_params};
use crate::api::toncenter_v2 as v2;
use crate::api::toncenter_v2::map_detect_address;
use crate::localnet::{Localnet, TransactionLookupKind};
use crate::server::{ApiCallAlreadyRecorded, ApiCallFamily, ApiCallInput, ApiCallLog, ApiCallType};
use crate::types::Hash256;
use axum::extract::OriginalUri;
use axum::response::{IntoResponse, Response};
use axum::{Json, extract::State, http::StatusCode};
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

macro_rules! validate {
    ($expression:expr) => {
        $expression.map_err(|error| ToncenterHttpError::unprocessable_entity(error.to_string()))?
    };
}

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
            let method_str = validate!(parse_method_name(&req.method));
            let seqno = validate!(parse_seqno(req.seqno));
            let result = node
                .run_get_method(req.address, method_str, req.stack, seqno)
                .await?;
            wire::JsonRpcResult::RunGetMethod(Box::new(v2::map_run_get_method(&result)?))
        }
        "runGetMethodStd" => {
            let req: RunGetMethodStdRequest = parse_params(params, method)?;
            let method_str = validate!(parse_method_name(&req.method));
            let seqno = validate!(parse_i32_seqno(req.seqno));
            let result = node
                .run_get_method_std(req.address, method_str, req.stack, seqno)
                .await?;
            wire::JsonRpcResult::RunGetMethodStd(Box::new(v2::map_run_get_method_std(&result)?))
        }
        "detectAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = validate!(parse_std_addr(&req.address));
            let given_type = detect_given_type(&req.address, flags.bounceable);
            wire::JsonRpcResult::DetectAddress(Box::new(map_detect_address(
                &addr, flags, given_type,
            )))
        }
        "detectHash" => {
            let req: DetectHashRequest = parse_params(params, method)?;
            let hash = validate!(parse_hash_any(&req.hash));
            wire::JsonRpcResult::DetectHash(Box::new(v2::map_detect_hash(&hash)))
        }
        "packAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = validate!(parse_std_addr(&req.address));
            wire::JsonRpcResult::String(v2::map_pack_address(&addr, flags.testnet))
        }
        "unpackAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, _) = validate!(parse_std_addr(&req.address));
            wire::JsonRpcResult::String(v2::map_unpack_address(&addr))
        }
        "getAddressInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::JsonRpcResult::AddressInformation(Box::new(
                node.get_address_information(req.address, seqno)
                    .await
                    .map(|r| v2::map_account_state(&r))?,
            ))
        }
        "getShardAccountCell" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::JsonRpcResult::ShardAccountCell(Box::new(
                node.get_shard_account_cell(req.address, seqno)
                    .await
                    .map(|r| v2::map_shard_account_cell(&r))?,
            ))
        }
        "getAddressBalance" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::JsonRpcResult::String(
                node.get_address_balance(req.address, seqno)
                    .await?
                    .to_string(),
            )
        }
        "getAddressState" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            let status = node.get_address_state(req.address, seqno).await?;
            wire::JsonRpcResult::String(v2::map_account_status(&status).to_owned())
        }
        "getLibraries" => {
            let req: LibrariesRequest = parse_params(params, method)?;
            let hashes = validate!(parse_libraries_request(&req.libraries));
            wire::JsonRpcResult::Libraries(Box::new(
                node.get_libraries(hashes)
                    .await
                    .map(|r| v2::map_libraries(&r))?,
            ))
        }
        "getExtendedAddressInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::JsonRpcResult::ExtendedAddressInformation(Box::new(
                node.get_address_information(req.address, seqno)
                    .await
                    .map(|r| v2::map_extended_account_state(&r))?,
            ))
        }
        "getWalletInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            wire::JsonRpcResult::WalletInformation(Box::new(
                resolve_wallet_information(node.as_ref(), &req).await?,
            ))
        }
        "getTokenData" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            wire::JsonRpcResult::TokenData(Box::new(resolve_token_data(node.as_ref(), &req).await?))
        }
        "getTransactions" => {
            let req: TransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_transactions_request(&req));
            let page = node
                .get_transactions_page_by_address(
                    request.address,
                    request.limit,
                    request.lt,
                    request.hash,
                    request.to_lt,
                )
                .await?;
            wire::JsonRpcResult::Transactions(v2::map_transactions(&page.transactions))
        }
        "getTransactionsStd" => {
            let req: TransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_transactions_std_request(&req));
            wire::JsonRpcResult::RawTransactions(Box::new(
                node.get_transactions_page_by_address(
                    request.address,
                    request.limit,
                    request.lt,
                    request.hash,
                    request.to_lt,
                )
                .await
                .map(|page| v2::map_transactions_std(&page))?,
            ))
        }
        "getConfigParam" => {
            let req: ConfigParamRequest = parse_params(params, method)?;
            let param = validate!(parse_config_param(&req));
            let seqno = validate!(parse_seqno(req.seqno));
            wire::JsonRpcResult::ConfigInfo(Box::new(
                node.get_config_param(param, seqno)
                    .await
                    .map(|r| v2::map_config_info(&r))?,
            ))
        }
        "getConfigAll" => {
            let req: ConfigAllRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::JsonRpcResult::ConfigInfo(Box::new(
                node.get_config_all(seqno)
                    .await
                    .map(|r| v2::map_config_info(&r))?,
            ))
        }
        "tryLocateTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let request = validate!(parse_try_locate_tx_request(&req));
            wire::JsonRpcResult::Transaction(Box::new(
                node.locate_transaction(
                    request.source,
                    request.destination,
                    request.created_lt,
                    TransactionLookupKind::Result,
                )
                .await
                .map(|r| v2::map_transaction(&r))?,
            ))
        }
        "tryLocateResultTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let request = validate!(parse_try_locate_tx_request(&req));
            wire::JsonRpcResult::Transaction(Box::new(
                node.locate_transaction(
                    request.source,
                    request.destination,
                    request.created_lt,
                    TransactionLookupKind::Result,
                )
                .await
                .map(|r| v2::map_transaction(&r))?,
            ))
        }
        "tryLocateSourceTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let request = validate!(parse_try_locate_tx_request(&req));
            wire::JsonRpcResult::Transaction(Box::new(
                node.locate_transaction(
                    request.source,
                    request.destination,
                    request.created_lt,
                    TransactionLookupKind::Source,
                )
                .await
                .map(|r| v2::map_transaction(&r))?,
            ))
        }
        "getBlockHeader" => {
            let req: BlockHeaderRequest = parse_params(params, method)?;
            let request = validate!(parse_block_header_request(&req));
            wire::JsonRpcResult::BlockHeader(Box::new(
                resolve_block_header(&node, &request)
                    .await
                    .map(|r| v2::map_block_header(&r))?,
            ))
        }
        "getBlockTransactions" => {
            let req: BlockTransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_block_transactions_request(&req));
            wire::JsonRpcResult::BlockTransactions(Box::new(
                resolve_block_transactions(&node, &request)
                    .await
                    .map(|r| v2::map_block_transactions(&r))?,
            ))
        }
        "getBlockTransactionsExt" => {
            let req: BlockTransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_block_transactions_request(&req));
            wire::JsonRpcResult::BlockTransactionsExt(Box::new(
                resolve_block_transactions(&node, &request)
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
            let seqno = validate!(parse_required_seqno(&req.seqno));
            wire::JsonRpcResult::Shards(Box::new(
                node.get_shards(seqno).await.map(|r| v2::map_shards(&r))?,
            ))
        }
        "lookupBlock" => {
            let req: LookupBlockRequest = parse_params(params, method)?;
            let request = validate!(parse_lookup_block_request(&req));
            wire::JsonRpcResult::BlockId(Box::new(
                node.lookup_block(
                    request.workchain,
                    request.shard,
                    request.seqno,
                    request.lt,
                    request.unixtime,
                )
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
    hash.parse()
}
