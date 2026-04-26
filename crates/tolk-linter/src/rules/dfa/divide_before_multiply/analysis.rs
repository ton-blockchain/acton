use rustc_hash::{FxHashMap, FxHashSet};
use tolk_dataflow::cfg::{ControlFlowGraph, NodeId};
use tolk_dataflow::solver::{DataflowAnalysis, DataflowResult, Direction, solve};
use tolk_resolver::Span;
use tolk_resolver::resolve_index::LocalDefId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DivisionOrigin {
    pub node: NodeId,
    pub span: Span,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DivisionTaintState {
    pub taint_origins: FxHashMap<LocalDefId, DivisionOrigin>,
}

const fn local_sort_key(local: LocalDefId) -> (u32, u32) {
    (local.file_id, local.local)
}

fn sorted_locals(locals: &FxHashSet<LocalDefId>) -> Vec<LocalDefId> {
    let mut values = locals.iter().copied().collect::<Vec<_>>();
    values.sort_by_key(|local| local_sort_key(*local));
    values
}

const fn origin_sort_key(origin: &DivisionOrigin) -> (u32, u32, usize) {
    (origin.span.start, origin.span.end, origin.node.index())
}

const fn multiplication_sort_key(span: Span) -> (u32, u32) {
    (span.start, span.end)
}

fn prefer_origin(candidate: DivisionOrigin, current: DivisionOrigin) -> bool {
    origin_sort_key(&candidate) < origin_sort_key(&current)
}

fn insert_origin(
    taint_origins: &mut FxHashMap<LocalDefId, DivisionOrigin>,
    local: LocalDefId,
    origin: DivisionOrigin,
) -> bool {
    if let Some(current) = taint_origins.get(&local).copied()
        && !prefer_origin(origin, current)
    {
        return false;
    }
    taint_origins.insert(local, origin);
    true
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DivisionTaintAnalysis;

impl DataflowAnalysis for DivisionTaintAnalysis {
    type State = DivisionTaintState;

    fn direction(&self) -> Direction {
        Direction::Forward
    }

    fn bottom(&self, _cfg: &ControlFlowGraph) -> Self::State {
        DivisionTaintState::default()
    }

    fn boundary(&self, _cfg: &ControlFlowGraph) -> Self::State {
        DivisionTaintState::default()
    }

    fn merge(&self, into: &mut Self::State, other: &Self::State) -> bool {
        let mut changed = false;
        for (local, origin) in &other.taint_origins {
            changed |= insert_origin(&mut into.taint_origins, *local, *origin);
        }
        changed
    }

    fn transfer(&self, cfg: &ControlFlowGraph, node: NodeId, state: &Self::State) -> Self::State {
        let flow_node = cfg.node(node);
        let mut next = state.clone();

        let read_origin = sorted_locals(&flow_node.reads)
            .into_iter()
            .filter_map(|local| state.taint_origins.get(&local).copied())
            .min_by_key(origin_sort_key);

        let division_origin = flow_node
            .taint
            .direct_assignment_division_spans
            .iter()
            .copied()
            .min_by_key(|span| (span.start, span.end))
            .map(|span| DivisionOrigin { node, span });

        let origin = division_origin.or(read_origin);
        if let Some(origin) = origin {
            for local in sorted_locals(&flow_node.writes) {
                let _ = insert_origin(&mut next.taint_origins, local, origin);
            }
        }

        next
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DivideBeforeMultiplyKind {
    Direct,
    Tainted,
}

#[derive(Debug, Clone)]
pub struct DivideBeforeMultiplyIssue {
    pub node: NodeId,
    pub span: Option<Span>,
    pub kind: DivideBeforeMultiplyKind,
    pub division_origin: Option<DivisionOrigin>,
}

#[must_use]
pub fn run_division_taint(cfg: &ControlFlowGraph) -> DataflowResult<DivisionTaintState> {
    solve(cfg, &DivisionTaintAnalysis)
}

#[must_use]
pub fn find_issues(
    cfg: &ControlFlowGraph,
    dataflow: &DataflowResult<DivisionTaintState>,
) -> Vec<DivideBeforeMultiplyIssue> {
    let mut issues = Vec::new();

    for node in cfg.nodes() {
        if node.taint.multiplication_operations.is_empty() {
            continue;
        }

        let direct_multiplication = node
            .taint
            .multiplication_operations
            .iter()
            .filter(|op| !op.division_operand_spans.is_empty())
            .min_by_key(|op| multiplication_sort_key(op.operator_span));
        if let Some(op) = direct_multiplication {
            let division_origin = op
                .division_operand_spans
                .iter()
                .copied()
                .min_by_key(|span| (span.start, span.end))
                .map(|span| DivisionOrigin {
                    node: node.id,
                    span,
                })
                .or_else(|| {
                    node.taint.has_division_operation.then_some(DivisionOrigin {
                        node: node.id,
                        span: node.span?,
                    })
                });

            issues.push(DivideBeforeMultiplyIssue {
                node: node.id,
                span: Some(op.operator_span),
                kind: DivideBeforeMultiplyKind::Direct,
                division_origin,
            });
            continue;
        }

        let tainted_multiplication = node
            .taint
            .multiplication_operations
            .iter()
            .filter_map(|op| {
                let origin = sorted_locals(&op.read_locals)
                    .into_iter()
                    .filter_map(|local| dataflow.in_at(node.id).taint_origins.get(&local).copied())
                    .min_by_key(origin_sort_key)?;
                if origin.node == node.id {
                    return None;
                }
                Some((op, origin))
            })
            .min_by_key(|(op, origin)| {
                let (start, end) = multiplication_sort_key(op.operator_span);
                (start, end, origin_sort_key(origin))
            });

        let Some((op, division_origin)) = tainted_multiplication else {
            continue;
        };

        issues.push(DivideBeforeMultiplyIssue {
            node: node.id,
            span: Some(op.operator_span),
            kind: DivideBeforeMultiplyKind::Tainted,
            division_origin: Some(division_origin),
        });
    }

    issues
}

#[derive(Debug, Clone)]
pub struct DivideBeforeMultiplyReport {
    pub dataflow: DataflowResult<DivisionTaintState>,
    pub issues: Vec<DivideBeforeMultiplyIssue>,
}

#[must_use]
pub fn run(cfg: &ControlFlowGraph) -> DivideBeforeMultiplyReport {
    let dataflow = run_division_taint(cfg);
    let issues = find_issues(cfg, &dataflow);
    DivideBeforeMultiplyReport { dataflow, issues }
}
