use super::{LITEAPI_CAPABILITIES, LITEAPI_VERSION};
use crate::liteapi::convert;
use crate::liteapi::convert::MASTERCHAIN_WORKCHAIN;
use crate::liteapi::proof;
use crate::localnet::{
    Localnet, LocalnetBlockHeader, LocalnetBlockId, LocalnetRunGetMethodResult, LocalnetTransaction,
};
use crate::types::{BocBytes, Hash256};
use crate::{LiteServerErrorCode, LocalnetError};
use anyhow::Context;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::{Instant, sleep};
use ton_liteapi::liteclient::types::LiteError;
use ton_liteapi::tl::common::{BlockIdExt, LibraryEntry, String as TlString, TransactionId3};
use ton_liteapi::tl::request::{
    GetAccountState, GetAllShardsInfo, GetBlock, GetBlockHeader, GetBlockProof, GetConfigAll,
    GetConfigParams, GetLibraries, GetLibrariesWithProof, GetOneTransaction, GetShardBlockProof,
    GetShardInfo, GetTransactions, ListBlockTransactions, LookupBlock, LookupBlockWithProof,
    Request, RunSmcMethod, SendMessage, WaitMasterchainSeqno, WrappedRequest,
};
use ton_liteapi::tl::response::{
    AccountState, AllShardsInfo, BlockData, BlockTransactions, BlockTransactionsExt, ConfigInfo,
    CurrentTime, Error as TlServerError, LibraryResult, LibraryResultWithProof, LookupBlockResult,
    PartialBlockProof, Response, RunMethodResult, SendMsgStatus, ShardBlockLink, ShardBlockProof,
    ShardInfo, TransactionId, TransactionInfo, TransactionList, Version,
};
use tvm_ffi::stack::Tuple;
use tycho_types::boc::Boc;
use tycho_types::boc::ser::BocHeader;

const SEND_MESSAGE_ACCEPTED_STATUS: u32 = 1;
const RUN_SMC_METHOD_RESULT_MODE: u32 = 1 << 2;
const RUN_SMC_METHOD_SUPPORTED_BITS: u32 = RUN_SMC_METHOD_RESULT_MODE;
const RUN_SMC_METHOD_MAX_PARAMS_BYTES: usize = 65_535;
const LITESERVER_ACCOUNT_NOT_FOUND_EXIT_CODE: i32 = -0x100;
const LOCALNET_NO_CODE_EXIT_CODE: i32 = -13;

/// Handles one decoded `LiteServer` request against the localnet node.
///
/// The transport layer has already unwrapped ADNL and `liteServer.query` by the
/// time this function runs. The handler is responsible for honoring optional
/// `waitMasterchainSeqno`, mapping TL request variants to existing localnet
/// async APIs, and returning TL response variants that tonutils-go can parse.
pub(super) async fn handle(
    node: Arc<Localnet>,
    wrapped: WrappedRequest,
) -> Result<Response, LiteError> {
    if let Some(wait) = wrapped.wait_masterchain_seqno {
        wait_masterchain_seqno(&node, wait)
            .await
            .map_err(lite_error)?;
    }
    handle_request(node, wrapped.request)
        .await
        .map_err(lite_error)
}

async fn handle_request(node: Arc<Localnet>, request: Request) -> anyhow::Result<Response> {
    match request {
        Request::GetMasterchainInfo => {
            let info = node.get_masterchain_info().await?;
            Ok(Response::MasterchainInfo(convert::masterchain_info(info)))
        }
        Request::GetMasterchainInfoExt(_) => get_masterchain_info_ext(&node).await,
        Request::GetTime => Ok(Response::CurrentTime(CurrentTime { now: now() })),
        Request::GetVersion => Ok(Response::Version(Version {
            mode: 0,
            version: LITEAPI_VERSION,
            capabilities: LITEAPI_CAPABILITIES,
            now: now(),
        })),
        Request::GetBlock(request) => get_block(&node, request).await,
        Request::GetBlockHeader(request) => get_block_header(&node, request).await,
        Request::SendMessage(request) => send_message(&node, request).await,
        Request::GetAccountState(request) | Request::GetAccountStatePrunned(request) => {
            get_account_state(&node, request).await
        }
        Request::GetShardInfo(request) => get_shard_info(&node, request).await,
        Request::GetAllShardsInfo(request) => get_all_shards_info(&node, request).await,
        Request::GetOneTransaction(request) => get_one_transaction(&node, request).await,
        Request::GetTransactions(request) => get_transactions(&node, request).await,
        Request::LookupBlock(request) => lookup_block(&node, request).await,
        Request::LookupBlockWithProof(request) => lookup_block_with_proof(&node, request).await,
        Request::ListBlockTransactions(request) => list_block_transactions(&node, request).await,
        Request::ListBlockTransactionsExt(request) => {
            list_block_transactions_ext(&node, request).await
        }
        Request::RunSmcMethod(request) => run_smc_method(&node, request).await,
        Request::GetConfigAll(request) => get_config(&node, ConfigRequest::from(request)).await,
        Request::GetConfigParams(request) => get_config(&node, ConfigRequest::from(request)).await,
        Request::GetLibraries(request) => get_libraries(&node, request).await,
        Request::GetLibrariesWithProof(request) => get_libraries_with_proof(&node, request).await,
        Request::GetBlockProof(request) => get_block_proof(request),
        Request::GetShardBlockProof(request) => get_shard_block_proof(&node, request).await,
        unsupported => Err(LocalnetError::protocol_violation(format!(
            "LiteAPI request is not implemented in localnet: {unsupported:?}"
        ))
        .into()),
    }
}

