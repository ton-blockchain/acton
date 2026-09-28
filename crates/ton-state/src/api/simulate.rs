//! Request-local transaction emulation over one committed database checkpoint.

#[cfg(test)]
mod tests;

mod response;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::{Engine, engine::general_purpose::STANDARD};
use rston::boc::Boc;
use rston::cell::{HashBytes, Lazy};
use rston::dict::Dict;
use rston::models::{IntAddr, Message, MsgInfo, OptionalAccount, ShardAccount};
use serde::Deserialize;
use ton_emulator::emulator::{Emulator, SendMessageResult, SendMessageResultSuccess};
use ton_emulator::world_state::{AccountsState, LocalAccountsState, WorldState};
use ton_executor::{ExecutorVerbosity, MissingLibrariesContext, missing_library_callback};
use ton_node_db::{StateSnapshot, StateView};
use ton_p2p::ExternalMessage;
use tracing::{info, warn};

use super::{Api, ApiError, get_method::previous_blocks};
use response::{SimulationResponse, TransactionRejection};

const MAX_TRANSACTIONS: usize = 128;
const MAX_DEPTH: usize = 32;
const MAX_LIBRARY_LOADS: usize = 64;
const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const TIME_BUDGET: Duration = Duration::from_secs(10);

/// TON Center emulation inputs supported by this node. Optional enrichment
/// flags are accepted only when false; unsupported features never silently vanish.
#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SimulationRequest {
    /// Single inbound external message, encoded as a standard base64 `BoC`
    boc: String,
    /// Bypass signature checks inside TVM; all other contract checks still run
    #[serde(default)]
    #[schema(default = false)]
    ignore_chksig: bool,
    /// Include deduplicated code/data `BoCs`, keyed by their base64 hashes
    #[serde(default)]
    #[schema(default = false)]
    include_code_data: bool,
    /// Must equal the current applied masterchain checkpoint; omitted uses latest
    mc_block_seqno: Option<u32>,
    /// Action classification is unavailable; only false is supported
    #[serde(default)]
    #[schema(default = false)]
    with_actions: bool,
    /// Address-book enrichment is unavailable; only false is supported
    #[serde(default)]
    #[schema(default = false)]
    include_address_book: bool,
    /// Metadata enrichment is unavailable; only false is supported
    #[serde(default)]
    #[schema(default = false)]
    include_metadata: bool,
}

