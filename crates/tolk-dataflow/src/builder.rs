use crate::cfg::{ControlFlowGraph, EdgeKind, FlowNodeKind, MultiplicationOperationFact, NodeId};
use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;
use std::sync::Arc;
use tolk_resolver::file_index::AstNodeSpanExt;
use tolk_resolver::file_index::SymbolId;
use tolk_resolver::resolve_index::{FileResolveIndex, LocalDefId, LocalDefKind, Resolved};
use tolk_syntax::ast::Node;
use tolk_syntax::ast::NodeTraversalExt;
use tolk_syntax::{
    Assert, Assign, AstNode, AstNodeBytesKind, Bin, Call, DotAccess, DotAccessField, Expr,
    FuncBody, FunctionLike, HasName, IfAlt, InstanceArg, Match, MatchArmBody, MatchPattern, Paren,
    SetAssign, Stmt, TopLevel, VarDeclPattern,
};

/// Builds CFG for supported top-level declarations (`fun`, `method`, `get fun`).
#[must_use]
pub fn build_cfg_for_top_level(
    top_level: &TopLevel<'_>,
    resolve_index: &FileResolveIndex,
) -> Option<ControlFlowGraph> {
    build_cfg_for_top_level_with_source(top_level, resolve_index, None)
}

/// Builds CFG for supported top-level declarations (`fun`, `method`, `get fun`),
/// additionally taking source text for analyses that need identifier text.
#[must_use]
pub fn build_cfg_for_top_level_with_source(
    top_level: &TopLevel<'_>,
    resolve_index: &FileResolveIndex,
    source: Option<&str>,
) -> Option<ControlFlowGraph> {
    match top_level {
        TopLevel::Func(func) => build_cfg_for_function_with_source(func, resolve_index, source),
        TopLevel::Method(method) => {
            build_cfg_for_function_with_source(method, resolve_index, source)
        }
        TopLevel::GetMethod(get_method) => {
            build_cfg_for_function_with_source(get_method, resolve_index, source)
        }
        _ => None,
    }
}

/// Builds CFG for function-like declaration with block body.
#[must_use]
pub fn build_cfg_for_function<'tree, F: FunctionLike<'tree>>(
    function: &F,
    resolve_index: &FileResolveIndex,
) -> Option<ControlFlowGraph> {
    build_cfg_for_function_with_source(function, resolve_index, None)
}

/// Builds CFG for function-like declaration with block body, with optional source text.
#[must_use]
pub fn build_cfg_for_function_with_source<'tree, F: FunctionLike<'tree>>(
    function: &F,
    resolve_index: &FileResolveIndex,
    source: Option<&str>,
) -> Option<ControlFlowGraph> {
    let body = function.body()?;
    let FuncBody::Block(block) = body else {
        return None;
    };

    let mut builder = CfgBuilder::new(resolve_index, source);
    let fragment = builder.build_block_fragment(block);

    builder
        .cfg
        .add_edge(builder.cfg.entry(), fragment.entry, EdgeKind::Unconditional);
    for exit in fragment.exits {
        builder
            .cfg
            .add_edge(exit, builder.cfg.exit(), EdgeKind::Unconditional);
    }

    Some(builder.cfg)
}

#[derive(Debug, Clone)]
struct Fragment {
    entry: NodeId,
    exits: Vec<NodeId>,
    nodes: Vec<NodeId>,
}

impl Fragment {
    fn single(node: NodeId) -> Self {
        Self {
            entry: node,
            exits: vec![node],
            nodes: vec![node],
        }
    }

