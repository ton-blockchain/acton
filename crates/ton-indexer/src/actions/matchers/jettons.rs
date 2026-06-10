use super::super::enrichment::{
    ActionInfo, ActionInfoBox, EnrichmentContext, message_destination, message_source,
};
use super::super::{
    Action, ActionKind, ActionProvider, BaseActionKind, BaseMatch, BaseMatcher, JettonTransferView,
    TraceNode, opcode_matches, opcodes,
};
use std::collections::BTreeSet;
use tycho_types::models::IntAddr;

pub(in crate::actions) struct JettonsProvider;

impl ActionProvider for JettonsProvider {
    fn base_matchers(&self) -> &'static [&'static dyn BaseMatcher] {
        &[&JettonTransferMatcher, &JettonMintMatcher]
    }

    fn describe(&self, action: &Action, ctx: &EnrichmentContext<'_>) -> Option<ActionInfoBox> {
        match action.kind {
            ActionKind::JettonTransfer => Some(Box::new(describe_transfer(action, ctx))),
            ActionKind::JettonMint => Some(Box::new(describe_mint(action, ctx))),
            ActionKind::DedustSwap
            | ActionKind::DedustPayout
            | ActionKind::StonfiSwap
            | ActionKind::PtonTransfer
            | ActionKind::TonTransfer
            | ActionKind::ContractCall => None,
        }
    }
}

struct JettonTransferMatcher;

impl BaseMatcher for JettonTransferMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::JETTON_TRANSFER) {
            return None;
        }

        let internal_transfer = root.child(opcodes::JETTON_INTERNAL_TRANSFER)?;
        let excess = internal_transfer.child(opcodes::EXCESS);
        let notify = internal_transfer.child(opcodes::JETTON_NOTIFY);

        let mut nodes = BTreeSet::from([root.id, internal_transfer.id]);
        if let Some(excess) = excess {
            nodes.insert(excess.id);
        }
        if let Some(notify) = notify {
            nodes.insert(notify.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::JettonTransfer,
            nodes,
            root_node: root.id,
            user_facing: true,
        })
    }
}

struct JettonMintMatcher;

impl BaseMatcher for JettonMintMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::JETTON_MINT) {
            return None;
        }

        let internal_transfer = root.child(opcodes::JETTON_INTERNAL_TRANSFER)?;
        let notification = internal_transfer.child(opcodes::JETTON_WALLET_TRANSFER_NOTIFICATION);
        let excess = internal_transfer.child(opcodes::EXCESS);

        let mut nodes = BTreeSet::from([root.id, internal_transfer.id]);
        if let Some(notification) = notification {
            nodes.insert(notification.id);
        }
        if let Some(excess) = excess {
            nodes.insert(excess.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::JettonMint,
            nodes,
            root_node: root.id,
            user_facing: true,
        })
    }
}

#[derive(Clone)]
struct JettonTransferInfo {
    amount: Option<u128>,
    source: Option<IntAddr>,
    destination: Option<IntAddr>,
}

impl std::fmt::Debug for JettonTransferInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JettonTransferInfo")
            .field("amount", &self.amount)
            .field("source", &self.source)
            .field("destination", &self.destination)
            .finish()
    }
}

impl ActionInfo for JettonTransferInfo {
    fn render(&self) -> String {
        self.amount.map_or_else(
            || "transferred jettons".to_owned(),
            |amount| format!("transferred {amount} jetton units"),
        )
    }
}

#[derive(Clone)]
struct JettonMintInfo {
    amount: Option<u128>,
    destination: Option<IntAddr>,
}

impl std::fmt::Debug for JettonMintInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JettonMintInfo")
            .field("amount", &self.amount)
            .field("destination", &self.destination)
            .finish()
    }
}

impl ActionInfo for JettonMintInfo {
    fn render(&self) -> String {
        self.amount.map_or_else(
            || "minted jettons".to_owned(),
            |amount| format!("minted {amount} jetton units"),
        )
    }
}

fn describe_transfer(action: &Action, ctx: &EnrichmentContext<'_>) -> JettonTransferInfo {
    let root = ctx.root_fact(action);
    let transfer = root.and_then(JettonTransferView::parse);

    JettonTransferInfo {
        amount: transfer.as_ref().and_then(JettonTransferView::amount),
        source: root.and_then(message_source).cloned(),
        destination: transfer
            .as_ref()
            .and_then(JettonTransferView::destination)
            .or_else(|| root.and_then(message_destination))
            .cloned(),
    }
}

fn describe_mint(action: &Action, ctx: &EnrichmentContext<'_>) -> JettonMintInfo {
    let amount_node = ctx.first_node_with_amount(action);

    JettonMintInfo {
        amount: amount_node.and_then(|node| node.coins("amount")),
        destination: amount_node
            .and_then(|node| node.address("destination"))
            .or_else(|| amount_node.and_then(message_destination))
            .cloned(),
    }
}
