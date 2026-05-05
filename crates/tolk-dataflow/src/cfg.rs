use rustc_hash::FxHashSet;
use std::fmt::Write;
use std::path::Path;
use tolk_resolver::Span;
use tolk_resolver::file_index::SymbolId;
use tolk_resolver::resolve_index::LocalDefId;

/// Stable identifier of a control-flow node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub usize);

impl NodeId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Stable identifier of a control-flow edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeId(pub usize);

impl EdgeId {
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// Classification of CFG nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowNodeKind {
    Entry,
    Exit,
    Nop,
    Expr,
    Condition,
    Assert,
    Return,
    Throw,
    Break,
    Continue,
    MatchPattern,
    CatchBinding,
    Join,
}

/// Classification of CFG edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    Unconditional,
    TrueBranch,
    FalseBranch,
    LoopBack,
    Break,
    Continue,
    Return,
    Throw,
    Exceptional,
}

/// Taint-oriented facts attached to a CFG node.
#[derive(Debug, Clone)]
pub struct MultiplicationOperationFact {
    /// Span of `*` operator token.
    pub operator_span: Span,
    /// Locals read by this multiplication expression.
    pub read_locals: FxHashSet<LocalDefId>,
    /// Spans of division operations used in lhs/rhs operands of this multiplication.
    pub division_operand_spans: Vec<Span>,
}

#[derive(Debug, Clone, Default)]
pub struct FlowNodeTaintFacts {
    /// Message roots directly referenced via field access in this node expression.
    pub direct_source_roots: FxHashSet<LocalDefId>,
    /// Whether this node proves admin authorization via sender check.
    pub has_admin_sender_check: bool,
    /// Whether this node performs storage write (`contract.setData`, `*.save()`).
    pub has_storage_write_sink: bool,
    /// Whether this node calls `random.initialize(...)` or `random.initializeBy(...)`.
    pub has_random_initialize_call: bool,
    /// Whether this node calls `random.uint256(...)` or `random.range(...)`.
    pub has_random_value_sink: bool,
    /// Whether this node contains division operator (`/`) in expression tree.
    pub has_division_operation: bool,
    /// Whether this node contains multiplication operator (`*`) in expression tree.
    pub has_multiplication_operation: bool,
    /// Whether this node contains multiplication whose lhs/rhs subtree has division.
    pub has_divide_before_multiply: bool,
    /// Multiplication expressions found in this node with per-expression facts.
    pub multiplication_operations: Vec<MultiplicationOperationFact>,
    /// Spans of division operations (`/`) found in this node expression.
    pub division_spans: Vec<Span>,
    /// Spans of division operations in direct RHS arithmetic assignment expressions.
    pub direct_assignment_division_spans: Vec<Span>,
    /// Global call targets referenced in this node.
    pub called_global_symbols: FxHashSet<SymbolId>,
}

/// Control-flow node plus dataflow-relevant read/write facts.
#[derive(Debug, Clone)]
pub struct FlowNode {
    pub id: NodeId,
    pub kind: FlowNodeKind,
    pub span: Option<Span>,
    pub reads: FxHashSet<LocalDefId>,
    pub writes: FxHashSet<LocalDefId>,
    pub taint: FlowNodeTaintFacts,
}

impl FlowNode {
    fn new(id: NodeId, kind: FlowNodeKind, span: Option<Span>) -> Self {
        Self {
            id,
            kind,
            span,
            reads: FxHashSet::default(),
            writes: FxHashSet::default(),
            taint: FlowNodeTaintFacts::default(),
        }
    }
}

/// Directed control-flow edge.
#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub id: EdgeId,
    pub from: NodeId,
    pub to: NodeId,
    pub kind: EdgeKind,
}

/// Built control-flow graph with predecessor/successor caches.
#[derive(Debug, Clone)]
pub struct ControlFlowGraph {
    entry: NodeId,
    exit: NodeId,
    nodes: Vec<FlowNode>,
    edges: Vec<FlowEdge>,
    succ: Vec<Vec<EdgeId>>,
    pred: Vec<Vec<EdgeId>>,
}

/// DOT renderer options for CFG export.
#[derive(Debug, Clone)]
pub struct DotOptions {
    pub rankdir: &'static str,
    pub include_spans: bool,
    pub include_reads_writes: bool,
}

impl Default for DotOptions {
    fn default() -> Self {
        Self {
            rankdir: "TB",
            include_spans: true,
            include_reads_writes: true,
        }
    }
}

impl Default for ControlFlowGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ControlFlowGraph {
    #[must_use]
    pub fn new() -> Self {
        let mut this = Self {
            entry: NodeId(0),
            exit: NodeId(0),
            nodes: Vec::new(),
            edges: Vec::new(),
            succ: Vec::new(),
            pred: Vec::new(),
        };

        let entry = this.add_node(FlowNodeKind::Entry, None);
        let exit = this.add_node(FlowNodeKind::Exit, None);
        this.entry = entry;
        this.exit = exit;

        this
    }

    #[must_use]
    pub const fn entry(&self) -> NodeId {
        self.entry
    }

    #[must_use]
    pub const fn exit(&self) -> NodeId {
        self.exit
    }

    #[must_use]
    pub const fn node_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub const fn edge_count(&self) -> usize {
        self.edges.len()
    }

    #[must_use]
    pub fn nodes(&self) -> &[FlowNode] {
        &self.nodes
    }

    #[must_use]
    pub fn edges(&self) -> &[FlowEdge] {
        &self.edges
    }

