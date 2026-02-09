use crate::flow_inference::{FlowContext, SinkExpr, UnreachableKind};
use crate::try_flow;
use crate::type_inference::TypeInferenceWalker;
use crate::type_interner::TyId;
use tolk_resolver::AstNodeSpanExt;
use tolk_syntax::*;

impl<'db, 'a, 't> TypeInferenceWalker<'db, 'a> {
    pub(crate) fn process_stmt(&mut self, v: Stmt<'t>, flow: FlowContext) -> FlowContext {
        match v {
            Stmt::ExprStmt(expr_stmt) => self.process_expr_stmt(expr_stmt, flow),
            Stmt::Block(block_stmt) => self.process_block_stmt(block_stmt, flow),
            Stmt::If(if_stmt) => self.process_if_stmt(if_stmt, flow),
            Stmt::While(while_stmt) => self.process_while_stmt(while_stmt, flow),
            Stmt::DoWhile(do_while_stmt) => self.process_do_while_stmt(do_while_stmt, flow),
            Stmt::Repeat(repeat_stmt) => self.process_repeat_stmt(repeat_stmt, flow),
            Stmt::TryCatch(try_catch_stmt) => self.process_try_catch_stmt(try_catch_stmt, flow),
            Stmt::Return(return_stmt) => self.process_return_stmt(return_stmt, flow),
            Stmt::Throw(throw_stmt) => self.process_throw_stmt(throw_stmt, flow),
            Stmt::Assert(assert_stmt) => self.process_assert_stmt(assert_stmt, flow),
            Stmt::Break(break_stmt) => self.process_break_stmt(break_stmt, flow),
            Stmt::Continue(continue_stmt) => self.process_continue_stmt(continue_stmt, flow),
            Stmt::Match(match_stmt) => self.process_match_stmt(match_stmt, flow),
            Stmt::EmptyStmt(_) => flow,
            Stmt::Unmapped(_) => flow,
        }
    }

    //+ CHECKED
    fn process_expr_stmt(&mut self, v: ExprStmt<'t>, flow: FlowContext) -> FlowContext {
        let expr = try_flow!(flow, v.expr());
        let expr_flow = self.infer_expr(expr, flow, false, None);
        expr_flow.out_flow
    }

    //+ CHECKED
    pub(crate) fn process_block_stmt(&mut self, v: Block<'t>, flow: FlowContext) -> FlowContext {
        let mut next_flow = flow;
        for stmt in v.stmts() {
            next_flow = self.process_stmt(stmt, next_flow);
        }
        next_flow
    }

    //+ CHECKED
    fn process_if_stmt(&mut self, v: If<'t>, flow: FlowContext) -> FlowContext {
        let condition = try_flow!(flow, v.condition());
        let body = try_flow!(flow, v.body());

        let after_cond = self.infer_expr(condition, flow, true, None);

        let true_flow = self.process_block_stmt(body, after_cond.true_flow);
        let false_flow = match v.alternative() {
            Some(alternative_node) => {
                let alt_stmt = alternative_node.as_stmt();
                self.process_stmt(alt_stmt, after_cond.false_flow)
            }
            None => after_cond.false_flow,
        };

        true_flow.merge_flow(&false_flow, self.intrn())
    }

    //+ CHECKED
    fn process_repeat_stmt(&mut self, v: Repeat<'t>, flow: FlowContext) -> FlowContext {
        let count = try_flow!(flow, v.count());
        let body = try_flow!(flow, v.body());

        let after_count = self.infer_expr(count, flow, false, None);
        self.process_block_stmt(body, after_count.out_flow)
    }

    //+ CHECKED
    fn process_while_stmt(&mut self, v: While<'t>, flow: FlowContext) -> FlowContext {
        // loops are inferred twice, to merge body outcome with the state before the loop
        // (a more correct approach would be not "twice", but "find a fixed point when state stop changing")
        // also remember, we don't have a `break` statement, that's why when loop exits, condition became false
        let condition = try_flow!(flow, v.condition());
        let body = try_flow!(flow, v.body());

        let loop_entry_flow = flow.clone();
        let after_cond = self.infer_expr(condition, flow, true, None);
        let body_flow = self.process_block_stmt(body, after_cond.true_flow);

        // second time, to refine all types
        let next_flow = loop_entry_flow.merge_flow(&body_flow, self.intrn());
        let after_cond2 = self.infer_expr(condition, next_flow, true, None);

        self.process_block_stmt(body, after_cond2.true_flow);

        after_cond2.false_flow
    }

    //+ CHECKED
    fn process_do_while_stmt(&mut self, v: DoWhile<'t>, flow: FlowContext) -> FlowContext {
        // do while is also handled twice; read comments above
        let condition = try_flow!(flow, v.condition());
        let body = try_flow!(flow, v.body());

        let loop_entry_flow = flow.clone();
        let next_flow = self.process_block_stmt(body, flow);

        let after_cond = self.infer_expr(condition, next_flow, true, None);

        // second time
        let next_flow = loop_entry_flow.merge_flow(&after_cond.true_flow, self.intrn());
        let body_flow = self.process_block_stmt(body, next_flow);
        let after_cond2 = self.infer_expr(condition, body_flow, true, None);

        after_cond2.false_flow
    }

    //+ CHECKED
    fn process_try_catch_stmt(&mut self, v: TryCatch<'t>, flow: FlowContext) -> FlowContext {
        let body = try_flow!(flow, v.body());
        let catch_clause = try_flow!(flow, v.catch());
        let catch_body = try_flow!(flow, catch_clause.body());

        let before_try = flow.clone();
        let try_end = self.process_block_stmt(body, flow);

        // `catch` has exactly 2 variables: excNo and arg (when missing, they are implicit underscores)
        // `arg` is a curious thing, it can be any TVM primitive, so assign unknown to it
        // hence, using `fInt(arg)` (int from parameter is a target type) or `arg as slice` works well
        // it's not truly correct, because `arg as (int,int)` also compiles, but can never happen, but let it be user responsibility
        let mut catch_flow = before_try;

        // first catch variable represents exit code, so it always an int.
        if let Some(catch_var1) = catch_clause.catch_var1() {
            let ty_int = self.const_intrn().ty_int;
            catch_flow = self.process_catch_variable(catch_var1, ty_int, catch_flow);
        }

        // second catch variable is additional argument and we don't know its type.
        if let Some(catch_var2) = catch_clause.catch_var2() {
            let ty_unknown = self.const_intrn().ty_unknown;
            catch_flow = self.process_catch_variable(catch_var2, ty_unknown, catch_flow);
        }

        let catch_end = self.process_block_stmt(catch_body, catch_flow);
        try_end.merge_flow(&catch_end, self.intrn())
    }

    //+ CHECKED
    fn process_catch_variable(&mut self, var: Ident, ty: TyId, flow: FlowContext) -> FlowContext {
        let mut flow = flow;
        let span = var.span();
        let name = try_flow!(flow, self.text_or_none(&var));
        let id = self.local_id_of(span);
        let sink = SinkExpr::from_def(name, id, 0);

        self.ctx.set_type(span, ty);
        flow.register_known_type(sink, ty);
        flow
    }

    //+ CHECKED
    pub(crate) fn process_return_stmt(&mut self, v: Return<'t>, flow: FlowContext) -> FlowContext {
        let expr = v.expr();

        let mut flow = if let Some(expr) = expr {
            let expr_flow = self.infer_expr(expr, flow, false, self.ctx.declared_return_ty);

            // store expression type for future unification, in C++ we store nodes itself,
            // but we don't want to do the same in Rust version since nodes require lifetimes.
            let ty = self.ctx.get_node_type_or_unknown(&expr);
            self.ctx.return_types.push(ty);
            expr_flow.out_flow
        } else {
            // for `return;` assume void type
            self.ctx.return_types.push(self.const_intrn().ty_void);
            flow
        };

        flow.mark_unreachable(UnreachableKind::ReturnStatement);
        flow
    }

    //+ CHECKED
    pub(crate) fn process_throw_stmt(&mut self, v: Throw<'t>, flow: FlowContext) -> FlowContext {
        // in C++ version we handle both possible expressions:
        // throw 0
        // throw (1, arg)
        //
        // here we do the same but implicitly
        let expr = try_flow!(flow, v.expr());
        let mut flow = self.infer_expr(expr, flow, false, None).out_flow;
        flow.mark_unreachable(UnreachableKind::ThrowStatement);
        flow
    }

    //+ CHECKED
    fn process_assert_stmt(&mut self, v: Assert<'t>, flow: FlowContext) -> FlowContext {
        let condition = try_flow!(flow, v.condition());
        let expr = try_flow!(flow, v.expr());

        let after_cond = self.infer_expr(condition, flow, true, None);
        self.infer_expr(expr, after_cond.false_flow, false, None);
        after_cond.true_flow
    }

    //+ CHECKED
    const fn process_break_stmt(&self, _: Break, flow: FlowContext) -> FlowContext {
        // for now there is no break statement in Tolk
        let mut flow = flow;
        flow.mark_unreachable(UnreachableKind::Break);
        flow
    }

    //+ CHECKED
    const fn process_continue_stmt(&self, _: Continue, flow: FlowContext) -> FlowContext {
        // for now there is no continue statement in Tolk
        let mut flow = flow;
        flow.mark_unreachable(UnreachableKind::Continue);
        flow
    }

    //+ CHECKED
    fn process_match_stmt(&mut self, v: MatchStmt<'t>, flow: FlowContext) -> FlowContext {
        let expr = try_flow!(flow, v.expr());
        let after_expr = self.infer_match(expr, flow, false, None);
        after_expr.out_flow
    }
}