/// Returns the block id matching an already supplied `LiteServer` request id.
///
/// Masterchain ids must point at real mined masterchain blocks. A masterchain
/// request whose hashes do not match the stored block for that seqno is rejected
/// instead of being mapped back to the basechain block.
async fn block_id_for_existing_request(
    node: &Localnet,
    request_id: &BlockIdExt,
    local_id: &LocalnetBlockId,
) -> anyhow::Result<BlockIdExt> {
    let workchain = request_id.workchain;
    if workchain != MASTERCHAIN_WORKCHAIN {
        return Ok(convert::block_id_ext(local_id));
    }

    let masterchain_header =
        masterchain_anchor_for_request(node, request_id, "LiteAPI request").await?;
    if masterchain_header.id.seqno != local_id.seqno {
        return Err(LocalnetError::protocol_violation(format!(
            "Requested masterchain block seqno {} does not match localnet shard block seqno {}",
            masterchain_header.id.seqno, local_id.seqno
        ))
        .into());
    }

    Ok(convert::block_id_ext(&masterchain_header.id))
}

/// Loads and validates a real mined masterchain block used as a request anchor.
///
/// `LiteAPI` methods such as `getShardInfo` carry a masterchain block id in
/// `request.id`, while separate request fields select the target shard. This
/// helper keeps those roles separate and rejects requests that try to use a
/// basechain block as the masterchain anchor.
async fn masterchain_anchor_for_request(
    node: &Localnet,
    request_id: &BlockIdExt,
    method: &str,
) -> anyhow::Result<LocalnetBlockHeader> {
    let workchain = request_id.workchain;
    if workchain != MASTERCHAIN_WORKCHAIN {
        return Err(LocalnetError::protocol_violation(format!(
            "{method} requires a masterchain block id as the anchor, got workchain {}",
            request_id.workchain
        ))
        .into());
    }

    let seqno = convert::seqno_from_i32(request_id.seqno)?;
    let header = node.get_masterchain_block_header(seqno).await?;
    let expected = convert::block_id_ext(&header.id);
    if &expected != request_id {
        return Err(LocalnetError::protocol_violation(format!(
            "{method} masterchain block id does not match localnet masterchain block for seqno {seqno}"
        ))
        .into());
    }

    Ok(header)
}

async fn get_masterchain_info_ext(node: &Localnet) -> anyhow::Result<Response> {
    let info = node.get_masterchain_info().await?;
    let header = if info.last.seqno == 0 {
        None
    } else {
        Some(node.get_masterchain_block_header(info.last.seqno).await?)
    };
    Ok(Response::MasterchainInfoExt(convert::masterchain_info_ext(
        info,
        header.as_ref(),
        now(),
    )?))
}

async fn get_block(node: &Localnet, request: GetBlock) -> anyhow::Result<Response> {
    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    let workchain = request.id.workchain;
    let (id, data) = if workchain == MASTERCHAIN_WORKCHAIN {
        let header = masterchain_anchor_for_request(node, &request.id, "getBlock").await?;
        (
            convert::block_id_ext(&header.id),
            node.get_masterchain_block_data(seqno).await?.0,
        )
    } else {
        let header = node.get_block_header(seqno).await?;
        (
            convert::block_id_ext(&header.id),
            node.get_block_data(seqno).await?.0,
        )
    };
    Ok(Response::BlockData(BlockData { id, data }))
}

async fn get_block_header(node: &Localnet, request: GetBlockHeader) -> anyhow::Result<Response> {
    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    let workchain = request.id.workchain;
    let response = if workchain == MASTERCHAIN_WORKCHAIN {
        let header = masterchain_anchor_for_request(node, &request.id, "getBlockHeader").await?;
        let header_proof = block_root_proof(node, header.id.workchain, header.id.seqno).await?;
        convert::block_header(
            header,
            request.with_state_update,
            request.with_value_flow,
            request.with_extra,
            request.with_shard_hashes,
            request.with_prev_blk_signatures,
            header_proof,
        )
    } else {
        let header = node.get_block_header(seqno).await?;
        let header_proof = block_root_proof(node, header.id.workchain, header.id.seqno).await?;
        convert::block_header(
            header,
            request.with_state_update,
            request.with_value_flow,
            request.with_extra,
            request.with_shard_hashes,
            request.with_prev_blk_signatures,
            header_proof,
        )
    };
    Ok(Response::BlockHeader(response))
}