    fn terminal(node: NodeId) -> Self {
        Self {
            entry: node,
            exits: Vec::new(),
            nodes: vec![node],
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct LoopContext {
    break_target: NodeId,
    continue_target: NodeId,
}

#[derive(Debug)]
struct CfgBuilder<'idx> {
    cfg: ControlFlowGraph,
    loops: Vec<LoopContext>,
    exception_targets: Vec<NodeId>,
    collector: UseDefCollector<'idx>,
    inbound_message_roots: FxHashSet<LocalDefId>,
    message_roots: FxHashSet<LocalDefId>,
}

impl<'idx> CfgBuilder<'idx> {
    fn new(resolve_index: &'idx FileResolveIndex, source: Option<&'idx str>) -> Self {
        let collector = UseDefCollector::new(resolve_index, source);
        let mut inbound_message_roots = FxHashSet::default();
        let mut message_roots = FxHashSet::default();
        for local in &resolve_index.locals {
            if matches!(local.kind, LocalDefKind::Param { .. }) && local.name.as_ref() == "in" {
                inbound_message_roots.insert(local.id);
                message_roots.insert(local.id);
            }
        }

        Self {
            cfg: ControlFlowGraph::new(),
            loops: Vec::new(),
            exception_targets: Vec::new(),
            collector,
            inbound_message_roots,
            message_roots,
        }
    }

    fn make_nop_fragment(&mut self, span: Option<tolk_resolver::Span>) -> Fragment {
        let node = self.cfg.add_node(FlowNodeKind::Nop, span);
        Fragment::single(node)
    }

    fn append_fragments(&mut self, mut left: Fragment, right: Fragment) -> Fragment {
        for exit in &left.exits {
            self.cfg
                .add_edge(*exit, right.entry, EdgeKind::Unconditional);
        }
        left.nodes.extend(right.nodes.iter().copied());
        left.exits = right.exits;
        left
    }

    fn build_block_fragment(&mut self, block: tolk_syntax::Block<'_>) -> Fragment {
        let mut stmt_iter = block.stmts();
        let Some(first_stmt) = stmt_iter.next() else {
            return self.make_nop_fragment(Some(block.span()));
        };

        let mut fragment = self.build_stmt_fragment(first_stmt);

        for stmt in stmt_iter {
            let next = self.build_stmt_fragment(stmt);
            fragment = self.append_fragments(fragment, next);
        }

        fragment
    }

    fn build_stmt_fragment(&mut self, stmt: Stmt<'_>) -> Fragment {
        match stmt {
            Stmt::ExprStmt(expr_stmt) => {
                if let Some(expr) = expr_stmt.expr() {
                    let node = self.cfg.add_node(FlowNodeKind::Expr, Some(expr.span()));
                    self.collect_expr_into_node(node, expr, AccessMode::Read);
                    Fragment::single(node)
                } else {
                    self.make_nop_fragment(Some(expr_stmt.span()))
                }
            }
            Stmt::Block(block_stmt) => self.build_block_fragment(block_stmt),
            Stmt::If(if_stmt) => self.build_if_fragment(if_stmt),
            Stmt::While(while_stmt) => self.build_while_fragment(while_stmt),
            Stmt::Repeat(repeat_stmt) => self.build_repeat_fragment(repeat_stmt),
            Stmt::TryCatch(try_catch_stmt) => self.build_try_catch_fragment(try_catch_stmt),
            Stmt::Return(return_stmt) => self.build_return_fragment(return_stmt),
            Stmt::DoWhile(do_while_stmt) => self.build_do_while_fragment(do_while_stmt),
            Stmt::Break(break_stmt) => self.build_break_fragment(break_stmt),
            Stmt::Continue(continue_stmt) => self.build_continue_fragment(continue_stmt),
            Stmt::Throw(throw_stmt) => self.build_throw_fragment(throw_stmt),
            Stmt::Assert(assert_stmt) => self.build_assert_fragment(assert_stmt),
            Stmt::Match(match_stmt) => self.build_match_stmt_fragment(match_stmt),
            Stmt::EmptyStmt(empty) => self.make_nop_fragment(Some(empty.span())),
            Stmt::Unmapped(unmapped) => {
                self.make_nop_fragment(Some(tolk_resolver::Span::from_syntax(&unmapped.0)))
            }
        }
    }

    fn build_if_fragment(&mut self, if_stmt: tolk_syntax::If<'_>) -> Fragment {
        let cond_span = if_stmt.condition().map(|cond| cond.span());
        let cond_node = self.cfg.add_node(
            FlowNodeKind::Condition,
            cond_span.or_else(|| Some(if_stmt.span())),
        );

        if let Some(condition) = if_stmt.condition() {
            self.collect_expr_into_node(cond_node, condition, AccessMode::Read);
        }

        let then_fragment = if let Some(body) = if_stmt.body() {
            self.build_block_fragment(body)
        } else {
            self.make_nop_fragment(Some(if_stmt.span()))
        };

        let else_fragment = match if_stmt.alternative() {
            Some(IfAlt::If(else_if)) => self.build_stmt_fragment(Stmt::If(else_if)),
            Some(IfAlt::Block(else_block)) => self.build_block_fragment(else_block),
            None => self.make_nop_fragment(Some(if_stmt.span())),
        };

        self.cfg
            .add_edge(cond_node, then_fragment.entry, EdgeKind::TrueBranch);
        self.cfg
            .add_edge(cond_node, else_fragment.entry, EdgeKind::FalseBranch);

        let mut nodes = vec![cond_node];
        nodes.extend(then_fragment.nodes.iter().copied());
        nodes.extend(else_fragment.nodes.iter().copied());

        let mut exits = then_fragment.exits;
        exits.extend(else_fragment.exits);

        Fragment {
            entry: cond_node,
            exits,
            nodes,
        }
    }

    fn build_while_fragment(&mut self, while_stmt: tolk_syntax::While<'_>) -> Fragment {
        let cond_span = while_stmt.condition().map(|cond| cond.span());
        let cond_node = self.cfg.add_node(
            FlowNodeKind::Condition,
            cond_span.or_else(|| Some(while_stmt.span())),
        );

        if let Some(condition) = while_stmt.condition() {
            self.collect_expr_into_node(cond_node, condition, AccessMode::Read);
        }

        let after_loop = self
            .cfg
            .add_node(FlowNodeKind::Join, Some(while_stmt.span()));

        self.loops.push(LoopContext {
            break_target: after_loop,
            continue_target: cond_node,
        });

        let body_fragment = if let Some(body) = while_stmt.body() {
            self.build_block_fragment(body)
        } else {
            self.make_nop_fragment(Some(while_stmt.span()))
        };

        self.loops.pop();

        self.cfg
            .add_edge(cond_node, body_fragment.entry, EdgeKind::TrueBranch);
        self.cfg
            .add_edge(cond_node, after_loop, EdgeKind::FalseBranch);

        for exit in &body_fragment.exits {
            self.cfg.add_edge(*exit, cond_node, EdgeKind::LoopBack);
        }

        let mut nodes = vec![cond_node, after_loop];
        nodes.extend(body_fragment.nodes.iter().copied());

        Fragment {
            entry: cond_node,
            exits: vec![after_loop],
            nodes,
        }
    }

    fn build_repeat_fragment(&mut self, repeat_stmt: tolk_syntax::Repeat<'_>) -> Fragment {
        let count_span = repeat_stmt.count().map(|count| count.span());
        let count_node = self.cfg.add_node(
            FlowNodeKind::Condition,
            count_span.or_else(|| Some(repeat_stmt.span())),
        );

        if let Some(count) = repeat_stmt.count() {
            self.collect_expr_into_node(count_node, count, AccessMode::Read);
        }

        let after_loop = self
            .cfg
            .add_node(FlowNodeKind::Join, Some(repeat_stmt.span()));

        self.loops.push(LoopContext {
            break_target: after_loop,
            continue_target: count_node,
        });

        let body_fragment = if let Some(body) = repeat_stmt.body() {
            self.build_block_fragment(body)
        } else {
            self.make_nop_fragment(Some(repeat_stmt.span()))
        };

        self.loops.pop();

        self.cfg
            .add_edge(count_node, body_fragment.entry, EdgeKind::TrueBranch);
        self.cfg
            .add_edge(count_node, after_loop, EdgeKind::FalseBranch);

        for exit in &body_fragment.exits {
            self.cfg.add_edge(*exit, count_node, EdgeKind::LoopBack);
        }

        let mut nodes = vec![count_node, after_loop];
        nodes.extend(body_fragment.nodes.iter().copied());

        Fragment {
            entry: count_node,
            exits: vec![after_loop],
            nodes,
        }
    }

    fn build_do_while_fragment(&mut self, do_while_stmt: tolk_syntax::DoWhile<'_>) -> Fragment {
        let cond_span = do_while_stmt.condition().map(|cond| cond.span());
        let cond_node = self.cfg.add_node(
            FlowNodeKind::Condition,
            cond_span.or_else(|| Some(do_while_stmt.span())),
        );

        if let Some(condition) = do_while_stmt.condition() {
            self.collect_expr_into_node(cond_node, condition, AccessMode::Read);
        }

        let after_loop = self
            .cfg
            .add_node(FlowNodeKind::Join, Some(do_while_stmt.span()));

        self.loops.push(LoopContext {
            break_target: after_loop,
            continue_target: cond_node,
        });

        let body_fragment = if let Some(body) = do_while_stmt.body() {
            self.build_block_fragment(body)
        } else {
            self.make_nop_fragment(Some(do_while_stmt.span()))
        };

        self.loops.pop();

        for exit in &body_fragment.exits {
            self.cfg.add_edge(*exit, cond_node, EdgeKind::Unconditional);
        }

        self.cfg
            .add_edge(cond_node, body_fragment.entry, EdgeKind::LoopBack);
        self.cfg
            .add_edge(cond_node, after_loop, EdgeKind::FalseBranch);

        let mut nodes = vec![cond_node, after_loop];
        nodes.extend(body_fragment.nodes.iter().copied());

        Fragment {
            entry: body_fragment.entry,
            exits: vec![after_loop],
            nodes,
        }
    }

    fn build_try_catch_fragment(&mut self, try_catch_stmt: tolk_syntax::TryCatch<'_>) -> Fragment {
        let catch_fragment = if let Some(catch_clause) = try_catch_stmt.catch() {
            self.build_catch_fragment(catch_clause)
        } else {
            self.make_nop_fragment(Some(try_catch_stmt.span()))
        };

        self.exception_targets.push(catch_fragment.entry);
        let try_fragment = if let Some(try_body) = try_catch_stmt.body() {
            self.build_block_fragment(try_body)
        } else {
            self.make_nop_fragment(Some(try_catch_stmt.span()))
        };
        self.exception_targets.pop();

        for node in &try_fragment.nodes {
            if self.node_may_throw(*node) {
                self.cfg
                    .add_edge(*node, catch_fragment.entry, EdgeKind::Exceptional);
            }
        }

        let join = self
            .cfg
            .add_node(FlowNodeKind::Join, Some(try_catch_stmt.span()));

        for exit in &try_fragment.exits {
            self.cfg.add_edge(*exit, join, EdgeKind::Unconditional);
        }
        for exit in &catch_fragment.exits {
            self.cfg.add_edge(*exit, join, EdgeKind::Unconditional);
        }

        let mut nodes = vec![join];
        nodes.extend(try_fragment.nodes.iter().copied());
        nodes.extend(catch_fragment.nodes.iter().copied());

        Fragment {
            entry: try_fragment.entry,
            exits: vec![join],
            nodes,
        }
    }

    fn build_catch_fragment(&mut self, catch_clause: tolk_syntax::CatchClause<'_>) -> Fragment {
        let catch_binding = self
            .cfg
            .add_node(FlowNodeKind::CatchBinding, Some(catch_clause.span()));

        let writes = &mut self.cfg.node_mut(catch_binding).writes;
        if let Some(var1) = catch_clause.catch_var1() {
            self.collector
                .collect_definition_ident(var1.syntax(), writes);
        }
        if let Some(var2) = catch_clause.catch_var2() {
            self.collector
                .collect_definition_ident(var2.syntax(), writes);
        }

        let body_fragment = if let Some(catch_body) = catch_clause.body() {
            self.build_block_fragment(catch_body)
        } else {
            self.make_nop_fragment(Some(catch_clause.span()))
        };

        self.cfg
            .add_edge(catch_binding, body_fragment.entry, EdgeKind::Unconditional);

        let mut nodes = vec![catch_binding];
        nodes.extend(body_fragment.nodes.iter().copied());

        Fragment {
            entry: catch_binding,
            exits: body_fragment.exits,
            nodes,
        }
    }

    fn build_return_fragment(&mut self, return_stmt: tolk_syntax::Return<'_>) -> Fragment {
        let node = self
            .cfg
            .add_node(FlowNodeKind::Return, Some(return_stmt.span()));

        if let Some(expr) = return_stmt.expr() {
            self.collect_expr_into_node(node, expr, AccessMode::Read);
        }

        self.cfg.add_edge(node, self.cfg.exit(), EdgeKind::Return);
        Fragment::terminal(node)
    }

    fn build_throw_fragment(&mut self, throw_stmt: tolk_syntax::Throw<'_>) -> Fragment {
        let node = self
            .cfg
            .add_node(FlowNodeKind::Throw, Some(throw_stmt.span()));

        if let Some(expr) = throw_stmt.expr() {
            self.collect_expr_into_node(node, expr, AccessMode::Read);
        }

        if let Some(catch_target) = self.exception_targets.last().copied() {
            self.cfg.add_edge(node, catch_target, EdgeKind::Exceptional);
        } else {
            self.cfg.add_edge(node, self.cfg.exit(), EdgeKind::Throw);
        }

        Fragment::terminal(node)
    }

    fn build_assert_fragment(&mut self, assert_stmt: Assert<'_>) -> Fragment {
        let node = self
            .cfg
            .add_node(FlowNodeKind::Assert, Some(assert_stmt.span()));

        if let Some(condition) = assert_stmt.condition() {
            self.collect_expr_into_node(node, condition, AccessMode::Read);
        }
        if let Some(exc) = assert_stmt.expr() {
            self.collect_expr_into_node(node, exc, AccessMode::Read);
        }

        if let Some(catch_target) = self.exception_targets.last().copied() {
            self.cfg.add_edge(node, catch_target, EdgeKind::Exceptional);
        } else {
            self.cfg.add_edge(node, self.cfg.exit(), EdgeKind::Throw);
        }

        Fragment::single(node)
    }

    fn build_break_fragment(&mut self, break_stmt: tolk_syntax::Break<'_>) -> Fragment {
        let node = self
            .cfg
            .add_node(FlowNodeKind::Break, Some(break_stmt.span()));

        if let Some(loop_ctx) = self.loops.last().copied() {
            self.cfg
                .add_edge(node, loop_ctx.break_target, EdgeKind::Break);
        } else {
            self.cfg.add_edge(node, self.cfg.exit(), EdgeKind::Break);
        }

        Fragment::terminal(node)
    }

    fn build_continue_fragment(&mut self, continue_stmt: tolk_syntax::Continue<'_>) -> Fragment {
        let node = self
            .cfg
            .add_node(FlowNodeKind::Continue, Some(continue_stmt.span()));

        if let Some(loop_ctx) = self.loops.last().copied() {
            self.cfg
                .add_edge(node, loop_ctx.continue_target, EdgeKind::Continue);
        } else {
            self.cfg.add_edge(node, self.cfg.exit(), EdgeKind::Continue);
        }

        Fragment::terminal(node)
    }

    fn build_match_stmt_fragment(&mut self, match_stmt: tolk_syntax::MatchStmt<'_>) -> Fragment {
        let Some(match_expr) = match_stmt.expr() else {
            return self.make_nop_fragment(Some(match_stmt.span()));
        };
        self.build_match_expr_fragment(match_expr)
    }

    fn build_match_expr_fragment(&mut self, match_expr: Match<'_>) -> Fragment {
        let dispatch = self
            .cfg
            .add_node(FlowNodeKind::Condition, Some(match_expr.span()));

        if let Some(subject) = match_expr.expr() {
            self.collect_expr_into_node(dispatch, subject, AccessMode::Read);
        }

        let join = self
            .cfg
            .add_node(FlowNodeKind::Join, Some(match_expr.span()));

        let mut pending_fail = Some(dispatch);
        let mut nodes = vec![dispatch, join];
        let mut arm_exits = Vec::new();

        for arm in match_expr.arms() {
            let pattern = arm.pattern();
            let pattern_span = match pattern {
                MatchPattern::Type(ty) => Some(ty.span()),
                MatchPattern::Expr(expr) => Some(expr.span()),
                MatchPattern::Else => Some(arm.span()),
            };

            let pattern_node = self.cfg.add_node(FlowNodeKind::MatchPattern, pattern_span);
            self.collect_match_pattern_into_node(pattern_node, pattern);

            if let Some(prev) = pending_fail {
                let edge_kind = if prev == dispatch {
                    EdgeKind::Unconditional
                } else {
                    EdgeKind::FalseBranch
                };
                self.cfg.add_edge(prev, pattern_node, edge_kind);
            }

            let body_fragment = self.build_match_arm_body_fragment(arm.body(), arm.span());
            let to_body = if matches!(pattern, MatchPattern::Else) {
                EdgeKind::Unconditional
            } else {
                EdgeKind::TrueBranch
            };
            self.cfg
                .add_edge(pattern_node, body_fragment.entry, to_body);

            arm_exits.extend(body_fragment.exits);
            nodes.push(pattern_node);
            nodes.extend(body_fragment.nodes);

            pending_fail = if matches!(pattern, MatchPattern::Else) {
                None
            } else {
                Some(pattern_node)
            };
        }

        if let Some(last_fail) = pending_fail {
            self.cfg.add_edge(last_fail, join, EdgeKind::FalseBranch);
        }

        for exit in arm_exits {
            self.cfg.add_edge(exit, join, EdgeKind::Unconditional);
        }

        Fragment {
            entry: dispatch,
            exits: vec![join],
            nodes,
        }
    }

    fn build_match_arm_body_fragment(
        &mut self,
        body: Option<MatchArmBody<'_>>,
        fallback_span: tolk_resolver::Span,
    ) -> Fragment {
        let Some(body) = body else {
            return self.make_nop_fragment(Some(fallback_span));
        };

        match body {
            MatchArmBody::Block(block) => self.build_block_fragment(block),
            MatchArmBody::Return(ret) => self.build_return_fragment(ret),
            MatchArmBody::Throw(throw) => self.build_throw_fragment(throw),
            MatchArmBody::Expr(expr) => {
                let node = self.cfg.add_node(FlowNodeKind::Expr, Some(expr.span()));
                self.collect_expr_into_node(node, expr, AccessMode::Read);
                Fragment::single(node)
            }
        }
    }

    fn collect_expr_into_node(&mut self, node_id: NodeId, expr: Expr<'_>, mode: AccessMode) {
        let node = self.cfg.node_mut(node_id);
        self.collector
            .collect_expr(expr, mode, &mut node.reads, &mut node.writes);
        self.collect_taint_for_expr(node_id, expr);
    }

    fn collect_match_pattern_into_node(&mut self, node_id: NodeId, pattern: MatchPattern<'_>) {
        if let MatchPattern::Expr(expr) = pattern {
            let node = self.cfg.node_mut(node_id);
            self.collector
                .collect_expr(expr, AccessMode::Read, &mut node.reads, &mut node.writes);
        }
    }

    fn node_may_throw(&self, node: NodeId) -> bool {
        matches!(
            self.cfg.node(node).kind,
            FlowNodeKind::Expr
                | FlowNodeKind::Condition
                | FlowNodeKind::Assert
                | FlowNodeKind::Return
                | FlowNodeKind::Throw
                | FlowNodeKind::MatchPattern
        )
    }

    fn collect_taint_for_expr(&mut self, node_id: NodeId, expr: Expr<'_>) {
        let collector = &self.collector;
        let is_message_from_slice = collector.contains_message_from_slice(
            expr,
            &self.message_roots,
            &self.inbound_message_roots,
        );
        if is_message_from_slice {
            let writes = self
                .cfg
                .node(node_id)
                .writes
                .iter()
                .copied()
                .collect::<Vec<_>>();
            self.message_roots.extend(writes);
        }

        let mut direct_roots = FxHashSet::default();
        collector.collect_message_field_roots(
            expr,
            &self.message_roots,
            &self.inbound_message_roots,
            &mut direct_roots,
        );

        if !direct_roots.is_empty() && !is_message_from_slice {
            self.cfg
                .node_mut(node_id)
                .taint
                .direct_source_roots
                .extend(direct_roots.iter().copied());
        }

        let is_assert_node = self.cfg.node(node_id).kind == FlowNodeKind::Assert;
        if is_assert_node
            && collector.contains_admin_sender_check(expr, &self.inbound_message_roots)
        {
            self.cfg.node_mut(node_id).taint.has_admin_sender_check = true;
        }

        if collector.contains_storage_write_sink(expr) {
            self.cfg.node_mut(node_id).taint.has_storage_write_sink = true;
        }

        if collector.contains_random_initialize_call(expr) {
            self.cfg.node_mut(node_id).taint.has_random_initialize_call = true;
        }

        if collector.contains_random_value_sink(expr) {
            self.cfg.node_mut(node_id).taint.has_random_value_sink = true;
        }

        let mut division_spans = Vec::new();
        collector.collect_division_spans(expr, &mut division_spans);
        if !division_spans.is_empty() {
            let node = self.cfg.node_mut(node_id);
            node.taint.has_division_operation = true;
            node.taint.division_spans.extend(division_spans);
            node.taint
                .division_spans
                .sort_by_key(|span| (span.start, span.end));
            node.taint.division_spans.dedup();
        }

        let mut direct_assignment_division_spans = Vec::new();
        collector
            .collect_direct_assignment_division_spans(expr, &mut direct_assignment_division_spans);
        if !direct_assignment_division_spans.is_empty() {
            let node = self.cfg.node_mut(node_id);
            node.taint
                .direct_assignment_division_spans
                .extend(direct_assignment_division_spans);
            node.taint
                .direct_assignment_division_spans
                .sort_by_key(|span| (span.start, span.end));
            node.taint.direct_assignment_division_spans.dedup();
        }

        let mut multiplication_operations = Vec::new();
        collector.collect_multiplication_operations(expr, &mut multiplication_operations);
        if !multiplication_operations.is_empty() {
            let has_divide_before_multiply = multiplication_operations
                .iter()
                .any(|op| !op.division_operand_spans.is_empty());

            let node = self.cfg.node_mut(node_id);
            node.taint.has_multiplication_operation = true;
            node.taint.has_divide_before_multiply = has_divide_before_multiply;
            node.taint
                .multiplication_operations
                .extend(multiplication_operations);
            node.taint
                .multiplication_operations
                .sort_by_key(|op| (op.operator_span.start, op.operator_span.end));
        }

        let mut called_globals = FxHashSet::default();
        collector.collect_called_globals(expr, &mut called_globals);
        if !called_globals.is_empty() {
            self.cfg
                .node_mut(node_id)
                .taint
                .called_global_symbols
                .extend(called_globals);
        }
    }
}

fn is_inbound_payload_field_name(name: &str) -> bool {
    name == "body" || name == "bouncedBody"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessMode {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug)]
struct UseDefCollector<'idx> {
    uses_by_start: FxHashMap<u32, LocalDefId>,
    global_uses_by_start: FxHashMap<u32, SymbolId>,
    defs_by_start: FxHashMap<u32, LocalDefId>,
    names_by_start: FxHashMap<u32, Arc<str>>,
    source: Option<&'idx str>,
    _resolve_index: &'idx FileResolveIndex,
}

impl<'idx> UseDefCollector<'idx> {
    fn new(resolve_index: &'idx FileResolveIndex, source: Option<&'idx str>) -> Self {
        let mut uses_by_start = FxHashMap::default();
        let mut global_uses_by_start = FxHashMap::default();
        let mut names_by_start = FxHashMap::default();
        for usage in &resolve_index.uses {
            names_by_start.insert(usage.span.start, usage.name.clone());
            match usage.resolved {
                Resolved::Local(local_id) => {
                    uses_by_start.insert(usage.span.start, local_id);
                }
                Resolved::Global(symbol_id) => {
                    global_uses_by_start.insert(usage.span.start, symbol_id);
                }
                Resolved::Unresolved => {}
            }
        }

        let mut defs_by_start = FxHashMap::default();
        for local in &resolve_index.locals {
            defs_by_start.insert(local.def_span.start, local.id);
            names_by_start
                .entry(local.def_span.start)
                .or_insert_with(|| local.name.clone());
        }

        Self {
            uses_by_start,
            global_uses_by_start,
            defs_by_start,
            names_by_start,
            source,
            _resolve_index: resolve_index,
        }
    }

