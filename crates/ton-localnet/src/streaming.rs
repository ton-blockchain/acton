use crate::api::toncenter_v3;
use crate::localnet::{
    Localnet, LocalnetJettonWalletsQuery, LocalnetNftItemsOrder, LocalnetNftItemsQuery,
    LocalnetTransaction, convert_to_tx_struct,
};
use crate::storage;
use crate::storage::TraceNode;
use crate::types::{Addr, Hash256};
use anyhow::Context;
use std::collections::{BTreeSet, HashMap};
use ton_api::toncenter::streaming::v2 as streaming;
use ton_api::toncenter::v3;
use ton_indexer::categorize_wallet;
use tycho_types::prelude::HashBytes;

#[derive(Clone, Copy, Debug)]
pub struct StreamingCommitEvent {
    pub tx_hash: Hash256,
}

#[derive(Clone, Debug)]
pub struct StreamingSubscription {
    pub addresses: BTreeSet<Addr>,
    pub trace_external_hash_norms: BTreeSet<String>,
    pub event_types: BTreeSet<streaming::EventType>,
    pub min_finality: streaming::Finality,
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
            min_finality: streaming::Finality::Finalized,
            action_types: BTreeSet::new(),
            supported_action_types: BTreeSet::from(["latest".to_string()]),
            include_address_book: false,
            include_metadata: false,
        }
    }
}