/// Builds a `MerkleProof` for the exact block root announced in a `LiteAPI` block id.
///
/// Tonlib virtualizes the exotic proof cell and checks that the virtualized root
/// matches the `BlockIdExt` carried next to the proof. Localnet keeps serialized
/// block `BoC`s for both the basechain shard and the masterchain anchor, so a
/// full-root proof is sufficient for header, lookup, and transaction-list root
/// validation.
async fn block_root_proof(node: &Localnet, workchain: i32, seqno: u32) -> anyhow::Result<Vec<u8>> {
    let block_data = if workchain == MASTERCHAIN_WORKCHAIN {
        node.get_masterchain_block_data(seqno).await?
    } else {
        node.get_block_data(seqno).await?
    };
    let block_cell =
        Boc::decode(&block_data).context("Failed to decode LiteAPI block proof root")?;
    proof::merkle_proof_boc(block_cell)
}

/// Handles `liteServer.sendMessage` by queueing a raw external-in message.
///
/// TL already carries the body as decoded bytes, so this path skips the
/// toncenter base64 layer and forwards the `BoC` into the same localnet queue
/// used by HTTP `sendBoc`. A successful enqueue returns status `1`, matching the
/// upstream liteserver accepted-message status used by `LiteAPI` clients.
async fn send_message(node: &Localnet, request: SendMessage) -> anyhow::Result<Response> {
    node.send_boc_bytes(BocBytes::from(request.body)).await?;
    Ok(Response::SendMsgStatus(SendMsgStatus {
        status: SEND_MESSAGE_ACCEPTED_STATUS,
    }))
}

async fn get_account_state(node: &Localnet, request: GetAccountState) -> anyhow::Result<Response> {
    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    let header = node.get_block_header(seqno).await?;
    let address = convert::addr_from_account_id(&request.account);
    let block_data = node.get_block_data(seqno).await?;
    let shard_state = node.get_shard_state_cell(seqno).await?;
    let masterchain_block_data = node.get_masterchain_block_data(seqno).await?;
    let masterchain_state = node.get_masterchain_state_cell(seqno).await?;
    let shard_account = node
        .get_shard_account_cell(address.to_string(), Some(seqno))
        .await?;
    let id = block_id_for_existing_request(node, &request.id, &header.id).await?;
    let cells = proof::account_state_cells(
        &shard_account,
        &block_data,
        &shard_state,
        &masterchain_block_data,
        &masterchain_state,
    )?;
    let shardblk = convert::block_id_ext(&header.id);

    Ok(Response::AccountState(AccountState {
        id,
        shardblk,
        shard_proof: cells.shard_proof,
        proof: cells.proof,
        state: cells.state,
    }))
}

/// Handles `liteServer.getShardInfo` for localnet's single full shard.
///
/// Real liteservers read a shard descriptor from the masterchain state. Localnet
/// already stores one canonical block stream, so the descriptor is synthesized
/// from the requested block header and points at that real block root/file hash.
/// With `exact=false`, any shard inside the same workchain resolves to the full
/// localnet shard; with `exact=true`, the requested shard id must match exactly.
async fn get_shard_info(node: &Localnet, request: GetShardInfo) -> anyhow::Result<Response> {
    let masterchain_header =
        masterchain_anchor_for_request(node, &request.id, "getShardInfo").await?;
    let seqno = masterchain_header.id.seqno;
    let header = node.get_block_header(seqno).await?;
    let local_shard = header.id.shard as u64;

    if request.workchain != header.id.workchain || (request.exact && request.shard != local_shard) {
        return Err(LocalnetError::protocol_violation(format!(
            "Shard {}:{} is not available in localnet block {}",
            request.workchain, request.shard, header.id.seqno
        ))
        .into());
    }

    let id = convert::block_id_ext(&masterchain_header.id);
    let shardblk = convert::block_id_ext(&header.id);
    Ok(Response::ShardInfo(ShardInfo {
        id,
        shardblk,
        shard_proof: proof::empty_cell_boc(),
        shard_descr: proof::shard_description_data(&header)?,
    }))
}

async fn get_all_shards_info(
    node: &Localnet,
    request: GetAllShardsInfo,
) -> anyhow::Result<Response> {
    let masterchain_header =
        masterchain_anchor_for_request(node, &request.id, "getAllShardsInfo").await?;
    let seqno = masterchain_header.id.seqno;
    let header = node.get_block_header(seqno).await?;
    let id = convert::block_id_ext(&masterchain_header.id);
    Ok(Response::AllShardsInfo(AllShardsInfo {
        id,
        proof: proof::empty_cell_boc(),
        data: proof::all_shards_info_data(&header)?,
    }))
}

async fn get_one_transaction(
    node: &Localnet,
    request: GetOneTransaction,
) -> anyhow::Result<Response> {
    let address = convert::addr_from_account_id(&request.account);
    let transactions = node
        .get_transactions(address.to_string(), 1, Some(request.lt), None, None)
        .await?;
    let transaction = transactions
        .into_iter()
        .find(|tx| tx.transaction_id.lt == request.lt)
        .map(|tx| tx.data.0)
        .unwrap_or_default();

    Ok(Response::TransactionInfo(TransactionInfo {
        id: request.id,
        proof: proof::empty_cell_boc(),
        transaction,
    }))
}

