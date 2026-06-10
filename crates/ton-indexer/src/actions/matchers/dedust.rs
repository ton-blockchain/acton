use super::super::{
    ActionKind, BaseActionGraph, BaseActionKind, BaseMatch, BaseMatcher, CompositeMatch,
    CompositeMatcher, TraceNode, opcode_matches, opcodes,
};
use std::collections::BTreeSet;

pub(in crate::actions) struct DedustNativeSwapLegMatcher;

impl BaseMatcher for DedustNativeSwapLegMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::DEDUST_VAULT_NATIVE_V2_SWAP) {
            return None;
        }

        let pool_swap = root.find_child_by_opcode(opcodes::DEDUST_POOL_V2_SWAP_EXTERNAL)?;
        let payout = pool_swap.find_child_by_opcode(opcodes::DEDUST_POOL_V2_PAY_OUT_FROM_POOL)?;
        let swap_event = pool_swap.find_child_by_opcode(opcodes::DEDUST_POOL_V2_SWAP_EVENT);

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
        if !opcode_matches(root, opcodes::DEDUST_POOL_V2_SWAP_EXTERNAL) {
            return None;
        }

        let payout = root.find_child_by_opcode(opcodes::DEDUST_POOL_V2_PAY_OUT_FROM_POOL)?;
        let swap_event = root.find_child_by_opcode(opcodes::DEDUST_POOL_V2_SWAP_EVENT);

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
        if !opcode_matches(root, opcodes::DEDUST_PAYOUT) {
            return None;
        }

        let ton_excesses = root.find_child_by_opcode(opcodes::DEDUST_TON_EXCESSES);
        let ton_pay = root.find_child_by_opcode(opcodes::DEDUST_TON_PAY);

        let mut nodes = BTreeSet::from([root.id]);
        if let Some(ton_excesses) = ton_excesses {
            nodes.insert(ton_excesses.id);
        }
        if let Some(ton_pay) = ton_pay {
            nodes.insert(ton_pay.id);
        }

        Some(BaseMatch {
            kind: BaseActionKind::DedustPayout,
            nodes,
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