    fn local_of_ident(&self, ident: Node<'_>) -> Option<LocalDefId> {
        let start = ident.start_byte() as u32;
        self.uses_by_start
            .get(&start)
            .copied()
            .or_else(|| self.defs_by_start.get(&start).copied())
    }

    fn global_of_ident(&self, ident: Node<'_>) -> Option<SymbolId> {
        let start = ident.start_byte() as u32;
        self.global_uses_by_start.get(&start).copied()
    }

    fn name_of_ident(&self, ident: Node<'_>) -> Option<&str> {
        let start = ident.start_byte() as u32;
        if let Some(name) = self.names_by_start.get(&start) {
            return Some(name.as_ref());
        }

        let source = self.source?;
        let text = ident.utf8_text(source.as_bytes()).ok()?;
        Some(text.trim_matches('`'))
    }

    fn call_name(&self, call: Call<'_>) -> Option<&str> {
        let ident = call.callee_identifier()?;
        self.name_of_ident(ident)
    }

    fn call_global_symbol(&self, call: Call<'_>) -> Option<SymbolId> {
        let ident = call.callee_identifier()?;
        self.global_of_ident(ident)
    }

    fn call_qualifier<'tree>(&self, call: Call<'tree>) -> Option<Expr<'tree>> {
        let callee = call.callee()?;
        match callee {
            Expr::DotAccess(dot_access) => dot_access.obj(),
            Expr::Instantiation(inst) => match inst.expr()? {
                Expr::DotAccess(dot_access) => dot_access.obj(),
                _ => None,
            },
            _ => None,
        }
    }

    fn expr_is_random_namespace(&self, expr: Expr<'_>) -> bool {
        match expr {
            Expr::Ident(ident) => self
                .name_of_ident(ident.syntax())
                .is_some_and(|name| name == "random"),
            Expr::Paren(paren) => paren
                .inner()
                .is_some_and(|inner| self.expr_is_random_namespace(inner)),
            Expr::NotNull(not_null) => not_null
                .inner()
                .is_some_and(|inner| self.expr_is_random_namespace(inner)),
            Expr::AsCast(as_cast) => as_cast
                .expr()
                .is_some_and(|inner| self.expr_is_random_namespace(inner)),
            Expr::Lazy(lazy) => lazy
                .expr()
                .is_some_and(|inner| self.expr_is_random_namespace(inner)),
            _ => false,
        }
    }

    fn is_random_method_call(&self, call: Call<'_>, method_names: &[&str]) -> bool {
        let Some(name) = self.call_name(call) else {
            return false;
        };
        if !method_names.contains(&name) {
            return false;
        }

        self.call_qualifier(call)
            .is_some_and(|qualifier| self.expr_is_random_namespace(qualifier))
    }

    fn contains_random_method_call(&self, expr: Expr<'_>, method_names: &[&str]) -> bool {
        match expr {
            Expr::Call(call) => {
                if self.is_random_method_call(call, method_names) {
                    return true;
                }

                if let Some(callee) = call.callee()
                    && self.contains_random_method_call(callee, method_names)
                {
                    return true;
                }

                for argument in call.arguments() {
                    if let Some(arg_expr) = argument.expr()
                        && self.contains_random_method_call(arg_expr, method_names)
                    {
                        return true;
                    }
                }

                false
            }
            Expr::DotAccess(dot_access) => dot_access
                .obj()
                .is_some_and(|obj| self.contains_random_method_call(obj, method_names)),
            Expr::Assign(assign) => {
                assign
                    .left()
                    .is_some_and(|left| self.contains_random_method_call(left, method_names))
                    || assign
                        .right()
                        .is_some_and(|right| self.contains_random_method_call(right, method_names))
            }
            Expr::SetAssign(assign) => {
                assign
                    .left()
                    .is_some_and(|left| self.contains_random_method_call(left, method_names))
                    || assign
                        .right()
                        .is_some_and(|right| self.contains_random_method_call(right, method_names))
            }
            Expr::Instantiation(inst) => inst
                .expr()
                .is_some_and(|inner| self.contains_random_method_call(inner, method_names)),
            Expr::Paren(paren) => paren
                .inner()
                .is_some_and(|inner| self.contains_random_method_call(inner, method_names)),
            Expr::Ternary(ternary) => {
                ternary.condition().is_some_and(|condition| {
                    self.contains_random_method_call(condition, method_names)
                }) || ternary.consequence().is_some_and(|consequence| {
                    self.contains_random_method_call(consequence, method_names)
                }) || ternary.alternative().is_some_and(|alternative| {
                    self.contains_random_method_call(alternative, method_names)
                })
            }
            Expr::Bin(bin) => {
                bin.left()
                    .is_some_and(|left| self.contains_random_method_call(left, method_names))
                    || bin
                        .right()
                        .is_some_and(|right| self.contains_random_method_call(right, method_names))
            }
            Expr::Unary(unary) => unary
                .argument()
                .is_some_and(|argument| self.contains_random_method_call(argument, method_names)),
            Expr::Lazy(lazy) => lazy
                .expr()
                .is_some_and(|inner| self.contains_random_method_call(inner, method_names)),
            Expr::AsCast(as_cast) => as_cast
                .expr()
                .is_some_and(|inner| self.contains_random_method_call(inner, method_names)),
            Expr::IsType(is_type) => is_type
                .expr()
                .is_some_and(|inner| self.contains_random_method_call(inner, method_names)),
            Expr::NotNull(not_null) => not_null
                .inner()
                .is_some_and(|inner| self.contains_random_method_call(inner, method_names)),
            Expr::ObjectLit(object_lit) => object_lit.arguments().any(|arg| {
                arg.value()
                    .is_some_and(|value| self.contains_random_method_call(value, method_names))
            }),
            Expr::Tensor(tensor) => tensor
                .elements()
                .any(|element| self.contains_random_method_call(element, method_names)),
            Expr::Tuple(tuple) => tuple
                .elements()
                .any(|element| self.contains_random_method_call(element, method_names)),
            Expr::Match(match_expr) => {
                if let Some(subject) = match_expr.expr()
                    && self.contains_random_method_call(subject, method_names)
                {
                    return true;
                }
                match_expr.arms().any(|arm| {
                    if let MatchPattern::Expr(pattern_expr) = arm.pattern()
                        && self.contains_random_method_call(pattern_expr, method_names)
                    {
                        return true;
                    }

                    arm.body().is_some_and(|body| match body {
                        MatchArmBody::Block(_) => false,
                        MatchArmBody::Return(ret) => ret.expr().is_some_and(|expr| {
                            self.contains_random_method_call(expr, method_names)
                        }),
                        MatchArmBody::Throw(throw_stmt) => throw_stmt.expr().is_some_and(|expr| {
                            self.contains_random_method_call(expr, method_names)
                        }),
                        MatchArmBody::Expr(expr) => {
                            self.contains_random_method_call(expr, method_names)
                        }
                    })
                })
            }
            Expr::VarDeclLhs(_)
            | Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Ident(_)
            | Expr::Unmapped(_) => false,
        }
    }

    fn contains_random_initialize_call(&self, expr: Expr<'_>) -> bool {
        self.contains_random_method_call(expr, &["initialize", "initializeBy"])
    }

    fn contains_random_value_sink(&self, expr: Expr<'_>) -> bool {
        self.contains_random_method_call(expr, &["uint256", "range"])
    }

    fn collect_multiplication_operations(
        &self,
        expr: Expr<'_>,
        out: &mut Vec<MultiplicationOperationFact>,
    ) {
        let Some(source) = self.source else {
            return;
        };

        for node in expr.syntax().traverse() {
            if node.kind() != "binary_operator" {
                continue;
            }

            let mul = Bin(node);
            if mul.operator_name(source) != "*" {
                continue;
            }

            let operator_span = mul.operator().map_or_else(
                || tolk_resolver::Span::from_syntax(&node),
                |op| tolk_resolver::Span::from_syntax(&op),
            );

            let mut read_locals = FxHashSet::default();
            let mut writes = FxHashSet::default();
            self.collect_expr(
                Expr::Bin(mul),
                AccessMode::Read,
                &mut read_locals,
                &mut writes,
            );

            let mut division_operand_spans = Vec::new();
            if let Some(left) = mul.left() {
                self.collect_division_spans(left, &mut division_operand_spans);
            }
            if let Some(right) = mul.right() {
                self.collect_division_spans(right, &mut division_operand_spans);
            }
            division_operand_spans.sort_by_key(|span| (span.start, span.end));
            division_operand_spans.dedup();

            out.push(MultiplicationOperationFact {
                operator_span,
                read_locals,
                division_operand_spans,
            });
        }
    }

    fn collect_division_spans(&self, expr: Expr<'_>, out: &mut Vec<tolk_resolver::Span>) {
        let Some(source) = self.source else {
            return;
        };

        for node in expr.syntax().traverse() {
            if node.kind() != "binary_operator" {
                continue;
            }
            let bin = Bin(node);
            if bin.operator_name(source) != "/" {
                continue;
            }
            out.push(tolk_resolver::Span::from_syntax(&node));
        }
    }

    fn collect_direct_assignment_division_spans(
        &self,
        expr: Expr<'_>,
        out: &mut Vec<tolk_resolver::Span>,
    ) {
        let Some(rhs) = (match expr {
            Expr::Assign(assign) => assign.right(),
            _ => None,
        }) else {
            return;
        };

        let rhs = self.strip_taint_wrappers(rhs);
        let Expr::Bin(bin) = rhs else {
            return;
        };
        if !self.is_arithmetic_binary_operator(bin) {
            return;
        }

        self.collect_division_spans(rhs, out);
    }

    fn strip_taint_wrappers<'tree>(&self, mut expr: Expr<'tree>) -> Expr<'tree> {
        loop {
            expr = match expr {
                Expr::Paren(paren) => match paren.inner() {
                    Some(inner) => inner,
                    None => return expr,
                },
                Expr::NotNull(not_null) => match not_null.inner() {
                    Some(inner) => inner,
                    None => return expr,
                },
                Expr::AsCast(as_cast) => match as_cast.expr() {
                    Some(inner) => inner,
                    None => return expr,
                },
                _ => return expr,
            };
        }
    }

    fn is_arithmetic_binary_operator(&self, bin: Bin<'_>) -> bool {
        let Some(source) = self.source else {
            return false;
        };

        matches!(bin.operator_name(source), "+" | "-" | "*" | "/" | "%")
    }

    fn collect_called_globals(&self, expr: Expr<'_>, out: &mut FxHashSet<SymbolId>) {
        self.collect_called_globals_inner(expr, out);
    }

    fn collect_called_globals_inner(&self, expr: Expr<'_>, out: &mut FxHashSet<SymbolId>) {
        match expr {
            Expr::Call(call) => {
                if let Some(symbol_id) = self.call_global_symbol(call) {
                    out.insert(symbol_id);
                }

                if let Some(callee) = call.callee() {
                    self.collect_called_globals_inner(callee, out);
                }
                for argument in call.arguments() {
                    if let Some(arg_expr) = argument.expr() {
                        self.collect_called_globals_inner(arg_expr, out);
                    }
                }
            }
            Expr::DotAccess(dot_access) => {
                if let Some(obj) = dot_access.obj() {
                    self.collect_called_globals_inner(obj, out);
                }
            }
            Expr::Assign(assign) => {
                if let Some(left) = assign.left() {
                    self.collect_called_globals_inner(left, out);
                }
                if let Some(right) = assign.right() {
                    self.collect_called_globals_inner(right, out);
                }
            }
            Expr::SetAssign(assign) => {
                if let Some(left) = assign.left() {
                    self.collect_called_globals_inner(left, out);
                }
                if let Some(right) = assign.right() {
                    self.collect_called_globals_inner(right, out);
                }
            }
            Expr::Instantiation(instantiation) => {
                if let Some(inner) = instantiation.expr() {
                    self.collect_called_globals_inner(inner, out);
                }
            }
            Expr::Paren(paren) => {
                if let Some(inner) = paren.inner() {
                    self.collect_called_globals_inner(inner, out);
                }
            }
            Expr::Ternary(ternary) => {
                if let Some(condition) = ternary.condition() {
                    self.collect_called_globals_inner(condition, out);
                }
                if let Some(consequence) = ternary.consequence() {
                    self.collect_called_globals_inner(consequence, out);
                }
                if let Some(alternative) = ternary.alternative() {
                    self.collect_called_globals_inner(alternative, out);
                }
            }
            Expr::Bin(bin) => {
                if let Some(left) = bin.left() {
                    self.collect_called_globals_inner(left, out);
                }
                if let Some(right) = bin.right() {
                    self.collect_called_globals_inner(right, out);
                }
            }
            Expr::Unary(unary) => {
                if let Some(argument) = unary.argument() {
                    self.collect_called_globals_inner(argument, out);
                }
            }
            Expr::Lazy(lazy) => {
                if let Some(inner) = lazy.expr() {
                    self.collect_called_globals_inner(inner, out);
                }
            }
            Expr::AsCast(as_cast) => {
                if let Some(inner) = as_cast.expr() {
                    self.collect_called_globals_inner(inner, out);
                }
            }
            Expr::IsType(is_type) => {
                if let Some(inner) = is_type.expr() {
                    self.collect_called_globals_inner(inner, out);
                }
            }
            Expr::NotNull(not_null) => {
                if let Some(inner) = not_null.inner() {
                    self.collect_called_globals_inner(inner, out);
                }
            }
            Expr::ObjectLit(object_lit) => {
                for argument in object_lit.arguments() {
                    if let Some(value) = argument.value() {
                        self.collect_called_globals_inner(value, out);
                    }
                }
            }
            Expr::Tensor(tensor) => {
                for element in tensor.elements() {
                    self.collect_called_globals_inner(element, out);
                }
            }
            Expr::Tuple(tuple) => {
                for element in tuple.elements() {
                    self.collect_called_globals_inner(element, out);
                }
            }
            Expr::Match(match_expr) => {
                if let Some(subject) = match_expr.expr() {
                    self.collect_called_globals_inner(subject, out);
                }
                for arm in match_expr.arms() {
                    if let MatchPattern::Expr(pattern_expr) = arm.pattern() {
                        self.collect_called_globals_inner(pattern_expr, out);
                    }

                    if let Some(body) = arm.body() {
                        match body {
                            MatchArmBody::Block(_) => {}
                            MatchArmBody::Return(ret) => {
                                if let Some(expr) = ret.expr() {
                                    self.collect_called_globals_inner(expr, out);
                                }
                            }
                            MatchArmBody::Throw(throw_stmt) => {
                                if let Some(expr) = throw_stmt.expr() {
                                    self.collect_called_globals_inner(expr, out);
                                }
                            }
                            MatchArmBody::Expr(expr) => {
                                self.collect_called_globals_inner(expr, out);
                            }
                        }
                    }
                }
            }
            Expr::VarDeclLhs(_)
            | Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Ident(_)
            | Expr::Unmapped(_) => {}
        }
    }

    fn expr_base_local(&self, expr: Expr<'_>) -> Option<LocalDefId> {
        match expr {
            Expr::Ident(ident) => self.local_of_ident(ident.syntax()),
            Expr::DotAccess(dot_access) => {
                dot_access.obj().and_then(|obj| self.expr_base_local(obj))
            }
            Expr::Paren(paren) => paren.inner().and_then(|inner| self.expr_base_local(inner)),
            Expr::NotNull(not_null) => not_null
                .inner()
                .and_then(|inner| self.expr_base_local(inner)),
            Expr::AsCast(as_cast) => as_cast.expr().and_then(|inner| self.expr_base_local(inner)),
            Expr::Lazy(lazy) => lazy.expr().and_then(|inner| self.expr_base_local(inner)),
            _ => None,
        }
    }

    fn collect_message_field_roots(
        &self,
        expr: Expr<'_>,
        message_roots: &FxHashSet<LocalDefId>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
        out: &mut FxHashSet<LocalDefId>,
    ) {
        self.collect_message_field_roots_inner(expr, message_roots, inbound_message_roots, out);
    }

    fn collect_message_field_roots_inner(
        &self,
        expr: Expr<'_>,
        message_roots: &FxHashSet<LocalDefId>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
        out: &mut FxHashSet<LocalDefId>,
    ) {
        match expr {
            Expr::DotAccess(dot_access) => {
                if let Some(base_local) = self.taint_source_root_for_dot_access(
                    dot_access,
                    message_roots,
                    inbound_message_roots,
                ) {
                    out.insert(base_local);
                }

                if let Some(obj) = dot_access.obj() {
                    self.collect_message_field_roots_inner(
                        obj,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Assign(assign) => {
                if let Some(left) = assign.left() {
                    self.collect_message_field_roots_inner(
                        left,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                if let Some(right) = assign.right() {
                    self.collect_message_field_roots_inner(
                        right,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::SetAssign(assign) => {
                if let Some(left) = assign.left() {
                    self.collect_message_field_roots_inner(
                        left,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                if let Some(right) = assign.right() {
                    self.collect_message_field_roots_inner(
                        right,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Call(call) => {
                if let Some(callee) = call.callee() {
                    self.collect_message_field_roots_inner(
                        callee,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                for argument in call.arguments() {
                    if let Some(arg_expr) = argument.expr() {
                        self.collect_message_field_roots_inner(
                            arg_expr,
                            message_roots,
                            inbound_message_roots,
                            out,
                        );
                    }
                }
            }
            Expr::Instantiation(instantiation) => {
                if let Some(inner) = instantiation.expr() {
                    self.collect_message_field_roots_inner(
                        inner,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Paren(paren) => {
                if let Some(inner) = paren.inner() {
                    self.collect_message_field_roots_inner(
                        inner,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Ternary(ternary) => {
                if let Some(condition) = ternary.condition() {
                    self.collect_message_field_roots_inner(
                        condition,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                if let Some(consequence) = ternary.consequence() {
                    self.collect_message_field_roots_inner(
                        consequence,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                if let Some(alternative) = ternary.alternative() {
                    self.collect_message_field_roots_inner(
                        alternative,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Bin(bin) => {
                if let Some(left) = bin.left() {
                    self.collect_message_field_roots_inner(
                        left,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                if let Some(right) = bin.right() {
                    self.collect_message_field_roots_inner(
                        right,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Unary(unary) => {
                if let Some(argument) = unary.argument() {
                    self.collect_message_field_roots_inner(
                        argument,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Lazy(lazy) => {
                if let Some(inner) = lazy.expr() {
                    self.collect_message_field_roots_inner(
                        inner,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::AsCast(as_cast) => {
                if let Some(inner) = as_cast.expr() {
                    self.collect_message_field_roots_inner(
                        inner,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::IsType(is_type) => {
                if let Some(inner) = is_type.expr() {
                    self.collect_message_field_roots_inner(
                        inner,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::NotNull(not_null) => {
                if let Some(inner) = not_null.inner() {
                    self.collect_message_field_roots_inner(
                        inner,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::ObjectLit(object_lit) => {
                for arg in object_lit.arguments() {
                    if let Some(value) = arg.value() {
                        self.collect_message_field_roots_inner(
                            value,
                            message_roots,
                            inbound_message_roots,
                            out,
                        );
                    }
                }
            }
            Expr::Tensor(tensor) => {
                for element in tensor.elements() {
                    self.collect_message_field_roots_inner(
                        element,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Tuple(tuple) => {
                for element in tuple.elements() {
                    self.collect_message_field_roots_inner(
                        element,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
            }
            Expr::Match(match_expr) => {
                if let Some(subject) = match_expr.expr() {
                    self.collect_message_field_roots_inner(
                        subject,
                        message_roots,
                        inbound_message_roots,
                        out,
                    );
                }
                for arm in match_expr.arms() {
                    if let MatchPattern::Expr(pattern_expr) = arm.pattern() {
                        self.collect_message_field_roots_inner(
                            pattern_expr,
                            message_roots,
                            inbound_message_roots,
                            out,
                        );
                    }
                    if let Some(body) = arm.body() {
                        match body {
                            MatchArmBody::Block(_) => {}
                            MatchArmBody::Return(ret) => {
                                if let Some(expr) = ret.expr() {
                                    self.collect_message_field_roots_inner(
                                        expr,
                                        message_roots,
                                        inbound_message_roots,
                                        out,
                                    );
                                }
                            }
                            MatchArmBody::Throw(throw_stmt) => {
                                if let Some(expr) = throw_stmt.expr() {
                                    self.collect_message_field_roots_inner(
                                        expr,
                                        message_roots,
                                        inbound_message_roots,
                                        out,
                                    );
                                }
                            }
                            MatchArmBody::Expr(expr) => {
                                self.collect_message_field_roots_inner(
                                    expr,
                                    message_roots,
                                    inbound_message_roots,
                                    out,
                                );
                            }
                        }
                    }
                }
            }
            Expr::VarDeclLhs(_)
            | Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Ident(_)
            | Expr::Unmapped(_) => {}
        }
    }

    fn taint_source_root_for_dot_access(
        &self,
        dot_access: DotAccess<'_>,
        message_roots: &FxHashSet<LocalDefId>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
    ) -> Option<LocalDefId> {
        let obj = dot_access.obj()?;
        let base_local = self.expr_base_local(obj)?;
        if !message_roots.contains(&base_local) {
            return None;
        }

        if inbound_message_roots.contains(&base_local)
            && !self
                .expr_has_inbound_payload_origin(Expr::DotAccess(dot_access), inbound_message_roots)
        {
            // For raw inbound message (`in`), only payload projections (`in.body*`, `in.bouncedBody*`)
            // are considered taint sources. Metadata fields like `in.senderAddress` are excluded.
            return None;
        }

        Some(base_local)
    }

    fn expr_has_inbound_payload_origin(
        &self,
        expr: Expr<'_>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
    ) -> bool {
        match expr {
            Expr::DotAccess(dot_access) => {
                if let Some(obj) = dot_access.obj() {
                    if let Some(base_local) = self.expr_base_local(obj)
                        && inbound_message_roots.contains(&base_local)
                        && self
                            .dot_access_field_name(dot_access)
                            .is_some_and(is_inbound_payload_field_name)
                    {
                        return true;
                    }

                    return self.expr_has_inbound_payload_origin(obj, inbound_message_roots);
                }
                false
            }
            Expr::Paren(paren) => paren.inner().is_some_and(|inner| {
                self.expr_has_inbound_payload_origin(inner, inbound_message_roots)
            }),
            Expr::NotNull(not_null) => not_null.inner().is_some_and(|inner| {
                self.expr_has_inbound_payload_origin(inner, inbound_message_roots)
            }),
            Expr::AsCast(as_cast) => as_cast.expr().is_some_and(|inner| {
                self.expr_has_inbound_payload_origin(inner, inbound_message_roots)
            }),
            Expr::Lazy(lazy) => lazy.expr().is_some_and(|inner| {
                self.expr_has_inbound_payload_origin(inner, inbound_message_roots)
            }),
            _ => false,
        }
    }

    fn dot_access_field_name(&self, dot_access: DotAccess) -> Option<&str> {
        let field = dot_access.field()?;
        match field {
            DotAccessField::Ident(ident) => self.name_of_ident(ident.syntax()),
            DotAccessField::NumericIndex(_) => None,
        }
    }

    fn contains_storage_write_sink(&self, expr: Expr<'_>) -> bool {
        match expr {
            Expr::Call(call) => {
                if self
                    .call_name(call)
                    .is_some_and(|name| name == "setData" || name == "save")
                // VERY simplified for now
                {
                    return true;
                }

                if let Some(callee) = call.callee()
                    && self.contains_storage_write_sink(callee)
                {
                    return true;
                }

                for argument in call.arguments() {
                    if let Some(arg_expr) = argument.expr()
                        && self.contains_storage_write_sink(arg_expr)
                    {
                        return true;
                    }
                }

                false
            }
            Expr::DotAccess(dot_access) => dot_access
                .obj()
                .is_some_and(|obj| self.contains_storage_write_sink(obj)),
            Expr::Assign(assign) => {
                assign
                    .left()
                    .is_some_and(|left| self.contains_storage_write_sink(left))
                    || assign
                        .right()
                        .is_some_and(|right| self.contains_storage_write_sink(right))
            }
            Expr::SetAssign(assign) => {
                assign
                    .left()
                    .is_some_and(|left| self.contains_storage_write_sink(left))
                    || assign
                        .right()
                        .is_some_and(|right| self.contains_storage_write_sink(right))
            }
            Expr::Instantiation(inst) => inst
                .expr()
                .is_some_and(|inner| self.contains_storage_write_sink(inner)),
            Expr::Paren(paren) => paren
                .inner()
                .is_some_and(|inner| self.contains_storage_write_sink(inner)),
            Expr::Ternary(ternary) => {
                ternary
                    .condition()
                    .is_some_and(|condition| self.contains_storage_write_sink(condition))
                    || ternary
                        .consequence()
                        .is_some_and(|consequence| self.contains_storage_write_sink(consequence))
                    || ternary
                        .alternative()
                        .is_some_and(|alternative| self.contains_storage_write_sink(alternative))
            }
            Expr::Bin(bin) => {
                bin.left()
                    .is_some_and(|left| self.contains_storage_write_sink(left))
                    || bin
                        .right()
                        .is_some_and(|right| self.contains_storage_write_sink(right))
            }
            Expr::Unary(unary) => unary
                .argument()
                .is_some_and(|argument| self.contains_storage_write_sink(argument)),
            Expr::Lazy(lazy) => lazy
                .expr()
                .is_some_and(|inner| self.contains_storage_write_sink(inner)),
            Expr::AsCast(as_cast) => as_cast
                .expr()
                .is_some_and(|inner| self.contains_storage_write_sink(inner)),
            Expr::IsType(is_type) => is_type
                .expr()
                .is_some_and(|inner| self.contains_storage_write_sink(inner)),
            Expr::NotNull(not_null) => not_null
                .inner()
                .is_some_and(|inner| self.contains_storage_write_sink(inner)),
            Expr::ObjectLit(object_lit) => object_lit.arguments().any(|arg| {
                arg.value()
                    .is_some_and(|value| self.contains_storage_write_sink(value))
            }),
            Expr::Tensor(tensor) => tensor
                .elements()
                .any(|element| self.contains_storage_write_sink(element)),
            Expr::Tuple(tuple) => tuple
                .elements()
                .any(|element| self.contains_storage_write_sink(element)),
            Expr::Match(match_expr) => {
                if let Some(subject) = match_expr.expr()
                    && self.contains_storage_write_sink(subject)
                {
                    return true;
                }
                match_expr.arms().any(|arm| {
                    if let MatchPattern::Expr(pattern_expr) = arm.pattern()
                        && self.contains_storage_write_sink(pattern_expr)
                    {
                        return true;
                    }

                    arm.body().is_some_and(|body| match body {
                        MatchArmBody::Block(_) => false,
                        MatchArmBody::Return(ret) => ret
                            .expr()
                            .is_some_and(|expr| self.contains_storage_write_sink(expr)),
                        MatchArmBody::Throw(throw_stmt) => throw_stmt
                            .expr()
                            .is_some_and(|expr| self.contains_storage_write_sink(expr)),
                        MatchArmBody::Expr(expr) => self.contains_storage_write_sink(expr),
                    })
                })
            }
            Expr::VarDeclLhs(_)
            | Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Ident(_)
            | Expr::Unmapped(_) => false,
        }
    }

    fn contains_admin_sender_check(
        &self,
        expr: Expr<'_>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
    ) -> bool {
        match expr {
            Expr::Bin(bin) => {
                if self.is_equality_bin(bin) {
                    let left = bin.left();
                    let right = bin.right();
                    if let (Some(left), Some(right)) = (left, right)
                        && ((self.is_inbound_sender_expr(left, inbound_message_roots)
                            && self.is_admin_address_expr(right))
                            || (self.is_inbound_sender_expr(right, inbound_message_roots)
                                && self.is_admin_address_expr(left)))
                    {
                        return true;
                    }
                }

                bin.left().is_some_and(|left| {
                    self.contains_admin_sender_check(left, inbound_message_roots)
                }) || bin.right().is_some_and(|right| {
                    self.contains_admin_sender_check(right, inbound_message_roots)
                })
            }
            Expr::Call(call) => {
                if let Some(callee) = call.callee()
                    && self.contains_admin_sender_check(callee, inbound_message_roots)
                {
                    return true;
                }
                for argument in call.arguments() {
                    if let Some(arg_expr) = argument.expr()
                        && self.contains_admin_sender_check(arg_expr, inbound_message_roots)
                    {
                        return true;
                    }
                }
                false
            }
            Expr::DotAccess(dot_access) => dot_access
                .obj()
                .is_some_and(|obj| self.contains_admin_sender_check(obj, inbound_message_roots)),
            Expr::Assign(assign) => {
                assign.left().is_some_and(|left| {
                    self.contains_admin_sender_check(left, inbound_message_roots)
                }) || assign.right().is_some_and(|right| {
                    self.contains_admin_sender_check(right, inbound_message_roots)
                })
            }
            Expr::SetAssign(assign) => {
                assign.left().is_some_and(|left| {
                    self.contains_admin_sender_check(left, inbound_message_roots)
                }) || assign.right().is_some_and(|right| {
                    self.contains_admin_sender_check(right, inbound_message_roots)
                })
            }
            Expr::Instantiation(inst) => inst.expr().is_some_and(|inner| {
                self.contains_admin_sender_check(inner, inbound_message_roots)
            }),
            Expr::Paren(paren) => paren.inner().is_some_and(|inner| {
                self.contains_admin_sender_check(inner, inbound_message_roots)
            }),
            Expr::Ternary(ternary) => {
                ternary.condition().is_some_and(|condition| {
                    self.contains_admin_sender_check(condition, inbound_message_roots)
                }) || ternary.consequence().is_some_and(|consequence| {
                    self.contains_admin_sender_check(consequence, inbound_message_roots)
                }) || ternary.alternative().is_some_and(|alternative| {
                    self.contains_admin_sender_check(alternative, inbound_message_roots)
                })
            }
            Expr::Unary(unary) => unary.argument().is_some_and(|argument| {
                self.contains_admin_sender_check(argument, inbound_message_roots)
            }),
            Expr::Lazy(lazy) => lazy.expr().is_some_and(|inner| {
                self.contains_admin_sender_check(inner, inbound_message_roots)
            }),
            Expr::AsCast(as_cast) => as_cast.expr().is_some_and(|inner| {
                self.contains_admin_sender_check(inner, inbound_message_roots)
            }),
            Expr::IsType(is_type) => is_type.expr().is_some_and(|inner| {
                self.contains_admin_sender_check(inner, inbound_message_roots)
            }),
            Expr::NotNull(not_null) => not_null.inner().is_some_and(|inner| {
                self.contains_admin_sender_check(inner, inbound_message_roots)
            }),
            Expr::ObjectLit(object_lit) => object_lit.arguments().any(|arg| {
                arg.value().is_some_and(|value| {
                    self.contains_admin_sender_check(value, inbound_message_roots)
                })
            }),
            Expr::Tensor(tensor) => tensor
                .elements()
                .any(|element| self.contains_admin_sender_check(element, inbound_message_roots)),
            Expr::Tuple(tuple) => tuple
                .elements()
                .any(|element| self.contains_admin_sender_check(element, inbound_message_roots)),
            Expr::Match(match_expr) => {
                if let Some(subject) = match_expr.expr()
                    && self.contains_admin_sender_check(subject, inbound_message_roots)
                {
                    return true;
                }
                false
            }
            Expr::VarDeclLhs(_)
            | Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Ident(_)
            | Expr::Unmapped(_) => false,
        }
    }

    fn is_inbound_sender_expr(
        &self,
        expr: Expr<'_>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
    ) -> bool {
        match expr {
            Expr::DotAccess(dot_access) => {
                let Some(obj) = dot_access.obj() else {
                    return false;
                };
                let Some(base_local) = self.expr_base_local(obj) else {
                    return false;
                };
                inbound_message_roots.contains(&base_local)
                    && self
                        .dot_access_field_name(dot_access)
                        .is_some_and(|name| name == "senderAddress")
            }
            Expr::Paren(paren) => paren
                .inner()
                .is_some_and(|inner| self.is_inbound_sender_expr(inner, inbound_message_roots)),
            Expr::NotNull(not_null) => not_null
                .inner()
                .is_some_and(|inner| self.is_inbound_sender_expr(inner, inbound_message_roots)),
            Expr::AsCast(as_cast) => as_cast
                .expr()
                .is_some_and(|inner| self.is_inbound_sender_expr(inner, inbound_message_roots)),
            Expr::Lazy(lazy) => lazy
                .expr()
                .is_some_and(|inner| self.is_inbound_sender_expr(inner, inbound_message_roots)),
            _ => false,
        }
    }

    fn is_admin_address_expr(&self, expr: Expr<'_>) -> bool {
        match expr {
            Expr::DotAccess(dot_access) => self
                .dot_access_field_name(dot_access)
                .is_some_and(|name| name == "adminAddress"),
            Expr::Paren(paren) => paren
                .inner()
                .is_some_and(|inner| self.is_admin_address_expr(inner)),
            Expr::NotNull(not_null) => not_null
                .inner()
                .is_some_and(|inner| self.is_admin_address_expr(inner)),
            Expr::AsCast(as_cast) => as_cast
                .expr()
                .is_some_and(|inner| self.is_admin_address_expr(inner)),
            Expr::Lazy(lazy) => lazy
                .expr()
                .is_some_and(|inner| self.is_admin_address_expr(inner)),
            _ => false,
        }
    }

    fn contains_message_from_slice(
        &self,
        expr: Expr<'_>,
        message_roots: &FxHashSet<LocalDefId>,
        inbound_message_roots: &FxHashSet<LocalDefId>,
    ) -> bool {
        match expr {
            Expr::Call(call) => {
                if self.call_name(call).is_some_and(|name| name == "fromSlice") {
                    for argument in call.arguments() {
                        let Some(arg_expr) = argument.expr() else {
                            continue;
                        };
                        let mut roots = FxHashSet::default();
                        self.collect_message_field_roots(
                            arg_expr,
                            message_roots,
                            inbound_message_roots,
                            &mut roots,
                        );
                        if !roots.is_empty() {
                            return true;
                        }
                    }
                }

                if let Some(callee) = call.callee()
                    && self.contains_message_from_slice(
                        callee,
                        message_roots,
                        inbound_message_roots,
                    )
                {
                    return true;
                }

                for argument in call.arguments() {
                    if let Some(arg_expr) = argument.expr()
                        && self.contains_message_from_slice(
                            arg_expr,
                            message_roots,
                            inbound_message_roots,
                        )
                    {
                        return true;
                    }
                }

                false
            }
            Expr::DotAccess(dot_access) => dot_access.obj().is_some_and(|obj| {
                self.contains_message_from_slice(obj, message_roots, inbound_message_roots)
            }),
            Expr::Assign(assign) => {
                assign.left().is_some_and(|left| {
                    self.contains_message_from_slice(left, message_roots, inbound_message_roots)
                }) || assign.right().is_some_and(|right| {
                    self.contains_message_from_slice(right, message_roots, inbound_message_roots)
                })
            }
            Expr::SetAssign(assign) => {
                assign.left().is_some_and(|left| {
                    self.contains_message_from_slice(left, message_roots, inbound_message_roots)
                }) || assign.right().is_some_and(|right| {
                    self.contains_message_from_slice(right, message_roots, inbound_message_roots)
                })
            }
            Expr::Instantiation(inst) => inst.expr().is_some_and(|inner| {
                self.contains_message_from_slice(inner, message_roots, inbound_message_roots)
            }),
            Expr::Paren(paren) => paren.inner().is_some_and(|inner| {
                self.contains_message_from_slice(inner, message_roots, inbound_message_roots)
            }),
            Expr::Ternary(ternary) => {
                ternary.condition().is_some_and(|condition| {
                    self.contains_message_from_slice(
                        condition,
                        message_roots,
                        inbound_message_roots,
                    )
                }) || ternary.consequence().is_some_and(|consequence| {
                    self.contains_message_from_slice(
                        consequence,
                        message_roots,
                        inbound_message_roots,
                    )
                }) || ternary.alternative().is_some_and(|alternative| {
                    self.contains_message_from_slice(
                        alternative,
                        message_roots,
                        inbound_message_roots,
                    )
                })
            }
            Expr::Bin(bin) => {
                bin.left().is_some_and(|left| {
                    self.contains_message_from_slice(left, message_roots, inbound_message_roots)
                }) || bin.right().is_some_and(|right| {
                    self.contains_message_from_slice(right, message_roots, inbound_message_roots)
                })
            }
            Expr::Unary(unary) => unary.argument().is_some_and(|argument| {
                self.contains_message_from_slice(argument, message_roots, inbound_message_roots)
            }),
            Expr::Lazy(lazy) => lazy.expr().is_some_and(|inner| {
                self.contains_message_from_slice(inner, message_roots, inbound_message_roots)
            }),
            Expr::AsCast(as_cast) => as_cast.expr().is_some_and(|inner| {
                self.contains_message_from_slice(inner, message_roots, inbound_message_roots)
            }),
            Expr::IsType(is_type) => is_type.expr().is_some_and(|inner| {
                self.contains_message_from_slice(inner, message_roots, inbound_message_roots)
            }),
            Expr::NotNull(not_null) => not_null.inner().is_some_and(|inner| {
                self.contains_message_from_slice(inner, message_roots, inbound_message_roots)
            }),
            Expr::ObjectLit(object_lit) => object_lit.arguments().any(|arg| {
                arg.value().is_some_and(|value| {
                    self.contains_message_from_slice(value, message_roots, inbound_message_roots)
                })
            }),
            Expr::Tensor(tensor) => tensor.elements().any(|element| {
                self.contains_message_from_slice(element, message_roots, inbound_message_roots)
            }),
            Expr::Tuple(tuple) => tuple.elements().any(|element| {
                self.contains_message_from_slice(element, message_roots, inbound_message_roots)
            }),
            Expr::Match(match_expr) => {
                if let Some(subject) = match_expr.expr()
                    && self.contains_message_from_slice(
                        subject,
                        message_roots,
                        inbound_message_roots,
                    )
                {
                    return true;
                }
                false
            }
            Expr::VarDeclLhs(_)
            | Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Ident(_)
            | Expr::Unmapped(_) => false,
        }
    }

    fn is_equality_bin(&self, bin: Bin<'_>) -> bool {
        let Some(op) = bin.operator() else {
            return false;
        };

        op.kind_bytes() == b"=="
    }

    fn collect_definition_ident(&self, ident: Node<'_>, writes: &mut FxHashSet<LocalDefId>) {
        let start = ident.start_byte() as u32;
        if let Some(local) = self
            .defs_by_start
            .get(&start)
            .copied()
            .or_else(|| self.uses_by_start.get(&start).copied())
        {
            writes.insert(local);
        }
    }

    fn collect_expr(
        &self,
        expr: Expr<'_>,
        mode: AccessMode,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        match expr {
            Expr::Ident(ident) => self.collect_ident_access(ident.syntax(), mode, reads, writes),
            Expr::Paren(Paren(paren)) => {
                let node = paren;
                if let Some(inner) = node.field::<Expr<'_>>("inner") {
                    self.collect_expr(inner, mode, reads, writes);
                }
            }
            Expr::Assign(assign) => self.collect_assign(assign, reads, writes),
            Expr::SetAssign(set_assign) => self.collect_set_assign(set_assign, reads, writes),
            Expr::VarDeclLhs(var_decl_lhs) => {
                if let Some(pattern) = var_decl_lhs.pattern() {
                    self.collect_var_pattern_writes(pattern, writes);
                }
            }
            Expr::DotAccess(dot_access) => self.collect_dot_access(dot_access, mode, reads, writes),
            Expr::Call(call) => self.collect_call(call, reads, writes),
            Expr::Instantiation(instantiation) => {
                if let Some(inner) = instantiation.expr() {
                    self.collect_expr(inner, AccessMode::Read, reads, writes);
                }
            }
            Expr::Ternary(ternary) => {
                if let Some(condition) = ternary.condition() {
                    self.collect_expr(condition, AccessMode::Read, reads, writes);
                }
                if let Some(consequence) = ternary.consequence() {
                    self.collect_expr(consequence, AccessMode::Read, reads, writes);
                }
                if let Some(alternative) = ternary.alternative() {
                    self.collect_expr(alternative, AccessMode::Read, reads, writes);
                }
            }
            Expr::Bin(bin) => {
                if let Some(left) = bin.left() {
                    self.collect_expr(left, AccessMode::Read, reads, writes);
                }
                if let Some(right) = bin.right() {
                    self.collect_expr(right, AccessMode::Read, reads, writes);
                }
            }
            Expr::Unary(unary) => {
                if let Some(argument) = unary.argument() {
                    self.collect_expr(argument, AccessMode::Read, reads, writes);
                }
            }
            Expr::Lazy(lazy) => {
                if let Some(inner) = lazy.expr() {
                    self.collect_expr(inner, AccessMode::Read, reads, writes);
                }
            }
            Expr::AsCast(as_cast) => {
                if let Some(inner) = as_cast.expr() {
                    self.collect_expr(inner, AccessMode::Read, reads, writes);
                }
            }
            Expr::IsType(is_type) => {
                if let Some(inner) = is_type.expr() {
                    self.collect_expr(inner, AccessMode::Read, reads, writes);
                }
            }
            Expr::NotNull(not_null) => {
                if let Some(inner) = not_null.inner() {
                    self.collect_expr(inner, AccessMode::Read, reads, writes);
                }
            }
            Expr::ObjectLit(object_lit) => {
                for arg in object_lit.arguments() {
                    self.collect_instance_arg(arg, reads, writes);
                }
            }
            Expr::Tensor(tensor) => {
                for element in tensor.elements() {
                    self.collect_expr(element, AccessMode::Read, reads, writes);
                }
            }
            Expr::Tuple(tuple) => {
                for element in tuple.elements() {
                    self.collect_expr(element, AccessMode::Read, reads, writes);
                }
            }
            Expr::Match(match_expr) => self.collect_match(match_expr, reads, writes),
            Expr::Lambda(_)
            | Expr::NumberLit(_)
            | Expr::StringLit(_)
            | Expr::BoolLit(_)
            | Expr::NullLit(_)
            | Expr::Underscore(_)
            | Expr::Unmapped(_) => {
                // Lambda body is not executed eagerly and should not influence enclosing CFG node.
            }
        }
    }

    fn collect_assign(
        &self,
        assign: Assign<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        if let Some(left) = assign.left() {
            self.collect_expr(left, AccessMode::Write, reads, writes);
        }
        if let Some(right) = assign.right() {
            self.collect_expr(right, AccessMode::Read, reads, writes);
        }
    }

    fn collect_set_assign(
        &self,
        assign: SetAssign<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        if let Some(left) = assign.left() {
            self.collect_expr(left, AccessMode::ReadWrite, reads, writes);
        }
        if let Some(right) = assign.right() {
            self.collect_expr(right, AccessMode::Read, reads, writes);
        }
    }

    fn collect_dot_access(
        &self,
        dot_access: DotAccess<'_>,
        mode: AccessMode,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        let obj_mode = match mode {
            AccessMode::Read => AccessMode::Read,
            AccessMode::Write | AccessMode::ReadWrite => AccessMode::ReadWrite,
        };

        if let Some(obj) = dot_access.obj() {
            self.collect_expr(obj, obj_mode, reads, writes);
        }
    }

    fn collect_call(
        &self,
        call: Call<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        if let Some(callee) = call.callee() {
            self.collect_expr(callee, AccessMode::Read, reads, writes);
        }

        for argument in call.arguments() {
            if let Some(expr) = argument.expr() {
                let mode = if argument.mutate() {
                    AccessMode::ReadWrite
                } else {
                    AccessMode::Read
                };
                self.collect_expr(expr, mode, reads, writes);
            }
        }
    }

    fn collect_instance_arg(
        &self,
        arg: InstanceArg<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        if let Some(value) = arg.value() {
            self.collect_expr(value, AccessMode::Read, reads, writes);
            return;
        }

        if let Some(name) = arg.name() {
            self.collect_ident_access(name.syntax(), AccessMode::Read, reads, writes);
        }
    }

    fn collect_match(
        &self,
        match_expr: Match<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        if let Some(subject) = match_expr.expr() {
            self.collect_expr(subject, AccessMode::Read, reads, writes);
        }

        for arm in match_expr.arms() {
            match arm.pattern() {
                MatchPattern::Expr(expr) => {
                    self.collect_expr(expr, AccessMode::Read, reads, writes);
                }
                MatchPattern::Type(_) | MatchPattern::Else => {}
            }

            if let Some(body) = arm.body() {
                self.collect_match_arm_body(body, reads, writes);
            }
        }
    }

    fn collect_match_arm_body(
        &self,
        body: MatchArmBody<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        match body {
            MatchArmBody::Block(block) => {
                for stmt in block.stmts() {
                    self.collect_stmt_inline(stmt, reads, writes);
                }
            }
            MatchArmBody::Return(ret) => {
                if let Some(expr) = ret.expr() {
                    self.collect_expr(expr, AccessMode::Read, reads, writes);
                }
            }
            MatchArmBody::Throw(throw_stmt) => {
                if let Some(expr) = throw_stmt.expr() {
                    self.collect_expr(expr, AccessMode::Read, reads, writes);
                }
            }
            MatchArmBody::Expr(expr) => self.collect_expr(expr, AccessMode::Read, reads, writes),
        }
    }

    fn collect_stmt_inline(
        &self,
        stmt: Stmt<'_>,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        match stmt {
            Stmt::Block(block) => {
                for nested in block.stmts() {
                    self.collect_stmt_inline(nested, reads, writes);
                }
            }
            Stmt::If(if_stmt) => {
                if let Some(cond) = if_stmt.condition() {
                    self.collect_expr(cond, AccessMode::Read, reads, writes);
                }
                if let Some(body) = if_stmt.body() {
                    for nested in body.stmts() {
                        self.collect_stmt_inline(nested, reads, writes);
                    }
                }
                if let Some(alt) = if_stmt.alternative() {
                    match alt {
                        IfAlt::If(else_if) => {
                            self.collect_stmt_inline(Stmt::If(else_if), reads, writes);
                        }
                        IfAlt::Block(else_block) => {
                            for nested in else_block.stmts() {
                                self.collect_stmt_inline(nested, reads, writes);
                            }
                        }
                    }
                }
            }
            Stmt::While(while_stmt) => {
                if let Some(cond) = while_stmt.condition() {
                    self.collect_expr(cond, AccessMode::Read, reads, writes);
                }
                if let Some(body) = while_stmt.body() {
                    for nested in body.stmts() {
                        self.collect_stmt_inline(nested, reads, writes);
                    }
                }
            }
            Stmt::Repeat(repeat_stmt) => {
                if let Some(count) = repeat_stmt.count() {
                    self.collect_expr(count, AccessMode::Read, reads, writes);
                }
                if let Some(body) = repeat_stmt.body() {
                    for nested in body.stmts() {
                        self.collect_stmt_inline(nested, reads, writes);
                    }
                }
            }
            Stmt::DoWhile(do_while) => {
                if let Some(body) = do_while.body() {
                    for nested in body.stmts() {
                        self.collect_stmt_inline(nested, reads, writes);
                    }
                }
                if let Some(cond) = do_while.condition() {
                    self.collect_expr(cond, AccessMode::Read, reads, writes);
                }
            }
            Stmt::TryCatch(try_catch) => {
                if let Some(body) = try_catch.body() {
                    for nested in body.stmts() {
                        self.collect_stmt_inline(nested, reads, writes);
                    }
                }
                if let Some(catch_clause) = try_catch.catch() {
                    if let Some(var1) = catch_clause.catch_var1() {
                        self.collect_definition_ident(var1.syntax(), writes);
                    }
                    if let Some(var2) = catch_clause.catch_var2() {
                        self.collect_definition_ident(var2.syntax(), writes);
                    }
                    if let Some(catch_body) = catch_clause.body() {
                        for nested in catch_body.stmts() {
                            self.collect_stmt_inline(nested, reads, writes);
                        }
                    }
                }
            }
            Stmt::Return(ret) => {
                if let Some(expr) = ret.expr() {
                    self.collect_expr(expr, AccessMode::Read, reads, writes);
                }
            }
            Stmt::Throw(throw_stmt) => {
                if let Some(expr) = throw_stmt.expr() {
                    self.collect_expr(expr, AccessMode::Read, reads, writes);
                }
            }
            Stmt::Assert(assert_stmt) => {
                if let Some(cond) = assert_stmt.condition() {
                    self.collect_expr(cond, AccessMode::Read, reads, writes);
                }
                if let Some(exc) = assert_stmt.expr() {
                    self.collect_expr(exc, AccessMode::Read, reads, writes);
                }
            }
            Stmt::Match(match_stmt) => {
                if let Some(match_expr) = match_stmt.expr() {
                    self.collect_match(match_expr, reads, writes);
                }
            }
            Stmt::ExprStmt(expr_stmt) => {
                if let Some(expr) = expr_stmt.expr() {
                    self.collect_expr(expr, AccessMode::Read, reads, writes);
                }
            }
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::EmptyStmt(_) | Stmt::Unmapped(_) => {}
        }
    }

    fn collect_var_pattern_writes(
        &self,
        pattern: VarDeclPattern<'_>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        match pattern {
            VarDeclPattern::TupleVars(tuple_vars) => {
                for nested in tuple_vars.vars() {
                    self.collect_var_pattern_writes(nested, writes);
                }
            }
            VarDeclPattern::TensorVars(tensor_vars) => {
                for nested in tensor_vars.vars() {
                    self.collect_var_pattern_writes(nested, writes);
                }
            }
            VarDeclPattern::VarDecl(var_decl) => {
                if let Some(name) = var_decl.name() {
                    if var_decl.is_redefinition() {
                        let start = name.syntax().start_byte() as u32;
                        if let Some(local) = self.uses_by_start.get(&start).copied() {
                            writes.insert(local);
                        } else if let Some(local) = self.defs_by_start.get(&start).copied() {
                            writes.insert(local);
                        }
                    } else {
                        self.collect_definition_ident(name.syntax(), writes);
                    }
                }
            }
        }
    }

    fn collect_ident_access(
        &self,
        ident: Node<'_>,
        mode: AccessMode,
        reads: &mut FxHashSet<LocalDefId>,
        writes: &mut FxHashSet<LocalDefId>,
    ) {
        let start = ident.start_byte() as u32;
        let local = self.uses_by_start.get(&start).copied();
        let Some(local) = local else {
            return;
        };

        match mode {
            AccessMode::Read => {
                reads.insert(local);
            }
            AccessMode::Write => {
                writes.insert(local);
            }
            AccessMode::ReadWrite => {
                reads.insert(local);
                writes.insert(local);
            }
        }
    }
}
