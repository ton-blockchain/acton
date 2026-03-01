use super::ast::{ExprAst, StmtAst, Var};
use super::render::{arg_as_i64, format_cell_literal, format_instruction_line};
use crate::types::{ArgValue, Code, Instruction, PlainInstruction};
use num_bigint::BigUint;
use std::collections::{BTreeMap, HashMap};
use tycho_types::util::Bitstring;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueType {
    Unknown,
    Int,
    Cell,
    Slice,
    Builder,
}

#[derive(Debug, Clone)]
enum StackValue {
    Expr(ExprAst),
    Continuation(Code),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LiftState {
    stack: Vec<StackValue>,
    params: Vec<String>,
    expr_types: HashMap<ExprAst, ValueType>,
    next_param: usize,
    next_temp: usize,
    has_explicit_return: bool,
}

impl LiftState {
    fn push_expr(&mut self, expr: impl Into<ExprAst>) {
        let expr = expr.into();
        let ty = infer_expr_type(&expr);
        self.push_typed_expr_ast(expr, ty);
    }

    fn push_typed_expr(&mut self, expr: impl Into<ExprAst>, ty: ValueType) {
        self.push_typed_expr_ast(expr.into(), ty);
    }

    fn push_typed_expr_ast(&mut self, expr: ExprAst, ty: ValueType) {
        self.expr_types.insert(expr.clone(), ty);
        self.stack.push(StackValue::Expr(expr));
    }

    fn expr_type_of(&self, expr: &ExprAst) -> ValueType {
        self.expr_types
            .get(expr)
            .copied()
            .unwrap_or_else(|| infer_expr_type(expr))
    }

    fn refine_expr_type(&mut self, expr: &ExprAst, expected: ValueType) {
        if expected == ValueType::Unknown {
            return;
        }

        let current = self.expr_type_of(expr);
        match current {
            ValueType::Unknown => {
                self.expr_types.insert(expr.clone(), expected);
            }
            _ if current == expected => {}
            _ => {
                self.expr_types.insert(expr.clone(), ValueType::Unknown);
            }
        }
    }

    fn push_cont(&mut self, code: Code) {
        self.stack.push(StackValue::Continuation(code));
    }

    fn pop_value(&mut self) -> StackValue {
        if let Some(v) = self.stack.pop() {
            return v;
        }

        let p = format!("arg{}", self.next_param);
        self.next_param += 1;
        if !self.params.contains(&p) {
            self.params.push(p.clone());
        }
        StackValue::Expr(ExprAst::Ident(p))
    }

    fn pop_expr(&mut self, stmts: &mut Vec<StmtAst>, depth: usize) -> ExprAst {
        match self.pop_value() {
            StackValue::Expr(v) => v,
            StackValue::Continuation(_) => {
                let name = self.new_temp();
                stmts.push(StmtAst::VarDecl {
                    binding: name.clone().into(),
                    expr: ExprAst::Number("0".to_string()),
                });
                push_line(
                    stmts,
                    depth,
                    ";; continuation used as scalar value".to_string(),
                );
                ExprAst::Ident(name)
            }
        }
    }

    fn pop_expr_expect(
        &mut self,
        stmts: &mut Vec<StmtAst>,
        depth: usize,
        expected: ValueType,
    ) -> ExprAst {
        let expr = self.pop_expr(stmts, depth);
        self.refine_expr_type(&expr, expected);
        expr
    }