    #[must_use]
    pub fn node(&self, id: NodeId) -> &FlowNode {
        &self.nodes[id.index()]
    }

    #[must_use]
    pub fn node_mut(&mut self, id: NodeId) -> &mut FlowNode {
        &mut self.nodes[id.index()]
    }

    #[must_use]
    pub fn edge(&self, id: EdgeId) -> &FlowEdge {
        &self.edges[id.index()]
    }

    pub fn add_node(&mut self, kind: FlowNodeKind, span: Option<Span>) -> NodeId {
        let id = NodeId(self.nodes.len());
        self.nodes.push(FlowNode::new(id, kind, span));
        self.succ.push(Vec::new());
        self.pred.push(Vec::new());
        id
    }

    /// Adds an edge if such edge (same from, to, kind) does not exist yet.
    pub fn add_edge(&mut self, from: NodeId, to: NodeId, kind: EdgeKind) -> Option<EdgeId> {
        let already_present = self.succ[from.index()].iter().any(|edge_id| {
            let edge = &self.edges[edge_id.index()];
            edge.to == to && edge.kind == kind
        });

        if already_present {
            return None;
        }

        let id = EdgeId(self.edges.len());
        self.edges.push(FlowEdge { id, from, to, kind });
        self.succ[from.index()].push(id);
        self.pred[to.index()].push(id);
        Some(id)
    }

    pub fn successors(&self, node: NodeId) -> impl Iterator<Item = &FlowEdge> {
        self.succ[node.index()]
            .iter()
            .map(|edge_id| &self.edges[edge_id.index()])
    }

    pub fn predecessors(&self, node: NodeId) -> impl Iterator<Item = &FlowEdge> {
        self.pred[node.index()]
            .iter()
            .map(|edge_id| &self.edges[edge_id.index()])
    }

    /// Collects all locals touched by read/write sets in this graph.
    #[must_use]
    pub fn all_locals(&self) -> FxHashSet<LocalDefId> {
        let mut locals = FxHashSet::default();
        for node in &self.nodes {
            locals.extend(node.reads.iter().copied());
            locals.extend(node.writes.iter().copied());
        }
        locals
    }

    /// Renders this CFG to Graphviz DOT.
    #[must_use]
    pub fn to_dot(&self) -> String {
        self.to_dot_with_options(&DotOptions::default())
    }

    /// Renders this CFG to Graphviz DOT with custom options.
    #[must_use]
    pub fn to_dot_with_options(&self, options: &DotOptions) -> String {
        let mut dot = String::new();
        writeln!(dot, "digraph tolk_cfg {{").expect("write to string");
        writeln!(dot, "  rankdir={};", options.rankdir).expect("write to string");
        writeln!(dot, "  node [shape=box, fontname=\"Menlo\", fontsize=10];")
            .expect("write to string");
        writeln!(dot, "  edge [fontname=\"Menlo\", fontsize=9];").expect("write to string");

        for node in &self.nodes {
            let mut label = format!("n{}\n{:?}", node.id.index(), node.kind);

            if options.include_spans
                && let Some(span) = node.span
            {
                let _ = write!(label, "\nspan:{}-{}", span.start, span.end);
            }

            if options.include_reads_writes {
                let reads = format_locals(&node.reads);
                let writes = format_locals(&node.writes);
                let _ = write!(label, "\nR:[{reads}]\nW:[{writes}]");
            }

            let shape = match node.kind {
                FlowNodeKind::Entry | FlowNodeKind::Exit => "oval",
                FlowNodeKind::Condition | FlowNodeKind::MatchPattern => "diamond",
                FlowNodeKind::Join => "circle",
                _ => "box",
            };

            writeln!(
                dot,
                "  n{} [shape=\"{}\", label=\"{}\"];",
                node.id.index(),
                shape,
                escape_dot_label(&label)
            )
            .expect("write to string");
        }

        for edge in &self.edges {
            let color = match edge.kind {
                EdgeKind::Unconditional => "black",
                EdgeKind::TrueBranch => "forestgreen",
                EdgeKind::FalseBranch => "crimson",
                EdgeKind::LoopBack => "dodgerblue4",
                EdgeKind::Break => "orange3",
                EdgeKind::Continue => "goldenrod3",
                EdgeKind::Return => "mediumpurple4",
                EdgeKind::Throw => "orangered4",
                EdgeKind::Exceptional => "red3",
            };

            writeln!(
                dot,
                "  n{} -> n{} [label=\"{:?}\", color=\"{}\"];",
                edge.from.index(),
                edge.to.index(),
                edge.kind,
                color
            )
            .expect("write to string");
        }

        writeln!(dot, "}}").expect("write to string");
        dot
    }

    /// Writes Graphviz DOT for this CFG to `path`.
    pub fn write_dot<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        std::fs::write(path, self.to_dot())
    }

    /// Writes Graphviz DOT for this CFG using custom options.
    pub fn write_dot_with_options<P: AsRef<Path>>(
        &self,
        path: P,
        options: &DotOptions,
    ) -> std::io::Result<()> {
        std::fs::write(path, self.to_dot_with_options(options))
    }
}

fn escape_dot_label(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 8);
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

fn format_locals(locals: &FxHashSet<LocalDefId>) -> String {
    let mut items: Vec<String> = locals
        .iter()
        .map(|local| format!("f{}:l{}", local.file_id, local.local))
        .collect();
    items.sort_unstable();
    items.join(",")
}
