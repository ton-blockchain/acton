use super::super::{BaseActionKind, BaseMatch, BaseMatcher, TraceNode, opcode_matches, opcodes};
use std::collections::BTreeSet;

pub(in crate::actions) struct PtonTransferMatcher;

impl BaseMatcher for PtonTransferMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::JETTON_TRANSFER) {
            return None;
        }

        let pton_transfer = root.find_child_by_opcode(opcodes::PTON_WALLET_V2_TON_TRANSFER)?;
        let excess = root.find_child_by_opcode(opcodes::EXCESS);

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