/// Handles `liteServer.getTransactions` using localnet's account transaction index.
///
/// The `LiteAPI` response pairs each transaction with the block id that contained
/// it and stores all transaction cells in a single multi-root `BoC`, matching the
/// upstream liteserver shape produced by `std_boc_serialize_multi`.
async fn get_transactions(node: &Localnet, request: GetTransactions) -> anyhow::Result<Response> {
    let address = convert::addr_from_account_id(&request.account);
    let requested = usize::try_from(request.count).unwrap_or(usize::MAX);
    let transactions = node
        .get_transactions_by_address(
            address,
            requested,
            Some(request.lt),
            Some(Hash256(request.hash.0)),
            None,
        )
        .await?;
    let ids = transaction_block_ids(node, &transactions).await?;
    let transactions = transaction_roots_boc(&transactions)?;

    Ok(Response::TransactionList(TransactionList {
        ids,
        transactions,
    }))
}

/// Executes `liteServer.runSmcMethod` through localnet's existing get-method engine.
///
/// The request format is already binary and carries both the numeric method id
/// and a serialized TVM stack, so this adapter only validates the `LiteServer`
/// mode bits, decodes the stack `BoC`, and forwards the typed values to the
/// localnet actor. Proof, c7, and library-extra response modes are rejected
/// because this response path returns only the execution result payload.
async fn run_smc_method(node: &Localnet, request: RunSmcMethod) -> anyhow::Result<Response> {
    if request.mode & !RUN_SMC_METHOD_SUPPORTED_BITS != 0 {
        return Err(LocalnetError::protocol_violation(format!(
            "Unsupported liteServer.runSmcMethod mode {}: localnet supports only result bit {}",
            request.mode, RUN_SMC_METHOD_RESULT_MODE
        ))
        .into());
    }

    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    let method_id = i32::try_from(request.method_id).map_err(|_| {
        LocalnetError::protocol_violation(format!(
            "runSmcMethod method_id {} exceeds i32 range",
            request.method_id
        ))
    })?;
    let stack = run_smc_method_params(request.params)?;
    let result = node
        .run_get_method_by_id(
            convert::addr_from_account_id(&request.account),
            method_id,
            stack,
            Some(seqno),
        )
        .await?;

    let id = block_id_for_existing_request(node, &request.id, &result.block_id).await?;
    let shardblk = convert::block_id_ext(&result.block_id);
    let include_result = request.mode & RUN_SMC_METHOD_RESULT_MODE != 0;
    let account_not_found = run_smc_method_account_not_found(&result);
    let exit_code = if account_not_found {
        LITESERVER_ACCOUNT_NOT_FOUND_EXIT_CODE
    } else {
        result.exit_code
    };
    let result_stack = include_result.then(|| {
        if account_not_found {
            Vec::new()
        } else {
            result.stack.0
        }
    });

    Ok(Response::RunMethodResult(RunMethodResult {
        mode: (),
        id,
        shardblk,
        shard_proof: None,
        proof: None,
        state_proof: None,
        init_c7: None,
        lib_extras: None,
        exit_code,
        result: result_stack,
    }))
}

async fn lookup_block(node: &Localnet, request: LookupBlock) -> anyhow::Result<Response> {
    let requested_workchain = request.id.workchain;
    let block_id = node
        .lookup_block(
            requested_workchain,
            request.id.shard.to_string(),
            request
                .seqno
                .map(|()| convert::seqno_from_i32(request.id.seqno))
                .transpose()?,
            request.lt,
            request.utime,
        )
        .await?;
    let response = if requested_workchain == MASTERCHAIN_WORKCHAIN {
        let header = node.get_masterchain_block_header(block_id.seqno).await?;
        let header_proof = block_root_proof(node, header.id.workchain, header.id.seqno).await?;
        convert::block_header(
            header,
            request.with_state_update,
            request.with_value_flow,
            request.with_extra,
            request.with_shard_hashes,
            request.with_prev_blk_signatures,
            header_proof,
        )
    } else {
        let header = node.get_block_header(block_id.seqno).await?;
        let header_proof = block_root_proof(node, header.id.workchain, header.id.seqno).await?;
        convert::block_header(
            header,
            request.with_state_update,
            request.with_value_flow,
            request.with_extra,
            request.with_shard_hashes,
            request.with_prev_blk_signatures,
            header_proof,
        )
    };
    Ok(Response::BlockHeader(response))
}

