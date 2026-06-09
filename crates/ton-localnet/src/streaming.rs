use crate::api::toncenter_v3;
use crate::localnet::{Localnet, LocalnetTransaction, convert_to_tx_struct};
use crate::storage::{JettonWalletMeta, TraceNode};
use crate::types::{Addr, Hash256};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};
use ton_indexer::categorize_wallet;
use tycho_types::models::{Base64StdAddrFlags, DisplayBase64StdAddr, StdAddr};
use tycho_types::prelude::HashBytes;

#[derive(Clone, Copy, Debug)]
pub struct StreamingCommitEvent {
    pub tx_hash: Hash256,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamingFinality {
    Pending,
    Confirmed,
    #[default]
    Finalized,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamingEventType {
    Transactions,
    Actions,
    Trace,
    AccountStateChange,
    JettonsChange,
    TraceInvalidated,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StreamingSubscribeRequest {
    pub id: Option<String>,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub trace_external_hash_norms: Vec<String>,
    #[serde(default)]
    pub types: Vec<StreamingEventType>,
    pub min_finality: Option<StreamingFinality>,
    #[serde(default)]
    pub action_types: Vec<String>,
    #[serde(default)]
    pub supported_action_types: Vec<String>,
    pub include_address_book: Option<bool>,
    pub include_metadata: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StreamingUnsubscribeRequest {
    pub id: Option<String>,
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(default)]
    pub trace_external_hash_norms: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamingOperation {
    Ping,
    Subscribe,
    Unsubscribe,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StreamingEnvelope {
    pub id: Option<String>,
    pub operation: StreamingOperation,
}

#[derive(Clone, Debug, Serialize)]
pub struct StreamingStatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
pub struct StreamingErrorResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub error: String,
}

#[derive(Clone, Debug)]
pub struct StreamingSubscription {
    pub addresses: BTreeSet<Addr>,
    pub trace_external_hash_norms: BTreeSet<String>,
    pub event_types: BTreeSet<StreamingEventType>,
    pub min_finality: StreamingFinality,
    pub action_types: BTreeSet<String>,
    pub supported_action_types: BTreeSet<String>,
    pub include_address_book: bool,
    pub include_metadata: bool,
}

impl Default for StreamingSubscription {
    fn default() -> Self {
        Self {
            addresses: BTreeSet::new(),
            trace_external_hash_norms: BTreeSet::new(),
            event_types: BTreeSet::new(),
            min_finality: StreamingFinality::Finalized,
            action_types: BTreeSet::new(),
            supported_action_types: BTreeSet::from(["latest".to_string()]),
            include_address_book: false,
            include_metadata: false,
        }
    }
}

impl StreamingSubscription {
    pub fn from_subscribe_request(req: &StreamingSubscribeRequest) -> anyhow::Result<Self> {
        validate_event_types(&req.types)?;

        let addresses = normalize_addresses(&req.addresses)?;
        let trace_external_hash_norms =
            normalize_trace_external_hash_norms(&req.trace_external_hash_norms)?;
        validate_subscription_shape(&req.types, &addresses, &trace_external_hash_norms)?;

        let supported_action_types = if req.supported_action_types.is_empty() {
            BTreeSet::from(["latest".to_string()])
        } else {
            req.supported_action_types.iter().cloned().collect()
        };

        Ok(Self {
            addresses,
            trace_external_hash_norms,
            event_types: req.types.iter().copied().collect(),
            min_finality: req.min_finality.unwrap_or_default(),
            action_types: req.action_types.iter().cloned().collect(),
            supported_action_types,
            include_address_book: req.include_address_book.unwrap_or(false),
            include_metadata: req.include_metadata.unwrap_or(false),
        })
    }

    pub fn unsubscribe(&mut self, req: &StreamingUnsubscribeRequest) -> anyhow::Result<()> {
        let addresses = normalize_addresses(&req.addresses)?;
        let traces = normalize_trace_external_hash_norms(&req.trace_external_hash_norms)?;

        for address in addresses {
            self.addresses.remove(&address);
        }
        for trace in traces {
            self.trace_external_hash_norms.remove(&trace);
        }
        Ok(())
    }

    fn has_type(&self, event_type: StreamingEventType) -> bool {
        self.event_types.contains(&event_type)
    }

    fn accepts_finality(&self, finality: StreamingFinality) -> bool {
        finality >= self.min_finality
    }

    fn interested_in_any_address(
        &self,
        event_type: StreamingEventType,
        addresses: &[Addr],
    ) -> bool {
        self.has_type(event_type)
            && addresses
                .iter()
                .any(|address| self.addresses.contains(address))
    }

    fn interested_in_trace(&self, trace_external_hash_norm: &str) -> bool {
        self.has_type(StreamingEventType::Trace)
            && self
                .trace_external_hash_norms
                .contains(trace_external_hash_norm)
    }
}

pub fn validate_unsubscribe_request(req: &StreamingUnsubscribeRequest) -> anyhow::Result<()> {
    if req.addresses.is_empty() && req.trace_external_hash_norms.is_empty() {
        anyhow::bail!("addresses or trace_external_hash_norms are required");
    }
    Ok(())
}

pub async fn notifications_for_commit(
    node: &Localnet,
    subscription: &StreamingSubscription,
    commit: StreamingCommitEvent,
) -> anyhow::Result<Vec<Value>> {
    let trace = node.get_traces(commit.tx_hash).await?;
    let trace_external_hash_norm = trace_external_hash_norm(&trace);
    let transactions = collect_trace_transactions(&trace)?;
    let event_addresses = collect_transaction_addresses(&transactions);
    let current_account = transactions
        .iter()
        .find(|tx| tx.hash == commit.tx_hash)
        .map(|tx| tx.address);

    let mut notifications = Vec::new();

    for finality in [
        StreamingFinality::Pending,
        StreamingFinality::Confirmed,
        StreamingFinality::Finalized,
    ] {
        if !subscription.accepts_finality(finality) {
            continue;
        }

        if let Some(notification) = transactions_notification(
            node,
            subscription,
            &trace_external_hash_norm,
            &transactions,
            finality,
        )
        .await?
        {
            notifications.push(notification);
        }

        if let Some(notification) = actions_notification(
            node,
            subscription,
            &trace_external_hash_norm,
            &event_addresses,
            finality,
        )
        .await?
        {
            notifications.push(notification);
        }

        if let Some(notification) = trace_notification(
            node,
            subscription,
            &trace_external_hash_norm,
            &trace,
            finality,
        )
        .await?
        {
            notifications.push(notification);
        }
    }

    for finality in [StreamingFinality::Confirmed, StreamingFinality::Finalized] {
        if !subscription.accepts_finality(finality) {
            continue;
        }

        if let Some(account) = current_account {
            if let Some(notification) =
                account_state_notification(node, subscription, account, finality).await?
            {
                notifications.push(notification);
            }

            if let Some(notification) =
                jettons_notification(node, subscription, account, finality).await?
            {
                notifications.push(notification);
            }
        }
    }

    Ok(notifications)
}

fn validate_event_types(types: &[StreamingEventType]) -> anyhow::Result<()> {
    if types.is_empty() {
        anyhow::bail!("types are required for subscription");
    }
    if types.contains(&StreamingEventType::TraceInvalidated) {
        anyhow::bail!("invalid event type: trace_invalidated");
    }
    Ok(())
}

fn validate_subscription_shape(
    types: &[StreamingEventType],
    addresses: &BTreeSet<Addr>,
    trace_external_hash_norms: &BTreeSet<String>,
) -> anyhow::Result<()> {
    let has_trace_type = types.contains(&StreamingEventType::Trace);
    let has_address_types = types
        .iter()
        .any(|event_type| *event_type != StreamingEventType::Trace);

    if !trace_external_hash_norms.is_empty() && !has_trace_type {
        anyhow::bail!("trace_external_hash_norms requires type \"trace\"");
    }
    if has_trace_type && trace_external_hash_norms.is_empty() {
        anyhow::bail!("trace_external_hash_norms are required for trace subscription");
    }
    if has_address_types && addresses.is_empty() {
        anyhow::bail!("addresses are required for subscription");
    }
    Ok(())
}

fn normalize_addresses(addresses: &[String]) -> anyhow::Result<BTreeSet<Addr>> {
    addresses
        .iter()
        .map(|address| {
            Localnet::parse_addr(address)
                .with_context(|| format!("invalid address in subscription: {address}"))
        })
        .collect()
}

fn normalize_trace_external_hash_norms(traces: &[String]) -> anyhow::Result<BTreeSet<String>> {
    let mut normalized = BTreeSet::new();
    for trace in traces {
        let trace = trace.trim();
        if trace.is_empty() {
            anyhow::bail!("trace_external_hash_norms contains empty value");
        }
        normalized.insert(trace.to_string());
    }
    Ok(normalized)
}

fn trace_external_hash_norm(trace: &TraceNode) -> String {
    trace
        .external_hash
        .unwrap_or(trace.transaction.meta.tx_hash)
        .to_base64()
}

fn collect_trace_transactions(trace: &TraceNode) -> anyhow::Result<Vec<LocalnetTransaction>> {
    let mut transactions = Vec::new();
    collect_trace_transactions_inner(trace, &mut transactions)?;
    transactions.sort_by(|a, b| {
        b.transaction_id
            .lt
            .cmp(&a.transaction_id.lt)
            .then_with(|| b.hash.cmp(&a.hash))
    });
    Ok(transactions)
}

fn collect_trace_transactions_inner(
    trace: &TraceNode,
    out: &mut Vec<LocalnetTransaction>,
) -> anyhow::Result<()> {
    out.push(convert_to_tx_struct(
        &trace.transaction,
        trace.transaction.tx_boc.clone(),
    )?);
    for child in &trace.children {
        collect_trace_transactions_inner(child, out)?;
    }
    Ok(())
}

fn collect_transaction_addresses(transactions: &[LocalnetTransaction]) -> BTreeSet<Addr> {
    let mut addresses = BTreeSet::new();
    for tx in transactions {
        addresses.insert(tx.address);
        if let Some(source) = tx.in_msg.source {
            addresses.insert(source);
        }
        if let Some(destination) = tx.in_msg.destination {
            addresses.insert(destination);
        }
        for message in &tx.out_msgs {
            if let Some(source) = message.source {
                addresses.insert(source);
            }
            if let Some(destination) = message.destination {
                addresses.insert(destination);
            }
        }
    }
    addresses
}

async fn transactions_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    trace_external_hash_norm: &str,
    transactions: &[LocalnetTransaction],
    finality: StreamingFinality,
) -> anyhow::Result<Option<Value>> {
    if !subscription.has_type(StreamingEventType::Transactions) {
        return Ok(None);
    }

    let filtered = transactions
        .iter()
        .filter(|tx| subscription.addresses.contains(&tx.address))
        .cloned()
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        return Ok(None);
    }

    let mapped = toncenter_v3::map_transactions_response(&filtered);
    let mut notification = json!({
        "type": StreamingEventType::Transactions,
        "finality": finality,
        "trace_external_hash_norm": trace_external_hash_norm,
        "transactions": mapped
            .get("transactions")
            .cloned()
            .unwrap_or_else(|| json!([])),
    });
    attach_extra_data(
        node,
        subscription,
        &mut notification,
        collect_transaction_addresses(&filtered),
    )
    .await?;
    Ok(Some(notification))
}

async fn actions_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    trace_external_hash_norm: &str,
    event_addresses: &BTreeSet<Addr>,
    finality: StreamingFinality,
) -> anyhow::Result<Option<Value>> {
    if !subscription.interested_in_any_address(
        StreamingEventType::Actions,
        &event_addresses.iter().copied().collect::<Vec<_>>(),
    ) || !subscription.action_types.is_empty()
    {
        return Ok(None);
    }

    let mut notification = json!({
        "type": StreamingEventType::Actions,
        "finality": finality,
        "trace_external_hash_norm": trace_external_hash_norm,
        "actions": [],
    });
    attach_extra_data(
        node,
        subscription,
        &mut notification,
        event_addresses.clone(),
    )
    .await?;
    Ok(Some(notification))
}

async fn trace_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    trace_external_hash_norm: &str,
    trace: &TraceNode,
    finality: StreamingFinality,
) -> anyhow::Result<Option<Value>> {
    if !subscription.interested_in_trace(trace_external_hash_norm) {
        return Ok(None);
    }

    let mapped = toncenter_v3::map_traces(trace);
    let Some(trace_entry) = mapped
        .get("traces")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
    else {
        return Ok(None);
    };

    let transactions = collect_trace_transactions(trace)?;
    let mut notification = json!({
        "type": StreamingEventType::Trace,
        "finality": finality,
        "trace_external_hash_norm": trace_external_hash_norm,
        "trace": trace_entry
            .get("trace")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "transactions": trace_entry
            .get("transactions")
            .cloned()
            .unwrap_or_else(|| json!({})),
        "actions": [],
    });
    attach_extra_data(
        node,
        subscription,
        &mut notification,
        collect_transaction_addresses(&transactions),
    )
    .await?;
    Ok(Some(notification))
}

