use super::super::{BaseActionKind, BaseMatch, BaseMatcher, TraceNode, opcode_matches, opcodes};
use std::collections::BTreeSet;

pub(in crate::actions) struct JettonTransferMatcher;

impl BaseMatcher for JettonTransferMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::JETTON_TRANSFER) {
            return None;
        }

        let internal_transfer = root.find_child_by_opcode(opcodes::JETTON_INTERNAL_TRANSFER)?;
        let excess = internal_transfer.find_child_by_opcode(opcodes::EXCESS);
        let notify = internal_transfer.find_child_by_opcode(opcodes::JETTON_NOTIFY);

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

pub(in crate::actions) struct JettonMintMatcher;

impl BaseMatcher for JettonMintMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::JETTON_MINT) {
            return None;
        }

        let internal_transfer = root.find_child_by_opcode(opcodes::JETTON_INTERNAL_TRANSFER)?;
        let notification =
            internal_transfer.find_child_by_opcode(opcodes::JETTON_WALLET_TRANSFER_NOTIFICATION);
        let excess = internal_transfer.find_child_by_opcode(opcodes::EXCESS);

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