    fn pop_cont(&mut self) -> Option<Code> {
        match self.stack.last() {
            Some(StackValue::Continuation(_)) => match self.stack.pop() {
                Some(StackValue::Continuation(code)) => Some(code),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn peek_expr_ast_for_return(&self) -> Option<ExprAst> {
        self.stack.last().and_then(|v| match v {
            StackValue::Expr(e) => Some(e.clone()),
            StackValue::Continuation(_) => None,
        })
    }

    pub(crate) fn peek_expr_type_for_return(&self) -> Option<ValueType> {
        self.stack.last().and_then(|v| match v {
            StackValue::Expr(e) => Some(self.expr_type_of(e)),
            StackValue::Continuation(_) => None,
        })
    }

    pub(crate) fn return_expr_asts(&self) -> Vec<ExprAst> {
        self.stack
            .iter()
            .filter_map(|v| match v {
                StackValue::Expr(e) => Some(e.clone()),
                StackValue::Continuation(_) => None,
            })
            .collect()
    }

    pub(crate) fn return_expr_types(&self) -> Vec<ValueType> {
        self.stack
            .iter()
            .filter_map(|v| match v {
                StackValue::Expr(e) => Some(self.expr_type_of(e)),
                StackValue::Continuation(_) => None,
            })
            .collect()
    }

    fn new_temp(&mut self) -> String {
        let name = format!("v{}", self.next_temp);
        self.next_temp += 1;
        name
    }

    fn absorb_counters(&mut self, other: &Self) {
        self.next_param = self.next_param.max(other.next_param);
        self.next_temp = self.next_temp.max(other.next_temp);
        for p in &other.params {
            if !self.params.contains(p) {
                self.params.push(p.clone());
            }
        }
        self.has_explicit_return |= other.has_explicit_return;
    }

    pub(crate) fn params(&self) -> &[String] {
        &self.params
    }

    pub(crate) fn has_explicit_return(&self) -> bool {
        self.has_explicit_return
    }

    pub(crate) fn param_type(&self, param: &str) -> ValueType {
        self.expr_type_of(&ExprAst::from(param))
    }

    pub(crate) fn seed_stack_with_exprs(&mut self, exprs: &[String]) {
        for expr in exprs {
            let ty = match expr.as_str() {
                "balance" | "msg_value" => ValueType::Int,
                "in_msg_full" => ValueType::Cell,
                "in_msg_body" => ValueType::Slice,
                _ => infer_expr_type(&ExprAst::from(expr.clone())),
            };
            self.push_typed_expr(expr.clone(), ty);
            if let Some(idx) = parse_arg_param_index(expr) {
                self.next_param = self.next_param.max(idx + 1);
                if !self.params.contains(expr) {
                    self.params.push(expr.clone());
                }
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LiftContext {
    pub(crate) calldict_arity: BTreeMap<u64, usize>,
    pub(crate) ifjmp_unit_return: bool,
    pub(crate) pushslice_helpers: BTreeMap<String, String>,
}

#[derive(Default)]
pub(crate) struct LiftResult {
    pub(crate) stmts: Vec<StmtAst>,
}

pub(crate) fn lift_instructions(
    instructions: &[Instruction],
    state: &mut LiftState,
    stmts: &mut Vec<StmtAst>,
    depth: usize,
    ctx: &LiftContext,
) {
    for instruction in instructions {
        match instruction {
            Instruction::Plain(plain) => lift_plain_instruction(plain, state, stmts, depth, ctx),
            Instruction::Ref(reference) => {
                if let ArgValue::Code { code, .. } = &reference.code {
                    // Code refs in a continuation chain are executed sequentially
                    // (implicit jump to next ref), so they must mutate the same state.
                    lift_instructions(&code.instructions, state, stmts, depth, ctx);
                }
            }
            Instruction::ExoticCell(_) => {
                push_line(
                    stmts,
                    depth,
                    ";; exotic cell ignored in structured pass".to_string(),
                );
            }
        }
    }
}

fn lift_plain_instruction(
    plain: &PlainInstruction,
    state: &mut LiftState,
    stmts: &mut Vec<StmtAst>,
    depth: usize,
    ctx: &LiftContext,
) {
    if is_push_cont(&plain.name)
        && let Some(code) = first_code_arg(plain)
    {
        state.push_cont(code);
        return;
    }

    match plain.name.as_str() {
        "IFJMP" | "IFJMPREF" | "IFNOTJMP" => {
            let cont = first_code_arg(plain).or_else(|| state.pop_cont());
            let Some(cont) = cont else {
                push_line(
                    stmts,
                    depth,
                    format!(";; {} without continuation", plain.name),
                );
                return;
            };
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let mut child = state.clone();
            let mut then_body = Vec::new();
            lift_instructions(
                &cont.instructions,
                &mut child,
                &mut then_body,
                depth + 1,
                ctx,
            );
            state.absorb_counters(&child);

            if ctx.ifjmp_unit_return {
                then_body.push(StmtAst::Return(None));
                state.has_explicit_return = true;
            }

            if !then_body.is_empty() {
                stmts.push(StmtAst::If {
                    negated: plain.name == "IFNOTJMP",
                    condition: cond,
                    then_body,
                    else_body: None,
                });
            }
            return;
        }
        "IF" | "IFREF" => {
            let cont = first_code_arg(plain).or_else(|| state.pop_cont());
            let Some(cont) = cont else {
                push_line(
                    stmts,
                    depth,
                    format!(";; {} without continuation", plain.name),
                );
                return;
            };
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let base_state = state.clone();
            let mut child = state.clone();
            let mut child_stmts = Vec::new();
            lift_instructions(
                &cont.instructions,
                &mut child,
                &mut child_stmts,
                depth + 1,
                ctx,
            );
            if let Some(cond_value) = parse_const_int_expr_ast(&cond) {
                state.absorb_counters(&child);
                if cond_value != 0 {
                    stmts.extend(child_stmts);
                    state.stack = child.stack;
                } else {
                    state.stack = base_state.stack;
                }
                return;
            }
            state.absorb_counters(&child);
            let mut pre_if_stmts = Vec::new();
            let merged = merge_if_stacks(
                &child,
                &base_state,
                state,
                &mut pre_if_stmts,
                &mut child_stmts,
                depth,
            );
            stmts.extend(pre_if_stmts);
            if !child_stmts.is_empty() {
                stmts.push(StmtAst::If {
                    negated: false,
                    condition: cond,
                    then_body: child_stmts,
                    else_body: None,
                });
            }
            if !merged {
                state.stack = base_state.stack;
            }
            return;
        }
        "IFELSE" | "IFREFELSE" => {
            let mut code_args = code_args(plain);
            let merge_base_next_temp = state.next_temp;

            let (then_cont, else_cont, cond) = match code_args.len() {
                n if n >= 2 => (
                    Some(code_args.remove(0)),
                    Some(code_args.remove(0)),
                    state.pop_expr_expect(stmts, depth, ValueType::Int),
                ),
                1 => {
                    let inline = code_args.remove(0);
                    let stacked = state.pop_cont();
                    let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
                    if plain.name == "IFREFELSE" {
                        (Some(inline), stacked, cond)
                    } else {
                        (stacked, Some(inline), cond)
                    }
                }
                _ => {
                    let else_c = state.pop_cont();
                    let then_c = state.pop_cont();
                    let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
                    (then_c, else_c, cond)
                }
            };

            let Some(then_cont) = then_cont else {
                push_line(
                    stmts,
                    depth,
                    ";; IFELSE missing then continuation".to_string(),
                );
                return;
            };
            let Some(else_cont) = else_cont else {
                push_line(
                    stmts,
                    depth,
                    ";; IFELSE missing else continuation".to_string(),
                );
                return;
            };

            let mut then_state = state.clone();
            let mut then_stmts = Vec::new();
            lift_instructions(
                &then_cont.instructions,
                &mut then_state,
                &mut then_stmts,
                depth + 1,
                ctx,
            );
            let mut else_state = state.clone();
            let mut else_stmts = Vec::new();
            lift_instructions(
                &else_cont.instructions,
                &mut else_state,
                &mut else_stmts,
                depth + 1,
                ctx,
            );
            if let Some(cond_value) = parse_const_int_expr_ast(&cond) {
                state.absorb_counters(&then_state);
                state.absorb_counters(&else_state);
                if cond_value != 0 {
                    stmts.extend(then_stmts);
                    state.stack = then_state.stack;
                } else {
                    stmts.extend(else_stmts);
                    state.stack = else_state.stack;
                }
                return;
            }
            state.absorb_counters(&then_state);
            state.absorb_counters(&else_state);
            let mut pre_if_stmts = Vec::new();
            let merged = merge_ifelse_stacks(
                &cond,
                &then_state,
                &else_state,
                state,
                &mut pre_if_stmts,
                &mut then_stmts,
                &mut else_stmts,
                depth,
                merge_base_next_temp,
            );
            stmts.extend(pre_if_stmts);
            if !then_stmts.is_empty() || !else_stmts.is_empty() {
                stmts.push(StmtAst::If {
                    negated: false,
                    condition: cond,
                    then_body: then_stmts,
                    else_body: Some(else_stmts),
                });
            }
            if !merged {
                state.stack.clear();
            }
            return;
        }
        "WHILE" => {
            let body_cont = state.pop_cont().or_else(|| first_code_arg(plain));
            let cond_cont = state.pop_cont();
            let Some(cond_cont) = cond_cont else {
                push_line(
                    stmts,
                    depth,
                    ";; WHILE missing condition continuation".to_string(),
                );
                return;
            };
            let Some(body_cont) = body_cont else {
                push_line(
                    stmts,
                    depth,
                    ";; WHILE missing body continuation".to_string(),
                );
                return;
            };

            let loop_cond = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: Var::name(loop_cond.clone()),
                expr: ExprAst::Number("0".to_string()),
            });

            let mut cond_state = state.clone();
            let mut cond_stmts = Vec::new();
            lift_instructions(
                &cond_cont.instructions,
                &mut cond_state,
                &mut cond_stmts,
                depth + 1,
                ctx,
            );
            let cond_expr = cond_state
                .peek_expr_ast_for_return()
                .unwrap_or_else(|| ExprAst::Number("0".to_string()));
            let mut body_state = state.clone();
            let mut body_stmts = Vec::new();
            lift_instructions(
                &body_cont.instructions,
                &mut body_state,
                &mut body_stmts,
                depth + 2,
                ctx,
            );

            let mut do_body = cond_stmts;
            do_body.push(StmtAst::Assign {
                target: loop_cond.clone(),
                expr: cond_expr,
            });
            do_body.push(StmtAst::If {
                negated: false,
                condition: ExprAst::Ident(loop_cond.clone()),
                then_body: body_stmts,
                else_body: None,
            });
            stmts.push(StmtAst::DoUntil {
                body: do_body,
                condition: ExprAst::Binary {
                    lhs: Box::new(ExprAst::Ident(loop_cond)),
                    op: "==".to_string(),
                    rhs: Box::new(ExprAst::Number("0".to_string())),
                    wrap_lhs: true,
                    wrap_rhs: false,
                },
            });
            state.absorb_counters(&cond_state);
            state.absorb_counters(&body_state);
            state.stack.clear();
            return;
        }
        "REPEAT" => {
            let cont = first_code_arg(plain).or_else(|| state.pop_cont());
            let Some(cont) = cont else {
                push_line(stmts, depth, ";; REPEAT missing continuation".to_string());
                return;
            };
            let count = state.pop_expr(stmts, depth);
            let mut child = state.clone();
            let mut body = Vec::new();
            lift_instructions(&cont.instructions, &mut child, &mut body, depth + 1, ctx);
            stmts.push(StmtAst::Repeat { count, body });

            state.absorb_counters(&child);
            state.stack.clear();
            return;
        }
        "UNTIL" => {
            let cont = state.pop_cont().or_else(|| first_code_arg(plain));
            let Some(cont) = cont else {
                push_line(stmts, depth, ";; UNTIL missing continuation".to_string());
                return;
            };

            let mut child = state.clone();
            let mut body = Vec::new();
            lift_instructions(&cont.instructions, &mut child, &mut body, depth + 1, ctx);
            let cond_expr = child
                .peek_expr_ast_for_return()
                .unwrap_or_else(|| ExprAst::Number("0".to_string()));
            stmts.push(StmtAst::DoUntil {
                body,
                condition: cond_expr,
            });

            state.absorb_counters(&child);
            state.stack.clear();
            return;
        }
        "IFRET" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            stmts.push(StmtAst::If {
                negated: false,
                condition: cond,
                then_body: vec![StmtAst::Return(None)],
                else_body: None,
            });
            state.has_explicit_return = true;
            return;
        }
        "IFNOTRET" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            stmts.push(StmtAst::If {
                negated: true,
                condition: cond,
                then_body: vec![StmtAst::Return(None)],
                else_body: None,
            });
            state.has_explicit_return = true;
            return;
        }
        _ => {}
    }

    match plain.name.as_str() {
        "XCHG" => {
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            match (i, j) {
                (Some(i), Some(j)) => stack_swap_from_top(state, i, j),
                (Some(i), None) => stack_swap_from_top(state, 0, i),
                _ => {}
            }
            return;
        }
        "DUP" => {
            let v = state.pop_value();
            let cloned = v.clone();
            state.stack.push(v);
            state.stack.push(cloned);
            return;
        }
        "OVER" => {
            if state.stack.len() >= 2 {
                let second = state.stack[state.stack.len() - 2].clone();
                state.stack.push(second);
            } else {
                let v = state.pop_value();
                state.stack.push(v.clone());
                state.stack.push(v);
            }
            return;
        }
        "SWAP" => {
            if state.stack.len() >= 2 {
                let n = state.stack.len();
                state.stack.swap(n - 1, n - 2);
            }
            return;
        }
        "XCHG_0I" => {
            if let Some(i) = plain.args.first().and_then(arg_stack_index) {
                stack_swap_from_top(state, 0, i);
            }
            return;
        }
        "XCHG_1I" => {
            // second arg is the effective target depth; first is usually s1 marker
            if let Some(i) = plain
                .args
                .get(1)
                .or_else(|| plain.args.first())
                .and_then(arg_stack_index)
            {
                stack_swap_from_top(state, 1, i);
            }
            return;
        }
        "XCHG_IJ" => {
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            if let (Some(i), Some(j)) = (i, j) {
                stack_swap_from_top(state, i, j);
            }
            return;
        }
        "XCHG2" => {
            // Equivalent: XCHG_1I s(i); XCHG_0I s(j)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_swap_from_top(state, 1, i);
            }
            if let Some(j) = j {
                stack_swap_from_top(state, 0, j);
            }
            return;
        }
        "XCHG3" => {
            // Equivalent: XCHG_IJ s2 s(i); XCHG_1I s(j); XCHG_0I s(k)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            let k = plain.args.get(2).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_swap_from_top(state, 2, i);
            }
            if let Some(j) = j {
                stack_swap_from_top(state, 1, j);
            }
            if let Some(k) = k {
                stack_swap_from_top(state, 0, k);
            }
            return;
        }
        "XCPU" => {
            // Equivalent: XCHG_0I s(i); PUSH s(j)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_swap_from_top(state, 0, i);
            }
            if let Some(j) = j {
                stack_push_from_top(state, j);
            }
            return;
        }
        "PUXC" => {
            // Equivalent: PUSH s(i); SWAP; XCHG_0I s(j)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_push_from_top(state, i);
            }
            if state.stack.len() >= 2 {
                let n = state.stack.len();
                state.stack.swap(n - 1, n - 2);
            }
            if let Some(j) = j {
                // PUXC final target is relative to stack before PUSH+SWAP.
                stack_swap_from_top(state, 0, j + 1);
            }
            return;
        }
        "PUSH2" => {
            // Equivalent: PUSH s(i); PUSH s(j+1)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_push_from_top(state, i);
            }
            if let Some(j) = j {
                stack_push_from_top(state, j + 1);
            }
            return;
        }
        "XC2PU" => {
            // Equivalent: XCHG2 s(i) s(j); PUSH s(k)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            let k = plain.args.get(2).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_swap_from_top(state, 1, i);
            }
            if let Some(j) = j {
                stack_swap_from_top(state, 0, j);
            }
            if let Some(k) = k {
                stack_push_from_top(state, k);
            }
            return;
        }
        "PU2XC" => {
            // Equivalent: PUSH s(i); SWAP; PUXC s(j) s(k-1)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            let k = plain.args.get(2).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_push_from_top(state, i);
            }
            if state.stack.len() >= 2 {
                let n = state.stack.len();
                state.stack.swap(n - 1, n - 2);
            }
            if let Some(j) = j {
                // PUXC stage: PUSH s(j)
                stack_push_from_top(state, j + 1);
                // PUXC stage: SWAP
                if state.stack.len() >= 2 {
                    let n = state.stack.len();
                    state.stack.swap(n - 1, n - 2);
                }
            }
            if let Some(k) = k {
                // PUXC stage: XCHG_0I s(k-1)
                stack_swap_from_top(state, 0, k + 2);
            }
            return;
        }
        "PUXC2" => {
            // Equivalent: PUSH s(i); XCHG_0I s2; XCHG2 s(j) s(k)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            let k = plain.args.get(2).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_push_from_top(state, i);
            }
            stack_swap_from_top(state, 0, 2);
            if let Some(j) = j {
                stack_swap_from_top(state, 1, j + 1);
            }
            if let Some(k) = k {
                stack_swap_from_top(state, 0, k + 1);
            }
            return;
        }
        "XCPUXC" => {
            // Equivalent: XCHG_1I s(i); PUXC s(j) s(k-1)
            let i = plain.args.first().and_then(arg_stack_index);
            let j = plain.args.get(1).and_then(arg_stack_index);
            let k = plain.args.get(2).and_then(arg_stack_index);
            if let Some(i) = i {
                stack_swap_from_top(state, 1, i);
            }
            if let Some(j) = j {
                stack_push_from_top(state, j);
                if state.stack.len() >= 2 {
                    let n = state.stack.len();
                    state.stack.swap(n - 1, n - 2);
                }
            }
            if let Some(k) = k {
                // Inner PUXC target is s(k-1).
                stack_swap_from_top(state, 0, k + 1);
            }
            return;
        }
        "2SWAP" => {
            if state.stack.len() >= 4 {
                let n = state.stack.len();
                state.stack.swap(n - 1, n - 3);
                state.stack.swap(n - 2, n - 4);
            }
            return;
        }
        "DROP" => {
            let _ = state.pop_value();
            return;
        }
        "DROP2" | "2DROP" => {
            let _ = state.pop_value();
            let _ = state.pop_value();
            return;
        }
        "BLKDROP" => {
            let count = plain.args.first().and_then(arg_as_i64).unwrap_or(0).max(0) as usize;
            for _ in 0..count {
                let _ = state.pop_value();
            }
            return;
        }
        "BLKDROP2" => {
            let i = plain.args.first().and_then(arg_as_i64).unwrap_or(0).max(0) as usize;
            let j = plain.args.get(1).and_then(arg_as_i64).unwrap_or(0).max(0) as usize;
            if i == 0 {
                return;
            }

            let required = i + j;
            if required > 0 {
                ensure_stack_depth(state, required - 1);
            }

            let len = state.stack.len();
            let end = len.saturating_sub(j);
            let start = end.saturating_sub(i);
            if start < end && end <= state.stack.len() {
                state.stack.drain(start..end);
            }
            return;
        }
        "PUSH" => {
            if let Some(depth_idx) = plain
                .args
                .first()
                .and_then(arg_scalar_text)
                .and_then(parse_stack_depth)
                && let Some(value) = stack_get_from_top(&state.stack, depth_idx)
            {
                state.stack.push(value.clone());
            }
            return;
        }
        "POP" => {
            let top = state.pop_value();
            if let Some(depth_idx) = plain
                .args
                .first()
                .and_then(arg_scalar_text)
                .and_then(parse_stack_depth)
            {
                // TVM `POP sN` is equivalent to `XCHG s0 sN; DROP`.
                // After popping top, the replacement target becomes s(N-1) in the remaining stack.
                if depth_idx > 0 {
                    stack_set_from_top(&mut state.stack, depth_idx - 1, top);
                }
            }
            return;
        }
        "NIP" => {
            if state.stack.len() >= 2 {
                let top = state.stack.pop().expect("stack len checked");
                let _ = state.stack.pop();
                state.stack.push(top);
            }
            return;
        }
        "ROT" => {
            if state.stack.len() >= 3 {
                let n = state.stack.len();
                state.stack.swap(n - 3, n - 2);
                state.stack.swap(n - 2, n - 1);
            }
            return;
        }
        "ROTREV" => {
            if state.stack.len() >= 3 {
                let n = state.stack.len();
                state.stack.swap(n - 1, n - 2);
                state.stack.swap(n - 2, n - 3);
            }
            return;
        }
        _ => {}
    }

    if plain.name.starts_with("PUSHINT") {
        if let Some(val) = plain.args.first().and_then(expr_from_arg_value) {
            state.push_expr(val);
        } else {
            state.push_expr(ExprAst::Number("0".to_string()));
        }
        return;
    }

    if plain.name == "PUSHPOW2" {
        let pow = first_arg_expr_or_zero(plain);
        state.push_typed_expr_ast(
            ExprAst::Binary {
                lhs: Box::new(ExprAst::Number("1".to_string())),
                op: "<<".to_string(),
                rhs: Box::new(pow),
                wrap_lhs: false,
                wrap_rhs: false,
            },
            ValueType::Int,
        );
        return;
    }

    if plain.name == "PUSHPOW2DEC" {
        let pow = first_arg_expr_or_zero(plain);
        let pow2 = ExprAst::Binary {
            lhs: Box::new(ExprAst::Number("1".to_string())),
            op: "<<".to_string(),
            rhs: Box::new(pow),
            wrap_lhs: false,
            wrap_rhs: false,
        };
        state.push_typed_expr_ast(
            ExprAst::Binary {
                lhs: Box::new(pow2),
                op: "-".to_string(),
                rhs: Box::new(ExprAst::Number("1".to_string())),
                wrap_lhs: true,
                wrap_rhs: false,
            },
            ValueType::Int,
        );
        return;
    }

    if plain.name == "PUSHSLICE" {
        if let Some(arg) = plain.args.first() {
            let expr = match arg {
                ArgValue::Cell(cell) => {
                    let literal = format_cell_literal(cell);
                    if let Some(helper) = ctx.pushslice_helpers.get(&literal) {
                        Some(call_expr(helper.clone(), Vec::<ExprAst>::new()))
                    } else {
                        func_slice_expr_from_cell_ast(cell)
                    }
                }
                _ => expr_from_arg_value(arg),
            };
            if let Some(expr) = expr {
                state.push_typed_expr_ast(expr, ValueType::Slice);
            }
        }
        return;
    }

    if plain.name == "PUSHNULL" {
        state.push_typed_expr_ast(ExprAst::NullLiteral, ValueType::Unknown);
        return;
    }

    if plain.name == "SDEQ" {
        let rhs = state.pop_expr_expect(stmts, depth, ValueType::Slice);
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Slice);
        let t = state.new_temp();
        stmts.push(StmtAst::VarDecl {
            binding: t.clone().into(),
            expr: call_expr("equal_slices_bits", vec![lhs, rhs]),
        });
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "SBITREFS" {
        let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
        let bits = state.new_temp();
        let refs = state.new_temp();
        stmts.push(StmtAst::VarDecl {
            binding: tensor_var(vec![bits.clone(), refs.clone()]),
            expr: call_expr("slice_bits_refs", vec![src]),
        });
        state.push_typed_expr(bits, ValueType::Int);
        state.push_typed_expr(refs, ValueType::Int);
        return;
    }

    if let Some(op) = binary_symbol(&plain.name) {
        let rhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        stmts.push(StmtAst::VarDecl {
            binding: t.clone().into(),
            expr: binary_expr(lhs, op, rhs),
        });
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if let Some((op, imm)) = immediate_binary_op(plain) {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        stmts.push(StmtAst::VarDecl {
            binding: t.clone().into(),
            expr: binary_expr(lhs, op, ExprAst::from(imm)),
        });
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "INC" || plain.name == "DEC" {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        let op = if plain.name == "INC" { "+" } else { "-" };
        stmts.push(StmtAst::VarDecl {
            binding: t.clone().into(),
            expr: ExprAst::Binary {
                lhs: Box::new(lhs),
                op: op.to_string(),
                rhs: Box::new(ExprAst::Number("1".to_string())),
                wrap_lhs: true,
                wrap_rhs: false,
            },
        });
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "NEGATE" {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        stmts.push(StmtAst::VarDecl {
            binding: t.clone().into(),
            expr: unary_expr("-", lhs),
        });
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "NOT" {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        stmts.push(StmtAst::VarDecl {
            binding: t.clone().into(),
            expr: unary_expr("~", lhs),
        });
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    match plain.name.as_str() {
        "LDU" | "LDUX" => {
            let dynamic_len = plain.name == "LDUX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                first_arg_expr_or_zero(plain)
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let value = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![next_slice.clone(), value.clone()]),
                expr: call_expr("load_uint", vec![src, bits]),
            });
            state.push_typed_expr(value, ValueType::Int);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "LDI" | "LDIX" => {
            let dynamic_len = plain.name == "LDIX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                first_arg_expr_or_zero(plain)
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let value = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![next_slice.clone(), value.clone()]),
                expr: call_expr("load_int", vec![src, bits]),
            });
            state.push_typed_expr(value, ValueType::Int);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "PLDU" | "PLDUX" => {
            let dynamic_len = plain.name == "PLDUX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                first_arg_expr_or_zero(plain)
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("preload_uint", vec![src, bits]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "PLDI" | "PLDIX" => {
            let dynamic_len = plain.name == "PLDIX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                first_arg_expr_or_zero(plain)
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("preload_int", vec![src, bits]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "LDSLICEX" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let bits = first_arg_expr_or_zero(plain);
            let next_slice = state.new_temp();
            let loaded = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![next_slice.clone(), loaded.clone()]),
                expr: call_expr("load_bits", vec![src, bits]),
            });
            state.push_typed_expr(loaded, ValueType::Slice);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "PLDSLICEX" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let bits = first_arg_expr_or_zero(plain);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("preload_bits", vec![src, bits]),
            });
            state.push_typed_expr(t, ValueType::Slice);
            return;
        }
        "LDMSGADDR" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let remainder = state.new_temp();
            let addr = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![remainder.clone(), addr.clone()]),
                expr: call_expr("load_msg_addr", vec![src]),
            });
            // TVM stack after LDMSGADDR is addr, remainder(top); stdlib
            // signature is (remainder, addr) because of asm(-> 1 0).
            state.push_typed_expr(addr, ValueType::Slice);
            state.push_typed_expr(remainder, ValueType::Slice);
            return;
        }
        "LDGRAMS" | "LDVARUINT16" | "LDREF" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let fn_name = stdlib_function_for_instruction(&plain.name)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("__{}", plain.name.to_lowercase()));
            let next_slice = state.new_temp();
            let value = state.new_temp();

            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![next_slice.clone(), value.clone()]),
                expr: call_expr(fn_name, vec![src]),
            });

            let value_ty = match plain.name.as_str() {
                "LDGRAMS" | "LDVARUINT16" => ValueType::Int,
                "LDREF" => ValueType::Cell,
                _ => ValueType::Unknown,
            };
            state.push_typed_expr(value, value_ty);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "LDOPTREF" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let value = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![next_slice.clone(), value.clone()]),
                expr: call_expr("load_maybe_ref", vec![src]),
            });
            state.push_typed_expr(value, ValueType::Cell);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "PLDOPTREF" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("preload_maybe_ref", vec![src]),
            });
            state.push_typed_expr(t, ValueType::Cell);
            return;
        }
        "CTOS" | "ENDC" | "HASHCU" | "SEMPTY" => {
            let in_ty = match plain.name.as_str() {
                "CTOS" => ValueType::Cell,
                "ENDC" => ValueType::Builder,
                "HASHCU" => ValueType::Cell,
                "SEMPTY" => ValueType::Slice,
                _ => ValueType::Unknown,
            };
            let src = state.pop_expr_expect(stmts, depth, in_ty);
            let t = state.new_temp();
            let fn_name = stdlib_function_for_instruction(&plain.name)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("__{}", plain.name.to_lowercase()));
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![src]),
            });
            let out_ty = match plain.name.as_str() {
                "CTOS" => ValueType::Slice,
                "ENDC" => ValueType::Cell,
                "HASHCU" | "SEMPTY" => ValueType::Int,
                _ => ValueType::Unknown,
            };
            state.push_typed_expr(t, out_ty);
            return;
        }
        "NEWC" => {
            state.push_typed_expr_ast(
                call_expr("begin_cell", Vec::<ExprAst>::new()),
                ValueType::Builder,
            );
            return;
        }
        "NEWDICT" => {
            state.push_typed_expr_ast(
                call_expr("new_dict", Vec::<ExprAst>::new()),
                ValueType::Cell,
            );
            return;
        }
        "DIVMOD" => {
            let rhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let q = state.new_temp();
            let r = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![q.clone(), r.clone()]),
                expr: call_expr("divmod", vec![lhs, rhs]),
            });
            state.push_typed_expr(q, ValueType::Int);
            state.push_typed_expr(r, ValueType::Int);
            return;
        }
        "MULDIVMOD" => {
            let z = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let y = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let x = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let q = state.new_temp();
            let r = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![q.clone(), r.clone()]),
                expr: call_expr("muldivmod", vec![x, y, z]),
            });
            state.push_typed_expr(q, ValueType::Int);
            state.push_typed_expr(r, ValueType::Int);
            return;
        }
        "MIN" | "MAX" => {
            let rhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            let fn_name = if plain.name == "MIN" { "min" } else { "max" };
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![lhs, rhs]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "ABS" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("abs", vec![src]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "MULDIV" | "MULDIVR" | "MULDIVC" => {
            let z = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let y = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let x = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            let fn_name = match plain.name.as_str() {
                "MULDIV" => "muldiv",
                "MULDIVR" => "muldivr",
                "MULDIVC" => "muldivc",
                _ => unreachable!(),
            };
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![x, y, z]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "STU" | "STI" => {
            let bits = first_arg_expr_or_zero(plain);
            let builder = state.pop_expr_expect(stmts, depth, ValueType::Builder);
            let value = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            let fn_name = if plain.name == "STU" {
                "store_uint"
            } else {
                "store_int"
            };
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![builder, value, bits]),
            });
            state.push_typed_expr(t, ValueType::Builder);
            return;
        }
        "STUX" | "STIX" => {
            let bits = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let builder = state.pop_expr_expect(stmts, depth, ValueType::Builder);
            let value = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            let fn_name = if plain.name == "STUX" {
                "store_uint"
            } else {
                "store_int"
            };
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![builder, value, bits]),
            });
            state.push_typed_expr(t, ValueType::Builder);
            return;
        }
        "STGRAMS" | "STVARUINT16" | "STSLICER" => {
            let value_ty = match plain.name.as_str() {
                "STSLICER" => ValueType::Slice,
                _ => ValueType::Int,
            };
            let value = state.pop_expr_expect(stmts, depth, value_ty);
            let builder = state.pop_expr_expect(stmts, depth, ValueType::Builder);
            let t = state.new_temp();
            let fn_name = stdlib_function_for_instruction(&plain.name)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("__{}", plain.name.to_lowercase()));
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![builder, value]),
            });
            state.push_typed_expr(t, ValueType::Builder);
            return;
        }
        "STREF" | "STDICT" | "STOPTREF" => {
            let builder = state.pop_expr_expect(stmts, depth, ValueType::Builder);
            let value = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            let t = state.new_temp();
            let fn_name = stdlib_function_for_instruction(&plain.name)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("__{}", plain.name.to_lowercase()));
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(fn_name, vec![builder, value]),
            });
            state.push_typed_expr(t, ValueType::Builder);
            return;
        }
        "SDSKIPFIRST" => {
            let len = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("skip_bits", vec![src, len]),
            });
            state.push_typed_expr(t, ValueType::Slice);
            return;
        }
        "INDEXVAR" => {
            let index = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let tuple = state.pop_expr(stmts, depth);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("at", vec![tuple, index]),
            });
            state.push_typed_expr(t, ValueType::Unknown);
            return;
        }
        "SKIPDICT" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("skip_dict", vec![src]),
            });
            state.push_typed_expr(t, ValueType::Slice);
            return;
        }
        "SKIPOPTREF" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let skipped = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![next_slice.clone(), skipped.clone()]),
                expr: call_expr("load_maybe_ref", vec![src]),
            });
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "ENDS" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            push_call(stmts, "end_parse", vec![src]);
            return;
        }
        "GETGLOB" => {
            let slot = plain
                .args
                .first()
                .and_then(arg_scalar_text)
                .unwrap_or_else(|| "0".to_string());
            state.push_expr(format!("__glob_{slot}"));
            return;
        }
        "SETGLOB" => {
            let value = state.pop_expr(stmts, depth);
            let slot = plain
                .args
                .first()
                .and_then(arg_scalar_text)
                .unwrap_or_else(|| "0".to_string());
            push_assign_expr(stmts, format!("__glob_{slot}"), value);
            return;
        }
        "CALLDICT" => {
            let target = plain
                .args
                .first()
                .and_then(arg_scalar_text)
                .unwrap_or_else(|| "0".to_string());
            let target_id = plain
                .args
                .first()
                .and_then(super::render::arg_as_u64)
                .or_else(|| target.parse::<u64>().ok());
            let arity = target_id
                .and_then(|id| ctx.calldict_arity.get(&id).copied())
                .unwrap_or(0);
            let mut args = Vec::with_capacity(arity);
            for _ in 0..arity {
                args.push(state.pop_expr(stmts, depth));
            }
            args.reverse();

            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr(format!("__dict_method_{target}"), args),
            });
            state.push_typed_expr(t, ValueType::Unknown);
            return;
        }
        "CALLXARGS" => {
            let argc = plain.args.first().and_then(arg_as_i64).unwrap_or(0).max(0) as usize;
            let method_id = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let mut args = Vec::with_capacity(argc);
            for _ in 0..argc {
                args.push(state.pop_expr(stmts, depth));
            }
            args.reverse();

            match argc {
                0 => push_call(stmts, "run_method0", vec![method_id.clone()]),
                1 => push_call(stmts, "run_method1", vec![method_id, args[0].clone()]),
                2 => push_call(
                    stmts,
                    "run_method2",
                    vec![method_id, args[0].clone(), args[1].clone()],
                ),
                3 => push_call(
                    stmts,
                    "run_method3",
                    vec![method_id, args[0].clone(), args[1].clone(), args[2].clone()],
                ),
                _ => push_line(
                    stmts,
                    depth,
                    format!(";; unsupported CALLXARGS arity {argc}"),
                ),
            }
            return;
        }
        "CALLREF" => {
            let cont = first_code_arg(plain).or_else(|| state.pop_cont());
            if let Some(cont) = cont {
                let mut child = state.clone();
                lift_instructions(&cont.instructions, &mut child, stmts, depth, ctx);
                state.absorb_counters(&child);
                state.stack = child.stack;
                state.has_explicit_return |= child.has_explicit_return;
                return;
            }
        }
        "PUSHCTR" => {
            match plain.args.first().and_then(arg_scalar_text).as_deref() {
                Some("c4") => state.push_typed_expr_ast(
                    call_expr("get_data", Vec::<ExprAst>::new()),
                    ValueType::Cell,
                ),
                Some("c3") => state.push_typed_expr_ast(
                    call_expr("get_c3", Vec::<ExprAst>::new()),
                    ValueType::Unknown,
                ),
                _ => {
                    push_line(
                        stmts,
                        depth,
                        format!(
                            ";; unhandled {}",
                            format_instruction_line(&Instruction::Plain(plain.clone()), depth)
                        ),
                    );
                }
            }
            return;
        }
        "POPCTR" => {
            let value = state.pop_expr(stmts, depth);
            match plain.args.first().and_then(arg_scalar_text).as_deref() {
                Some("c4") => {
                    state.refine_expr_type(&value, ValueType::Cell);
                    push_call(stmts, "set_data", vec![value]);
                }
                Some("c3") => push_call(stmts, "set_c3", vec![value]),
                _ => {
                    push_line(
                        stmts,
                        depth,
                        format!(
                            ";; unhandled {}",
                            format_instruction_line(&Instruction::Plain(plain.clone()), depth)
                        ),
                    );
                }
            }
            return;
        }
        "MYADDR" => {
            state.push_typed_expr_ast(
                call_expr("my_address", Vec::<ExprAst>::new()),
                ValueType::Slice,
            );
            return;
        }
        "MYCODE" => {
            state.push_typed_expr_ast(call_expr("my_code", Vec::<ExprAst>::new()), ValueType::Cell);
            return;
        }
        "DUEPAYMENT" => {
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("my_storage_due", Vec::<ExprAst>::new()),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "ISNULL" => {
            let src = state.pop_expr(stmts, depth);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("null?", vec![src]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "NOP" => {
            return;
        }
        "DUMP" => {
            let value = state.pop_expr(stmts, depth);
            push_call(stmts, "~dump", vec![value]);
            return;
        }
        "STRDUMP" => {
            let value = state.pop_expr(stmts, depth);
            push_call(stmts, "~strdump", vec![value]);
            return;
        }
        "REWRITESTDADDR" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let workchain = state.new_temp();
            let addr = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: tensor_var(vec![workchain.clone(), addr.clone()]),
                expr: call_expr("parse_std_addr", vec![src]),
            });
            // parse_std_addr returns workchain id and address integer.
            state.push_typed_expr(workchain, ValueType::Int);
            state.push_typed_expr(addr, ValueType::Int);
            return;
        }
        "GETORIGINALFWDFEE" => {
            // stdlib: get_original_fwd_fee(workchain, fwd_fee) asm(fwd_fee workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let fwd_fee = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("get_original_fwd_fee", vec![workchain, fwd_fee]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETGASFEE" => {
            // stdlib: get_compute_fee(workchain, gas_used) asm(gas_used workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let gas_used = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("get_compute_fee", vec![workchain, gas_used]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETFORWARDFEESIMPLE" => {
            // stdlib: get_simple_forward_fee(workchain, bits, cells)
            // asm(cells bits workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let bits = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let cells = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("get_simple_forward_fee", vec![workchain, bits, cells]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETFORWARDFEE" => {
            // stdlib: get_forward_fee(workchain, bits, cells) asm(cells bits workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let bits = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let cells = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("get_forward_fee", vec![workchain, bits, cells]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETSTORAGEFEE" => {
            // stdlib: get_storage_fee(workchain, seconds, bits, cells)
            // asm(cells bits seconds workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let seconds = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let bits = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let cells = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("get_storage_fee", vec![workchain, seconds, bits, cells]),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETPRECOMPILEDGAS" => {
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("get_precompiled_gas_consumption", Vec::<ExprAst>::new()),
            });
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "DICTUSETREF" => {
            let key_len = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let dict = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            let index = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let value = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            let t = state.new_temp();
            stmts.push(StmtAst::VarDecl {
                binding: t.clone().into(),
                expr: call_expr("udict_set_ref", vec![dict, key_len, index, value]),
            });
            state.push_typed_expr(t, ValueType::Cell);
            return;
        }
        "THROWIF" | "THROWIFNOT" | "THROWIFNOT_SHORT" => {
            let code = first_arg_expr_or_zero(plain);
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            if plain.name == "THROWIF" {
                push_call(stmts, "throw_if", vec![code, cond]);
            } else {
                push_call(stmts, "throw_unless", vec![code, cond]);
            }
            return;
        }
        "THROWARG" => {
            let code = first_arg_expr_or_zero(plain);
            let arg = state.pop_expr(stmts, depth);
            push_call(stmts, "throw_arg", vec![arg, code]);
            return;
        }
        "THROWARGIF" => {
            let code = first_arg_expr_or_zero(plain);
            let cond = state.pop_expr(stmts, depth);
            let arg = state.pop_expr(stmts, depth);
            push_call(stmts, "throw_arg_if", vec![arg, code, cond]);
            return;
        }
        "THROWARGIFNOT" => {
            let code = first_arg_expr_or_zero(plain);
            let cond = state.pop_expr(stmts, depth);
            let arg = state.pop_expr(stmts, depth);
            push_call(stmts, "throw_arg_unless", vec![arg, code, cond]);
            return;
        }
        "THROWANY" => {
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_call(stmts, "throw", vec![code]);
            return;
        }
        "THROWARGANY" => {
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let arg = state.pop_expr(stmts, depth);
            push_call(stmts, "throw_arg", vec![arg, code]);
            return;
        }
        "THROWANYIFNOT" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_call(stmts, "throw_unless", vec![code, cond]);
            return;
        }
        "THROWARGANYIFNOT" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let arg = state.pop_expr(stmts, depth);
            push_call(stmts, "throw_arg_unless", vec![arg, code, cond]);
            return;
        }
        "THROWANYIF" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_call(stmts, "throw_if", vec![code, cond]);
            return;
        }
        "THROWARGANYIF" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let arg = state.pop_expr(stmts, depth);
            push_call(stmts, "throw_arg_if", vec![arg, code, cond]);
            return;
        }
        "RAWRESERVE" => {
            let mode = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let amount = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_call(stmts, "raw_reserve", vec![amount, mode]);
            return;
        }
        "SENDRAWMSG" => {
            let mode = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let msg = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            push_call(stmts, "send_raw_message", vec![msg, mode]);
            return;
        }
        "SETCODE" => {
            let code = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            push_call(stmts, "set_code", vec![code]);
            return;
        }
        "RET" | "RETALT" => {
            stmts.push(StmtAst::Return(None));
            state.has_explicit_return = true;
            return;
        }
        _ => {}
    }

    push_line(
        stmts,
        depth,
        format!(
            ";; unhandled {}",
            format_instruction_line(&Instruction::Plain(plain.clone()), depth)
        ),
    );
}

