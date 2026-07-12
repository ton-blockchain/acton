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

use serde::Deserialize;
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
