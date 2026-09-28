#[cfg(test)]
pub(crate) mod tests;

mod storage;
pub(crate) mod transaction;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::Infallible;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use futures::stream;
use rston::boc::Boc;
use rston::cell::{Cell, HashBytes};
use rston::models::{AccountState, StdAddr, StdAddrFormat};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};
use tolk_source_map::abi::ContractABI;
use ton_indexer_core::Batch;
use ton_node_db::{AccountSnapshot, StateSnapshot};
use tracing::{debug, warn};

use self::storage::StorageWatch;

const MAX_SUBSCRIBERS: usize = 64;
const MAX_ADDRESSES: usize = 100;
const QUEUE_EVENTS: usize = 32;
const QUEUE_BYTES: usize = 2 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 1024 * 1024;
const OPEN: u8 = 0;
const LAGGED: u8 = 1;
const FAILED: u8 = 2;
const CLOSED: u8 = 3;
const STORAGE_FAILED: u8 = 4;

/// Live subscriptions to batches committed by the P2P state synchronizer.
/// Only ABI schemas and field fingerprints survive storage publication. Each
/// connection owns a bounded queue; overflow terminates it with an explicit gap.
#[derive(Clone)]
pub(crate) struct Subscriptions {
    subscribers: Arc<Mutex<Vec<Subscriber>>>,
    connections: Arc<Semaphore>,
    state: Option<watch::Receiver<StateSnapshot>>,
}

struct Subscriber {
    addresses: HashSet<String>,
    types: Vec<SubscriptionType>,
    include_code_data: bool,
    sender: mpsc::Sender<QueuedEvent>,
    budget: Arc<Semaphore>,
    terminal: Arc<AtomicU8>,
    code_data: HashMap<String, CodeDataHashes>,
    storage: Option<StorageWatch>,
}

/// Cell identities, not retained contract payloads. A baseline advances only
/// after enqueueing an event; overflow closes the connection instead of skipping it.
#[derive(Clone, Copy, Default)]
struct CodeDataHashes {
    code: Option<HashBytes>,
    data: Option<HashBytes>,
}

struct QueuedEvent {
    json: Arc<str>,
    _bytes: OwnedSemaphorePermit,
}

struct Connection {
    receiver: mpsc::Receiver<QueuedEvent>,
    terminal: Arc<AtomicU8>,
    _slot: OwnedSemaphorePermit,
    first: bool,
    done: bool,
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self {
            subscribers: Arc::default(),
            connections: Arc::new(Semaphore::new(MAX_SUBSCRIBERS)),
            state: None,
        }
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct Subscription {
    /// Accounts to observe, in raw or user-friendly form
    #[schema(
        min_items = 1,
        max_items = 100,
        example = json!(["-1:3333333333333333333333333333333333333333333333333333333333333333"]),
    )]
    addresses: Vec<String>,
    /// Event types to receive; omitted or null defaults to `["transactions"]`
    #[serde(default)]
    #[schema(min_items = 1, example = json!(["transactions", "account_states"]))]
    types: Option<Vec<SubscriptionType>>,
    /// Only "finalized" is supported; omitted or null uses this default
    #[serde(default)]
    #[schema(example = "finalized")]
    min_finality: Option<String>,
    /// Include code/data in account state events; false omits both on every event.
    /// Omitted or null defaults to true. Transaction events are unaffected
    #[serde(default)]
    #[schema(default = true, example = false)]
    include_code_data: Option<bool>,
    /// Tolk ABI with a storage type; required only for `storage_fields` subscriptions.
    /// One ABI applies to all subscribed addresses, at most 16 for this event type
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    abi: Option<ContractABI>,
    /// Selected storage paths, for example seqno or settings.owner. Typed cells
    /// are transparent. Required with `storage_fields`; 1–64 unique paths
    #[serde(default)]
    #[schema(min_items = 1, max_items = 64, example = json!(["seqno", "settings.owner"]))]
    fields: Option<Vec<String>>,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