fn is_push_cont(name: &str) -> bool {
    name.starts_with("PUSHCONT")
}

fn first_code_arg(plain: &PlainInstruction) -> Option<Code> {
    plain.args.iter().find_map(|arg| {
        if let ArgValue::Code { code, .. } = arg {
            return Some((**code).clone());
        }
        None
    })
}

fn code_args(plain: &PlainInstruction) -> Vec<Code> {
    plain
        .args
        .iter()
        .filter_map(|arg| {
            if let ArgValue::Code { code, .. } = arg {
                return Some((**code).clone());
            }
            None
        })
        .collect()
}

fn merge_if_stacks(
    then_state: &LiftState,
    else_state: &LiftState,
    state: &mut LiftState,
    pre_if_stmts: &mut Vec<StmtAst>,
    then_stmts: &mut Vec<StmtAst>,
    _depth: usize,
) -> bool {
    if then_state.stack.len() != else_state.stack.len() {
        return false;
    }

    state.stack.clear();
    for (then_value, else_value) in then_state.stack.iter().zip(&else_state.stack) {
        let (StackValue::Expr(then_expr), StackValue::Expr(else_expr)) = (then_value, else_value)
        else {
            return false;
        };

        if then_expr == else_expr {
            let ty = then_state.expr_type_of(then_expr);
            state.push_typed_expr_ast(then_expr.clone(), ty);
            continue;
        }

        let merged = state.new_temp();
        pre_if_stmts.push(StmtAst::VarDecl {
            binding: merged.clone().into(),
            expr: else_expr.clone(),
        });
        push_assign_expr(then_stmts, merged.clone(), then_expr.clone());
        let then_ty = then_state.expr_type_of(then_expr);
        let else_ty = else_state.expr_type_of(else_expr);
        let merged_ty = merged_value_type(then_expr, then_ty, else_expr, else_ty);
        state.push_typed_expr(merged, merged_ty);
    }

    true
}

