//! Account history from retained blocks, using the TON Center v2 wire types.

#[cfg(test)]
mod tests;

use anyhow::{Context, Result, ensure};
use axum::extract::rejection::QueryRejection;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE},
};
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, HashBytes, Lazy};
use rston::models::{Message, MsgInfo, StdAddr, StdAddrFormat, Transaction, TxInfo};
use ton_node_db::BlockIndex;
use toncenter::v2::{self as v2, requests::TransactionsRequest, responses as wire};

use super::{Api, ApiError};

/// Account transactions
///
/// Read retained account history newest-first. Supply both lt and hash to start
/// at that transaction, inclusively. Otherwise start at the applied account state
///
/// Supports limit 1–100 (default 10). Nonzero `to_lt` and `archival=true` are unsupported
/// Missing starting history returns 404; a gap after some results returns a shorter
/// page. Message bodies are returned as raw `BoCs` without comment decoding
#[utoipa::path(
    get,
    path = "/api/transactions",
    operation_id = "transactions",
    params(
        ("address" = String, Query, description = "Raw or user-friendly account address", example = "-1:3333333333333333333333333333333333333333333333333333333333333333"),
        ("limit" = Option<u32>, Query, minimum = 1, maximum = 100, description = "Maximum number of transactions; defaults to 10"),
        ("lt" = Option<String>, Query, description = "Inclusive starting logical time; requires hash"),
        ("hash" = Option<String>, Query, description = "Starting transaction hash in hex or base64; requires lt"),
    ),
    responses(
        (status = 200, description = "Retained transactions, newest-first", body = v2::TonlibResponse<Vec<wire::Transaction>>),
        (status = 400, description = "Invalid cursor, query or unsupported filter", body = v2::TonlibErrorResponse),
        (status = 404, description = "Starting transaction is not retained locally", body = v2::TonlibErrorResponse),
        (status = 409, description = "Cursor is newer than the applied account state", body = v2::TonlibErrorResponse),
        (status = 500, description = "History read failed", body = v2::TonlibErrorResponse),
    ),
)]
pub(super) async fn get_transactions(
    State(api): State<Api>,
    query: Result<Query<TransactionsRequest>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return ApiError::new(StatusCode::BAD_REQUEST, "invalid query parameters").into_response();
    };
    let query = match HistoryQuery::parse(query) {
        Ok(query) => query,
        Err(error) => return error.into_response(),
    };
    let index = api.history.clone();

    super::read(api, "transactions", move |snapshot, _| {
        let account = snapshot.get_account(&query.address)?;
        let latest = account.account.map_or((0, HashBytes::ZERO), |account| {
            (account.last_trans_lt, account.last_trans_hash)
        });

        read_history(
            &index,
            &query.address,
            query.cursor.unwrap_or(latest),
            query.limit,
            account.shard_block.seqno,
        )
    })
    .await
}

struct HistoryQuery {
    address: StdAddr,
    limit: usize,
    cursor: Option<(u64, HashBytes)>,
}

impl HistoryQuery {
    fn parse(query: TransactionsRequest) -> Result<Self, ApiError> {
        let invalid = || ApiError::new(StatusCode::BAD_REQUEST, "invalid transaction query");
        let integer = |value: v2::Int64Input| match value {
            v2::Int64Input::String(value) => value.parse::<u64>().map_err(|_| invalid()),
            v2::Int64Input::Number(value) => u64::try_from(value).map_err(|_| invalid()),
        };
        let (address, _) =
            StdAddr::from_str_ext(&query.address, StdAddrFormat::any()).map_err(|_| invalid())?;
        let limit = query.limit.map(integer).transpose()?.unwrap_or(10);
        if !(1..=100).contains(&limit) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "limit must be between 1 and 100",
            ));
        }
        if query.to_lt.map(integer).transpose()?.unwrap_or(0) != 0 {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "to_lt is not supported",
            ));
        }
        match query.archival {
            None | Some(v2::BoolInput::Bool(false) | v2::BoolInput::Number(0)) => {}
            Some(v2::BoolInput::String(value)) if value == "false" || value == "0" => {}
            _ => {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "archival is not supported",
                ));
            }
        }

        let cursor = match (query.lt, query.hash) {
            (None, None) => None,
            (Some(lt), Some(hash)) => {
                let hash = hash
                    .parse::<HashBytes>()
                    .ok()
                    .or_else(|| {
                        STANDARD
                            .decode(&hash)
                            .or_else(|_| URL_SAFE.decode(&hash))
                            .ok()
                            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
                            .map(HashBytes)
                    })
                    .ok_or_else(invalid)?;
                let lt = integer(lt)?;
                if lt == 0 {
                    return Err(invalid());
                }
                Some((lt, hash))
            }
            _ => {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "lt and hash must be supplied together",
                ));
            }
        };

        Ok(Self {
            address,
            limit: limit as usize,
            cursor,
        })
    }
}