enum SubscriptionType {
    Transactions,
    AccountStates,
    StorageFields,
}

#[derive(Serialize)]
struct TransactionEvent<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    finality: &'static str,
    transaction: &'a toncenter::v3::responses::Transaction,
}

/// A changed account at the fully committed checkpoint. The state uses the same
/// wire contract as the HTTP query, except that unchanged code/data are omitted
/// after the first event for each account on this connection. Subscriptions with
/// `include_code_data: false` never receive either field.
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct AccountStateEvent {
    #[serde(rename = "type")]
    #[schema(example = "account_state")]
    kind: &'static str,
    #[schema(example = "finalized")]
    finality: &'static str,
    /// Canonical raw address, including its workchain
    address: String,
    /// HTTP account fields; missing code/data retain their previous values,
    /// while an empty string clears the respective field. With
    /// `include_code_data: false`, both fields are always absent
    #[schema(schema_with = account_state_schema)]
    account_state: serde_json::Map<String, serde_json::Value>,
}

/// A complete storage `BoC` gated by the selected ABI fields. Initial snapshots
/// have no changed fields. An empty data string means the account has no storage.
#[derive(Serialize, utoipa::ToSchema)]
pub(crate) struct StorageUpdateEvent<'a> {
    #[serde(rename = "type")]
    #[schema(example = "storage_update")]
    kind: &'static str,
    #[schema(example = "finalized")]
    finality: &'static str,
    address: &'a str,
    /// Masterchain checkpoint of the complete committed state
    mc_seqno: u32,
    /// Full original storage `BoC` in base64, or an empty string when absent
    data: String,
    /// True only for the first snapshot of this account on this connection
    initial: bool,
    /// Changed subscribed paths, in request order; empty for an initial snapshot
    changed_fields: Vec<String>,
}

// Keep the HTTP field types and nested schemas, changing only presence rules.
fn account_state_schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
    let mut schema =
        <toncenter::v2::responses::AddressInformation as utoipa::PartialSchema>::schema();
    if let utoipa::openapi::RefOr::T(utoipa::openapi::schema::Schema::Object(object)) = &mut schema
    {
        object
            .required
            .retain(|field| field != "code" && field != "data");
    }
    schema
}

impl Subscriptions {
    /// Initial storage snapshots are pinned while registration holds the same
    /// lock as publication. State synchronization can continue independently.
    pub(crate) fn new(state: watch::Receiver<StateSnapshot>) -> Self {
        Self {
            state: Some(state),
            ..Self::default()
        }
    }

    fn register(
        &self,
        mut subscriber: Subscriber,
        mut read_account: impl FnMut(&StdAddr) -> Result<AccountSnapshot>,
    ) -> Result<()> {
        let mut subscribers = self.subscribers.lock().expect("subscription lock poisoned");
        subscribers.retain(|subscriber| !subscriber.sender.is_closed());
        ensure!(!self.connections.is_closed(), "shutting_down");
        if subscriber.storage.is_some() {
            let mut addresses = subscriber.addresses.iter().cloned().collect::<Vec<_>>();
            addresses.sort();
            for address in addresses {
                let snapshot = read_account(&address.parse()?)?;
                let data = account_data(&snapshot)?;
                subscriber.send_storage(
                    &address,
                    snapshot.masterchain_block.seqno,
                    data.as_ref(),
                )?;
                ensure!(
                    subscriber.terminal.load(Ordering::Acquire) == OPEN,
                    "initial storage snapshots exceed queue capacity"
                );
            }
        }
        subscribers.push(subscriber);
        drop(subscribers);
        Ok(())
    }

    /// Serves live account and transaction events. The event envelope is local;
    /// it does not claim TON Center's trace-grouped streaming semantics.
    pub(crate) fn router(self) -> Router {
        Router::new()
            .route("/api/streaming/sse", post(subscribe))
            .with_state(self)
    }