fn merge_ifelse_stacks(
    cond: &ExprAst,
    then_state: &LiftState,
    else_state: &LiftState,
    state: &mut LiftState,
    pre_if_stmts: &mut Vec<StmtAst>,
    then_stmts: &mut Vec<StmtAst>,
    else_stmts: &mut Vec<StmtAst>,
    _depth: usize,
    base_next_temp: usize,
) -> bool {
    if then_state.stack.len() != else_state.stack.len() {
        return false;
    }

    state.stack.clear();
    for (then_value, else_value) in then_state.stack.iter().zip(&else_state.stack) {
        let (StackValue::Expr(then_expr), StackValue::Expr(else_expr)) = (then_value, else_value)
        else {
            return false;
        };

        if then_expr == else_expr {
            let ty = then_state.expr_type_of(then_expr);
            state.push_typed_expr_ast(then_expr.clone(), ty);
            continue;
        }

        let merged = state.new_temp();
        let then_ty = then_state.expr_type_of(then_expr);
        let else_ty = else_state.expr_type_of(else_expr);
        let merged_ty = merged_value_type(then_expr, then_ty, else_expr, else_ty);
        if !is_branch_local_temp_expr(then_expr, base_next_temp)
            && !is_branch_local_temp_expr(else_expr, base_next_temp)
        {
            pre_if_stmts.push(StmtAst::VarDecl {
                binding: merged.clone().into(),
                expr: ExprAst::Ternary {
                    condition: Box::new(cond.clone()),
                    then_expr: Box::new(then_expr.clone()),
                    else_expr: Box::new(else_expr.clone()),
                },
            });
        } else {
            let init_expr = if merged_ty == ValueType::Int {
                ExprAst::Number("0".to_string())
            } else {
                ExprAst::NullLiteral
            };
            pre_if_stmts.push(StmtAst::VarDecl {
                binding: merged.clone().into(),
                expr: init_expr,
            });
            push_assign_expr(then_stmts, merged.clone(), then_expr.clone());
            push_assign_expr(else_stmts, merged.clone(), else_expr.clone());
        }
        state.push_typed_expr(merged, merged_ty);
    }

    true
}