impl StreamingSubscription {
    pub fn from_subscribe_request(req: &streaming::Subscription) -> anyhow::Result<Self> {
        validate_event_types(&req.types)?;

        let addresses = normalize_addresses(&req.addresses)?;
        let trace_external_hash_norms =
            validate_trace_external_hash_norms(&req.trace_external_hash_norms)?;
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

    pub fn unsubscribe(&mut self, req: &streaming::UnsubscribeRequest) -> anyhow::Result<()> {
        let addresses = normalize_addresses(&req.addresses)?;
        let traces = validate_trace_external_hash_norms(&req.trace_external_hash_norms)?;

        for address in addresses {
            self.addresses.remove(&address);
        }
        for trace in traces {
            self.trace_external_hash_norms.remove(&trace);
        }
        Ok(())
    }

    fn has_type(&self, event_type: streaming::EventType) -> bool {
        self.event_types.contains(&event_type)
    }

    fn accepts_finality(&self, finality: streaming::Finality) -> bool {
        finality >= self.min_finality
    }

    fn interested_in_any_address(
        &self,
        event_type: streaming::EventType,
        addresses: &[Addr],
    ) -> bool {
        self.has_type(event_type)
            && addresses
                .iter()
                .any(|address| self.addresses.contains(address))
    }

    fn interested_in_trace(&self, trace_external_hash_norm: &str) -> bool {
        self.has_type(streaming::EventType::Trace)
            && self
                .trace_external_hash_norms
                .contains(trace_external_hash_norm)
    }
}

pub fn validate_unsubscribe_request(req: &streaming::UnsubscribeRequest) -> anyhow::Result<()> {
    if req.addresses.is_empty() && req.trace_external_hash_norms.is_empty() {
        anyhow::bail!("addresses or trace_external_hash_norms are required");
    }
    Ok(())
}

pub async fn notifications_for_commit(
    node: &Localnet,
    subscription: &StreamingSubscription,
    commit: StreamingCommitEvent,
) -> anyhow::Result<Vec<streaming::Notification>> {
    let trace = node.get_traces(commit.tx_hash).await?;
    let trace_external_hash_norm = trace.effective_external_hash_norm().to_base64();
    let transactions = collect_trace_transactions(&trace)?;
    let event_addresses = collect_transaction_addresses(&transactions);
    let current_account = transactions
        .iter()
        .find(|tx| tx.hash == commit.tx_hash)
        .map(|tx| tx.address);

    let mut notifications = Vec::new();

    for finality in [
        streaming::Finality::Pending,
        streaming::Finality::Confirmed,
        streaming::Finality::Finalized,
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

    for finality in [
        streaming::Finality::Confirmed,
        streaming::Finality::Finalized,
    ] {
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

fn validate_event_types(types: &[streaming::EventType]) -> anyhow::Result<()> {
    if types.is_empty() {
        anyhow::bail!("types are required for subscription");
    }
    Ok(())
}

fn validate_subscription_shape(
    types: &[streaming::EventType],
    addresses: &BTreeSet<Addr>,
    trace_external_hash_norms: &BTreeSet<String>,
) -> anyhow::Result<()> {
    let has_trace_type = types.contains(&streaming::EventType::Trace);
    let has_address_types = types
        .iter()
        .any(|event_type| *event_type != streaming::EventType::Trace);

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
            Addr::parse(address)
                .with_context(|| format!("invalid address in subscription: {address}"))
        })
        .collect()
}

fn validate_trace_external_hash_norms(traces: &[String]) -> anyhow::Result<BTreeSet<String>> {
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
    finality: streaming::Finality,
) -> anyhow::Result<Option<streaming::Notification>> {
    if !subscription.has_type(streaming::EventType::Transactions) {
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

    let response = toncenter_v3::map_transactions_response(&filtered);
    let (address_book, metadata) = build_extra_data(
        node,
        subscription,
        &collect_transaction_addresses(&filtered),
    )
    .await?;
    Ok(Some(streaming::Notification::Transactions {
        finality,
        trace_external_hash_norm: trace_external_hash_norm.to_owned(),
        transactions: response.transactions,
        address_book,
        metadata,
    }))
}

async fn actions_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    trace_external_hash_norm: &str,
    event_addresses: &BTreeSet<Addr>,
    finality: streaming::Finality,
) -> anyhow::Result<Option<streaming::Notification>> {
    if !subscription.interested_in_any_address(
        streaming::EventType::Actions,
        &event_addresses.iter().copied().collect::<Vec<_>>(),
    ) || !subscription.action_types.is_empty()
    {
        return Ok(None);
    }

    let (address_book, metadata) = build_extra_data(node, subscription, event_addresses).await?;
    Ok(Some(streaming::Notification::Actions {
        finality,
        trace_external_hash_norm: trace_external_hash_norm.to_owned(),
        actions: Vec::new(),
        address_book,
        metadata,
    }))
}

async fn trace_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    trace_external_hash_norm: &str,
    trace: &TraceNode,
    finality: streaming::Finality,
) -> anyhow::Result<Option<streaming::Notification>> {
    if !subscription.interested_in_trace(trace_external_hash_norm) {
        return Ok(None);
    }

    let mut mapped = toncenter_v3::map_traces(trace);
    let trace_entry = mapped
        .traces
        .pop()
        .context("typed trace mapper returned no traces")?;
    let transactions = collect_trace_transactions(trace)?;
    let (address_book, metadata) = build_extra_data(
        node,
        subscription,
        &collect_transaction_addresses(&transactions),
    )
    .await?;

    Ok(Some(streaming::Notification::Trace {
        finality,
        trace_external_hash_norm: trace_external_hash_norm.to_owned(),
        trace: Box::new(
            trace_entry
                .trace
                .context("typed trace mapper omitted trace tree")?,
        ),
        transactions: trace_entry.transactions,
        actions: Some(trace_entry.actions),
        address_book,
        metadata,
    }))
}

async fn account_state_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    account: Addr,
    finality: streaming::Finality,
) -> anyhow::Result<Option<streaming::Notification>> {
    if !subscription.interested_in_any_address(streaming::EventType::AccountStateChange, &[account])
    {
        return Ok(None);
    }

    let state = node
        .get_address_information(account.to_string(), None)
        .await?;
    let state = map_account_state(&state);

    Ok(Some(streaming::Notification::AccountStateChange {
        finality,
        account: account.to_string(),
        state,
    }))
}

async fn jettons_notification(
    node: &Localnet,
    subscription: &StreamingSubscription,
    account: Addr,
    finality: streaming::Finality,
) -> anyhow::Result<Option<streaming::Notification>> {
    if !subscription.has_type(streaming::EventType::JettonsChange) {
        return Ok(None);
    }

    let Some(wallet) = node
        .get_jetton_wallets(LocalnetJettonWalletsQuery {
            addresses: vec![account.to_string()],
            owner_addresses: Vec::new(),
            jetton_addresses: Vec::new(),
            exclude_zero_balance: Some(false),
            sort: None,
            limit: Some(1),
            offset: Some(0),
        })
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

    let (address_book, metadata) = build_extra_data(
        node,
        subscription,
        &BTreeSet::from([wallet.address, wallet.owner_address, wallet.jetton_address]),
    )
    .await?;

    Ok(Some(streaming::Notification::JettonsChange {
        finality,
        jetton: toncenter_v3::map_jetton_wallet(&wallet),
        address_book,
        metadata,
    }))
}

