#[cfg(test)]
mod tests;

pub(crate) mod get_method;
pub(crate) mod simulate;
pub(crate) mod transactions;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::{Engine, engine::general_purpose::STANDARD};
use rston::boc::Boc;
use rston::models::{AccountState, BlockId, StdAddr, StdAddrFormat};
use serde::Serialize;
use tokio::sync::{Semaphore, watch};
use ton_node_db::{AccountSnapshot, BlockIndex, StateSnapshot};
use toncenter::v2::requests::AddressInformationRequest;
use toncenter::v2::{self as v2, responses as wire};
use tracing::{debug, error};

#[derive(Clone)]
struct Api {
    state: watch::Receiver<StateSnapshot>,
    zero_state: BlockId,
    history: Arc<BlockIndex>,
    execution_slot: Arc<Semaphore>,
}

/// Each request pins a complete committed frontier before dispatching its read.
/// State application runs independently of account lookup and serialization.
pub(crate) fn router(
    state: watch::Receiver<StateSnapshot>,
    zero_state: BlockId,
    history: Arc<BlockIndex>,
) -> Router {
    Router::new()
        .route("/api/masterchainInfo", get(masterchain_info))
        .route("/api/account", get(address_information))
        .route("/api/runGetMethod", post(get_method::run_get_method))
        .route("/api/simulate", post(simulate::simulate))
        .route("/api/transactions", get(transactions::get_transactions))
        .fallback(|| async { ApiError::new(StatusCode::NOT_FOUND, "unknown API method") })
        .with_state(Api {
            state,
            zero_state,
            history,
            // The native emulator changes process-global logging state. Keep
            // executions serial and reject overload instead of queuing work.
            execution_slot: Arc::new(Semaphore::new(1)),
        })
}

/// Applied masterchain checkpoint
///
/// Read the last fully applied masterchain checkpoint and network zerostate
#[utoipa::path(
    get,
    path = "/api/masterchainInfo",
    operation_id = "masterchainInfo",
    responses(
        (status = 200, description = "Applied checkpoint", body = v2::TonlibResponse<wire::MasterchainInfo>),
        (status = 500, description = "State read failed", body = v2::TonlibErrorResponse),
    ),
)]
async fn masterchain_info(State(api): State<Api>) -> Response {
    read(api, "masterchainInfo", |store, zero_state| {
        let state = store.masterchain_state()?;

        Ok(wire::MasterchainInfo {
            type_tag: Default::default(),
            last: block_id(state.block_id()),
            state_root_hash: STANDARD.encode(state.root_hash()),
            init: block_id(zero_state),
        })
    })
    .await
}

/// Account information
///
/// Read account state from one applied checkpoint, including balance in nanograms
/// and code/data as base64 `BoCs`. The suspended field is currently always false
#[utoipa::path(
    get,
    path = "/api/account",
    operation_id = "account",
    params(
        ("address" = String, Query, description = "Raw or user-friendly account address", example = "-1:3333333333333333333333333333333333333333333333333333333333333333"),
        ("seqno" = Option<u32>, Query, description = "Must equal the current applied checkpoint; omit for the latest applied state", maximum = 2147483647),
    ),
    responses(
        (status = 200, description = "Account state; absent accounts are uninitialized with zero balance", body = v2::TonlibResponse<wire::AddressInformation>),
        (status = 400, description = "Invalid address or query", body = v2::TonlibErrorResponse),
        (status = 409, description = "Requested checkpoint is unavailable", body = v2::TonlibErrorResponse),
        (status = 500, description = "State read failed", body = v2::TonlibErrorResponse),
    ),
)]
async fn address_information(
    State(api): State<Api>,
    query: Result<Query<AddressInformationRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return ApiError::new(StatusCode::BAD_REQUEST, "invalid query parameters").into_response();
    };
    let Ok((address, _)) = StdAddr::from_str_ext(&query.address, StdAddrFormat::any()) else {
        return ApiError::new(StatusCode::BAD_REQUEST, "invalid account address").into_response();
    };

    read(api, "account", move |store, _| {
        if let Some(seqno) = query.seqno {
            let seqno = match seqno {
                v2::Int32Input::Number(value) => Some(value),
                v2::Int32Input::String(value) => value.parse::<i32>().ok(),
            }
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "invalid masterchain seqno"))?;

            if seqno != store.head().seqno {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "only the current applied masterchain seqno is available",
                )
                .into());
            }
        }

        let snapshot = store.get_account(&address)?;
        account_info(snapshot)
    })
    .await
}