    /// Publishes an already committed batch. The caller must never call this
    /// before applying all masterchain and shard state updates successfully.
    /// `read_account` must read the same immutable checkpoint as the batch. It is
    /// called once per changed account with interested state subscribers, after
    /// transaction events. Multiple transactions collapse into the final state.
    /// Read or encoding errors require [`Self::fail`] to report the delivery gap.
    pub(crate) fn publish(
        &self,
        batch: &Batch,
        mut read_account: impl FnMut(&StdAddr) -> Result<AccountSnapshot>,
    ) -> Result<()> {
        let started = Instant::now();
        let mut sent = 0;
        let mut changed_accounts = BTreeMap::new();
        {
            let mut subscribers = self.subscribers.lock().expect("subscription lock poisoned");
            subscribers.retain(|subscriber| !subscriber.sender.is_closed());
            if subscribers.is_empty() {
                return Ok(());
            }
        }

        for block in batch.blocks() {
            for lazy in block.transactions() {
                let tx = lazy.load().with_context(|| {
                    format!(
                        "cannot decode transaction {} in block {}",
                        lazy.inner().repr_hash(),
                        block.id()
                    )
                })?;
                let account = format!("{}:{}", block.id().workchain, tx.account);
                let (transactions, states) = {
                    let subscribers = self.subscribers.lock().expect("subscription lock poisoned");
                    let interested = |kind| {
                        subscribers.iter().any(|subscriber| {
                            subscriber.addresses.contains(&account)
                                && subscriber.types.contains(&kind)
                        })
                    };
                    (
                        interested(SubscriptionType::Transactions),
                        interested(SubscriptionType::AccountStates)
                            || interested(SubscriptionType::StorageFields),
                    )
                };
                if states {
                    changed_accounts.insert(
                        account.clone(),
                        StdAddr::new(block.id().workchain.try_into()?, tx.account),
                    );
                }
                if !transactions {
                    continue;
                }

                let tx = transaction::convert(block.id(), batch.checkpoint().seqno, lazy, &tx)
                    .with_context(|| {
                        format!(
                            "cannot map transaction {} in block {}",
                            lazy.inner().repr_hash(),
                            block.id()
                        )
                    })?;
                self.send(
                    &account,
                    SubscriptionType::Transactions,
                    &TransactionEvent {
                        kind: "transaction",
                        finality: "finalized",
                        transaction: &tx,
                    },
                )?;
                sent += 1;
            }
        }

        let mut states = 0;
        for (address, account) in changed_accounts {
            if !self
                .subscribers
                .lock()
                .expect("subscription lock poisoned")
                .iter()
                .any(|subscriber| {
                    !subscriber.sender.is_closed()
                        && subscriber.addresses.contains(&address)
                        && (subscriber.types.contains(&SubscriptionType::AccountStates)
                            || subscriber.storage.is_some())
                })
            {
                continue;
            }

            let snapshot = read_account(&account)
                .with_context(|| format!("cannot read streaming account {address}"))?;
            ensure!(
                snapshot.masterchain_block == batch.checkpoint().try_into()?,
                "streaming account {address} belongs to a different checkpoint"
            );
            let data = account_data(&snapshot)?;
            self.send_storage(&address, snapshot.masterchain_block.seqno, data.as_ref());
            let account_states = self
                .subscribers
                .lock()
                .expect("subscription lock poisoned")
                .iter()
                .any(|subscriber| {
                    subscriber.addresses.contains(&address)
                        && subscriber.types.contains(&SubscriptionType::AccountStates)
                });
            if !account_states {
                continue;
            }
            let mut hashes = CodeDataHashes::default();
            if let Some(shard_account) = &snapshot.account
                && let Some(account) = shard_account.load_account()?
                && let AccountState::Active(state) = account.state
            {
                hashes.code = state.code.as_ref().map(|cell| *cell.repr_hash());
                hashes.data = state.data.as_ref().map(|cell| *cell.repr_hash());
            }
            let mut event = AccountStateEvent {
                kind: "account_state",
                finality: "finalized",
                address,
                account_state: serde_json::from_value(serde_json::to_value(
                    crate::api::account_info(snapshot)?,
                )?)?,
            };
            self.send_account_state(&mut event, hashes)?;
            states += 1;
        }

        debug!(
            operation = "state_stream",
            target = %batch.checkpoint(),
            transactions = sent,
            account_states = states,
            duration_ms = started.elapsed().as_millis(),
            outcome = "published",
            "published committed account events",
        );
        Ok(())
    }

