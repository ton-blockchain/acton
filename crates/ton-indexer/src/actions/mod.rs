mod matchers;

use matchers::{
    DedustJettonSwapLegMatcher, DedustNativeSwapLegMatcher, DedustPayoutMatcher, DedustSwapMatcher,
    JettonMintMatcher, JettonTransferMatcher, PtonTransferMatcher, StonfiSwapMatcher,
};
use std::collections::{BTreeMap, BTreeSet};

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
    DedustJettonSwapLeg,
    DedustPayout,
    StonfiSwap,
    PtonTransfer,
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
    DedustPayout,
    StonfiSwap,
    PtonTransfer,
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

#[derive(Debug)]
pub struct BaseActionGraph<'a> {
    base_actions: &'a [BaseAction],
    children: BTreeMap<BaseActionId, BTreeSet<BaseActionId>>,
}

impl<'a> BaseActionGraph<'a> {
    fn new(trace: &Trace, base_actions: &'a [BaseAction]) -> Self {
        let node_owners = base_actions
            .iter()
            .flat_map(|action| {
                action
                    .nodes
                    .iter()
                    .map(move |node_id| (*node_id, action.id))
            })
            .collect::<BTreeMap<_, _>>();
        let mut children = base_actions
            .iter()
            .map(|action| (action.id, BTreeSet::new()))
            .collect();

        collect_base_action_graph_edges(&trace.root, &node_owners, &mut children);

        Self {
            base_actions,
            children,
        }
    }

    #[must_use]
    pub const fn base_actions(&self) -> &[BaseAction] {
        self.base_actions
    }

    pub fn children_of(&self, action_id: BaseActionId) -> impl Iterator<Item = &BaseAction> + '_ {
        self.children
            .get(&action_id)
            .into_iter()
            .flat_map(|children| children.iter())
            .filter_map(|child_id| self.action(*child_id))
    }

    fn action(&self, action_id: BaseActionId) -> Option<&BaseAction> {
        self.base_actions
            .iter()
            .find(|action| action.id == action_id)
    }
}

pub trait CompositeMatcher {
    fn try_match(&self, graph: &BaseActionGraph<'_>) -> Vec<CompositeMatch>;
}

#[must_use]
pub fn extract_actions(trace: &Trace) -> Extraction {
    let mut base_actions = extract_base_actions(trace);
    add_fallback_base_actions(&trace.root, &mut base_actions);

    let base_action_graph = BaseActionGraph::new(trace, &base_actions);
    let composite_actions = extract_composite_actions(&base_action_graph);
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
        &DedustJettonSwapLegMatcher,
        &DedustPayoutMatcher,
        &StonfiSwapMatcher,
        &PtonTransferMatcher,
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

fn extract_composite_actions(graph: &BaseActionGraph<'_>) -> Vec<CompositeMatch> {
    let matchers: &[&dyn CompositeMatcher] = &[&StonfiSwapMatcher, &DedustSwapMatcher];

    let mut composite_actions = Vec::new();
    let mut consumed_base_actions = BTreeSet::new();

    for matcher in matchers {
        for composite_match in matcher.try_match(graph) {
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

fn collect_base_action_graph_edges(
    node: &TraceNode,
    node_owners: &BTreeMap<NodeId, BaseActionId>,
    children: &mut BTreeMap<BaseActionId, BTreeSet<BaseActionId>>,
) {
    let parent_owner = node_owners.get(&node.id).copied();

    for child in &node.children {
        let child_owner = node_owners.get(&child.id).copied();
        if let (Some(parent_owner), Some(child_owner)) = (parent_owner, child_owner)
            && parent_owner != child_owner
        {
            children
                .entry(parent_owner)
                .or_default()
                .insert(child_owner);
        }

        collect_base_action_graph_edges(child, node_owners, children);
    }
}

const fn next_base_action_id(base_actions: &[BaseAction]) -> BaseActionId {
    base_actions.len() as BaseActionId
}

const fn action_kind_for_base(kind: BaseActionKind) -> Option<ActionKind> {
    match kind {
        BaseActionKind::DedustNativeSwapLeg
        | BaseActionKind::DedustJettonSwapLeg
        | BaseActionKind::StonfiSwap => None,
        BaseActionKind::DedustPayout => Some(ActionKind::DedustPayout),
        BaseActionKind::PtonTransfer => Some(ActionKind::PtonTransfer),
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

    pub(super) fn find_child_by_opcode(&self, opcode: &str) -> Option<&Self> {
        self.children
            .iter()
            .find(|child| opcode_matches(child, opcode))
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
