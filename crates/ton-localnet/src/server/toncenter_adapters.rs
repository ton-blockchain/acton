//! HTTP request adapters that intentionally differ from `ton_api::toncenter`.
//!
//! Canonical `OpenAPI` request types live in `ton-api`. These adapters exist only where the
//! current localnet transport accepts a narrower or differently encoded query. Keep every
//! deviation documented on the adapter so it cannot be mistaken for the `TonCenter` contract.
//! Response-side deviations are documented in `api::toncenter_v2` and `api::toncenter_v3`.
//!
//! Emulate v1 currently implements only `/emulateTrace`; the published `/emulateTonConnect`
//! operation is not routed by localnet yet.
//!
//! Streaming v2 uses the canonical wire types, but localnet behavior is narrower: subscribing to
//! `trace_invalidated` is rejected and no invalidation notifications are emitted; action
//! classification is not implemented, so an `actions` subscription currently emits an empty list
//! only when `action_types` is omitted.

use serde::{Deserialize, Serialize};
use ton_api::toncenter::streaming::v2 as streaming;

/// Localnet accepts an optional correlation `id` in the SSE body and echoes it in status/error
/// responses. The official SSE request is exactly `streaming::Subscription` and documents no id;
/// correlation ids belong to the WebSocket protocol.
#[derive(Deserialize)]
pub(super) struct StreamingSseSubscriptionAdapter {
    pub id: Option<String>,
    #[serde(flatten)]
    pub subscription: streaming::Subscription,
}

/// REST encodes all hashes in one comma-separated string; `OpenAPI` `LibrariesRequest` uses an
/// array of hash strings.
/// Localnet supports one `tx_hash` or `msg_hash` and the non-standard `hash` alias.
/// `OpenAPI` `TracesQuery` accepts repeated `account`, `trace_id`, `tx_hash`, and `msg_hash`
/// values plus range, action, pagination, and sorting filters.
#[derive(Deserialize)]
pub struct LibrariesRestQuery {
    pub libraries: String,
}

/// Localnet resolves block-oriented v2 calls by `seqno` alone and ignores optional workchain and
/// shard hints. `OpenAPI` uses operation-specific requests such as `BlockHeaderRequest`, where
/// `workchain`, `shard`, and `seqno` are required and root/file hashes may also be supplied.
#[derive(Deserialize)]
pub struct BlockQueryAdapter {
    #[allow(dead_code)]
    pub workchain: Option<i32>,
    #[allow(dead_code)]
    pub shard: Option<String>,
    pub seqno: i32,
}

/// Localnet accepts at most one `account` and `exclude_account` value.
/// `OpenAPI` `TransactionsQuery` models both fields as repeated arrays; all other fields match.
#[derive(Deserialize)]
pub struct TracesQueryAdapter {
    #[serde(alias = "hash")]
    pub tx_hash: Option<String>,
    pub msg_hash: Option<String>,
}

/// Localnet accepts at most one account and trace id.
/// `OpenAPI` `PendingTransactionsQuery` models both fields as repeated arrays.
#[derive(Deserialize)]
pub struct TransactionsQueryAdapter {
    pub workchain: Option<i32>,
    pub shard: Option<String>,
    pub seqno: Option<u32>,
    pub mc_seqno: Option<u32>,
    pub account: Option<String>,
    pub exclude_account: Option<String>,
    pub hash: Option<String>,
    pub lt: Option<u64>,
    pub start_utime: Option<u32>,
    pub end_utime: Option<u32>,
    pub start_lt: Option<u64>,
    pub end_lt: Option<u64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

/// Localnet accepts at most one address and admin address.
/// `OpenAPI` `JettonMastersQuery` models both filters as repeated arrays.
#[derive(Deserialize)]
pub struct PendingTransactionsQueryAdapter {
    pub account: Option<String>,
    pub trace_id: Option<String>,
}

#[derive(Deserialize)]
pub struct JettonMastersQueryAdapter {
    pub address: Option<String>,
    pub admin_address: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Localnet accepts one value for each address filter and does not expose `OpenAPI`'s `sort`.
/// `OpenAPI` `JettonWalletsQuery` models the three address filters as repeated arrays.
#[derive(Deserialize, Serialize)]
pub struct JettonWalletsQueryAdapter {
    pub address: Option<String>,
    pub owner_address: Option<String>,
    pub jetton_address: Option<String>,
    pub exclude_zero_balance: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Localnet accepts one value for each address/index filter.
/// `OpenAPI` `NftItemsQuery` models those four filters as repeated arrays.
#[derive(Deserialize, Serialize)]
pub struct NftItemsQueryAdapter {
    pub address: Option<String>,
    pub owner_address: Option<String>,
    pub collection_address: Option<String>,
    pub index: Option<String>,
    pub include_on_sale: Option<bool>,
    pub sort_by_last_transaction_lt: Option<bool>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