    fn send_storage(&self, address: &str, seqno: u32, data: Option<&Cell>) {
        let mut subscribers = self.subscribers.lock().expect("subscription lock poisoned");
        for subscriber in &mut *subscribers {
            if subscriber.sender.is_closed() || !subscriber.addresses.contains(address) {
                continue;
            }
            let started = Instant::now();
            if let Err(error) = subscriber.send_storage(address, seqno, data) {
                warn!(operation = "storage_stream", target = address, mc_seqno = seqno,
                    duration_ms = started.elapsed().as_millis(), outcome = "decode_failed",
                    error = %format!("{error:#}"), "ending storage subscription");
                let encoded = json!({"type": "error", "error": "storage_decode_failed",
                    "address": address, "mc_seqno": seqno})
                .to_string();
                if subscriber.enqueue(encoded.into()) {
                    subscriber.terminal.store(STORAGE_FAILED, Ordering::Release);
                }
            }
        }
        subscribers.retain(|subscriber| {
            !subscriber.sender.is_closed() && subscriber.terminal.load(Ordering::Acquire) == OPEN
        });
    }

    fn send_account_state(
        &self,
        event: &mut AccountStateEvent,
        hashes: CodeDataHashes,
    ) -> Result<()> {
        let code = event
            .account_state
            .remove("code")
            .context("missing account code")?;
        let data = event
            .account_state
            .remove("data")
            .context("missing account data")?;
        // Subscribers can join at different checkpoints. Share the four possible
        // encodings without retaining large code/data strings between batches.
        let mut encodings: [Option<Arc<str>>; 4] = Default::default();
        let mut subscribers = self.subscribers.lock().expect("subscription lock poisoned");
        for subscriber in &mut *subscribers {
            if subscriber.sender.is_closed()
                || !subscriber.addresses.contains(&event.address)
                || !subscriber.types.contains(&SubscriptionType::AccountStates)
            {
                continue;
            }
            let previous = subscriber.code_data.get(&event.address);
            let include_code = subscriber.include_code_data
                && previous.is_none_or(|previous| previous.code != hashes.code);
            let include_data = subscriber.include_code_data
                && previous.is_none_or(|previous| previous.data != hashes.data);
            let variant = usize::from(include_code) | (usize::from(include_data) << 1);
            let encoded = if let Some(encoded) = &encodings[variant] {
                Arc::clone(encoded)
            } else {
                event.account_state.remove("code");
                event.account_state.remove("data");
                if include_code {
                    event.account_state.insert("code".into(), code.clone());
                }
                if include_data {
                    event.account_state.insert("data".into(), data.clone());
                }
                let encoded = serde_json::to_string(event)?;
                ensure!(
                    encoded.len() <= MAX_EVENT_BYTES,
                    "stream event exceeds 1 MiB"
                );
                let encoded: Arc<str> = encoded.into();
                encodings[variant] = Some(Arc::clone(&encoded));
                encoded
            };
            if subscriber.enqueue(encoded) && subscriber.include_code_data {
                subscriber.code_data.insert(event.address.clone(), hashes);
            }
        }
        subscribers.retain(|subscriber| {
            !subscriber.sender.is_closed() && subscriber.terminal.load(Ordering::Acquire) == OPEN
        });
        drop(subscribers);
        Ok(())
    }

