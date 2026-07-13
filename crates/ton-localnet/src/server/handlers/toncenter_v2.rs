use super::utils::{ToncenterHttpError, get_extra, handle_result, parse_method_name};
use crate::api::toncenter_v2 as v2;
use crate::localnet::{
    Localnet, LocalnetBlockHeader, LocalnetBlockTransactions, TransactionLookupKind,
};
use crate::types::{Addr, Hash256};
use axum::{
    Json,
    extract::{Query, RawQuery, State},
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use ton_api::toncenter::v2::StringOrNumber;
use ton_api::toncenter::v2::requests::{
    AddressInformationRequest, AddressRequest, ConfigAllRequest, ConfigParamRequest,
    DetectHashRequest, LibrariesRequest, LookupBlockRequest, RunGetMethodRequest,
    RunGetMethodStdRequest, SendBocRequest, SeqnoRequest, TransactionsRequest, TryLocateTxRequest,
};
use ton_api::toncenter::v2::requests::{BlockHeaderRequest, BlockTransactionsRequest};
use tycho_types::models::{StdAddr, StdAddrFormat};

macro_rules! parse {
    ($expression:expr) => {
        match $expression {
            Ok(value) => value,
            Err(error) => return v2_unprocessable_entity(error),
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
        async move {
            let result = node
                .run_get_method(payload.address, method_str, payload.stack, seqno)
                .await?;
            v2::map_run_get_method(&result)
        },
        Clone::clone,
    )
    .await
}

pub async fn run_get_method_std(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<RunGetMethodStdRequest>,
) -> Response {
    let method_str = parse!(parse_method_name(&payload.method));
    let seqno = parse!(parse_i32_seqno(payload.seqno));

    handle_result(
        async move {
            let result = node
                .run_get_method_std(payload.address, method_str, payload.stack, seqno)
                .await?;
            v2::map_run_get_method_std(&result)
        },
        Clone::clone,
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
    handle_result(node.get_address_state(payload.address, seqno), |status| {
        v2::map_account_status(status).to_owned()
    })
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
    handle_result(resolve_wallet_information(&node, &payload), Clone::clone).await
}

pub(super) async fn resolve_wallet_information(
    node: &Localnet,
    payload: &AddressInformationRequest,
) -> anyhow::Result<ton_api::toncenter::v2::responses::WalletInformation> {
    let request_seqno = parse_seqno(payload.seqno.clone())?;
    let info = node
        .get_address_information(payload.address.clone(), request_seqno)
        .await?;
    let wallet_seqno = if v2::wallet_type_name_from_code_hash(info.code_hash.as_ref()).is_some() {
        node.run_get_method(
            payload.address.clone(),
            "seqno".to_owned(),
            Vec::new(),
            request_seqno,
        )
        .await
        .ok()
        .and_then(|result| v2::map_wallet_seqno(&result))
    } else {
        None
    };

    Ok(v2::map_wallet_information(&info, wallet_seqno))
}

pub async fn get_token_data(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<AddressInformationRequest>,
) -> Response {
    handle_result(resolve_token_data(&node, &payload), Clone::clone).await
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

pub(super) async fn resolve_token_data(
    node: &Localnet,
    payload: &AddressInformationRequest,
) -> anyhow::Result<ton_api::toncenter::v2::responses::TokenData> {
    let seqno = parse_seqno(payload.seqno.clone())?;
    let address = Addr::parse(&payload.address)
        .map_err(|error| ToncenterHttpError::unprocessable_entity(error.to_string()))?;
    let mut infos = node.get_address_infos(vec![address], seqno).await?;
    let info = infos
        .pop()
        .ok_or_else(|| anyhow::anyhow!("Address information not found"))?;
    let jetton_wallet_code = match info.jetton_wallet_code_hash() {
        Some(hash) => node.get_cell_boc(hash).await?,
        None => None,
    };

    v2::map_token_data(&info, jetton_wallet_code.as_ref()).ok_or_else(|| {
        ToncenterHttpError::conflict(format!(
            "Smart contract {} is not Jetton or NFT",
            payload.address
        ))
    })
}

pub async fn get_libraries(
    State(node): State<Arc<Localnet>>,
    RawQuery(raw_query): RawQuery,
) -> Response {
    let payload = parse!(serde_html_form::from_str::<LibrariesRequest>(
        raw_query.as_deref().unwrap_or_default()
    ));
    let hashes = parse!(parse_libraries_request(&payload.libraries));
    handle_result(node.get_libraries(hashes), |res| v2::map_libraries(res)).await
}

pub async fn get_transactions(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TransactionsRequest>,
) -> Response {
    let request = parse!(parse_transactions_request(&payload));
    handle_result(
        node.get_transactions_page_by_address(
            request.address,
            request.limit,
            request.lt,
            request.hash,
            request.to_lt,
        ),
        |page| v2::map_transactions(&page.transactions),
    )
    .await
}

pub async fn get_transactions_std(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TransactionsRequest>,
) -> Response {
    let request = parse!(parse_transactions_std_request(&payload));
    handle_result(
        node.get_transactions_page_by_address(
            request.address,
            request.limit,
            request.lt,
            request.hash,
            request.to_lt,
        ),
        v2::map_transactions_std,
    )
    .await
}

pub async fn try_locate_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Response {
    let request = parse!(parse_try_locate_tx_request(&payload));
    handle_result(
        node.locate_transaction(
            request.source,
            request.destination,
            request.created_lt,
            TransactionLookupKind::Result,
        ),
        v2::map_transaction,
    )
    .await
}

pub async fn try_locate_result_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Response {
    let request = parse!(parse_try_locate_tx_request(&payload));
    handle_result(
        node.locate_transaction(
            request.source,
            request.destination,
            request.created_lt,
            TransactionLookupKind::Result,
        ),
        v2::map_transaction,
    )
    .await
}

pub async fn try_locate_source_tx(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<TryLocateTxRequest>,
) -> Response {
    let request = parse!(parse_try_locate_tx_request(&payload));
    handle_result(
        node.locate_transaction(
            request.source,
            request.destination,
            request.created_lt,
            TransactionLookupKind::Source,
        ),
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
    Query(payload): Query<BlockHeaderRequest>,
) -> Response {
    let request = parse!(parse_block_header_request(&payload));
    handle_result(resolve_block_header(&node, &request), v2::map_block_header).await
}

pub async fn get_block_transactions_ext_post(
    State(node): State<Arc<Localnet>>,
    Json(payload): Json<BlockTransactionsRequest>,
) -> Response {
    let request = parse!(parse_block_transactions_request(&payload));
    handle_result(
        resolve_block_transactions(&node, &request),
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
    Query(payload): Query<BlockTransactionsRequest>,
) -> Response {
    let request = parse!(parse_block_transactions_request(&payload));
    handle_result(
        resolve_block_transactions(&node, &request),
        v2::map_block_transactions,
    )
    .await
}

pub async fn get_block_transactions_ext(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<BlockTransactionsRequest>,
) -> Response {
    let request = parse!(parse_block_transactions_request(&payload));
    handle_result(
        resolve_block_transactions(&node, &request),
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
    Query(payload): Query<SeqnoRequest>,
) -> Response {
    let seqno = parse!(parse_required_seqno(&payload.seqno));
    handle_result(node.get_shards(seqno), |shards| v2::map_shards(shards)).await
}

pub async fn lookup_block(
    State(node): State<Arc<Localnet>>,
    Query(payload): Query<LookupBlockRequest>,
) -> Response {
    let request = parse!(parse_lookup_block_request(&payload));
    handle_result(
        node.lookup_block(
            request.workchain,
            request.shard,
            request.seqno,
            request.lt,
            request.unixtime,
        ),
        v2::map_lookup_block,
    )
    .await
}

fn v2_unprocessable_entity(error: impl std::fmt::Display) -> Response {
    (
        axum::http::StatusCode::UNPROCESSABLE_ENTITY,
        Json(ton_api::toncenter::v2::TonlibErrorResponse {
            ok: false,
            error: error.to_string(),
            code: 422,
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
    StdAddr::from_str_ext(address, StdAddrFormat::any()).map_err(|error| {
        ToncenterHttpError::unprocessable_entity(format!("Invalid address format: {error}"))
    })
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
        .map_err(|_| ToncenterHttpError::unprocessable_entity("Invalid hash format"))
}

pub(super) struct ParsedTryLocateTxRequest {
    pub source: Addr,
    pub destination: Addr,
    pub created_lt: u64,
}

pub(super) fn parse_try_locate_tx_request(
    payload: &TryLocateTxRequest,
) -> anyhow::Result<ParsedTryLocateTxRequest> {
    let created_lt = payload.created_lt.to_i64().map_err(|_| {
        ToncenterHttpError::unprocessable_entity("created_lt should be a signed 64-bit integer")
    })?;
    if created_lt < 0 {
        return Err(ToncenterHttpError::unprocessable_entity(
            "created_lt should be non-negative",
        ));
    }

    Ok(ParsedTryLocateTxRequest {
        source: Addr::parse(&payload.source)?,
        destination: Addr::parse(&payload.destination)?,
        created_lt: created_lt as u64,
    })
}

pub(super) fn parse_config_param(payload: &ConfigParamRequest) -> anyhow::Result<u32> {
    let ((Some(raw), None) | (None, Some(raw))) = (&payload.param, &payload.config_id) else {
        return Err(ToncenterHttpError::unprocessable_entity(
            "only one of config_id or param should be specified",
        ));
    };
    let param = raw.to_i32().map_err(|_| {
        ToncenterHttpError::unprocessable_entity(
            "config param must be a non-negative 32-bit integer",
        )
    })?;
    u32::try_from(param).map_err(|_| {
        ToncenterHttpError::unprocessable_entity(
            "config param must be a non-negative 32-bit integer",
        )
    })
}

pub(super) fn parse_libraries_request(raw: &[String]) -> anyhow::Result<Vec<Hash256>> {
    raw.iter().map(String::as_str).map(parse_hash_any).collect()
}

#[derive(Debug)]
pub(super) struct BlockSelector {
    workchain: i32,
    shard: i64,
    seqno: u32,
    root_hash: Option<Hash256>,
    file_hash: Option<Hash256>,
}

pub(super) struct ParsedBlockTransactionsRequest {
    selector: BlockSelector,
    count: usize,
    after: Option<(u64, Hash256)>,
}

fn parse_block_selector(
    workchain: &StringOrNumber,
    shard: &StringOrNumber,
    seqno: &StringOrNumber,
    root_hash: Option<&String>,
    file_hash: Option<&String>,
) -> anyhow::Result<BlockSelector> {
    let workchain = workchain.to_i32()?;
    if !matches!(workchain, -1 | 0) {
        anyhow::bail!("`workchain` must be -1 or 0");
    }
    let shard = shard.to_i64()?;
    if shard != i64::MIN {
        anyhow::bail!("localnet supports only shard {:#x}", i64::MIN);
    }
    let seqno = parse_required_seqno(seqno)?;

    Ok(BlockSelector {
        workchain,
        shard,
        seqno,
        root_hash: root_hash.map(|hash| parse_hash_any(hash)).transpose()?,
        file_hash: file_hash.map(|hash| parse_hash_any(hash)).transpose()?,
    })
}

pub(super) fn parse_block_header_request(
    payload: &BlockHeaderRequest,
) -> anyhow::Result<BlockSelector> {
    parse_block_selector(
        &payload.workchain,
        &payload.shard,
        &payload.seqno,
        payload.root_hash.as_ref(),
        payload.file_hash.as_ref(),
    )
}

pub(super) async fn resolve_block_header(
    node: &Localnet,
    selector: &BlockSelector,
) -> anyhow::Result<LocalnetBlockHeader> {
    let block = if selector.workchain == -1 {
        node.get_masterchain_block_header(selector.seqno).await?
    } else {
        node.get_block_header(selector.seqno).await?
    };
    validate_block_id(&block.id, selector)?;
    Ok(block)
}

pub(super) fn parse_block_transactions_request(
    payload: &BlockTransactionsRequest,
) -> anyhow::Result<ParsedBlockTransactionsRequest> {
    let selector = parse_block_selector(
        &payload.workchain,
        &payload.shard,
        &payload.seqno,
        payload.root_hash.as_ref(),
        payload.file_hash.as_ref(),
    )?;

    let count = payload
        .count
        .as_ref()
        .map(StringOrNumber::to_i32)
        .transpose()?
        .unwrap_or(40);
    if count <= 0 {
        anyhow::bail!("count should be positive");
    }
    if count > 10_000 {
        anyhow::bail!("count should be less or equal 10000");
    }

    let after_lt = payload
        .after_lt
        .as_ref()
        .map(StringOrNumber::to_i64)
        .transpose()?;
    if after_lt.is_some_and(|after_lt| after_lt < 0) {
        anyhow::bail!("after_lt should be non-negative");
    }
    let after_hash = payload
        .after_hash
        .as_ref()
        .filter(|hash| !hash.is_empty())
        .map(|hash| parse_hash_any(hash))
        .transpose()?;
    if after_lt.is_some() != after_hash.is_some() {
        anyhow::bail!("after_lt and after_hash should be used together");
    }

    Ok(ParsedBlockTransactionsRequest {
        selector,
        count: count as usize,
        after: after_lt
            .zip(after_hash)
            .map(|(after_lt, after_hash)| (after_lt as u64, after_hash)),
    })
}

pub(super) async fn resolve_block_transactions(
    node: &Localnet,
    request: &ParsedBlockTransactionsRequest,
) -> anyhow::Result<LocalnetBlockTransactions> {
    let selector = &request.selector;
    let mut block = if selector.workchain == -1 {
        let header = node.get_masterchain_block_header(selector.seqno).await?;
        LocalnetBlockTransactions {
            id: header.id,
            transactions: Vec::new(),
            requested_count: 0,
            incomplete: false,
            msg_hash: None,
            msg_hash_norm: None,
        }
    } else {
        node.get_block_transactions(selector.seqno).await?
    };
    validate_block_id(&block.id, selector)?;
    paginate_block_transactions(&mut block, request.count, request.after);
    Ok(block)
}

fn validate_block_id(
    block_id: &crate::localnet::LocalnetBlockId,
    selector: &BlockSelector,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        block_id.workchain == selector.workchain,
        "workchain mismatch"
    );
    anyhow::ensure!(block_id.shard == selector.shard, "shard mismatch");
    anyhow::ensure!(block_id.seqno == selector.seqno, "seqno mismatch");
    if let (Some(root_hash), Some(file_hash)) = (selector.root_hash, selector.file_hash) {
        anyhow::ensure!(block_id.root_hash == root_hash, "root_hash mismatch");
        anyhow::ensure!(block_id.file_hash == file_hash, "file_hash mismatch");
    }
    Ok(())
}

fn paginate_block_transactions(
    block: &mut LocalnetBlockTransactions,
    count: usize,
    after: Option<(u64, Hash256)>,
) {
    let start = if let Some((after_lt, after_hash)) = after {
        block
            .transactions
            .iter()
            .position(|transaction| {
                transaction.transaction_id.lt == after_lt
                    && transaction.address.addr == after_hash.0
            })
            .map_or(0, |index| index + 1)
    } else {
        0
    };
    let mut transactions = block.transactions.split_off(start);
    block.incomplete = transactions.len() > count;
    transactions.truncate(count);
    block.transactions = transactions;
    block.requested_count = count;
}

pub(super) struct ParsedLookupBlockRequest {
    pub workchain: i32,
    pub shard: i64,
    pub seqno: Option<u32>,
    pub lt: Option<u64>,
    pub unixtime: Option<u32>,
}

pub(super) fn parse_lookup_block_request(
    payload: &LookupBlockRequest,
) -> anyhow::Result<ParsedLookupBlockRequest> {
    let seqno = payload
        .seqno
        .as_ref()
        .map(parse_required_seqno)
        .transpose()?;

    let lt = payload
        .lt
        .as_ref()
        .map(StringOrNumber::to_i64)
        .transpose()?;
    if lt.is_some_and(|lt| lt < 0) {
        anyhow::bail!("lt should be non-negative");
    }

    let unixtime = payload
        .unixtime
        .as_ref()
        .map(StringOrNumber::to_i32)
        .transpose()?;
    if unixtime.is_some_and(|unixtime| unixtime < 0) {
        anyhow::bail!("unixtime should be non-negative");
    }

    if usize::from(seqno.is_some()) + usize::from(lt.is_some()) + usize::from(unixtime.is_some())
        != 1
    {
        anyhow::bail!("exactly one of seqno, lt, unixtime should be specified");
    }

    Ok(ParsedLookupBlockRequest {
        workchain: payload.workchain.to_i32()?,
        shard: payload.shard.to_i64()?,
        seqno,
        lt: lt.map(|lt| lt as u64),
        unixtime: unixtime.map(|unixtime| unixtime as u32),
    })
}

pub(super) fn parse_seqno(seqno: Option<StringOrNumber>) -> anyhow::Result<Option<u32>> {
    seqno.as_ref().map(parse_required_seqno).transpose()
}

pub(super) fn parse_required_seqno(seqno: &StringOrNumber) -> anyhow::Result<u32> {
    let seqno = seqno.to_i32().map_err(|_| {
        ToncenterHttpError::unprocessable_entity("seqno should be a signed 32-bit integer")
    })?;
    if seqno <= 0 {
        return Err(ToncenterHttpError::unprocessable_entity(
            "seqno should be positive",
        ));
    }
    Ok(seqno as u32)
}

pub(super) fn parse_i32_seqno(seqno: Option<i32>) -> anyhow::Result<Option<u32>> {
    parse_seqno(seqno.map(Into::into))
}

pub(super) struct ParsedTransactionsRequest {
    pub address: Addr,
    pub limit: usize,
    pub lt: Option<u64>,
    pub hash: Option<Hash256>,
    pub to_lt: Option<u64>,
}

#[derive(Clone, Copy)]
enum ZeroLtPolicy {
    Absent,
    Cursor,
}

pub(super) fn parse_transactions_request(
    payload: &TransactionsRequest,
) -> anyhow::Result<ParsedTransactionsRequest> {
    parse_transactions_request_with_policy(payload, ZeroLtPolicy::Absent)
}

pub(super) fn parse_transactions_std_request(
    payload: &TransactionsRequest,
) -> anyhow::Result<ParsedTransactionsRequest> {
    parse_transactions_request_with_policy(payload, ZeroLtPolicy::Cursor)
}

fn parse_transactions_request_with_policy(
    payload: &TransactionsRequest,
    zero_lt_policy: ZeroLtPolicy,
) -> anyhow::Result<ParsedTransactionsRequest> {
    let limit = payload
        .limit
        .as_ref()
        .map(StringOrNumber::to_i64)
        .transpose()?
        .unwrap_or(10);
    if limit <= 0 {
        anyhow::bail!("limit should be positive");
    }
    if limit > 1000 {
        anyhow::bail!("limit should be less or equal 1000");
    }
    let lt = payload
        .lt
        .as_ref()
        .map(StringOrNumber::to_i64)
        .transpose()?;
    if lt.is_some_and(|lt| lt < 0) {
        anyhow::bail!("lt should be non-negative");
    }
    let to_lt = payload
        .to_lt
        .as_ref()
        .map(StringOrNumber::to_i64)
        .transpose()?;
    let has_lt = match zero_lt_policy {
        ZeroLtPolicy::Absent => lt.is_some_and(|value| value != 0),
        ZeroLtPolicy::Cursor => lt.is_some(),
    };
    let has_hash = payload.hash.as_ref().is_some_and(|hash| !hash.is_empty());
    if has_lt != has_hash {
        anyhow::bail!("lt and hash should be used together");
    }

    let hash = payload
        .hash
        .as_deref()
        .filter(|_| has_hash)
        .map(str::parse)
        .transpose()
        .map_err(|_| ToncenterHttpError::unprocessable_entity("failed to parse hash"))?;
    Ok(ParsedTransactionsRequest {
        address: Addr::parse(&payload.address)?,
        limit: limit as usize,
        lt: has_lt.then(|| lt.unwrap_or_default() as u64),
        hash,
        to_lt: to_lt.filter(|to_lt| *to_lt > 0).map(|to_lt| to_lt as u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_libraries_query_accepts_empty_input() {
        assert!(parse_libraries_request(&[]).unwrap().is_empty());
    }

    #[test]
    fn parse_libraries_query_rejects_invalid_hash() {
        let err = parse_libraries_request(&["not-a-hash".to_owned()])
            .expect_err("invalid hash must be rejected");
        assert!(
            err.to_string().contains("Invalid hash format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn parse_libraries_query_accepts_repeated_hashes() {
        let hash_a = "11".repeat(32);
        let hash_b = "22".repeat(32);

        let parsed =
            parse_libraries_request(&[hash_a, hash_b]).expect("valid repeated hashes must parse");
        assert_eq!(parsed, vec![Hash256([0x11; 32]), Hash256([0x22; 32])]);
    }
}