async fn account_state_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    account: Addr,
    finality: StreamingFinality,
) -> anyhow::Result<Option<Value>> {
    if !subscription.interested_in_any_address(StreamingEventType::AccountStateChange, &[account]) {
        return Ok(None);
    }

    let state = node
        .get_address_information(account.to_string(), None)
        .await?;
    let state = map_account_state(&state);
    Ok(Some(json!({
        "type": StreamingEventType::AccountStateChange,
        "finality": finality,
        "account": account.to_string(),
        "state": state,
    })))
}

async fn jettons_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    account: Addr,
    finality: StreamingFinality,
) -> anyhow::Result<Option<Value>> {
    if !subscription.has_type(StreamingEventType::JettonsChange) {
        return Ok(None);
    }

    let Some(wallet) = node
        .get_jetton_wallets(
            Some(account.to_string()),
            None,
            None,
            Some(false),
            Some(1),
            Some(0),
        )
        .await?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };

    if !subscription.addresses.contains(&wallet.address)
        && !subscription.addresses.contains(&wallet.owner_address)
    {
        return Ok(None);
    }

    let mut notification = json!({
        "type": StreamingEventType::JettonsChange,
        "finality": finality,
        "jetton": map_jetton_wallet(&wallet),
    });
    attach_extra_data(
        node,
        subscription,
        &mut notification,
        BTreeSet::from([wallet.address, wallet.owner_address, wallet.jetton_address]),
    )
    .await?;
    Ok(Some(notification))
}

