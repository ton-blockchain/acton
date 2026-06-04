use super::super::{
    ActionKind, BaseActionGraph, BaseActionKind, BaseMatch, BaseMatcher, CompositeMatch,
    CompositeMatcher, TraceNode, opcode_matches,
};
use std::collections::BTreeSet;

pub(in crate::actions) struct StonfiSwapMatcher;

impl BaseMatcher for StonfiSwapMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "StonfiSwapV2") {
            return None;
        }

        let pay_to = root.find_child_by_opcode("StonfiPayToV2")?;

        Some(BaseMatch {
            kind: BaseActionKind::StonfiSwap,
            nodes: BTreeSet::from([root.id, pay_to.id]),
            root_node: root.id,
            user_facing: false,
        })
    }
}

impl CompositeMatcher for StonfiSwapMatcher {
    fn try_match(&self, graph: &BaseActionGraph<'_>) -> Vec<CompositeMatch> {
        graph
            .base_actions()
            .iter()
            .filter(|action| action.kind == BaseActionKind::StonfiSwap)
            .filter_map(|stonfi_swap| {
                let jetton_transfer = graph.children_of(stonfi_swap.id).find(|action| {
                    matches!(
                        action.kind,
                        BaseActionKind::JettonTransfer | BaseActionKind::PtonTransfer
                    )
                })?;

                let mut nodes = stonfi_swap.nodes.clone();
                nodes.extend(jetton_transfer.nodes.iter().copied());

                Some(CompositeMatch {
                    kind: ActionKind::StonfiSwap,
                    base_actions: vec![stonfi_swap.id, jetton_transfer.id],
                    nodes,
                })
            })
            .collect()
    }
}