/// Handles `liteServer.lookupBlockWithProof` for localnet masterchain/shard blocks.
///
/// `tonlibjson` uses this proof-bearing lookup path for block and shard helper
/// methods. Localnet resolves the requested block through the same single-shard
/// lookup code as `liteServer.lookupBlock`, then returns a one-link proof chain
/// from the masterchain anchor at the same seqno to the basechain shard block.
/// Tonlib verifies the link through `McBlockExtra.shards`; localnet does not
/// model validator signatures, shard splits, or shard merges in this proof.
async fn lookup_block_with_proof(
    node: &Localnet,
    request: LookupBlockWithProof,
) -> anyhow::Result<Response> {
    let requested_workchain = request.id.workchain;
    let block_id = node
        .lookup_block(
            requested_workchain,
            request.id.shard.to_string(),
            Some(convert::seqno_from_i32(request.id.seqno)?),
            request.lt,
            request.utime,
        )
        .await?;
    let header = if requested_workchain == MASTERCHAIN_WORKCHAIN {
        node.get_masterchain_block_header(block_id.seqno).await?
    } else {
        node.get_block_header(block_id.seqno).await?
    };

    let id = convert::block_id_ext(&header.id);
    let result_mc_block_id = if requested_workchain == MASTERCHAIN_WORKCHAIN {
        id.clone()
    } else {
        let masterchain_header = node.get_masterchain_block_header(block_id.seqno).await?;
        convert::block_id_ext(&masterchain_header.id)
    };

    let (client_mc_state_proof, mc_block_proof) =
        lookup_block_masterchain_proofs(node, &request.mc_block_id, &result_mc_block_id).await?;
    let header_cell = lookup_block_header_cell(node, requested_workchain, &header).await?;
    let header_proof = proof::merkle_proof_boc(header_cell)?;
    let prev_header_proof = if let Some(prev_seqno) = header.prev_seqno {
        let prev_header = if requested_workchain == MASTERCHAIN_WORKCHAIN {
            node.get_masterchain_block_header(prev_seqno).await?
        } else {
            node.get_block_header(prev_seqno).await?
        };
        let prev_header_cell =
            lookup_block_header_cell(node, requested_workchain, &prev_header).await?;
        proof::merkle_proof_boc(prev_header_cell)?
    } else {
        proof::empty_cell_boc()
    };

    let shard_links = if requested_workchain == MASTERCHAIN_WORKCHAIN {
        Vec::new()
    } else {
        vec![ShardBlockLink {
            id: id.clone(),
            proof: block_root_proof(node, result_mc_block_id.workchain, block_id.seqno).await?,
        }]
    };

    Ok(Response::LookupBlockResult(LookupBlockResult {
        id,
        mode: (),
        mc_block_id: result_mc_block_id,
        client_mc_state_proof,
        mc_block_proof,
        shard_links,
        header: header_proof,
        prev_header: prev_header_proof,
    }))
}

/// Builds the client-masterchain proof pair used by `lookupBlockWithProof`.
///
/// Tonlib passes its latest trusted masterchain block as `client_mc_block_id`.
/// When the resolved block is anchored by an older masterchain block, tonlib
/// first validates the latest block proof, extracts its state, and checks that
/// `old_mc_blocks` contains the older anchor. The first returned field is the
/// proof of the client masterchain block itself; the second is a proof of that
/// block's post-state root.
async fn lookup_block_masterchain_proofs(
    node: &Localnet,
    client_mc_block_id: &BlockIdExt,
    result_mc_block_id: &BlockIdExt,
) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
    if client_mc_block_id == result_mc_block_id {
        return Ok((proof::empty_cell_boc(), proof::empty_cell_boc()));
    }

    let client_mc_header =
        masterchain_anchor_for_request(node, client_mc_block_id, "lookupBlockWithProof").await?;
    let client_mc_block_boc = node
        .get_masterchain_block_data(client_mc_header.id.seqno)
        .await?;
    let client_mc_state = node
        .get_masterchain_state_cell(client_mc_header.id.seqno)
        .await?;
    let client_mc_block =
        Boc::decode(&client_mc_block_boc).context("Failed to decode client masterchain block")?;

    Ok((
        proof::merkle_proof_boc(client_mc_block)?,
        proof::state_proof_from_cell(client_mc_state)?,
    ))
}

async fn lookup_block_header_cell(
    node: &Localnet,
    requested_workchain: i32,
    header: &LocalnetBlockHeader,
) -> anyhow::Result<tycho_types::cell::Cell> {
    let data = if requested_workchain == MASTERCHAIN_WORKCHAIN {
        node.get_masterchain_block_data(header.id.seqno).await?.0
    } else {
        node.get_block_data(header.id.seqno).await?.0
    };
    Boc::decode(&data).context("Failed to decode lookup block header proof cell")
}

/// Decodes the `params` field from `liteServer.runSmcMethod` into a TVM stack.
///
/// `LiteServer` clients send method arguments as a `BoC` containing a serialized
/// `VmStack`. Empty params are accepted as an empty stack, matching upstream
/// liteserver behavior for get-methods without arguments.
fn run_smc_method_params(params: Vec<u8>) -> anyhow::Result<Tuple> {
    if params.len() > RUN_SMC_METHOD_MAX_PARAMS_BYTES {
        return Err(LocalnetError::protocol_violation(format!(
            "runSmcMethod params are too large: {} bytes, maximum is {}",
            params.len(),
            RUN_SMC_METHOD_MAX_PARAMS_BYTES
        ))
        .into());
    }

    if params.is_empty() {
        return Ok(Tuple::empty());
    }

    let cell = Boc::decode(&params).map_err(|error| {
        LocalnetError::protocol_violation(format!(
            "Failed to decode runSmcMethod params BoC: {error}"
        ))
    })?;
    Tuple::deserialize(&cell).map_err(|error| {
        LocalnetError::protocol_violation(format!(
            "Failed to deserialize runSmcMethod params as TVM stack: {error}"
        ))
        .into()
    })
}