fn push_line(stmts: &mut Vec<StmtAst>, _depth: usize, line: String) {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return;
    }
    let comment = if trimmed.starts_with(";;") {
        trimmed.to_string()
    } else {
        format!(";; {trimmed}")
    };
    stmts.push(StmtAst::Comment(comment));
}

fn push_assign_expr(stmts: &mut Vec<StmtAst>, target: impl Into<String>, expr: ExprAst) {
    stmts.push(StmtAst::Assign {
        target: target.into(),
        expr,
    });
}

fn expr_from_arg_value(arg: &ArgValue) -> Option<ExprAst> {
    match arg {
        ArgValue::Int(value) => Some(ExprAst::Number(value.to_string())),
        ArgValue::UInt(value) => Some(ExprAst::Number(value.to_string())),
        ArgValue::Control(control) => Some(ExprAst::Ident(format!("c{}", control.idx))),
        ArgValue::StackRegister(reg) => Some(ExprAst::Ident(format!("s{}", reg.idx))),
        ArgValue::Cell(cell) => Some(ExprAst::Atom(format_cell_literal(cell))),
        ArgValue::Code { .. } | ArgValue::CodeDictionary(_) => None,
    }
}

fn first_arg_expr_or_zero(plain: &PlainInstruction) -> ExprAst {
    plain
        .args
        .first()
        .and_then(expr_from_arg_value)
        .unwrap_or_else(|| ExprAst::Number("0".to_string()))
}

