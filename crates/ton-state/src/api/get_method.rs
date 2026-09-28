//! `TONLib`-compatible get methods over one immutable committed state frontier.

#[cfg(test)]
mod tests;

mod stack;

use anyhow::{Context, Result, ensure};
use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::{Engine, engine::general_purpose::STANDARD};
use rston::boc::Boc;
use rston::cell::{Cell, CellFamily, HashBytes};
use rston::dict::Dict;
use rston::models::{AccountState, BlockId, StdAddr, StdAddrFormat};
use ton_executor::ExecutorVerbosity;
use ton_executor::get::{GetExecutor, GetMethodResult, RunGetMethodArgs};
use ton_executor::message::{PrevBlockId, PrevBlocksInfo};
use ton_node_db::{AccountSnapshot, MasterchainContext, StateView};
use toncenter::v2::{self as v2, Int32Input, requests::RunGetMethodRequest, responses as wire};
use tvm_ffi::stack::{Tuple, TupleItem};

use super::{Api, ApiError, block_id, read};

const GAS_LIMIT: &str = "1000000";
const MAX_LIBRARY_LOADS: usize = 8;

/// Run a get method
///
/// Executes locally with `TONLib`'s 1,000,000 gas limit. Name or numeric method ID,
/// legacy TON Center stack, account, config, and libraries use one committed
/// checkpoint. VM failure still returns HTTP 200 with its `exit_code`. State is
/// never changed. Only the current checkpoint is available via seqno
#[utoipa::path(
    post,
    path = "/api/runGetMethod",
    operation_id = "runGetMethod",
    request_body = RunGetMethodRequest,
    responses(
        (status = 200, description = "VM result, including unsuccessful exit codes", body = v2::TonlibResponse<wire::RunGetMethodResult>),
        (status = 400, description = "Invalid address, method, stack, or JSON", body = v2::TonlibErrorResponse),
        (status = 409, description = "Requested checkpoint is unavailable", body = v2::TonlibErrorResponse),
        (status = 413, description = "Request exceeds the HTTP body limit", body = v2::TonlibErrorResponse),
        (status = 429, description = "Get-method executor is busy", body = v2::TonlibErrorResponse),
        (status = 500, description = "State read or execution infrastructure failed", body = v2::TonlibErrorResponse),
    ),
)]
pub(super) async fn run_get_method(
    State(api): State<Api>,
    request: Result<Json<RunGetMethodRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => {
            let status = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            };
            return ApiError::new(status, "invalid runGetMethod JSON body").into_response();
        }
    };
    let Ok((address, _)) = StdAddr::from_str_ext(&request.address, StdAddrFormat::any()) else {
        return ApiError::new(StatusCode::BAD_REQUEST, "invalid account address").into_response();
    };
    let Ok(permit) = api.execution_slot.clone().try_acquire_owned() else {
        return ApiError::new(StatusCode::TOO_MANY_REQUESTS, "get-method executor is busy")
            .into_response();
    };

    read(api, "runGetMethod", move |store, _| {
        // Keep the permit in the blocking task even if the HTTP client disconnects.
        let _permit = permit;
        if let Some(seqno) = request.seqno {
            let seqno = u32::try_from(seqno)
                .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid masterchain seqno"))?;
            if seqno != store.head().seqno {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "only the current applied masterchain seqno is available",
                )
                .into());
            }
        }
        let method = method_id(request.method)?;
        let input = stack::input(request.stack).map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid or oversized TVM input stack",
            )
        })?;
        let account = store.get_account(&address)?;
        let master = store.masterchain_state()?;
        execute(account, &address, method, input, &master)
    })
    .await
}

fn method_id(method: Int32Input) -> Result<i32> {
    match method {
        Int32Input::Number(id) => Ok(id),
        Int32Input::String(name) => {
            if name.is_empty() || name.len() > 128 || name.contains('\0') {
                return Err(
                    ApiError::new(StatusCode::BAD_REQUEST, "invalid get-method name").into(),
                );
            }
            // TON Center also accepts decimal/hex numeric IDs encoded as strings.
            if let Ok(number) = stack::number(&name) {
                return i32::try_from(number).map_err(|_| {
                    ApiError::new(StatusCode::BAD_REQUEST, "get-method ID does not fit int32")
                        .into()
                });
            }
            let crc = crc::Crc::<u16>::new(&crc::CRC_16_XMODEM).checksum(name.as_bytes());
            Ok(i32::from(crc) | 0x10000)
        }
    }
}