/// Simulate a message trace
///
/// Executes an external message and its internal messages, including bounces,
/// without broadcasting or changing node state. Returns a TON Center-style
/// emulation response directly, without an ok/result envelope. Account states,
/// libraries, config and previous blocks come from one applied checkpoint.
/// Time is fixed at the checkpoint's masterchain time; the random seed is its
/// state root hash. Internal messages run in breadth-first order. These choices
/// make requests reproducible but cannot predict future block ordering or time.
///
/// Limits: 128 transactions, 32 levels including the root, and 10 seconds checked
/// between native executions. Truncated traces set `is_incomplete`. A single native
/// execution finishes under the network's gas limits and cannot be interrupted.
/// Transactions have emulated=true and finality=pending; `block_ref` shard/seqno
/// are zero placeholders. No actions, address book or metadata are returned
#[utoipa::path(
    post,
    path = "/api/simulate",
    operation_id = "simulate",
    request_body = SimulationRequest,
    responses(
        (status = 200, description = "Emulated trace, possibly incomplete", body = SimulationResponse),
        (status = 400, description = "Invalid request or unsupported enrichment", body = toncenter::v2::TonlibErrorResponse),
        (status = 409, description = "Requested checkpoint is unavailable", body = toncenter::v2::TonlibErrorResponse),
        (status = 413, description = "Message exceeds 65,535 decoded bytes or HTTP body limit", body = toncenter::v2::TonlibErrorResponse),
        (status = 422, description = "Message rejected by the emulator", body = TransactionRejection),
        (status = 429, description = "Native execution slot is busy", body = toncenter::v2::TonlibErrorResponse),
        (status = 500, description = "State read, response limit or execution infrastructure failed", body = toncenter::v2::TonlibErrorResponse),
        (status = 504, description = "Budget expired before the first transaction", body = toncenter::v2::TonlibErrorResponse),
    ),
)]
pub(super) async fn simulate(
    State(api): State<Api>,
    request: Result<Json<SimulationRequest>, JsonRejection>,
) -> Response {
    let request = match request {
        Ok(Json(request)) => request,
        Err(error) => {
            let status = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
                StatusCode::PAYLOAD_TOO_LARGE
            } else {
                StatusCode::BAD_REQUEST
            };
            return ApiError::new(status, "invalid simulate JSON body").into_response();
        }
    };
    if request.with_actions || request.include_address_book || request.include_metadata {
        return ApiError::new(
            StatusCode::BAD_REQUEST,
            "actions, address book and metadata are not supported",
        )
        .into_response();
    }
    if request.boc.len() > ExternalMessage::MAX_BYTES.div_ceil(3) * 4 {
        return ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, "message exceeds 65535 bytes")
            .into_response();
    }
    let Ok(permit) = api.execution_slot.clone().try_acquire_owned() else {
        return ApiError::new(StatusCode::TOO_MANY_REQUESTS, "native executor is busy")
            .into_response();
    };
    let snapshot = api.state.borrow().clone();
    let started = Instant::now();
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        // Native work owns admission even when the client disconnects.
        let _permit = permit;
        let bytes = STANDARD
            .decode(&request.boc)
            .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "invalid message base64"))?;
        let message = ExternalMessage::new(bytes).map_err(|_| {
            ApiError::new(StatusCode::BAD_REQUEST, "invalid inbound external message")
        })?;
        if request
            .mc_block_seqno
            .is_some_and(|seqno| seqno != snapshot.head().seqno)
        {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "only the current applied masterchain seqno is available",
            )
            .into());
        }
        let target = message.destination().to_string();
        info!(operation = "simulate", %target, mc_seqno = snapshot.head().seqno,
            duration_ms = started.elapsed().as_millis(), outcome = "started",
            "emulating message trace");
        let response = execute(&snapshot, &message, &request, started)?;
        let bytes = serde_json::to_vec(&response)?;
        ensure!(
            bytes.len() <= MAX_RESPONSE_BYTES,
            "simulation response exceeds 16 MiB"
        );
        info!(operation = "simulate", %target, duration_ms = started.elapsed().as_millis(),
            transactions = response.transactions.len(), incomplete = response.is_incomplete,
            outcome = "completed", "emulated message trace");
        Ok(bytes)
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(std::convert::identity);

    match result {
        Ok(bytes) => ([(header::CONTENT_TYPE, "application/json")], bytes).into_response(),
        Err(error) => {
            if let Some(error) = error.downcast_ref::<ApiError>() {
                return error.clone().into_response();
            }
            if let Some(error) = error.downcast_ref::<TransactionRejection>() {
                info!(
                    operation = "simulate",
                    target = "native_executor",
                    duration_ms = started.elapsed().as_millis(),
                    outcome = "rejected",
                    vm_exit_code = error.vm_exit_code,
                    "message did not produce a transaction"
                );
                return (StatusCode::UNPROCESSABLE_ENTITY, Json(error.clone())).into_response();
            }
            warn!(operation = "simulate", target = "native_executor",
                duration_ms = started.elapsed().as_millis(), outcome = "failed",
                error = %format!("{error:#}"), "could not simulate trace");
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "simulation failed; see service logs",
            )
            .into_response()
        }
    }
}

/// One completed node; parent indices refer only to earlier completed nodes.
struct Executed {
    parent: Option<usize>,
    workchain: i32,
    result: SendMessageResultSuccess,
}