    fn send(&self, address: &str, kind: SubscriptionType, event: &impl Serialize) -> Result<()> {
        let encoded = serde_json::to_string(event)?;
        ensure!(
            encoded.len() <= MAX_EVENT_BYTES,
            "stream event exceeds 1 MiB"
        );
        let encoded: Arc<str> = encoded.into();
        let mut subscribers = self.subscribers.lock().expect("subscription lock poisoned");
        subscribers.retain(|subscriber| {
            if subscriber.sender.is_closed() {
                return false;
            }
            if !subscriber.addresses.contains(address) || !subscriber.types.contains(&kind) {
                return true;
            }

            subscriber.enqueue(Arc::clone(&encoded))
        });
        drop(subscribers);
        Ok(())
    }

    /// Ends current subscriptions after an encoding failure. Reconnection starts
    /// a new live subscription; it cannot recover the failed batch.
    pub(crate) fn fail(&self) {
        self.finish(FAILED);
    }

    /// Wakes idle streams and ends them during HTTP shutdown.
    pub(crate) fn close(&self) {
        self.connections.close();
        self.finish(CLOSED);
    }

    fn finish(&self, terminal: u8) {
        let mut subscribers = self.subscribers.lock().expect("subscription lock poisoned");
        for subscriber in subscribers.drain(..) {
            subscriber.terminal.store(terminal, Ordering::Release);
        }
    }
}

impl Subscriber {
    fn send_storage(&mut self, address: &str, seqno: u32, data: Option<&Cell>) -> Result<()> {
        let Some(storage) = &mut self.storage else {
            return Ok(());
        };
        let Some((initial, changed_fields)) = storage.update(address, seqno, data)? else {
            return Ok(());
        };
        let event = StorageUpdateEvent {
            kind: "storage_update",
            finality: "finalized",
            address,
            mc_seqno: seqno,
            data: data.map(Boc::encode_base64).unwrap_or_default(),
            initial,
            changed_fields,
        };
        let encoded = serde_json::to_string(&event)?;
        ensure!(
            encoded.len() <= MAX_EVENT_BYTES,
            "stream event exceeds 1 MiB"
        );
        self.enqueue(encoded.into());
        Ok(())
    }

    fn enqueue(&self, encoded: Arc<str>) -> bool {
        let Ok(bytes) = self
            .budget
            .clone()
            .try_acquire_many_owned(encoded.len() as u32)
        else {
            self.terminal.store(LAGGED, Ordering::Release);
            return false;
        };
        if self
            .sender
            .try_send(QueuedEvent {
                json: encoded,
                _bytes: bytes,
            })
            .is_err()
        {
            self.terminal.store(LAGGED, Ordering::Release);
            return false;
        }
        true
    }
}