/// Maps the raw account without interpreting its contract code. Missing
/// dictionary entries have the same zero-balance representation as `account_none`.
pub(crate) fn account_info(snapshot: AccountSnapshot) -> Result<wire::AddressInformation> {
    let mut info = wire::AddressInformation {
        type_tag: Default::default(),
        balance: "0".into(),
        extra_currencies: Vec::new(),
        last_transaction_id: wire::InternalTransactionId {
            type_tag: Default::default(),
            lt: "0".into(),
            hash: STANDARD.encode([0_u8; 32]),
        },
        block_id: block_id(snapshot.masterchain_block),
        code: String::new(),
        data: String::new(),
        frozen_hash: String::new(),
        sync_utime: i64::from(snapshot.gen_utime),
        state: wire::AccountStateEnum::Uninitialized,
        // TODO: Read account suspension from masterchain config parameter 44.
        suspended: false,
    };
    let Some(shard_account) = snapshot.account else {
        return Ok(info);
    };
    info.last_transaction_id.lt = shard_account.last_trans_lt.to_string();
    info.last_transaction_id.hash = STANDARD.encode(shard_account.last_trans_hash);

    let Some(account) = shard_account.load_account()? else {
        return Ok(info);
    };
    info.balance = account.balance.tokens.to_string();
    for currency in account.balance.other.as_dict().iter() {
        let (id, amount) = currency?;
        info.extra_currencies.push(wire::ExtraCurrencyBalance {
            type_tag: Default::default(),
            id: id as i32,
            amount: amount.to_string(),
        });
    }

    match account.state {
        AccountState::Uninit => {}
        AccountState::Active(state) => {
            info.state = wire::AccountStateEnum::Active;
            info.code = state.code.map(Boc::encode_base64).unwrap_or_default();
            info.data = state.data.map(Boc::encode_base64).unwrap_or_default();
        }
        AccountState::Frozen(hash) => {
            info.state = wire::AccountStateEnum::Frozen;
            info.frozen_hash = STANDARD.encode(hash);
        }
    }

    Ok(info)
}

pub(crate) fn block_id(id: BlockId) -> wire::TonBlockIdExt {
    wire::TonBlockIdExt {
        type_tag: Default::default(),
        workchain: i64::from(id.shard.workchain()),
        shard: (id.shard.prefix() as i64).to_string(),
        seqno: i64::from(id.seqno),
        root_hash: STANDARD.encode(id.root_hash),
        file_hash: STANDARD.encode(id.file_hash),
    }
}

async fn read<T: Serialize + Send + 'static>(
    api: Api,
    method: &'static str,
    query: impl FnOnce(&StateSnapshot, BlockId) -> Result<T> + Send + 'static,
) -> Response {
    let started = Instant::now();
    let snapshot = api.state.borrow().clone();
    let result = tokio::task::spawn_blocking(move || query(&snapshot, api.zero_state))
        .await
        .map_err(anyhow::Error::from)
        .and_then(std::convert::identity);

    match result {
        Ok(result) => {
            debug!(
                operation = "http_query",
                target = method,
                duration_ms = started.elapsed().as_millis(),
                outcome = "success",
                "read the applied state",
            );
            Json(v2::TonlibResponse {
                ok: true,
                result,
                extra: String::new(),
                jsonrpc: None,
                id: None,
            })
            .into_response()
        }
        Err(error) => {
            if let Some(error) = error.downcast_ref::<ApiError>() {
                return error.clone().into_response();
            }

            error!(
                operation = "http_query",
                target = method,
                duration_ms = started.elapsed().as_millis(),
                outcome = "failed",
                error = %format!("{error:#}"),
                "could not read the applied state",
            );
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "state query failed; see service logs",
            )
            .into_response()
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: &'static str,
}

impl ApiError {
    pub(crate) const fn new(status: StatusCode, message: &'static str) -> Self {
        Self { status, message }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(v2::TonlibErrorResponse {
                ok: false,
                error: self.message.into(),
                code: i32::from(self.status.as_u16()),
                extra: None,
                jsonrpc: None,
                id: None,
            }),
        )
            .into_response()
    }
}
