use super::super::{
    ActionKind, BaseActionGraph, BaseActionKind, BaseMatch, BaseMatcher, CompositeMatch,
    CompositeMatcher, TraceNode, opcode_matches,
};
use std::collections::BTreeSet;

pub(in crate::actions) struct DedustNativeSwapLegMatcher;

impl BaseMatcher for DedustNativeSwapLegMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "DedustVaultNativeV2Swap") {
            return None;
        }

        let pool_swap = root.find_child_by_opcode("DedustPoolV2SwapExternal")?;
        let payout = pool_swap.find_child_by_opcode("DedustPoolV2PayOutFromPool")?;
        let swap_event = pool_swap.find_child_by_opcode("DedustPoolV2SwapEvent");

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

pub(in crate::actions) struct DedustJettonSwapLegMatcher;

impl BaseMatcher for DedustJettonSwapLegMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "DedustPoolV2SwapExternal") {
            return None;
        }

        let payout = root.find_child_by_opcode("DedustPoolV2PayOutFromPool")?;
        let swap_event = root.find_child_by_opcode("DedustPoolV2SwapEvent");

        let mut nodes = BTreeSet::from([root.id, payout.id]);
        if let Some(swap_event) = swap_event {
            nodes.insert(swap_event.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustJettonSwapLeg,
            nodes,
            root_node: root.id,
            user_facing: false,
        })
    }
}

pub(in crate::actions) struct DedustPayoutMatcher;

impl BaseMatcher for DedustPayoutMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, "DedustPayout") {
            return None;
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustPayout,
            nodes: BTreeSet::from([root.id]),
            root_node: root.id,
            user_facing: true,
        })
    }
}

pub(in crate::actions) struct DedustSwapMatcher;

impl CompositeMatcher for DedustSwapMatcher {
    fn try_match(&self, graph: &BaseActionGraph<'_>) -> Vec<CompositeMatch> {
        graph
            .base_actions()
            .iter()
            .filter(|action| {
                matches!(
                    action.kind,
                    BaseActionKind::DedustNativeSwapLeg | BaseActionKind::DedustJettonSwapLeg
                )
            })
            .filter_map(|swap_leg| {
                let payout_action = graph.children_of(swap_leg.id).find(|action| {
                    matches!(
                        action.kind,
                        BaseActionKind::JettonTransfer
                            | BaseActionKind::PtonTransfer
                            | BaseActionKind::DedustPayout
                    )
                })?;

                let mut nodes = swap_leg.nodes.clone();
                nodes.extend(payout_action.nodes.iter().copied());

                Some(CompositeMatch {
                    kind: ActionKind::DedustSwap,
                    base_actions: vec![swap_leg.id, payout_action.id],
                    nodes,
                })
            })
            .collect()
    }
}