fn arg_scalar_text(arg: &ArgValue) -> Option<String> {
    let expr = expr_from_arg_value(arg)?;
    match expr {
        ExprAst::Ident(text) | ExprAst::Atom(text) | ExprAst::Number(text) => Some(text),
        ExprAst::NullLiteral => Some("null()".to_string()),
        _ => None,
    }
}

fn call_expr<T>(callee: impl Into<String>, args: Vec<T>) -> ExprAst
where
    T: Into<ExprAst>,
{
    ExprAst::Call {
        callee: callee.into(),
        args: args.into_iter().map(Into::into).collect(),
    }
}

fn push_call<T>(stmts: &mut Vec<StmtAst>, callee: impl Into<String>, args: Vec<T>)
where
    T: Into<ExprAst>,
{
    stmts.push(StmtAst::Call {
        callee: callee.into(),
        args: args.into_iter().map(Into::into).collect(),
    });
}

fn tensor_var(items: Vec<String>) -> Var {
    Var::tensor(items.into_iter().map(Var::name).collect())
}

fn unary_expr(op: impl Into<String>, expr: impl Into<ExprAst>) -> ExprAst {
    ExprAst::Unary {
        op: op.into(),
        expr: Box::new(expr.into()),
    }
}

fn binary_expr(lhs: impl Into<ExprAst>, op: impl Into<String>, rhs: impl Into<ExprAst>) -> ExprAst {
    ExprAst::Binary {
        lhs: Box::new(lhs.into()),
        op: op.into(),
        rhs: Box::new(rhs.into()),
        wrap_lhs: true,
        wrap_rhs: true,
    }
}

