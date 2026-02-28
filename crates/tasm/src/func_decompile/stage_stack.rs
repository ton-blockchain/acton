use super::ast::StmtAst;
use super::render::{arg_as_i64, arg_to_string, format_func_slice_expr, format_instruction_line};
use crate::types::{ArgValue, Code, Instruction, PlainInstruction};
use std::collections::BTreeMap;

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
    Expr(String),
    Continuation(Code),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LiftState {
    stack: Vec<StackValue>,
    params: Vec<String>,
    expr_types: BTreeMap<String, ValueType>,
    next_param: usize,
    next_temp: usize,
    has_explicit_return: bool,
}

impl LiftState {
    fn push_expr(&mut self, expr: impl Into<String>) {
        let expr = expr.into();
        let ty = infer_literal_type(&expr);
        self.push_typed_expr(expr, ty);
    }

    fn push_typed_expr(&mut self, expr: impl Into<String>, ty: ValueType) {
        let expr = expr.into();
        self.expr_types.insert(expr.clone(), ty);
        self.stack.push(StackValue::Expr(expr));
    }

    fn expr_type_of(&self, expr: &str) -> ValueType {
        self.expr_types
            .get(expr)
            .copied()
            .unwrap_or_else(|| infer_literal_type(expr))
    }

    fn refine_expr_type(&mut self, expr: &str, expected: ValueType) {
        if expected == ValueType::Unknown {
            return;
        }

        let current = self.expr_type_of(expr);
        match current {
            ValueType::Unknown => {
                self.expr_types.insert(expr.to_string(), expected);
            }
            _ if current == expected => {}
            _ => {
                self.expr_types.insert(expr.to_string(), ValueType::Unknown);
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
        StackValue::Expr(p)
    }

    fn pop_expr(&mut self, stmts: &mut Vec<StmtAst>, depth: usize) -> String {
        match self.pop_value() {
            StackValue::Expr(v) => v,
            StackValue::Continuation(_) => {
                let name = self.new_temp();
                push_line(
                    stmts,
                    depth,
                    format!("var {name} = 0; ;; continuation used as scalar value"),
                );
                name
            }
        }
    }

    fn pop_expr_expect(
        &mut self,
        stmts: &mut Vec<StmtAst>,
        depth: usize,
        expected: ValueType,
    ) -> String {
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

    pub(crate) fn peek_expr_for_return(&self) -> Option<String> {
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

    pub(crate) fn return_exprs(&self) -> Vec<String> {
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
        self.expr_type_of(param)
    }

    pub(crate) fn seed_stack_with_exprs(&mut self, exprs: &[String]) {
        for expr in exprs {
            let ty = match expr.as_str() {
                "balance" | "msg_value" => ValueType::Int,
                "in_msg_full" => ValueType::Cell,
                "in_msg_body" => ValueType::Slice,
                _ => infer_literal_type(expr),
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
            if let Some(cond_value) = parse_const_int_expr(&cond) {
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
                depth
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
            if let Some(cond_value) = parse_const_int_expr(&cond) {
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
                name: loop_cond.clone(),
                expr: "0".to_string(),
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
                .peek_expr_for_return()
                .unwrap_or_else(|| "0".to_string());
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
                condition: loop_cond.clone(),
                then_body: body_stmts,
                else_body: None,
            });
            stmts.push(StmtAst::DoUntil {
                body: do_body,
                condition: format!("({loop_cond}) == 0"),
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
                .peek_expr_for_return()
                .unwrap_or_else(|| "0".to_string());
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
            let count = plain
                .args
                .first()
                .and_then(arg_as_i64)
                .unwrap_or(0)
                .max(0) as usize;
            for _ in 0..count {
                let _ = state.pop_value();
            }
            return;
        }
        "BLKDROP2" => {
            let i = plain
                .args
                .first()
                .and_then(arg_as_i64)
                .unwrap_or(0)
                .max(0) as usize;
            let j = plain
                .args
                .get(1)
                .and_then(arg_as_i64)
                .unwrap_or(0)
                .max(0) as usize;
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
                .and_then(arg_to_string)
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
                .and_then(arg_to_string)
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
        if let Some(val) = plain.args.first().and_then(arg_to_string) {
            state.push_expr(val);
        } else {
            state.push_expr("0");
        }
        return;
    }

    if plain.name == "PUSHPOW2" {
        let pow = plain
            .args
            .first()
            .and_then(arg_to_string)
            .unwrap_or_else(|| "0".to_string());
        state.push_expr(format!("(1 << {pow})"));
        return;
    }

    if plain.name == "PUSHPOW2DEC" {
        let pow = plain
            .args
            .first()
            .and_then(arg_to_string)
            .unwrap_or_else(|| "0".to_string());
        state.push_expr(format!("((1 << {pow}) - 1)"));
        return;
    }

    if plain.name == "PUSHSLICE" {
        if let Some(arg) = plain.args.first() {
            let slice_value = match arg {
                ArgValue::Cell(cell) => {
                    let literal = super::render::format_cell_literal(cell);
                    if let Some(helper) = ctx.pushslice_helpers.get(&literal) {
                        format!("{helper}()")
                    } else {
                        format_func_slice_expr(cell)
                    }
                }
                _ => arg_to_string(arg).unwrap_or_default(),
            };
            if !slice_value.is_empty() {
                state.push_typed_expr(slice_value, ValueType::Slice);
            }
        }
        return;
    }

    if plain.name == "PUSHNULL" {
        state.push_expr("null()");
        return;
    }

    if plain.name == "SDEQ" {
        let rhs = state.pop_expr_expect(stmts, depth, ValueType::Slice);
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Slice);
        let t = state.new_temp();
        push_line(
            stmts,
            depth,
            format!("var {t} = equal_slices_bits({lhs}, {rhs});"),
        );
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "SBITREFS" {
        let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
        let bits = state.new_temp();
        let refs = state.new_temp();
        push_line(
            stmts,
            depth,
            format!("var ({bits}, {refs}) = slice_bits_refs({src});"),
        );
        state.push_typed_expr(bits, ValueType::Int);
        state.push_typed_expr(refs, ValueType::Int);
        return;
    }

    if let Some(op) = binary_symbol(&plain.name) {
        let rhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        push_line(stmts, depth, format!("var {t} = ({lhs}) {op} ({rhs});"));
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if let Some((op, imm)) = immediate_binary_op(plain) {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        push_line(stmts, depth, format!("var {t} = ({lhs}) {op} ({imm});"));
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "INC" || plain.name == "DEC" {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        let op = if plain.name == "INC" { "+" } else { "-" };
        push_line(stmts, depth, format!("var {t} = ({lhs}) {op} 1;"));
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "NEGATE" {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        push_line(stmts, depth, format!("var {t} = -({lhs});"));
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    if plain.name == "NOT" {
        let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
        let t = state.new_temp();
        push_line(stmts, depth, format!("var {t} = ~({lhs});"));
        state.push_typed_expr(t, ValueType::Int);
        return;
    }

    match plain.name.as_str() {
        "LDU" | "LDUX" => {
            let dynamic_len = plain.name == "LDUX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                plain
                    .args
                    .first()
                    .and_then(arg_to_string)
                    .unwrap_or_else(|| "0".to_string())
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let value = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var ({next_slice}, {value}) = load_uint({src}, {bits});"),
            );
            state.push_typed_expr(value, ValueType::Int);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "LDI" | "LDIX" => {
            let dynamic_len = plain.name == "LDIX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                plain
                    .args
                    .first()
                    .and_then(arg_to_string)
                    .unwrap_or_else(|| "0".to_string())
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let value = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var ({next_slice}, {value}) = load_int({src}, {bits});"),
            );
            state.push_typed_expr(value, ValueType::Int);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "PLDU" | "PLDUX" => {
            let dynamic_len = plain.name == "PLDUX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                plain
                    .args
                    .first()
                    .and_then(arg_to_string)
                    .unwrap_or_else(|| "0".to_string())
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = preload_uint({src}, {bits});"));
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "PLDI" | "PLDIX" => {
            let dynamic_len = plain.name == "PLDIX";
            let bits = if dynamic_len {
                state.pop_expr_expect(stmts, depth, ValueType::Int)
            } else {
                plain
                    .args
                    .first()
                    .and_then(arg_to_string)
                    .unwrap_or_else(|| "0".to_string())
            };
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = preload_int({src}, {bits});"));
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "LDSLICEX" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let bits = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let next_slice = state.new_temp();
            let loaded = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var ({next_slice}, {loaded}) = load_bits({src}, {bits});"),
            );
            state.push_typed_expr(loaded, ValueType::Slice);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "PLDSLICEX" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let bits = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = preload_bits({src}, {bits});"));
            state.push_typed_expr(t, ValueType::Slice);
            return;
        }
        "LDMSGADDR" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let remainder = state.new_temp();
            let addr = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var ({remainder}, {addr}) = load_msg_addr({src});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var ({next_slice}, {value}) = {fn_name}({src});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var ({next_slice}, {value}) = load_maybe_ref({src});"),
            );
            state.push_typed_expr(value, ValueType::Cell);
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "PLDOPTREF" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = preload_maybe_ref({src});"));
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
            push_line(stmts, depth, format!("var {t} = {fn_name}({src});"));
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
            state.push_typed_expr("begin_cell()", ValueType::Builder);
            return;
        }
        "NEWDICT" => {
            state.push_typed_expr("new_dict()", ValueType::Cell);
            return;
        }
        "DIVMOD" => {
            let rhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let q = state.new_temp();
            let r = state.new_temp();
            push_line(stmts, depth, format!("var ({q}, {r}) = divmod({lhs}, {rhs});"));
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
            push_line(
                stmts,
                depth,
                format!("var ({q}, {r}) = muldivmod({x}, {y}, {z});"),
            );
            state.push_typed_expr(q, ValueType::Int);
            state.push_typed_expr(r, ValueType::Int);
            return;
        }
        "MIN" | "MAX" => {
            let rhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let lhs = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            let fn_name = if plain.name == "MIN" { "min" } else { "max" };
            push_line(stmts, depth, format!("var {t} = {fn_name}({lhs}, {rhs});"));
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "ABS" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = abs({src});"));
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
            push_line(stmts, depth, format!("var {t} = {fn_name}({x}, {y}, {z});"));
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "STU" | "STI" => {
            let bits = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let builder = state.pop_expr_expect(stmts, depth, ValueType::Builder);
            let value = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            let fn_name = if plain.name == "STU" {
                "store_uint"
            } else {
                "store_int"
            };
            push_line(
                stmts,
                depth,
                format!("var {t} = {fn_name}({builder}, {value}, {bits});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var {t} = {fn_name}({builder}, {value}, {bits});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var {t} = {fn_name}({builder}, {value});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var {t} = {fn_name}({builder}, {value});"),
            );
            state.push_typed_expr(t, ValueType::Builder);
            return;
        }
        "SDSKIPFIRST" => {
            let len = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = skip_bits({src}, {len});"));
            state.push_typed_expr(t, ValueType::Slice);
            return;
        }
        "INDEXVAR" => {
            let index = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let tuple = state.pop_expr(stmts, depth);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = at({tuple}, {index});"));
            state.push_typed_expr(t, ValueType::Unknown);
            return;
        }
        "SKIPDICT" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = skip_dict({src});"));
            state.push_typed_expr(t, ValueType::Slice);
            return;
        }
        "SKIPOPTREF" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let next_slice = state.new_temp();
            let skipped = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var ({next_slice}, {skipped}) = load_maybe_ref({src});"),
            );
            state.push_typed_expr(next_slice, ValueType::Slice);
            return;
        }
        "ENDS" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            push_line(stmts, depth, format!("end_parse({src});"));
            return;
        }
        "GETGLOB" => {
            let slot = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            state.push_expr(format!("__glob_{slot}"));
            return;
        }
        "SETGLOB" => {
            let value = state.pop_expr(stmts, depth);
            let slot = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            push_line(stmts, depth, format!("__glob_{slot} = {value};"));
            return;
        }
        "CALLDICT" => {
            let target = plain
                .args
                .first()
                .and_then(arg_to_string)
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
            let args_joined = args.join(", ");
            if args_joined.is_empty() {
                push_line(stmts, depth, format!("var {t} = __dict_method_{target}();"));
            } else {
                push_line(
                    stmts,
                    depth,
                    format!("var {t} = __dict_method_{target}({args_joined});"),
                );
            }
            state.push_typed_expr(t, ValueType::Unknown);
            return;
        }
        "CALLXARGS" => {
            let argc = plain
                .args
                .first()
                .and_then(arg_as_i64)
                .unwrap_or(0)
                .max(0) as usize;
            let method_id = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let mut args = Vec::with_capacity(argc);
            for _ in 0..argc {
                args.push(state.pop_expr(stmts, depth));
            }
            args.reverse();

            match argc {
                0 => push_line(stmts, depth, format!("run_method0({method_id});")),
                1 => push_line(stmts, depth, format!("run_method1({method_id}, {});", args[0])),
                2 => push_line(
                    stmts,
                    depth,
                    format!("run_method2({method_id}, {}, {});", args[0], args[1]),
                ),
                3 => push_line(
                    stmts,
                    depth,
                    format!(
                        "run_method3({method_id}, {}, {}, {});",
                        args[0], args[1], args[2]
                    ),
                ),
                _ => push_line(
                    stmts,
                    depth,
                    format!(
                        ";; unsupported CALLXARGS arity {argc} with method id {method_id}"
                    ),
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
            match plain.args.first().and_then(arg_to_string).as_deref() {
                Some("c4") => state.push_typed_expr("get_data()", ValueType::Cell),
                Some("c3") => state.push_typed_expr("get_c3()", ValueType::Unknown),
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
            match plain.args.first().and_then(arg_to_string).as_deref() {
                Some("c4") => {
                    state.refine_expr_type(&value, ValueType::Cell);
                    push_line(stmts, depth, format!("set_data({value});"));
                }
                Some("c3") => push_line(stmts, depth, format!("set_c3({value});")),
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
            state.push_typed_expr("my_address()", ValueType::Slice);
            return;
        }
        "MYCODE" => {
            state.push_typed_expr("my_code()", ValueType::Cell);
            return;
        }
        "DUEPAYMENT" => {
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = my_storage_due();"));
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "ISNULL" => {
            let src = state.pop_expr(stmts, depth);
            let t = state.new_temp();
            push_line(stmts, depth, format!("var {t} = null?({src});"));
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "NOP" => {
            return;
        }
        "DUMP" => {
            let value = state.pop_expr(stmts, depth);
            push_line(stmts, depth, format!("~dump({value});"));
            return;
        }
        "STRDUMP" => {
            let value = state.pop_expr(stmts, depth);
            push_line(stmts, depth, format!("~strdump({value});"));
            return;
        }
        "REWRITESTDADDR" => {
            let src = state.pop_expr_expect(stmts, depth, ValueType::Slice);
            let workchain = state.new_temp();
            let addr = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var ({workchain}, {addr}) = parse_std_addr({src});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var {t} = get_original_fwd_fee({workchain}, {fwd_fee});"),
            );
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETGASFEE" => {
            // stdlib: get_compute_fee(workchain, gas_used) asm(gas_used workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let gas_used = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var {t} = get_compute_fee({workchain}, {gas_used});"),
            );
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
            push_line(
                stmts,
                depth,
                format!("var {t} = get_simple_forward_fee({workchain}, {bits}, {cells});"),
            );
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETFORWARDFEE" => {
            // stdlib: get_forward_fee(workchain, bits, cells) asm(cells bits workchain)
            let workchain = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let bits = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let cells = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let t = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var {t} = get_forward_fee({workchain}, {bits}, {cells});"),
            );
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
            push_line(
                stmts,
                depth,
                format!(
                    "var {t} = get_storage_fee({workchain}, {seconds}, {bits}, {cells});"
                ),
            );
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "GETPRECOMPILEDGAS" => {
            let t = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var {t} = get_precompiled_gas_consumption();"),
            );
            state.push_typed_expr(t, ValueType::Int);
            return;
        }
        "DICTUSETREF" => {
            let key_len = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let dict = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            let index = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let value = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            let t = state.new_temp();
            push_line(
                stmts,
                depth,
                format!("var {t} = udict_set_ref({dict}, {key_len}, {index}, {value});"),
            );
            state.push_typed_expr(t, ValueType::Cell);
            return;
        }
        "THROWIF" | "THROWIFNOT" | "THROWIFNOT_SHORT" => {
            let code = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            if plain.name == "THROWIF" {
                push_line(stmts, depth, format!("throw_if({code}, {cond});"));
            } else {
                push_line(stmts, depth, format!("throw_unless({code}, {cond});"));
            }
            return;
        }
        "THROWARG" => {
            let code = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let arg = state.pop_expr(stmts, depth);
            push_line(stmts, depth, format!("throw_arg({arg}, {code});"));
            return;
        }
        "THROWARGIF" => {
            let code = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let cond = state.pop_expr(stmts, depth);
            let arg = state.pop_expr(stmts, depth);
            push_line(stmts, depth, format!("throw_arg_if({arg}, {code}, {cond});"));
            return;
        }
        "THROWARGIFNOT" => {
            let code = plain
                .args
                .first()
                .and_then(arg_to_string)
                .unwrap_or_else(|| "0".to_string());
            let cond = state.pop_expr(stmts, depth);
            let arg = state.pop_expr(stmts, depth);
            push_line(
                stmts,
                depth,
                format!("throw_arg_unless({arg}, {code}, {cond});"),
            );
            return;
        }
        "THROWANY" => {
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_line(stmts, depth, format!("throw({code});"));
            return;
        }
        "THROWARGANY" => {
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let arg = state.pop_expr(stmts, depth);
            push_line(stmts, depth, format!("throw_arg({arg}, {code});"));
            return;
        }
        "THROWANYIFNOT" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_line(stmts, depth, format!("throw_unless({code}, {cond});"));
            return;
        }
        "THROWARGANYIFNOT" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let arg = state.pop_expr(stmts, depth);
            push_line(
                stmts,
                depth,
                format!("throw_arg_unless({arg}, {code}, {cond});"),
            );
            return;
        }
        "THROWANYIF" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_line(stmts, depth, format!("throw_if({code}, {cond});"));
            return;
        }
        "THROWARGANYIF" => {
            let cond = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let code = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let arg = state.pop_expr(stmts, depth);
            push_line(stmts, depth, format!("throw_arg_if({arg}, {code}, {cond});"));
            return;
        }
        "RAWRESERVE" => {
            let mode = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let amount = state.pop_expr_expect(stmts, depth, ValueType::Int);
            push_line(stmts, depth, format!("raw_reserve({amount}, {mode});"));
            return;
        }
        "SENDRAWMSG" => {
            let mode = state.pop_expr_expect(stmts, depth, ValueType::Int);
            let msg = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            push_line(stmts, depth, format!("send_raw_message({msg}, {mode});"));
            return;
        }
        "SETCODE" => {
            let code = state.pop_expr_expect(stmts, depth, ValueType::Cell);
            push_line(stmts, depth, format!("set_code({code});"));
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
    depth: usize,
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
            state.push_typed_expr(then_expr.clone(), ty);
            continue;
        }

        let merged = state.new_temp();
        push_line(pre_if_stmts, depth, format!("var {merged} = {else_expr};"));
        push_line(then_stmts, depth + 1, format!("{merged} = {then_expr};"));
        let then_ty = then_state.expr_type_of(then_expr);
        let else_ty = else_state.expr_type_of(else_expr);
        let merged_ty = merged_value_type(then_expr, then_ty, else_expr, else_ty);
        state.push_typed_expr(merged, merged_ty);
    }

    true
}

fn merge_ifelse_stacks(
    cond: &str,
    then_state: &LiftState,
    else_state: &LiftState,
    state: &mut LiftState,
    pre_if_stmts: &mut Vec<StmtAst>,
    then_stmts: &mut Vec<StmtAst>,
    else_stmts: &mut Vec<StmtAst>,
    depth: usize,
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
            state.push_typed_expr(then_expr.clone(), ty);
            continue;
        }

        let merged = state.new_temp();
        let then_ty = then_state.expr_type_of(then_expr);
        let else_ty = else_state.expr_type_of(else_expr);
        let merged_ty = merged_value_type(then_expr, then_ty, else_expr, else_ty);
        if !is_branch_local_temp_expr(then_expr, base_next_temp)
            && !is_branch_local_temp_expr(else_expr, base_next_temp)
        {
            push_line(
                pre_if_stmts,
                depth,
                format!("var {merged} = ({cond}) ? ({then_expr}) : ({else_expr});"),
            );
        } else {
            let init_expr = if merged_ty == ValueType::Int { "0" } else { "null()" };
            push_line(pre_if_stmts, depth, format!("var {merged} = {init_expr};"));
            push_line(then_stmts, depth + 1, format!("{merged} = {then_expr};"));
            push_line(else_stmts, depth + 1, format!("{merged} = {else_expr};"));
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
    if trimmed.starts_with(";;") {
        stmts.push(StmtAst::Comment(trimmed.to_string()));
    } else {
        stmts.push(StmtAst::Expr(trimmed.to_string()));
    }
}

fn is_branch_local_temp_expr(expr: &str, base_next_temp: usize) -> bool {
    expr.strip_prefix('v')
        .and_then(|s| s.parse::<usize>().ok())
        .is_some_and(|idx| idx >= base_next_temp)
}

fn merged_value_type(
    then_expr: &str,
    then_ty: ValueType,
    else_expr: &str,
    else_ty: ValueType,
) -> ValueType {
    if then_ty == else_ty {
        return then_ty;
    }
    if then_expr == "null()" {
        return else_ty;
    }
    if else_expr == "null()" {
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
    let imm = plain.args.first().and_then(arg_to_string)?;
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

fn infer_literal_type(expr: &str) -> ValueType {
    match expr {
        "begin_cell()" => ValueType::Builder,
        "get_data()" | "my_code()" => ValueType::Cell,
        "my_address()" => ValueType::Slice,
        _ if expr.starts_with("x{") => ValueType::Slice,
        _ if expr.parse::<i128>().is_ok() => ValueType::Int,
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
        _ => arg_to_string(arg).and_then(|s| parse_stack_position(&s)),
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
    StackValue::Expr(p)
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