fn map_account_state(state: &crate::localnet::LocalnetAccountState) -> Value {
    let mapped =
        toncenter_v3::map_account_states(std::slice::from_ref(state), &HashMap::new(), true);
    mapped
        .get("accounts")
        .and_then(Value::as_array)
        .and_then(|accounts| accounts.first())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

fn map_jetton_wallet(wallet: &JettonWalletMeta) -> Value {
    let wallets = vec![wallet.clone()];
    toncenter_v3::map_jetton_wallets(&wallets)
        .get("jetton_wallets")
        .and_then(Value::as_array)
        .and_then(|wallets| wallets.first())
        .cloned()
        .unwrap_or_else(|| json!({}))
}

async fn attach_extra_data(
    node: &Localnet,
    subscription: &StreamingSubscription,
    notification: &mut Value,
    addresses: BTreeSet<Addr>,
) -> anyhow::Result<()> {
    if !subscription.include_address_book && !subscription.include_metadata {
        return Ok(());
    }

    let (address_book, metadata) = build_extra_data(node, subscription, &addresses).await?;
    if let Some(root) = notification.as_object_mut() {
        if let Some(address_book) = address_book {
            root.insert("address_book".to_string(), address_book);
        }
        if let Some(metadata) = metadata {
            root.insert("metadata".to_string(), metadata);
        }
    }
    Ok(())
}

async fn build_extra_data(
    node: &Localnet,
    subscription: &StreamingSubscription,
    addresses: &BTreeSet<Addr>,
) -> anyhow::Result<(Option<Value>, Option<Value>)> {
    let mut address_book = Map::new();
    let mut metadata = Map::new();
    let mut extra_jetton_masters = BTreeSet::new();

    for address in addresses {
        let info = collect_address_info(node, *address).await?;
        extra_jetton_masters.extend(info.extra_jetton_masters.iter().copied());

        if subscription.include_address_book {
            address_book.insert(
                address.to_string(),
                json!({
                    "user_friendly": as_user_friendly(*address),
                    "domain": Value::Null,
                    "interfaces": info.interfaces.into_iter().collect::<Vec<_>>(),
                }),
            );
        }

        if subscription.include_metadata && !info.token_info.is_empty() {
            metadata.insert(
                address.to_string(),
                json!({
                    "is_indexed": true,
                    "token_info": info.token_info,
                }),
            );
        }
    }

    if subscription.include_metadata {
        for master_address in extra_jetton_masters {
            let key = master_address.to_string();
            if metadata.contains_key(&key) {
                continue;
            }
            let info = collect_address_info(node, master_address).await?;
            if !info.token_info.is_empty() {
                metadata.insert(
                    key,
                    json!({
                        "is_indexed": true,
                        "token_info": info.token_info,
                    }),
                );
            }
        }
    }

    Ok((
        subscription
            .include_address_book
            .then_some(Value::Object(address_book)),
        subscription
            .include_metadata
            .then_some(Value::Object(metadata)),
    ))
}

#[derive(Default)]
struct AddressInfo {
    interfaces: BTreeSet<String>,
    token_info: Vec<Value>,
    extra_jetton_masters: BTreeSet<Addr>,
}

async fn collect_address_info(node: &Localnet, address: Addr) -> anyhow::Result<AddressInfo> {
    let mut info = AddressInfo::default();
    let address_str = address.to_string();

    if let Ok(state) = node
        .get_address_information(address_str.clone(), None)
        .await
        && let Some(code_hash) = state.code_hash
    {
        let wallet_type = categorize_wallet(HashBytes(code_hash.0));
        if let Some(interface_name) = wallet_type.interface_name() {
            info.interfaces.insert(interface_name.to_string());
        }
    }

    let wallets = node
        .get_jetton_wallets(
            Some(address_str.clone()),
            None,
            None,
            Some(false),
            Some(1),
            Some(0),
        )
        .await?;
    if let Some(wallet) = wallets.first() {
        info.interfaces.insert("jetton_wallet".to_string());
        info.token_info
            .push(toncenter_v3::map_jetton_wallet_token_info(wallet));
        info.extra_jetton_masters.insert(wallet.jetton_address);
    }

    let masters = node
        .get_jetton_masters(Some(address_str.clone()), None, Some(1), Some(0))
        .await?;
    if let Some(master) = masters.first() {
        info.interfaces.insert("jetton_master".to_string());
        info.token_info
            .push(toncenter_v3::map_jetton_master_token_info(master));
    }

    let items = node
        .get_nft_items(
            Some(address_str.clone()),
            None,
            None,
            None,
            Some(false),
            Some(1),
            Some(0),
        )
        .await?;
    if let Some(item) = items.first() {
        info.interfaces.insert("nft_item".to_string());
        info.token_info
            .push(toncenter_v3::map_nft_item_token_info(item));
    }

    let collections = node
        .get_nft_items(
            None,
            None,
            Some(address_str),
            None,
            Some(false),
            Some(1),
            Some(0),
        )
        .await?;
    if let Some(item) = collections.first() {
        info.interfaces.insert("nft_collection".to_string());
        info.token_info
            .push(toncenter_v3::map_nft_collection_token_info(item));
    }

    Ok(info)
}

fn as_user_friendly(address: Addr) -> String {
    let workchain = i8::try_from(address.workchain).ok().unwrap_or_default();
    let std_addr = StdAddr::new(workchain, HashBytes(address.addr));
    DisplayBase64StdAddr {
        addr: &std_addr,
        flags: Base64StdAddrFlags {
            testnet: false,
            base64_url: true,
            bounceable: false,
        },
    }
    .to_string()
}