/// Finalized transactions, account states and storage fields
///
/// Subscribe to finalized updates for 1–100 accounts. The first data event
/// is `{"status":"subscribed"}`. Later events contain `type=transaction`,
/// `finality=finalized` and one `transaction` object with TON Center v3 fields,
/// or `type=account_state`, `finality=finalized`, a raw `address` and an
/// `account_state` object with the fields of `/api/account`. Code and
/// data are sent on the first event for each account, then only when changed.
/// Missing code/data mean unchanged; empty strings mean cleared. Every other
/// field is always present. Reconnecting resets these per-account baselines
/// Set `include_code_data: false` to omit both fields from every account event,
/// including the first. The default is true; transaction and storage events
/// are unaffected
///
/// Select `transactions`, `account_states`, `storage_fields` or a combination in `types`.
/// Account states are emitted once per account after the complete batch commits, following
/// its transaction events. Account state events have no initial snapshot
///
/// For `storage_fields`, provide a Tolk `abi` and 1–64 unique `fields` paths.
/// At most 16 addresses can share one ABI. Each receives an initial
/// `storage_update` with `initial: true` and empty `changed_fields`. Later events
/// contain the complete base64 `data` `BoC`, `mc_seqno`, `initial: false`, and only
/// changed subscribed paths. Empty data means absent storage. Nested structs and
/// typed cells are supported; dictionary-key paths and recursive schemas are not.
/// Changes that revert within one batch do not produce an event. ABI mismatch
/// ends only that connection with `storage_decode_failed`. Custom serializers
/// are unsupported. `include_code_data` does not affect storage events
///
/// Keepalive comments arrive after 15 seconds without an event. Delivery is live
/// only: no replay or Last-Event-ID support. Slow consumers receive an error
/// event and are disconnected. Cancel the request to close the subscription
#[utoipa::path(
    post,
    path = "/api/streaming/sse",
    operation_id = "subscribe",
    request_body = Subscription,
    responses(
        (status = 200, description = "Live SSE stream", body = String, content_type = "text/event-stream", example = "data: {\"status\":\"subscribed\"}\n\n"),
        (status = 400, description = "Invalid subscription or unsupported replay", body = Object, example = json!({"error": "invalid_subscription"})),
        (status = 503, description = "Connection limit reached or service shutting down", body = Object, example = json!({"error": "too_many_subscribers"})),
    ),
)]
async fn subscribe(
    State(subscriptions): State<Subscriptions>,
    headers: HeaderMap,
    body: Result<Json<Subscription>, JsonRejection>,
) -> Response {
    let state = subscriptions.state.clone();
    let mut snapshot = None;
    open_subscription(subscriptions, headers, body, move |address| {
        // This reader is first called under register's publication lock.
        if snapshot.is_none() {
            snapshot = Some(
                state
                    .as_ref()
                    .context("state snapshots unavailable")?
                    .borrow()
                    .clone(),
            );
        }
        snapshot
            .as_ref()
            .expect("snapshot pinned")
            .get_account(address)
    })
    .await
}