fn map_account_state(state: &crate::localnet::LocalnetAccountState) -> v3::AccountState {
    v3::AccountState {
        hash: state.account_state_hash.to_base64(),
        balance: Some(state.balance.to_string()),
        account_status: Some(
            match state.state {
                storage::AccountStatus::Active => "active",
                storage::AccountStatus::Uninit => "uninit",
                storage::AccountStatus::Frozen => "frozen",
                storage::AccountStatus::Nonexist => "nonexist",
            }
            .to_owned(),
        ),
        code_boc: None,
        code_hash: state.code_hash.as_ref().map(Hash256::to_base64),
        data_boc: None,
        data_hash: state.data_hash.as_ref().map(Hash256::to_base64),
        extra_currencies: Some(HashMap::new()),
        frozen_hash: state.frozen_hash.as_ref().map(Hash256::to_base64),
    }
}

async fn build_extra_data(
    node: &Localnet,
    subscription: &StreamingSubscription,
    addresses: &BTreeSet<Addr>,
) -> anyhow::Result<(Option<v3::AddressBook>, Option<v3::Metadata>)> {
    if !subscription.include_address_book && !subscription.include_metadata {
        return Ok((None, None));
    }
    let mut address_book = v3::AddressBook::new();
    let mut metadata = v3::Metadata::new();
    let mut extra_jetton_masters = BTreeSet::new();

    for address in addresses {
        let info = collect_address_info(node, *address).await?;
        extra_jetton_masters.extend(info.extra_jetton_masters.iter().copied());

        if subscription.include_address_book {
            address_book.insert(
                address.to_string(),
                v3::AddressBookRow {
                    user_friendly: Some(address.as_user_friendly()),
                    domain: None,
                    interfaces: Some(info.interfaces.into_iter().collect()),
                },
            );
        }

        if subscription.include_metadata && !info.token_info.is_empty() {
            metadata.insert(
                address.to_string(),
                v3::AddressMetadata {
                    is_indexed: true,
                    token_info: info.token_info,
                },
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
                    v3::AddressMetadata {
                        is_indexed: true,
                        token_info: info.token_info,
                    },
                );
            }
        }
    }

    Ok((
        subscription.include_address_book.then_some(address_book),
        subscription.include_metadata.then_some(metadata),
    ))
}

#[derive(Default)]
struct AddressInfo {
    interfaces: BTreeSet<String>,
    token_info: Vec<v3::TokenInfo>,
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
        .get_jetton_wallets(LocalnetJettonWalletsQuery {
            addresses: vec![address_str.clone()],
            owner_addresses: Vec::new(),
            jetton_addresses: Vec::new(),
            exclude_zero_balance: Some(false),
            sort: None,
            limit: Some(1),
            offset: Some(0),
        })
        .await?;
    if let Some(wallet) = wallets.first() {
        info.interfaces.insert("jetton_wallet".to_string());
        info.token_info
            .push(toncenter_v3::map_jetton_wallet_token_info(wallet));
        info.extra_jetton_masters.insert(wallet.jetton_address);
    }

    let masters = node
        .get_jetton_masters(vec![address_str.clone()], Vec::new(), Some(1), Some(0))
        .await?;
    if let Some(master) = masters.first() {
        info.interfaces.insert("jetton_master".to_string());
        info.token_info
            .push(toncenter_v3::map_jetton_master_token_info(master));
    }

    let items = node
        .get_nft_items(LocalnetNftItemsQuery {
            addresses: vec![address_str.clone()],
            owner_addresses: Vec::new(),
            collection_addresses: Vec::new(),
            indexes: Vec::new(),
            order: LocalnetNftItemsOrder::Insertion,
            limit: Some(1),
            offset: Some(0),
        })
        .await?;
    if let Some(item) = items.first() {
        info.interfaces.insert("nft_item".to_string());
        info.token_info
            .push(toncenter_v3::map_nft_item_token_info(item));
    }

    let collections = node
        .get_nft_items(LocalnetNftItemsQuery {
            addresses: Vec::new(),
            owner_addresses: Vec::new(),
            collection_addresses: vec![address_str],
            indexes: Vec::new(),
            order: LocalnetNftItemsOrder::CollectionIndex,
            limit: Some(1),
            offset: Some(0),
        })
        .await?;
    if let Some(item) = collections.first() {
        info.interfaces.insert("nft_collection".to_string());
        info.token_info
            .push(toncenter_v3::map_nft_collection_token_info(item));
    }

    Ok(info)
}
