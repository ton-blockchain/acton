mod matchers;

use matchers::{
    DedustNativeSwapLegMatcher, DedustSwapMatcher, JettonMintMatcher, JettonTransferMatcher,
};
use std::collections::BTreeSet;

pub type NodeId = u64;
pub type BaseActionId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    pub root: TraceNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
    pub id: NodeId,
    pub opcode_name: Option<String>,
    pub children: Vec<TraceNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseActionKind {
    DedustNativeSwapLeg,
    JettonTransfer,
    JettonMint,
    TonTransfer,
    ContractCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseAction {
    pub id: BaseActionId,
    pub kind: BaseActionKind,
    pub nodes: BTreeSet<NodeId>,
    pub root_node: NodeId,
    pub user_facing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    DedustSwap,
    JettonTransfer,
    JettonMint,
    TonTransfer,
    ContractCall,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub kind: ActionKind,
    pub nodes: BTreeSet<NodeId>,
    pub base_actions: Vec<BaseActionId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    pub actions: Vec<Action>,
    pub base_actions: Vec<BaseAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseMatch {
    pub kind: BaseActionKind,
    pub nodes: BTreeSet<NodeId>,
    pub root_node: NodeId,
    pub user_facing: bool,
}

pub trait BaseMatcher {
    fn try_match(&self, root: &TraceNode) -> Option<BaseMatch>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeMatch {
    pub kind: ActionKind,
    pub base_actions: Vec<BaseActionId>,
    pub nodes: BTreeSet<NodeId>,
}

pub trait CompositeMatcher {
    fn try_match(&self, trace: &Trace, base_actions: &[BaseAction]) -> Vec<CompositeMatch>;
}

#[must_use]
pub fn extract_actions(trace: &Trace) -> Extraction {
    let mut base_actions = extract_base_actions(trace);
    add_fallback_base_actions(&trace.root, &mut base_actions);

    let composite_actions = extract_composite_actions(trace, &base_actions);
    let consumed_base_actions = composite_actions
        .iter()
        .flat_map(|action| action.base_actions.iter().copied())
        .collect::<BTreeSet<_>>();

    let primitive_actions = base_actions.iter().filter_map(|base_action| {
        if !base_action.user_facing || consumed_base_actions.contains(&base_action.id) {
            return None;
        }

        Some(Action {
            kind: action_kind_for_base(base_action.kind)?,
            nodes: base_action.nodes.clone(),
            base_actions: vec![base_action.id],
        })
    });

    let actions = composite_actions
        .into_iter()
        .map(|action| Action {
            kind: action.kind,
            nodes: action.nodes,
            base_actions: action.base_actions,
        })
        .chain(primitive_actions)
        .collect();

    Extraction {
        actions,
        base_actions,
    }
}

fn extract_base_actions(trace: &Trace) -> Vec<BaseAction> {
    let matchers: &[&dyn BaseMatcher] = &[
        &DedustNativeSwapLegMatcher,
        &JettonTransferMatcher,
        &JettonMintMatcher,
    ];

    let mut base_actions = Vec::new();
    let mut consumed_nodes = BTreeSet::new();
    collect_base_actions(
        &trace.root,
        matchers,
        &mut consumed_nodes,
        &mut base_actions,
    );
    base_actions
}

fn collect_base_actions(
    node: &TraceNode,
    matchers: &[&dyn BaseMatcher],
    consumed_nodes: &mut BTreeSet<NodeId>,
    base_actions: &mut Vec<BaseAction>,
) {
    if !consumed_nodes.contains(&node.id) {
        for matcher in matchers {
            let Some(base_match) = matcher.try_match(node) else {
                continue;
            };

            if base_match
                .nodes
                .iter()
                .any(|node_id| consumed_nodes.contains(node_id))
            {
                continue;
            }

            consumed_nodes.extend(base_match.nodes.iter().copied());
            base_actions.push(BaseAction {
                id: next_base_action_id(base_actions),
                kind: base_match.kind,
                nodes: base_match.nodes,
                root_node: base_match.root_node,
                user_facing: base_match.user_facing,
            });
            break;
        }
    }

    for child in &node.children {
        collect_base_actions(child, matchers, consumed_nodes, base_actions);
    }
}

fn add_fallback_base_actions(root: &TraceNode, base_actions: &mut Vec<BaseAction>) {
    let consumed_nodes = base_actions
        .iter()
        .flat_map(|action| action.nodes.iter().copied())
        .collect::<BTreeSet<_>>();

    for node in root.descendants_preorder() {
        if consumed_nodes.contains(&node.id) {
            continue;
        }

        let kind = if normalized_opcode(node).is_none() {
            BaseActionKind::TonTransfer
        } else {
            BaseActionKind::ContractCall
        };

        base_actions.push(BaseAction {
            id: next_base_action_id(base_actions),
            kind,
            nodes: BTreeSet::from([node.id]),
            root_node: node.id,
            user_facing: true,
        });
    }
}

fn extract_composite_actions(trace: &Trace, base_actions: &[BaseAction]) -> Vec<CompositeMatch> {
    let matchers: &[&dyn CompositeMatcher] = &[&DedustSwapMatcher];

    let mut composite_actions = Vec::new();
    let mut consumed_base_actions = BTreeSet::new();

    for matcher in matchers {
        for composite_match in matcher.try_match(trace, base_actions) {
            if composite_match
                .base_actions
                .iter()
                .any(|id| consumed_base_actions.contains(id))
            {
                continue;
            }

            consumed_base_actions.extend(composite_match.base_actions.iter().copied());
            composite_actions.push(composite_match);
        }
    }

    composite_actions
}

const fn next_base_action_id(base_actions: &[BaseAction]) -> BaseActionId {
    base_actions.len() as BaseActionId
}

const fn action_kind_for_base(kind: BaseActionKind) -> Option<ActionKind> {
    match kind {
        BaseActionKind::DedustNativeSwapLeg => None,
        BaseActionKind::JettonTransfer => Some(ActionKind::JettonTransfer),
        BaseActionKind::JettonMint => Some(ActionKind::JettonMint),
        BaseActionKind::TonTransfer => Some(ActionKind::TonTransfer),
        BaseActionKind::ContractCall => Some(ActionKind::ContractCall),
    }
}

impl TraceNode {
    fn descendants_preorder(&self) -> Vec<&Self> {
        let mut nodes = vec![self];
        for child in &self.children {
            nodes.extend(child.descendants_preorder());
        }
        nodes
    }

    pub(super) fn find_descendant_by_opcode(&self, opcode: &str) -> Option<&Self> {
        self.children.iter().find_map(|child| {
            if opcode_matches(child, opcode) {
                Some(child)
            } else {
                child.find_descendant_by_opcode(opcode)
            }
        })
    }

    pub(super) fn find_child_by_opcode(&self, opcode: &str) -> Option<&Self> {
        self.children
            .iter()
            .find(|child| opcode_matches(child, opcode))
    }

    pub(super) fn contains_descendant(&self, ancestor_id: NodeId, descendant_id: NodeId) -> bool {
        let Some(ancestor) = self.find_by_id(ancestor_id) else {
            return false;
        };

        ancestor
            .descendants_preorder()
            .into_iter()
            .any(|node| node.id == descendant_id)
    }

    fn find_by_id(&self, node_id: NodeId) -> Option<&Self> {
        if self.id == node_id {
            return Some(self);
        }

        self.children
            .iter()
            .find_map(|child| child.find_by_id(node_id))
    }
}

pub(super) fn opcode_matches(node: &TraceNode, expected: &str) -> bool {
    normalized_opcode(node).is_some_and(|opcode| opcode.eq_ignore_ascii_case(expected))
}

fn normalized_opcode(node: &TraceNode) -> Option<&str> {
    let opcode = node.opcode_name.as_deref()?.trim();
    if opcode.is_empty() {
        None
    } else {
        Some(opcode)
    }
}