/// Maps localnet's no-code sentinel to the exit code used by upstream `LiteServer`.
///
/// The HTTP toncenter-compatible API historically reports local no-code
/// get-method calls as `-13`. `LiteServer` returns `-256` for an absent,
/// uninitialized, frozen, or otherwise non-runnable account, and Go `LiteAPI`
/// clients rely on that value for account-not-found handling.
fn run_smc_method_account_not_found(result: &LocalnetRunGetMethodResult) -> bool {
    result.exit_code == LOCALNET_NO_CODE_EXIT_CODE
        && result.gas_used == 0
        && result.vm_log.is_empty()
}

async fn list_block_transactions(
    node: &Localnet,
    request: ListBlockTransactions,
) -> anyhow::Result<Response> {
    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    // The masterchain is only an anchor for shard discovery; localnet stores
    // executable transactions on the real basechain shard block.
    let workchain = request.id.workchain;
    if workchain == MASTERCHAIN_WORKCHAIN {
        let header = node.get_masterchain_block_header(seqno).await?;
        let proof = block_transactions_proof(
            node,
            request.id.workchain,
            seqno,
            request.want_proof.is_some(),
        )
        .await?;
        return Ok(Response::BlockTransactions(BlockTransactions {
            id: convert::block_id_ext(&header.id),
            req_count: request.count,
            incomplete: false,
            ids: Vec::new(),
            proof,
        }));
    }

    let block = node.get_block_transactions(seqno).await?;
    let id = convert::block_id_ext(&block.id);
    let proof = block_transactions_proof(
        node,
        request.id.workchain,
        seqno,
        request.want_proof.is_some(),
    )
    .await?;
    let (transactions, incomplete) =
        limit_block_transactions(block.transactions, request.after, request.count);
    let ids = transactions
        .into_iter()
        .map(transaction_id)
        .collect::<Vec<_>>();

    Ok(Response::BlockTransactions(BlockTransactions {
        id,
        req_count: request.count,
        incomplete,
        ids,
        proof,
    }))
}

/// Handles `liteServer.listBlockTransactionsExt` for localnet blocks.
///
/// This mirrors `liteServer.listBlockTransactions` pagination but returns the
/// actual transaction cells as a multi-root `BoC`. When the client asks for a
/// proof, localnet returns the same full-root block `MerkleProof` used by the
/// compact transaction-list response, which is sufficient for tonlib's root hash
/// validation.
async fn list_block_transactions_ext(
    node: &Localnet,
    request: ListBlockTransactions,
) -> anyhow::Result<Response> {
    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    // See `list_block_transactions`: masterchain transaction lists are empty by
    // construction, while the same seqno's basechain shard carries real txs.
    let workchain = request.id.workchain;
    if workchain == MASTERCHAIN_WORKCHAIN {
        let header = node.get_masterchain_block_header(seqno).await?;
        let proof = block_transactions_proof(
            node,
            request.id.workchain,
            seqno,
            request.want_proof.is_some(),
        )
        .await?;
        return Ok(Response::BlockTransactionsExt(BlockTransactionsExt {
            id: convert::block_id_ext(&header.id),
            req_count: request.count,
            incomplete: false,
            transactions: transaction_roots_boc(&[])?,
            proof,
        }));
    }

    let block = node.get_block_transactions(seqno).await?;
    let id = convert::block_id_ext(&block.id);
    let proof = block_transactions_proof(
        node,
        request.id.workchain,
        seqno,
        request.want_proof.is_some(),
    )
    .await?;
    let (transactions, incomplete) =
        limit_block_transactions(block.transactions, request.after, request.count);
    let transactions = transaction_roots_boc(&transactions)?;

    Ok(Response::BlockTransactionsExt(BlockTransactionsExt {
        id,
        req_count: request.count,
        incomplete,
        transactions,
        proof,
    }))
}

/// Returns an optional transaction-list proof matching the requested block.
///
/// `liteServer.listBlockTransactions` and `listBlockTransactionsExt` only attach
/// the block proof when clients set `want_proof`. Without that flag, upstream
/// liteservers return an empty byte slice, and keeping the same behavior avoids
/// making tonlib parse a non-proof cell on lightweight pagination requests.
async fn block_transactions_proof(
    node: &Localnet,
    workchain: i32,
    seqno: u32,
    want_proof: bool,
) -> anyhow::Result<Vec<u8>> {
    if want_proof {
        block_root_proof(node, workchain, seqno).await
    } else {
        Ok(Vec::new())
    }
}