/// Mirrors `TONLib`'s `SmartContract` context: shard time and balance, zero random
/// seed and logical times, network config, and previous masterchain blocks.
/// Libraries are fetched on demand from the pinned masterchain view; retries
/// start from the original state and stack and never commit VM side effects.
fn execute(
    snapshot: AccountSnapshot,
    address: &StdAddr,
    method: i32,
    input: Tuple,
    master: &StateView,
) -> Result<wire::RunGetMethodResult> {
    let mut response = wire::RunGetMethodResult {
        type_tag: Default::default(),
        gas_used: 0,
        stack: Vec::new(),
        exit_code: 0,
        block_id: block_id(snapshot.masterchain_block),
        last_transaction_id: wire::InternalTransactionId {
            type_tag: Default::default(),
            lt: snapshot
                .account
                .as_ref()
                .map_or(0, |a| a.last_trans_lt)
                .to_string(),
            hash: STANDARD.encode(
                snapshot
                    .account
                    .as_ref()
                    .map_or(HashBytes::ZERO, |a| a.last_trans_hash),
            ),
        },
    };
    let account = snapshot
        .account
        .map(|a| a.load_account())
        .transpose()?
        .flatten();
    let executable = account.as_ref().and_then(|account| match &account.state {
        AccountState::Active(state) => state.code.as_ref().map(|code| (account, state, code)),
        _ => None,
    });
    let Some((account, state, code)) = executable else {
        // TONLib reports -13 for absent/uninitialized/frozen accounts and
        // preserves the arguments followed by the method selector.
        let mut output = input;
        output.push(TupleItem::Int(method.into()));
        response.exit_code = -13;
        response.stack = stack::output(&output)?;
        return Ok(response);
    };
    account
        .balance
        .tokens
        .to_string()
        .parse::<u64>()
        .context("account balance exceeds the TONLib uint64 execution interface")?;
    let context = master.execution_context()?;
    let config = Boc::encode_base64(
        context
            .config
            .params
            .as_dict()
            .root()
            .as_ref()
            .context("empty blockchain config")?,
    );
    let mut args = RunGetMethodArgs {
        code: Boc::encode_base64(code),
        data: Boc::encode_base64(state.data.clone().unwrap_or_else(Cell::empty_cell)),
        verbosity: ExecutorVerbosity::Off,
        debug_enabled: false,
        address: address.to_string(),
        unixtime: i64::from(snapshot.gen_utime),
        balance: account.balance.tokens.to_string(),
        gas_limit: GAS_LIMIT.into(),
        method_id: method,
        prev_blocks_info: Some(previous_blocks(&context)?.to_stack_entry_boc_base64()?),
        ..Default::default()
    };
    for entry in account.balance.other.as_dict().iter() {
        let (id, amount) = entry?;
        args.extra_currencies
            .insert(id.to_string(), amount.to_string());
    }
    let input = Boc::encode_base64(input.serialize()?);
    let mut libraries = Dict::<HashBytes, Cell>::new();
    for attempt in 0..=MAX_LIBRARY_LOADS {
        args.libs = libraries
            .root()
            .as_ref()
            .map(Boc::encode_base64)
            .unwrap_or_default();
        let executor = GetExecutor::new(&args)?;
        let result = executor.run_get_method(&input, &args, Some(&config))?;
        let GetMethodResult::Success(result) = result else {
            anyhow::bail!("get-method emulator could not produce a serializable result");
        };
        if let Some(hash) = &result.missing_library {
            ensure!(
                attempt < MAX_LIBRARY_LOADS,
                "get method exceeded library lookup limit"
            );
            let hash: HashBytes = hash.parse().context("invalid missing-library hash")?;
            if !libraries.contains_key(hash)?
                && let Some(library) = master.get_library(&hash)?
            {
                libraries.set(hash, library)?;
                continue;
            }
        }
        response.gas_used = result.gas_used.parse().context("invalid VM gas usage")?;
        response.exit_code = result.vm_exit_code;
        response.stack = stack::output(&stack::decode(&result.stack)?)?;
        return Ok(response);
    }
    unreachable!("library retries return an error when exhausted")
}

/// Preserves the checkpoint's newest-first block windows in the TVM c7 context.
/// The hundred-block window is supplied only for network versions that support it.
pub(super) fn previous_blocks(context: &MasterchainContext) -> Result<PrevBlocksInfo> {
    let convert = |id: &BlockId| PrevBlockId {
        workchain: id.shard.workchain(),
        shard: id.shard.prefix() as i64,
        seqno: id.seqno,
        root_hash: id.root_hash.0,
        file_hash: id.file_hash.0,
    };
    Ok(PrevBlocksInfo::new(
        context.last_mc_blocks.iter().map(convert).collect(),
        convert(&context.last_key_block),
        (context.config.get_global_version()?.version >= 9)
            .then(|| context.last_mc_blocks_100.iter().map(convert).collect()),
    ))
}
