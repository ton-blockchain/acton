use super::super::{BaseActionKind, BaseMatch, BaseMatcher, TraceNode, opcode_matches};
use std::collections::BTreeSet;

pub(in crate::actions) struct JettonTransferMatcher;

impl BaseMatcher for JettonTransferMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "JettonTransfer") {
            return None;
        }

        let internal_transfer = root.find_child_by_opcode("JettonInternalTransfer")?;
        let excess = internal_transfer.find_child_by_opcode("Excess");
        let notify = internal_transfer.find_child_by_opcode("JettonNotify");

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
        if !opcode_matches(root, "JettonMint") {
            return None;
        }

        let internal_transfer = root.find_child_by_opcode("JettonInternalTransfer")?;
        let notification =
            internal_transfer.find_child_by_opcode("JettonWalletTransferNotification");
        let excess = internal_transfer.find_child_by_opcode("Excess");

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
