use super::super::{
    ActionKind, BaseAction, BaseActionKind, BaseMatch, BaseMatcher, CompositeMatch,
    CompositeMatcher, Trace, TraceNode, opcode_matches,
};
use std::collections::BTreeSet;

pub(in crate::actions) struct DedustNativeSwapLegMatcher;

impl BaseMatcher for DedustNativeSwapLegMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "DedustVaultNativeV2Swap") {
            return None;
        }

        let pool_swap = root.find_descendant_by_opcode("DedustPoolV2SwapExternal")?;
        let payout = pool_swap.find_descendant_by_opcode("DedustPoolV2PayOutFromPool")?;
        let swap_event = pool_swap.find_descendant_by_opcode("DedustPoolV2SwapEvent");

        let mut nodes = BTreeSet::from([root.id, pool_swap.id, payout.id]);
        if let Some(swap_event) = swap_event {
            nodes.insert(swap_event.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustNativeSwapLeg,
            nodes,
            root_node: root.id,
            user_facing: false,
        })
    }
}

pub(in crate::actions) struct DedustSwapMatcher;

impl CompositeMatcher for DedustSwapMatcher {
    fn try_match(&self, trace: &Trace, base_actions: &[BaseAction]) -> Vec<CompositeMatch> {
        base_actions
            .iter()
            .filter(|action| action.kind == BaseActionKind::DedustNativeSwapLeg)
            .filter_map(|swap_leg| {
                let jetton_transfer = base_actions
                    .iter()
                    .filter(|action| action.kind == BaseActionKind::JettonTransfer)
                    .find(|action| {
                        swap_leg.nodes.iter().any(|node_id| {
                            trace.root.contains_descendant(*node_id, action.root_node)
                        })
                    })?;

                let mut nodes = swap_leg.nodes.clone();
                nodes.extend(jetton_transfer.nodes.iter().copied());

                Some(CompositeMatch {
                    kind: ActionKind::DedustSwap,
                    base_actions: vec![swap_leg.id, jetton_transfer.id],
                    nodes,
                })
            })
            .collect()
    }
}