fn is_branch_local_temp_expr(expr: &ExprAst, base_next_temp: usize) -> bool {
    match expr {
        ExprAst::Ident(name) | ExprAst::Atom(name) => name
            .strip_prefix('v')
            .and_then(|s| s.parse::<usize>().ok())
            .is_some_and(|idx| idx >= base_next_temp),
        _ => false,
    }
}

fn merged_value_type(
    then_expr: &ExprAst,
    then_ty: ValueType,
    else_expr: &ExprAst,
    else_ty: ValueType,
) -> ValueType {
    if then_ty == else_ty {
        return then_ty;
    }
    if matches!(then_expr, ExprAst::NullLiteral) {
        return else_ty;
    }
    if matches!(else_expr, ExprAst::NullLiteral) {
        return then_ty;
    }
    ValueType::Unknown
}

fn binary_symbol(name: &str) -> Option<&'static str> {
    match name {
        "ADD" => Some("+"),
        "SUB" => Some("-"),
        "MUL" => Some("*"),
        "DIV" => Some("/"),
        "DIVR" => Some("~/"),
        "DIVC" => Some("^/"),
        "MOD" => Some("%"),
        "MODR" => Some("~%"),
        "MODC" => Some("^%"),
        "LSHIFT" => Some("<<"),
        "RSHIFT" => Some(">>"),
        "RSHIFTR" => Some("~>>"),
        "RSHIFTC" => Some("^>>"),
        "AND" => Some("&"),
        "OR" => Some("|"),
        "XOR" => Some("^"),
        "GREATER" => Some(">"),
        "LESS" => Some("<"),
        "EQUAL" => Some("=="),
        "NEQ" => Some("!="),
        "LEQ" => Some("<="),
        "GEQ" => Some(">="),
        "CMP" => Some("<=>"),
        _ => None,
    }
}

