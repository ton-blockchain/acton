use super::super::enrichment::{ActionInfo, ActionInfoBox, EnrichmentContext};
use super::super::{
    Action, ActionKind, ActionProvider, BaseActionGraph, BaseActionKind, BaseMatch, BaseMatcher,
    CompositeMatch, CompositeMatcher, TraceNode, opcode_matches, opcodes,
};
use std::collections::BTreeSet;

pub(in crate::actions) struct StonfiProvider;

impl ActionProvider for StonfiProvider {
    fn base_matchers(&self) -> &'static [&'static dyn BaseMatcher] {
        &[&StonfiSwapMatcher]
    }

    fn composite_matchers(&self) -> &'static [&'static dyn CompositeMatcher] {
        &[&StonfiSwapMatcher]
    }

    fn describe(&self, action: &Action, _ctx: &EnrichmentContext<'_>) -> Option<ActionInfoBox> {
        if action.kind != ActionKind::StonfiSwap {
            return None;
        }

        Some(Box::new(StonfiSwapInfo))
    }
}

struct StonfiSwapMatcher;

impl BaseMatcher for StonfiSwapMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch> {
        if !opcode_matches(root, opcodes::STONFI_SWAP_V2) {
            return None;
        }

        let pay_to = root.child(opcodes::STONFI_PAY_TO_V2)?;

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

#[derive(Debug, Clone)]
struct StonfiSwapInfo;

impl ActionInfo for StonfiSwapInfo {
    fn render(&self) -> String {
        "STON.fi swap".to_owned()
    }
}