/// Registration invokes the reader synchronously under the publication lock.
/// It must pin one checkpoint on its first call and use it for every account.
async fn open_subscription(
    subscriptions: Subscriptions,
    headers: HeaderMap,
    body: Result<Json<Subscription>, JsonRejection>,
    read_account: impl FnMut(&StdAddr) -> Result<AccountSnapshot> + Send + 'static,
) -> Response {
    let Ok(Json(body)) = body else {
        return reject(StatusCode::BAD_REQUEST, "invalid_subscription");
    };
    if headers.contains_key("last-event-id") {
        return reject(StatusCode::BAD_REQUEST, "replay_not_supported");
    }
    let types = body
        .types
        .unwrap_or_else(|| vec![SubscriptionType::Transactions]);
    if types.is_empty() {
        return reject(StatusCode::BAD_REQUEST, "expected_nonempty_types");
    }
    if body
        .min_finality
        .as_deref()
        .is_some_and(|value| value != "finalized")
    {
        return reject(StatusCode::BAD_REQUEST, "only_finalized_events_supported");
    }
    if body.addresses.is_empty() || body.addresses.len() > MAX_ADDRESSES {
        return reject(StatusCode::BAD_REQUEST, "expected_1_to_100_addresses");
    }
    let addresses = body
        .addresses
        .iter()
        .map(|address| {
            StdAddr::from_str_ext(address, StdAddrFormat::any())
                .map(|(address, _)| address.to_string())
        })
        .collect::<Result<HashSet<_>, _>>();
    let Ok(addresses) = addresses else {
        return reject(StatusCode::BAD_REQUEST, "invalid_address");
    };

    let storage = if types.contains(&SubscriptionType::StorageFields) {
        if addresses.len() > 16 {
            return reject(
                StatusCode::BAD_REQUEST,
                "storage_fields_accepts_at_most_16_addresses",
            );
        }
        let (Some(abi), Some(fields)) = (body.abi, body.fields) else {
            return reject(
                StatusCode::BAD_REQUEST,
                "storage_fields_requires_abi_and_fields",
            );
        };
        match StorageWatch::new(abi, fields) {
            Ok(storage) => Some(storage),
            Err(error) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "invalid_storage_subscription", "message": format!("{error:#}"),
                    })),
                )
                    .into_response();
            }
        }
    } else {
        if body.abi.is_some() || body.fields.is_some() {
            return reject(
                StatusCode::BAD_REQUEST,
                "abi_and_fields_require_storage_fields",
            );
        }
        None
    };

    let (sender, receiver) = mpsc::channel(QUEUE_EVENTS);
    let terminal = Arc::new(AtomicU8::new(OPEN));
    let Ok(slot) = subscriptions.connections.clone().try_acquire_owned() else {
        return reject(StatusCode::SERVICE_UNAVAILABLE, "too_many_subscribers");
    };
    let subscriber = Subscriber {
        addresses,
        types,
        include_code_data: body.include_code_data.unwrap_or(true),
        sender,
        budget: Arc::new(Semaphore::new(QUEUE_BYTES)),
        terminal: Arc::clone(&terminal),
        code_data: HashMap::new(),
        storage,
    };
    let connections = Arc::clone(&subscriptions.connections);
    let started = Instant::now();
    let registered =
        tokio::task::spawn_blocking(move || subscriptions.register(subscriber, read_account)).await;
    match registered {
        Ok(Ok(())) => {
            debug!(
                operation = "state_subscription",
                target = "sse",
                duration_ms = started.elapsed().as_millis(),
                outcome = "subscribed",
                "registered live subscription"
            );
        }
        Ok(Err(_)) if connections.is_closed() => {
            return reject(StatusCode::SERVICE_UNAVAILABLE, "shutting_down");
        }
        Ok(Err(error)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "storage_initialization_failed", "message": format!("{error:#}"),
                })),
            )
                .into_response();
        }
        Err(_) => return reject(StatusCode::INTERNAL_SERVER_ERROR, "subscription_failed"),
    }
    response(Connection {
        receiver,
        terminal,
        _slot: slot,
        first: true,
        done: false,
    })
}

fn response(connection: Connection) -> Response {
    let events = stream::unfold(connection, |mut connection| async move {
        if connection.done {
            return None;
        }
        let event = if connection.first {
            connection.first = false;
            Event::default().data(r#"{"status":"subscribed"}"#)
        } else {
            let next = connection.receiver.recv().await;
            let reason = connection.terminal.load(Ordering::Acquire);
            if reason == CLOSED {
                return None;
            }
            if reason == LAGGED || reason == FAILED {
                let error = if reason == LAGGED {
                    "slow_consumer"
                } else {
                    "stream_failed"
                };
                warn!(
                    operation = "state_subscription",
                    target = "sse",
                    outcome = error,
                    "ending subscription with a gap"
                );
                let event =
                    Event::default().data(json!({"type": "error", "error": error}).to_string());
                connection.done = true;
                return Some((Ok::<_, Infallible>(event), connection));
            }
            Event::default().data(&*next?.json)
        };
        Some((Ok::<_, Infallible>(event), connection))
    });

    (
        [("cache-control", "no-cache"), ("x-accel-buffering", "no")],
        Sse::new(events).keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keepalive"),
        ),
    )
        .into_response()
}

fn account_data(snapshot: &AccountSnapshot) -> Result<Option<Cell>> {
    if let Some(shard_account) = &snapshot.account
        && let Some(account) = shard_account.load_account()?
        && let AccountState::Active(state) = account.state
    {
        return Ok(state.data);
    }
    Ok(None)
}

fn reject(status: StatusCode, error: &'static str) -> Response {
    (status, Json(json!({"error": error}))).into_response()
}
