use super::super::enrichment::{
    ActionInfo, ActionInfoBox, EnrichmentContext, format_ton_amount, message_destination,
    message_source,
};
use super::super::{
    Action, ActionKind, ActionProvider, BaseActionKind, BaseMatch, BaseMatcher, JettonTransferView,
    TraceNode, opcode_matches, opcodes,
};
use std::collections::BTreeSet;
use tycho_types::models::IntAddr;

pub(in crate::actions) struct PtonProvider;

impl ActionProvider for PtonProvider {
    fn base_matchers(&self) -> &'static [&'static dyn BaseMatcher] {
        &[&PtonTransferMatcher]
    }

    fn describe(&self, action: &Action, ctx: &EnrichmentContext<'_>) -> Option<ActionInfoBox> {
        if action.kind != ActionKind::PtonTransfer {
            return None;
        }

        Some(Box::new(describe_transfer(action, ctx)))
    }
}

struct PtonTransferMatcher;

impl BaseMatcher for PtonTransferMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::JETTON_TRANSFER) {
            return None;
        }

        let pton_transfer = root.child(opcodes::PTON_WALLET_V2_TON_TRANSFER)?;
        let excess = root.child(opcodes::EXCESS);

        let mut nodes = BTreeSet::from([root.id, pton_transfer.id]);
        if let Some(excess) = excess {
            nodes.insert(excess.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::PtonTransfer,
            nodes,
            root_node: root.id,
            user_facing: true,
        })
    }
}

#[derive(Clone)]
struct PtonTransferInfo {
    amount: Option<u128>,
    source: Option<IntAddr>,
    destination: Option<IntAddr>,
}

impl std::fmt::Debug for PtonTransferInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PtonTransferInfo")
            .field("amount", &self.amount)
            .field("source", &self.source)
            .field("destination", &self.destination)
            .finish()
    }
}

impl ActionInfo for PtonTransferInfo {
    fn render(&self) -> String {
        self.amount.map_or_else(
            || "transferred TON through pTON".to_owned(),
            |amount| format!("transferred {} through pTON", format_ton_amount(amount)),
        )
    }
}

fn describe_transfer(action: &Action, ctx: &EnrichmentContext<'_>) -> PtonTransferInfo {
    let root = ctx.root_fact(action);
    let transfer = root.and_then(JettonTransferView::parse);

    PtonTransferInfo {
        amount: transfer.as_ref().and_then(JettonTransferView::amount),
        source: root.and_then(message_source).cloned(),
        destination: transfer
            .as_ref()
            .and_then(JettonTransferView::destination)
            .or_else(|| root.and_then(message_destination))
            .cloned(),
    }
}