/// A FIFO makes repeated deliveries to an account observe earlier local changes.
/// Only executed messages enter the returned tree; omitted branches remain visible
/// in their parents' `out_msgs`, accompanied by the response's incomplete flag.
fn execute(
    snapshot: &StateSnapshot,
    message: &ExternalMessage,
    request: &SimulationRequest,
    started: Instant,
) -> Result<SimulationResponse> {
    let master = snapshot.masterchain_state()?;
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
    let previous = previous_blocks(&context)?;
    let mut world = WorldState::new(
        AccountsState::Local(LocalAccountsState::new()),
        Some(&config),
    )?;
    world.set_now(master.gen_utime()?);
    world.set_random_seed(Some(master.root_hash().0));
    world.set_ignore_chksig(request.ignore_chksig);
    let emulator = Emulator::new(ExecutorVerbosity::Off, Some(&config))?;
    let mut lt = master.gen_lt()?;
    let mut library_loads = 0;
    let mut pending = VecDeque::from([(message.root().clone(), None, 0_usize)]);
    let mut executed = Vec::new();
    let mut incomplete = false;

    while let Some((cell, parent, depth)) = pending.pop_front() {
        if started.elapsed() >= TIME_BUDGET {
            incomplete = true;
            break;
        }
        let parsed = cell.parse::<Message>()?;
        let (destination, created_lt) = match parsed.info {
            MsgInfo::ExtIn(info) => (info.dst, 0),
            MsgInfo::Int(info) => (info.dst, info.created_lt),
            MsgInfo::ExtOut(_) => unreachable!("external outputs are not queued"),
        };
        let IntAddr::Std(address) = destination else {
            anyhow::bail!("unsupported variable destination address");
        };
        ensure!(address.anycast.is_none(), "unsupported anycast destination");
        if !world.state().accounts().contains_key(&address) {
            let account = snapshot
                .get_account(&address)?
                .account
                .unwrap_or(ShardAccount {
                    account: Lazy::new(&OptionalAccount(None))?,
                    last_trans_hash: HashBytes::ZERO,
                    last_trans_lt: 0,
                });
            world.update_account(&address, &account);
        }
        // Advance beyond both the checkpoint and incoming message/account LT.
        // Leave the same logical-time interval that the Acton emulator uses.
        lt = lt
            .max(created_lt)
            .max(world.get_account(&address).last_trans_lt)
            .checked_add(1_000_000)
            .context("simulation logical time overflow")?;
        let mut prepared =
            Emulator::prepare_send_transaction(&mut world, cell, &Dict::new(), None)?;
        prepared.run_args.lt = lt;
        prepared.run_args.prev_blocks_info = Some(previous.clone());
        prepared.run_args.debug_enabled = false;
        let Some(result) = execute_transaction(
            &emulator,
            &mut world,
            prepared,
            &master,
            &mut library_loads,
            started,
        )?
        else {
            incomplete = true;
            break;
        };
        let result = match result {
            SendMessageResult::Success(result) => result,
            SendMessageResult::Error(error) => {
                return Err(TransactionRejection {
                    error: if error.external_not_accepted {
                        "external message was not accepted"
                    } else {
                        "transaction emulation failed"
                    }
                    .into(),
                    vm_exit_code: error.vm_exit_code,
                }
                .into());
            }
        };

        let index = executed.len();
        for cell in result.transaction.out_msgs.values() {
            let cell = cell?;
            if !matches!(cell.parse::<Message>()?.info, MsgInfo::Int(_)) {
                continue;
            }
            if depth + 1 >= MAX_DEPTH || index + 1 + pending.len() >= MAX_TRANSACTIONS {
                incomplete = true;
            } else {
                pending.push_back((cell, Some(index), depth + 1));
            }
        }
        executed.push(Executed {
            parent,
            workchain: address.workchain.into(),
            result,
        });
    }
    if executed.is_empty() {
        return Err(ApiError::new(StatusCode::GATEWAY_TIMEOUT, "simulation budget expired").into());
    }
    response::build(
        executed,
        snapshot.head().seqno,
        message.hash(),
        master.root_hash(),
        incomplete,
        request.include_code_data,
    )
}

/// Library retries use the original account, message, time and seed. Finalization
/// commits to the request-local world only after all available libraries resolve.
fn execute_transaction(
    emulator: &Emulator,
    world: &mut WorldState,
    mut prepared: ton_emulator::emulator::PreparedSendTransaction,
    master: &StateView,
    library_loads: &mut usize,
    started: Instant,
) -> Result<Option<SendMessageResult>> {
    let mut libraries = prepared
        .run_args
        .libs
        .as_ref()
        .map(Boc::decode_base64)
        .transpose()?
        .map_or_else(Dict::new, |root| {
            Dict::<HashBytes, rston::models::LibDescr>::from_raw(Some(root))
        });
    loop {
        if started.elapsed() >= TIME_BUDGET {
            return Ok(None);
        }
        let mut missing = MissingLibrariesContext::default();
        emulator
            .executor
            .register_missing_library_callback(&mut missing, missing_library_callback)?;
        let (result, _) = emulator
            .executor
            .run_transaction(&prepared.message_b64, &prepared.run_args)
            // The executor's parse error can contain its entire JSON output,
            // including message payloads. Never propagate that output into logs.
            .map_err(|_| anyhow::anyhow!("native transaction execution failed"))?;
        let missing = missing.into_set();
        let mut loaded = false;
        for hash in &missing {
            let hash: HashBytes = hash.parse().context("invalid missing-library hash")?;
            if libraries.contains_key(hash)? {
                continue;
            }
            if let Some(cell) = master.get_library(&hash)? {
                ensure!(
                    *library_loads < MAX_LIBRARY_LOADS,
                    "simulation library limit exceeded"
                );
                *library_loads += 1;
                world.register_lib(cell.clone());
                let mut publishers = Dict::new();
                publishers.set(HashBytes::ZERO, ())?;
                libraries.set(
                    hash,
                    rston::models::LibDescr {
                        lib: cell,
                        publishers,
                    },
                )?;
                loaded = true;
            }
        }
        if !loaded {
            return Emulator::finalize_send_transaction(world, prepared, result, None, missing)
                .map(Some);
        }
        prepared.run_args.libs = libraries.root().as_ref().map(Boc::encode_base64);
    }
}