fn immediate_binary_op(plain: &PlainInstruction) -> Option<(&'static str, String)> {
    let imm = plain.args.first().and_then(arg_scalar_text)?;
    match plain.name.as_str() {
        "ADDINT" => Some(("+", imm)),
        "MULINT" => Some(("*", imm)),
        "LSHIFT" => Some(("<<", imm)),
        "RSHIFT" => Some((">>", imm)),
        "RSHIFTR" => Some(("~>>", imm)),
        "RSHIFTC" => Some(("^>>", imm)),
        "EQINT" => Some(("==", imm)),
        "LESSINT" => Some(("<", imm)),
        "GTINT" => Some((">", imm)),
        _ => None,
    }
}

fn stdlib_function_for_instruction(name: &str) -> Option<&'static str> {
    match name {
        "CTOS" => Some("begin_parse"),
        "LDREF" => Some("load_ref"),
        "LDGRAMS" => Some("load_grams"),
        "LDVARUINT16" => Some("load_coins"),
        "LDMSGADDR" => Some("load_msg_addr"),
        "LDOPTREF" => Some("load_maybe_ref"),
        "PLDOPTREF" => Some("preload_maybe_ref"),
        "SEMPTY" => Some("slice_empty?"),
        "ENDC" => Some("end_cell"),
        "HASHCU" => Some("cell_hash"),
        "STREF" => Some("store_ref"),
        "STSLICER" => Some("store_slice"),
        "STGRAMS" => Some("store_grams"),
        "STVARUINT16" => Some("store_coins"),
        "STDICT" => Some("store_dict"),
        "STOPTREF" => Some("store_maybe_ref"),
        _ => None,
    }
}

fn func_slice_expr_from_cell_ast(cell: &tycho_types::cell::Cell) -> Option<ExprAst> {
    let slice = cell.as_slice_allow_exotic();
    if slice.size_refs() != 0 {
        // Non-empty refs cannot be represented as a plain slice constant in FunC.
        return Some(ExprAst::Atom("\"\"".to_string()));
    }

    let bits_hex = slice.display_data().to_string();
    bitstring_hex_to_func_slice_ast(&bits_hex)
}

fn bitstring_hex_to_func_slice_ast(bits_hex: &str) -> Option<ExprAst> {
    let (bytes, bit_len) = Bitstring::from_hex_str(bits_hex).ok()?;
    if bit_len == 0 {
        return Some(ExprAst::Atom("\"\"".to_string()));
    }

    let total_bits = bit_len as usize;
    let mut offset = 0usize;
    let mut builder_expr = call_expr("begin_cell", Vec::<ExprAst>::new());
    while offset < total_bits {
        let chunk_len = (total_bits - offset).min(256);
        let chunk_value = bits_to_biguint(&bytes, offset, chunk_len);
        builder_expr = call_expr(
            "store_uint",
            vec![
                builder_expr,
                ExprAst::Number(chunk_value.to_string()),
                ExprAst::Number(chunk_len.to_string()),
            ],
        );
        offset += chunk_len;
    }

    Some(call_expr(
        "begin_parse",
        vec![call_expr("end_cell", vec![builder_expr])],
    ))
}

fn bits_to_biguint(bytes: &[u8], start_bit: usize, bit_len: usize) -> BigUint {
    let mut value = BigUint::default();
    for bit_idx in start_bit..(start_bit + bit_len) {
        value <<= 1usize;
        if bit_is_one(bytes, bit_idx) {
            value += BigUint::from(1u8);
        }
    }
    value
}

fn bit_is_one(bytes: &[u8], bit_idx: usize) -> bool {
    let byte = bytes.get(bit_idx / 8).copied().unwrap_or(0);
    let shift = 7 - (bit_idx % 8);
    ((byte >> shift) & 1) != 0
}

fn infer_expr_type(expr: &ExprAst) -> ValueType {
    match expr {
        ExprAst::Ident(_) => ValueType::Unknown,
        ExprAst::Number(_) => ValueType::Int,
        ExprAst::NullLiteral => ValueType::Unknown,
        ExprAst::Call { callee, args } if args.is_empty() => match callee.as_str() {
            "begin_cell" => ValueType::Builder,
            "new_dict" | "get_data" | "my_code" => ValueType::Cell,
            "my_address" => ValueType::Slice,
            _ => ValueType::Unknown,
        },
        ExprAst::Atom(text) => match text.as_str() {
            _ if text.starts_with("x{") => ValueType::Slice,
            _ if text.parse::<i128>().is_ok() => ValueType::Int,
            _ => ValueType::Unknown,
        },
        _ => ValueType::Unknown,
    }
}

fn parse_stack_depth(token: String) -> Option<usize> {
    let pos = parse_stack_position(&token)?;
    (pos >= 0).then_some(pos as usize)
}

fn arg_stack_index(arg: &ArgValue) -> Option<i64> {
    match arg {
        ArgValue::StackRegister(reg) => Some(reg.idx),
        _ => arg_scalar_text(arg).and_then(|s| parse_stack_position(&s)),
    }
}

fn parse_stack_position(token: &str) -> Option<i64> {
    let raw = token.strip_prefix('s')?;
    if raw.starts_with('(') && raw.ends_with(')') && raw.len() >= 3 {
        return raw[1..raw.len() - 1].parse::<i64>().ok();
    }
    raw.parse::<i64>().ok()
}

fn normalize_stack_index(idx: i64) -> usize {
    idx.max(0) as usize
}

fn stack_index_from_top(len: usize, depth_idx: usize) -> usize {
    len.saturating_sub(1 + depth_idx)
}

fn fresh_param_stack_value(state: &mut LiftState) -> StackValue {
    let p = format!("arg{}", state.next_param);
    state.next_param += 1;
    if !state.params.contains(&p) {
        state.params.push(p.clone());
    }
    StackValue::Expr(ExprAst::Ident(p))
}

fn ensure_stack_depth(state: &mut LiftState, depth_idx: usize) {
    while state.stack.len() <= depth_idx {
        let v = fresh_param_stack_value(state);
        state.stack.insert(0, v);
    }
}

fn stack_swap_from_top(state: &mut LiftState, a: i64, b: i64) {
    let a = normalize_stack_index(a);
    let b = normalize_stack_index(b);
    let max_depth = a.max(b);
    ensure_stack_depth(state, max_depth);
    let ia = stack_index_from_top(state.stack.len(), a);
    let ib = stack_index_from_top(state.stack.len(), b);
    state.stack.swap(ia, ib);
}

fn stack_push_from_top(state: &mut LiftState, depth: i64) {
    let depth = normalize_stack_index(depth);
    ensure_stack_depth(state, depth);
    let idx = stack_index_from_top(state.stack.len(), depth);
    if let Some(value) = state.stack.get(idx).cloned() {
        state.stack.push(value);
    }
}

fn stack_get_from_top(stack: &[StackValue], depth_idx: usize) -> Option<&StackValue> {
    if stack.len() <= depth_idx {
        return None;
    }
    let idx = stack.len().saturating_sub(1 + depth_idx);
    stack.get(idx)
}

fn stack_set_from_top(stack: &mut Vec<StackValue>, depth_idx: usize, value: StackValue) {
    if stack.len() <= depth_idx {
        return;
    }
    let idx = stack.len().saturating_sub(1 + depth_idx);
    if let Some(slot) = stack.get_mut(idx) {
        *slot = value;
    }
}

fn parse_arg_param_index(name: &str) -> Option<usize> {
    name.strip_prefix("arg")?.parse::<usize>().ok()
}

fn parse_const_int_expr(expr: &str) -> Option<i128> {
    let trimmed = expr.trim();
    if let Ok(value) = trimmed.parse::<i128>() {
        return Some(value);
    }
    if trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() >= 3 {
        return trimmed[1..trimmed.len() - 1].trim().parse::<i128>().ok();
    }
    None
}

fn parse_const_int_expr_ast(expr: &ExprAst) -> Option<i128> {
    match expr {
        ExprAst::Number(value) => value.parse::<i128>().ok(),
        ExprAst::Ident(_) => None,
        ExprAst::Atom(value) => parse_const_int_expr(value),
        _ => None,
    }
}