/// Applies `LiteAPI` block-transaction pagination to an in-memory localnet block.
///
/// The `after` cursor removes transactions through the cursor transaction, then
/// `count` caps the returned slice. The boolean mirrors liteserver's
/// `incomplete` flag and tells callers whether more transactions remain after
/// the returned page.
fn limit_block_transactions(
    mut transactions: Vec<LocalnetTransaction>,
    after: Option<TransactionId3>,
    count: u32,
) -> (Vec<LocalnetTransaction>, bool) {
    if let Some(after) = after
        && let Some(index) = transactions
            .iter()
            .position(|tx| tx.address.addr == after.account.0 && tx.transaction_id.lt == after.lt)
    {
        transactions.drain(..=index);
    }

    let requested = usize::try_from(count).unwrap_or(usize::MAX);
    let incomplete = transactions.len() > requested;
    transactions.truncate(requested);
    (transactions, incomplete)
}

/// Resolves the containing block id for each transaction in a `LiteAPI` response.
///
/// `liteServer.transactionList` does not embed transaction ids; instead, it
/// carries a vector of block ids aligned with the multi-root transaction `BoC`.
/// Localnet stores each transaction's masterchain block seqno, so this helper
/// loads the corresponding real block headers and caches repeated seqnos within
/// one response.
async fn transaction_block_ids(
    node: &Localnet,
    transactions: &[LocalnetTransaction],
) -> anyhow::Result<Vec<BlockIdExt>> {
    let mut by_seqno: BTreeMap<u32, BlockIdExt> = BTreeMap::new();
    let mut ids = Vec::with_capacity(transactions.len());

    for transaction in transactions {
        let id = if let Some(id) = by_seqno.get(&transaction.mc_block_seqno) {
            id.clone()
        } else {
            let header = node.get_block_header(transaction.mc_block_seqno).await?;
            let id = convert::block_id_ext(&header.id);
            by_seqno.insert(transaction.mc_block_seqno, id.clone());
            id
        };
        ids.push(id);
    }

    Ok(ids)
}

