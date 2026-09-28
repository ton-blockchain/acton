use super::toncenter_v2::{
    parse_block_data_request, parse_block_header_request, parse_block_transactions_request,
    parse_config_param, parse_get_method_seqno, parse_libraries_request,
    parse_lookup_block_request, parse_required_seqno, parse_seqno, parse_transactions_request,
    parse_transactions_std_request, parse_try_locate_tx_request, resolve_block_data,
    resolve_block_header, resolve_block_transactions, resolve_block_transactions_ext,
    resolve_extended_address_information, resolve_lookup_block, resolve_shards, resolve_token_data,
    resolve_wallet_information,
};
use super::utils::{ToncenterHttpError, error_status, get_extra, parse_method_name, parse_params};
use crate::api::toncenter_v2 as v2;
use crate::api::toncenter_v2::map_detect_address;
use crate::localnet::{Localnet, TransactionLookupKind};
use crate::types::Hash256;
use axum::response::{IntoResponse, Response};
use axum::{Json, extract::State, http::StatusCode};
use rston::models::{StdAddr, StdAddrFormat};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use toncenter::v2 as wire;
use toncenter::v2::requests::{
    AddressInformationRequest, AddressRequest, BlockDataRequest, BlockHeaderRequest,
    BlockTransactionsRequest, ConfigAllRequest, ConfigParamRequest, DetectHashRequest,
    LibrariesRequest, LookupBlockRequest, RunGetMethodRequest, RunGetMethodStdRequest,
    SendBocRequest, SeqnoRequest, TransactionsRequest, TryLocateTxRequest,
};

macro_rules! validate {
    ($expression:expr) => {
        $expression.map_err(|error| ToncenterHttpError::unprocessable_entity(error.to_string()))?
    };
}

