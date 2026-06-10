use super::{Action, BaseAction, BaseActionId, Extraction, NodeFact, TraceFacts, matchers};
use std::fmt::Debug;
use tycho_types::models::IntAddr;

#[derive(Debug)]
pub struct EnrichedAction {
    pub action: Action,
    pub info: Box<dyn ActionInfo>,
}

pub trait ActionInfo: Debug {
    fn render(&self) -> String;
}

pub(in crate::actions) type ActionInfoBox = Box<dyn ActionInfo>;

#[derive(Debug, Clone)]
pub struct AssetAmount {
    pub asset: Asset,
    pub amount: u128,
}

#[derive(Debug, Clone)]
pub enum Asset {
    Ton,
    Jetton { wallet: Option<IntAddr> },
    Unknown,
}

#[must_use]
pub fn enrich_actions(extraction: &Extraction, facts: &TraceFacts) -> Vec<EnrichedAction> {
    let ctx = EnrichmentContext { extraction, facts };
    extraction
        .actions
        .iter()
        .map(|action| EnrichedAction {
            action: action.clone(),
            info: matchers::describe_action(action, &ctx)
                .unwrap_or_else(|| enrich_common(action, &ctx)),
        })
        .collect()
}

#[must_use]
pub fn render_action(action: &EnrichedAction) -> String {
    action.info.render()
}

pub(in crate::actions) fn format_asset_amount(amount: &AssetAmount) -> String {
    match &amount.asset {
        Asset::Ton => format_ton_amount(amount.amount),
        Asset::Jetton { .. } => format!("{} jetton units", amount.amount),
        Asset::Unknown => format!("{} units", amount.amount),
    }
}

pub(in crate::actions) fn format_ton_amount(amount: u128) -> String {
    const NANOTONS_PER_TON: u128 = 1_000_000_000;

    let whole = amount / NANOTONS_PER_TON;
    let fractional = amount % NANOTONS_PER_TON;
    if fractional == 0 {
        return format!("{whole} TON");
    }

    let fractional = format!("{fractional:09}");
    format!("{}.{} TON", whole, fractional.trim_end_matches('0'))
}

#[derive(Clone)]
struct TonTransferInfo {
    amount: Option<u128>,
    source: Option<IntAddr>,
    destination: Option<IntAddr>,
}

impl Debug for TonTransferInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TonTransferInfo")
            .field("amount", &self.amount)
            .field("source", &self.source)
            .field("destination", &self.destination)
            .finish()
    }
}

impl ActionInfo for TonTransferInfo {
    fn render(&self) -> String {
        self.amount.map_or_else(
            || "transferred TON".to_owned(),
            |amount| format!("transferred {}", format_ton_amount(amount)),
        )
    }
}

#[derive(Clone)]
struct ContractCallInfo {
    opcode: Option<u32>,
    value: Option<u128>,
    destination: Option<IntAddr>,
}

impl Debug for ContractCallInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContractCallInfo")
            .field("opcode", &self.opcode)
            .field("value", &self.value)
            .field("destination", &self.destination)
            .finish()
    }
}

impl ActionInfo for ContractCallInfo {
    fn render(&self) -> String {
        self.opcode.map_or_else(
            || "called contract".to_owned(),
            |opcode| format!("called contract with opcode 0x{opcode:08x}"),
        )
    }
}

fn enrich_common(action: &Action, ctx: &EnrichmentContext<'_>) -> ActionInfoBox {
    let root = ctx.root_fact(action);

    match action.kind {
        super::ActionKind::TonTransfer => Box::new(TonTransferInfo {
            amount: root.and_then(|node| node.message.as_ref().map(|msg| msg.value)),
            source: root.and_then(message_source).cloned(),
            destination: root.and_then(message_destination).cloned(),
        }),
        super::ActionKind::ContractCall => Box::new(ContractCallInfo {
            opcode: root.and_then(|node| node.opcode),
            value: root.and_then(|node| node.message.as_ref().map(|msg| msg.value)),
            destination: root.and_then(message_destination).cloned(),
        }),
        _ => Box::new(ContractCallInfo {
            opcode: root.and_then(|node| node.opcode),
            value: root.and_then(|node| node.message.as_ref().map(|msg| msg.value)),
            destination: root.and_then(message_destination).cloned(),
        }),
    }
}

pub(in crate::actions) struct EnrichmentContext<'a> {
    extraction: &'a Extraction,
    facts: &'a TraceFacts,
}

impl<'a> EnrichmentContext<'a> {
    pub(in crate::actions) fn root_fact(&self, action: &Action) -> Option<&'a NodeFact> {
        let root_node = action
            .base_actions
            .first()
            .and_then(|id| self.base_action(*id))
            .map(|base_action| base_action.root_node)?;
        self.facts.get(root_node)
    }

    pub(in crate::actions) fn base_action(&self, id: BaseActionId) -> Option<&'a BaseAction> {
        self.extraction
            .base_actions
            .iter()
            .find(|base_action| base_action.id == id)
    }

    pub(in crate::actions) fn find_action_base(
        &self,
        action: &Action,
        predicate: impl Fn(&BaseAction) -> bool,
    ) -> Option<&'a BaseAction> {
        action
            .base_actions
            .iter()
            .filter_map(|id| self.base_action(*id))
            .find(|base_action| predicate(base_action))
    }

    pub(in crate::actions) fn fact_for_base(
        &self,
        base_action: &BaseAction,
    ) -> Option<&'a NodeFact> {
        self.facts.get(base_action.root_node)
    }

    pub(in crate::actions) fn first_node_with_amount(
        &self,
        action: &Action,
    ) -> Option<&'a NodeFact> {
        action
            .nodes
            .iter()
            .filter_map(|node_id| self.facts.get(*node_id))
            .find(|node| node.coins("amount").is_some())
    }
}

pub(in crate::actions) fn message_source(node: &NodeFact) -> Option<&IntAddr> {
    node.message.as_ref()?.source.as_ref()
}

pub(in crate::actions) fn message_destination(node: &NodeFact) -> Option<&IntAddr> {
    node.message.as_ref()?.destination.as_ref()
}