/// Serializes transaction cells as one multi-root `BoC`.
///
/// Upstream liteserver uses the same shape for `liteServer.transactionList` and
/// `liteServer.blockTransactionsExt`: every returned transaction is a separate
/// root cell in one `BoC`, preserving the original transaction serialization.
fn transaction_roots_boc(transactions: &[LocalnetTransaction]) -> anyhow::Result<Vec<u8>> {
    if transactions.is_empty() {
        return Ok(Vec::new());
    }

    let cells = transactions
        .iter()
        .map(|tx| {
            Boc::decode(&tx.data)
                .with_context(|| format!("Failed to decode transaction {} BoC", tx.hash.to_hex()))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let mut header: BocHeader<'_> = BocHeader::with_capacity(cells.len());

    for cell in &cells {
        header.add_root(cell.as_ref());
    }

    let mut result = Vec::new();
    header.encode(&mut result);
    Ok(result)
}

async fn get_config(node: &Localnet, request: ConfigRequest) -> anyhow::Result<Response> {
    let seqno = convert::seqno_from_i32(request.id.seqno)?;
    let header = node.get_block_header(seqno).await?;
    let id = block_id_for_existing_request(node, &request.id, &header.id).await?;
    let workchain = request.id.workchain;
    let (state_proof, config_proof) = if workchain == MASTERCHAIN_WORKCHAIN {
        let masterchain_block_data = node.get_masterchain_block_data(seqno).await?;
        let masterchain_state = node.get_masterchain_state_cell(seqno).await?;
        proof::config_proofs(&masterchain_block_data, masterchain_state)?
    } else {
        (proof::empty_cell_boc(), proof::empty_cell_boc())
    };

    Ok(Response::ConfigInfo(ConfigInfo {
        mode: (),
        id,
        state_proof,
        config_proof,
        with_state_root: request.with_state_root,
        with_libraries: request.with_libraries,
        with_state_extra_root: request.with_state_extra_root,
        with_shard_hashes: request.with_shard_hashes,
        with_validator_set: request.with_validator_set,
        with_special_smc: request.with_special_smc,
        with_accounts_root: request.with_accounts_root,
        with_prev_blocks: request.with_prev_blocks,
        with_workchain_info: request.with_workchain_info,
        with_capabilities: request.with_capabilities,
        extract_from_key_block: request.extract_from_key_block,
    }))
}

struct ConfigRequest {
    id: BlockIdExt,
    with_state_root: Option<()>,
    with_libraries: Option<()>,
    with_state_extra_root: Option<()>,
    with_shard_hashes: Option<()>,
    with_validator_set: Option<()>,
    with_special_smc: Option<()>,
    with_accounts_root: Option<()>,
    with_prev_blocks: Option<()>,
    with_workchain_info: Option<()>,
    with_capabilities: Option<()>,
    extract_from_key_block: Option<()>,
}

impl From<GetConfigAll> for ConfigRequest {
    fn from(value: GetConfigAll) -> Self {
        Self {
            id: value.id,
            with_state_root: value.with_state_root,
            with_libraries: value.with_libraries,
            with_state_extra_root: value.with_state_extra_root,
            with_shard_hashes: value.with_shard_hashes,
            with_validator_set: value.with_validator_set,
            with_special_smc: value.with_special_smc,
            with_accounts_root: value.with_accounts_root,
            with_prev_blocks: value.with_prev_blocks,
            with_workchain_info: value.with_workchain_info,
            with_capabilities: value.with_capabilities,
            extract_from_key_block: value.extract_from_key_block,
        }
    }
}

impl From<GetConfigParams> for ConfigRequest {
    fn from(value: GetConfigParams) -> Self {
        Self {
            id: value.id,
            with_state_root: value.with_state_root,
            with_libraries: value.with_libraries,
            with_state_extra_root: value.with_state_extra_root,
            with_shard_hashes: value.with_shard_hashes,
            with_validator_set: value.with_validator_set,
            with_special_smc: value.with_special_smc,
            with_accounts_root: value.with_accounts_root,
            with_prev_blocks: value.with_prev_blocks,
            with_workchain_info: value.with_workchain_info,
            with_capabilities: value.with_capabilities,
            extract_from_key_block: value.extract_from_key_block,
        }
    }
}

async fn get_libraries(node: &Localnet, request: GetLibraries) -> anyhow::Result<Response> {
    let hashes = request
        .library_list
        .into_iter()
        .map(|hash| Hash256(hash.0))
        .collect::<Vec<_>>();
    let libraries = node.get_libraries(hashes).await?;
    let result = libraries
        .into_iter()
        .filter_map(|library| {
            library.found.then(|| LibraryEntry {
                hash: convert::int256(library.hash.0),
                data: library.data.map_or_else(Vec::new, |data| data.0),
            })
        })
        .collect();

    Ok(Response::LibraryResult(LibraryResult { result }))
}

async fn get_libraries_with_proof(
    node: &Localnet,
    request: GetLibrariesWithProof,
) -> anyhow::Result<Response> {
    let hashes = request
        .library_list
        .into_iter()
        .map(|hash| Hash256(hash.0))
        .collect::<Vec<_>>();
    let libraries = node.get_libraries(hashes).await?;
    let result = libraries
        .into_iter()
        .filter_map(|library| {
            library.found.then(|| LibraryEntry {
                hash: convert::int256(library.hash.0),
                data: library.data.map_or_else(Vec::new, |data| data.0),
            })
        })
        .collect();

    Ok(Response::LibraryResultWithProof(LibraryResultWithProof {
        id: request.id,
        mode: (),
        result,
        state_proof: proof::empty_cell_boc(),
        data_proof: proof::empty_cell_boc(),
    }))
}

/// Builds the degenerate `liteServer.getBlockProof` response for a known block.
///
/// `tonlibjson` asks for a block proof even when the configured trusted block
/// and the target block are identical. In that case no Merkle/link step is
/// required: the proof is complete from the block to itself. Requests spanning
/// distinct blocks fail explicitly because localnet does not produce validator
/// block proof chains.
fn get_block_proof(request: GetBlockProof) -> anyhow::Result<Response> {
    let to = request
        .target_block
        .clone()
        .unwrap_or_else(|| request.known_block.clone());

    if request.known_block != to {
        return Err(LocalnetError::protocol_violation(
            "liteServer.getBlockProof supports only identical known and target blocks",
        )
        .into());
    }

    Ok(Response::PartialBlockProof(PartialBlockProof {
        complete: true,
        from: request.known_block,
        to,
        steps: Vec::new(),
    }))
}

/// Returns the local shard-block link shape for `liteServer.getShardBlockProof`.
///
/// The response identifies the latest masterchain block and echoes the requested
/// shard block id, but it does not include a validator shard-block proof chain.
/// Tonlib callers that require proof validation still reject this response.
async fn get_shard_block_proof(
    node: &Localnet,
    request: GetShardBlockProof,
) -> anyhow::Result<Response> {
    let masterchain_id = convert::block_id_ext(&node.get_masterchain_info().await?.last);
    Ok(Response::ShardBlockProof(ShardBlockProof {
        masterchain_id,
        links: vec![ShardBlockLink {
            id: request.id,
            proof: proof::empty_cell_boc(),
        }],
    }))
}

async fn wait_masterchain_seqno(node: &Localnet, wait: WaitMasterchainSeqno) -> anyhow::Result<()> {
    let timeout = Duration::from_millis(u64::from(wait.timeout_ms));
    let deadline = Instant::now() + timeout;

    loop {
        let info = node.get_masterchain_info().await?;
        if info.last.seqno >= wait.seqno {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(LocalnetError::MasterchainWaitTimeout { seqno: wait.seqno }.into());
        }
        sleep(Duration::from_millis(50)).await;
    }
}

fn transaction_id(transaction: LocalnetTransaction) -> TransactionId {
    TransactionId {
        mode: (),
        account: Some(convert::int256(transaction.address.addr)),
        lt: Some(transaction.transaction_id.lt),
        hash: Some(convert::int256(transaction.hash.0)),
        metadata: None,
    }
}

fn now() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as u32)
}

fn lite_error(error: anyhow::Error) -> LiteError {
    let code = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<LocalnetError>())
        .map_or(LiteServerErrorCode::Error, LocalnetError::lite_server_code);
    let message = error.to_string();

    LiteError::ServerError(TlServerError {
        code: i32::from(code),
        message: TlString::new(message),
    })
}