pub async fn json_rpc(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<Value>,
) -> impl IntoResponse {
    tracing::debug!(
        "JSON-RPC request: method={:?}, id={:?}",
        payload.get("method"),
        payload.get("id")
    );

    let result: anyhow::Result<Response> = json_rpc_router(node, payload).await;
    result.unwrap_or_else(|e| json_rpc_error(error_status(&e), e.to_string()))
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

async fn json_rpc_router(node: Arc<Localnet>, payload: Value) -> anyhow::Result<Response> {
    let params = normalize_json_rpc_params(payload.get("params").cloned())?;
    let method = payload
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| ToncenterHttpError::unprocessable_entity("method must be a string"))?;

    let result: wire::responses::RpcResult = match method {
        "sendBoc" => {
            let req: SendBocRequest = parse_params(params, method)?;
            wire::responses::RpcResult::ResultOk(Box::new(
                node.send_boc(req.boc).await.map(|r| v2::map_send_boc(&r))?,
            ))
        }
        "sendBocReturnHash" => {
            let req: SendBocRequest = parse_params(params, method)?;
            wire::responses::RpcResult::ExtMessageInfo(Box::new(
                node.send_boc(req.boc)
                    .await
                    .map(|r| v2::map_send_boc_return_hash(&r))?,
            ))
        }
        "runGetMethod" => {
            let req: RunGetMethodRequest<Value> = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method);
            let seqno = validate!(parse_get_method_seqno(req.seqno.map(i64::from)));
            let result = node
                .run_get_method(req.address, method_str, req.stack, seqno)
                .await?;
            return Ok(json_rpc_success(v2::map_run_get_method(&result)?));
        }
        "runGetMethodStd" => {
            let req: RunGetMethodStdRequest = parse_params(params, method)?;
            let method_str = parse_method_name(&req.method);
            let seqno = validate!(parse_get_method_seqno(req.seqno));
            let result = node
                .run_get_method_std(req.address, method_str, req.stack, seqno)
                .await?;
            wire::responses::RpcResult::RunGetMethodStdResult(Box::new(v2::map_run_get_method_std(
                &result,
            )?))
        }
        "detectAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = validate!(parse_std_addr(&req.address));
            let given_type = detect_given_type(&req.address, flags.bounceable);
            wire::responses::RpcResult::DetectAddress(Box::new(map_detect_address(
                &addr, flags, given_type,
            )))
        }
        "detectHash" => {
            let req: DetectHashRequest = parse_params(params, method)?;
            let hash = validate!(parse_hash_any(&req.hash));
            wire::responses::RpcResult::DetectHash(Box::new(v2::map_detect_hash(&hash)))
        }
        "packAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, flags) = validate!(parse_std_addr(&req.address));
            wire::responses::RpcResult::String(v2::map_pack_address(&addr, flags.testnet))
        }
        "unpackAddress" => {
            let req: AddressRequest = parse_params(params, method)?;
            let (addr, _) = validate!(parse_std_addr(&req.address));
            wire::responses::RpcResult::String(v2::map_unpack_address(&addr))
        }
        "getAddressInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::responses::RpcResult::AddressInformation(Box::new(
                node.get_address_information(req.address, seqno)
                    .await
                    .map(|r| v2::map_account_state(&r))?,
            ))
        }
        "getShardAccountCell" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::responses::RpcResult::TvmCell(Box::new(
                node.get_shard_account_cell(req.address, seqno)
                    .await
                    .map(|r| v2::map_shard_account_cell(&r))?,
            ))
        }
        "getAddressBalance" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::responses::RpcResult::String(
                node.get_address_balance(req.address, seqno)
                    .await?
                    .to_string(),
            )
        }
        "getAddressState" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            let status = node.get_address_state(req.address, seqno).await?;
            wire::responses::RpcResult::String(v2::map_account_status(&status).to_string())
        }
        "getLibraries" => {
            let req: LibrariesRequest = parse_params(params, method)?;
            let hashes = validate!(parse_libraries_request(
                req.libraries.as_deref().unwrap_or_default()
            ));
            wire::responses::RpcResult::LibraryResult(Box::new(
                node.get_libraries(hashes)
                    .await
                    .map(|r| v2::map_libraries(&r))?,
            ))
        }
        "getExtendedAddressInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            wire::responses::RpcResult::ExtendedAddressInformation(Box::new(
                resolve_extended_address_information(node.as_ref(), &req).await?,
            ))
        }
        "getWalletInformation" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            wire::responses::RpcResult::WalletInformation(Box::new(
                resolve_wallet_information(node.as_ref(), &req).await?,
            ))
        }
        "getTokenData" => {
            let req: AddressInformationRequest = parse_params(params, method)?;
            wire::responses::RpcResult::TokenData(Box::new(
                resolve_token_data(node.as_ref(), &req).await?,
            ))
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
            wire::responses::RpcResult::Transactions(v2::map_transactions(&page.transactions))
        }
        "getTransactionsStd" => {
            let req: TransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_transactions_std_request(&req));
            wire::responses::RpcResult::TransactionsStd(Box::new(
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
            wire::responses::RpcResult::ConfigInfo(Box::new(
                node.get_config_param(param, seqno)
                    .await
                    .map(|r| v2::map_config_info(&r))?,
            ))
        }
        "getConfigAll" => {
            let req: ConfigAllRequest = parse_params(params, method)?;
            let seqno = validate!(parse_seqno(req.seqno));
            wire::responses::RpcResult::ConfigInfo(Box::new(
                node.get_config_all(seqno)
                    .await
                    .map(|r| v2::map_config_info(&r))?,
            ))
        }
        "tryLocateTx" => {
            let req: TryLocateTxRequest = parse_params(params, method)?;
            let request = validate!(parse_try_locate_tx_request(&req));
            wire::responses::RpcResult::Transaction(Box::new(
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
            wire::responses::RpcResult::Transaction(Box::new(
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
            wire::responses::RpcResult::Transaction(Box::new(
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
            wire::responses::RpcResult::BlockHeader(Box::new(
                resolve_block_header(&node, &request).await?,
            ))
        }
        "getBlock" => {
            let req: BlockDataRequest = parse_params(params, method)?;
            let request = validate!(parse_block_data_request(&req));
            wire::responses::RpcResult::BlockData(Box::new(
                resolve_block_data(&node, &request).await?,
            ))
        }
        "getBlockTransactions" => {
            let req: BlockTransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_block_transactions_request(&req));
            wire::responses::RpcResult::BlockTransactions(Box::new(
                resolve_block_transactions(&node, &request).await?,
            ))
        }
        "getBlockTransactionsExt" => {
            let req: BlockTransactionsRequest = parse_params(params, method)?;
            let request = validate!(parse_block_transactions_request(&req));
            wire::responses::RpcResult::BlockTransactionsExt(Box::new(
                resolve_block_transactions_ext(&node, &request).await?,
            ))
        }
        "getMasterchainInfo" => wire::responses::RpcResult::MasterchainInfo(Box::new(
            node.get_masterchain_info()
                .await
                .map(|r| v2::map_masterchain_info(&r))?,
        )),
        "getConsensusBlock" => wire::responses::RpcResult::ConsensusBlock(Box::new(
            node.get_consensus_block()
                .await
                .map(|r| v2::map_consensus_block(&r))?,
        )),
        "getOutMsgQueueSize" => wire::responses::RpcResult::OutMsgQueueSizes(Box::new(
            node.get_masterchain_info()
                .await
                .map(|r| v2::map_out_msg_queue_sizes(&r))?,
        )),
        "getShards" => {
            let req: SeqnoRequest = parse_params(params, method)?;
            let seqno = validate!(parse_required_seqno(&req.seqno));
            wire::responses::RpcResult::Shards(Box::new(resolve_shards(&node, seqno).await?))
        }
        "lookupBlock" => {
            let req: LookupBlockRequest = parse_params(params, method)?;
            let request = validate!(parse_lookup_block_request(&req));
            wire::responses::RpcResult::TonBlockIdExt(Box::new(
                resolve_lookup_block(&node, &request).await?,
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
        Json(wire::TonlibResponse {
            jsonrpc: None,
            id: None,
            ok: true,
            result,
            extra: get_extra(),
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
            extra: Some(get_extra()),
            jsonrpc: None,
            id: None,
        }),
    )
        .into_response()
}

fn parse_std_addr(address: &str) -> anyhow::Result<(StdAddr, rston::models::Base64StdAddrFlags)> {
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
