use super::super::{BaseActionKind, BaseMatch, BaseMatcher, TraceNode, opcode_matches};
use std::collections::BTreeSet;

pub(in crate::actions) struct PtonTransferMatcher;

impl BaseMatcher for PtonTransferMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "JettonTransfer") {
            return None;
        }

        let pton_transfer = root.find_child_by_opcode("PtonTonTransfer")?;
        let excess = root.find_child_by_opcode("Excess");

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