fn read_history(
    index: &BlockIndex,
    address: &StdAddr,
    (mut lt, mut hash): (u64, HashBytes),
    limit: usize,
    max_block_seqno: u32,
) -> Result<Vec<wire::Transaction>> {
    let mut reader = index.reader();
    let mut result = Vec::with_capacity(limit);

    while lt != 0 && result.len() < limit {
        let Some(found) = reader.get(address, lt)? else {
            if result.is_empty() {
                return Err(ApiError::new(
                    StatusCode::NOT_FOUND,
                    "transaction history is not retained locally",
                )
                .into());
            }
            break;
        };
        if found.block.seqno > max_block_seqno {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "cursor is newer than the applied shard checkpoint",
            )
            .into());
        }
        if found.transaction.inner().repr_hash() != &hash {
            if result.is_empty() {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "transaction cursor hash mismatch",
                )
                .into());
            }
            anyhow::bail!("transaction predecessor hash mismatch for {address} at {lt}");
        }
        let tx = found.transaction.load()?;
        ensure!(
            tx.account == address.address && tx.lt == lt,
            "transaction dictionary key mismatch"
        );
        ensure!(
            tx.prev_trans_lt < tx.lt,
            "transaction predecessor LT must decrease"
        );
        result.push(convert(address, &found.transaction, &tx)?);
        lt = tx.prev_trans_lt;
        hash = tx.prev_trans_hash;
    }

    Ok(result)
}

/// Maps a retained transaction to the shared v2 history and submission response.
pub(crate) fn convert(
    address: &StdAddr,
    cell: &Lazy<Transaction>,
    tx: &Transaction,
) -> Result<wire::Transaction> {
    let storage: u128 = match tx.load_info()? {
        TxInfo::Ordinary(info) => info
            .storage_phase
            .map_or(0, |phase| phase.storage_fees_collected.into()),
        TxInfo::TickTock(info) => info.storage_phase.storage_fees_collected.into(),
    };
    let out_msgs = tx
        .out_msgs
        .values()
        .map(|cell| message(&cell?))
        .collect::<Result<Vec<_>>>()?;
    // TONLib's fee includes the forwarding fees carried by outgoing messages.
    let mut fee = tx.total_fees.tokens.into_inner();
    for message in &out_msgs {
        let forwarding = message.fwd_fee.parse::<u128>()?;
        fee = fee
            .checked_add(forwarding)
            .context("transaction fee overflow")?;
    }
    let other_fee = fee
        .checked_sub(storage)
        .context("storage fee exceeds total fee")?;

    Ok(wire::Transaction {
        type_tag: Default::default(),
        address: wire::AccountAddress {
            type_tag: Default::default(),
            account_address: address.display_base64(true).to_string(),
        },
        account: address.to_string(),
        utime: i64::from(tx.now),
        data: Boc::encode_base64(cell.inner()),
        transaction_id: wire::InternalTransactionId {
            type_tag: Default::default(),
            lt: tx.lt.to_string(),
            hash: STANDARD.encode(cell.inner().repr_hash()),
        },
        fee: fee.to_string(),
        storage_fee: storage.to_string(),
        other_fee: other_fee.to_string(),
        in_msg: tx.in_msg.as_ref().map(message).transpose()?,
        out_msgs,
    })
}

fn message(cell: &Cell) -> Result<wire::Message> {
    let message = cell.parse::<Message>()?;
    let body = CellBuilder::build_from(message.body)?;
    let init = message
        .init
        .as_ref()
        .map(CellBuilder::build_from)
        .transpose()?;
    let mut result = wire::Message {
        type_tag: Default::default(),
        hash: STANDARD.encode(cell.repr_hash()),
        source: String::new(),
        destination: String::new(),
        value: "0".into(),
        extra_currencies: Vec::new(),
        fwd_fee: "0".into(),
        ihr_fee: "0".into(),
        created_lt: "0".into(),
        body_hash: STANDARD.encode(body.repr_hash()),
        msg_data: wire::MsgData::MsgDataRaw(Box::new(wire::MsgDataRaw {
            type_tag: Default::default(),
            body: Some(Boc::encode_base64(&body)),
            init_state: Some(init.as_ref().map(Boc::encode_base64).unwrap_or_default()),
        })),
        message: None,
        message_decode_error: None,
    };
    match message.info {
        MsgInfo::Int(info) => {
            result.source = info.src.to_string();
            result.destination = info.dst.to_string();
            result.value = info.value.tokens.to_string();
            result.fwd_fee = info.fwd_fee.to_string();
            result.created_lt = info.created_lt.to_string();
            result.extra_currencies = info
                .value
                .other
                .as_dict()
                .iter()
                .map(|entry| {
                    let (id, amount) = entry?;
                    Ok(wire::ExtraCurrencyBalance {
                        type_tag: Default::default(),
                        id: i32::from_be_bytes(id.to_be_bytes()),
                        amount: amount.to_string(),
                    })
                })
                .collect::<Result<_>>()?;
        }
        MsgInfo::ExtIn(info) => result.destination = info.dst.to_string(),
        MsgInfo::ExtOut(info) => {
            result.source = info.src.to_string();
            result.created_lt = info.created_lt.to_string();
        }
    }
    Ok(result)
}
