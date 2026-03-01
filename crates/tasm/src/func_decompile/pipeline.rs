use super::ast::{ExprAst, MethodAst, StmtAst, Var, render_method_ast};
use super::method_model::{
    MethodKind, ReturnKind, build_method_signature_ast, classify_method, collect_call_targets,
    extract_method_dictionary, infer_params_for_method, infer_return_kind, render_method_signature,
};
use super::render::{format_cell_literal, format_instruction_line};
use super::stage_patterns::{MethodPatterns, apply_pattern_rewrites};
use super::stage_stack::{LiftContext, LiftResult, LiftState, lift_instructions};
use crate::decompile::Disassembler;
use crate::types::{ArgValue, Code, Instruction, Method};
use anyhow::{Context, anyhow};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;

#[derive(Debug, Clone)]
pub struct FuncDecompilerOptions {
    pub include_raw_tasm_fallback: bool,
    pub max_raw_tasm_lines_per_method: usize,
}

fn stdlib_include_path() -> String {
    "stdlib.fc".to_string()
}

fn collect_pushslice_helpers(methods: &[Method]) -> BTreeMap<String, String> {
    fn walk(instructions: &[Instruction], out: &mut BTreeMap<String, String>, next_idx: &mut usize) {
        for instruction in instructions {
            match instruction {
                Instruction::Plain(plain) => {
                    if plain.name == "PUSHSLICE"
                        && let Some(ArgValue::Cell(cell)) = plain.args.first()
                        && cell.as_slice_allow_exotic().size_refs() == 0
                    {
                        let literal = format_cell_literal(cell);
                        out.entry(literal).or_insert_with(|| {
                            let name = format!("__tasm_slice_lit_{}", *next_idx);
                            *next_idx += 1;
                            name
                        });
                    }
                }
                Instruction::Ref(reference) => {
                    if let ArgValue::Code { code, .. } = &reference.code {
                        walk(&code.instructions, out, next_idx);
                    }
                }
                Instruction::ExoticCell(_) => {}
            }
        }
    }

    let mut out = BTreeMap::new();
    let mut next_idx = 0usize;
    for method in methods {
        walk(&method.instructions, &mut out, &mut next_idx);
    }
    out
}

fn initial_stack_params_for_method(kind: MethodKind, params: &[String]) -> Vec<String> {
    if kind == MethodKind::RecvInternal {
        return vec![
            "balance".to_string(),
            "msg_value".to_string(),
            "in_msg_full".to_string(),
            "in_msg_body".to_string(),
        ];
    }
    params.to_vec()
}

fn select_recv_internal_params(stmts: &[StmtAst]) -> Vec<String> {
    if stmts.iter().any(|stmt| stmt_contains_ident(stmt, "balance")) {
        vec![
            "balance".to_string(),
            "msg_value".to_string(),
            "in_msg_full".to_string(),
            "in_msg_body".to_string(),
        ]
    } else {
        vec![
            "msg_value".to_string(),
            "in_msg_full".to_string(),
            "in_msg_body".to_string(),
        ]
    }
}

fn stmt_contains_ident(stmt: &StmtAst, ident: &str) -> bool {
    match stmt {
        StmtAst::Comment(line) | StmtAst::Expr(line) => contains_ident_text(line, ident),
        StmtAst::VarDecl { binding, expr } => {
            tensor_contains_ident(binding, ident) || expr_contains_ident(expr, ident)
        }
        StmtAst::Assign { target, expr } => {
            contains_ident_text(target, ident) || expr_contains_ident(expr, ident)
        }
        StmtAst::Return(Some(expr)) => expr_contains_ident(expr, ident),
        StmtAst::Return(None) => false,
        StmtAst::Call { callee, args } => {
            contains_ident_text(callee, ident) || args.iter().any(|a| expr_contains_ident(a, ident))
        }
        StmtAst::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            expr_contains_ident(condition, ident)
                || then_body.iter().any(|s| stmt_contains_ident(s, ident))
                || else_body
                    .as_ref()
                    .is_some_and(|body| body.iter().any(|s| stmt_contains_ident(s, ident)))
        }
        StmtAst::Repeat { count, body } => {
            expr_contains_ident(count, ident) || body.iter().any(|s| stmt_contains_ident(s, ident))
        }
        StmtAst::DoUntil { body, condition } => {
            expr_contains_ident(condition, ident)
                || body.iter().any(|s| stmt_contains_ident(s, ident))
        }
    }
}

fn expr_contains_ident(expr: &ExprAst, ident: &str) -> bool {
    match expr {
        ExprAst::Atom(text) => contains_ident_text(text, ident),
        ExprAst::Number(_) => false,
        ExprAst::NullLiteral => false,
        ExprAst::Unary { expr, .. } => expr_contains_ident(expr, ident),
        ExprAst::Binary { lhs, rhs, .. } => {
            expr_contains_ident(lhs, ident) || expr_contains_ident(rhs, ident)
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            expr_contains_ident(condition, ident)
                || expr_contains_ident(then_expr, ident)
                || expr_contains_ident(else_expr, ident)
        }
        ExprAst::Tuple(items) => items.iter().any(|item| expr_contains_ident(item, ident)),
        ExprAst::Call { callee, args } => {
            contains_ident_text(callee, ident)
                || args.iter().any(|arg| expr_contains_ident(arg, ident))
        }
    }
}

fn tensor_contains_ident(tensor: &Var, ident: &str) -> bool {
    match tensor {
        Var::Name(name) => name == ident || contains_ident_text(name, ident),
        Var::Tensor(items) => items.iter().any(|item| tensor_contains_ident(item, ident)),
    }
}

fn contains_ident_text(text: &str, ident: &str) -> bool {
    let bytes = text.as_bytes();
    let needle = ident.as_bytes();
    if needle.is_empty() || bytes.len() < needle.len() {
        return false;
    }

    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if &bytes[i..i + needle.len()] == needle {
            let left_ok = i == 0 || !is_ident_char(bytes[i - 1]);
            let right_idx = i + needle.len();
            let right_ok = right_idx == bytes.len() || !is_ident_char(bytes[right_idx]);
            if left_ok && right_ok {
                return true;
            }
            i = right_idx;
        } else {
            i += 1;
        }
    }
    false
}

fn is_ident_char(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

impl Default for FuncDecompilerOptions {
    fn default() -> Self {
        Self {
            include_raw_tasm_fallback: true,
            max_raw_tasm_lines_per_method: 256,
        }
    }
}

#[derive(Debug)]
pub struct FuncDecompiler {
    disassembler: Disassembler,
    options: FuncDecompilerOptions,
}

impl Default for FuncDecompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl FuncDecompiler {
    #[must_use]
    pub fn new() -> Self {
        Self::with_options(FuncDecompilerOptions::default())
    }

    #[must_use]
    pub fn with_options(options: FuncDecompilerOptions) -> Self {
        Self {
            disassembler: Disassembler::new(),
            options,
        }
    }

    pub fn decompile_cell(&self, cell: &Cell) -> anyhow::Result<String> {
        let code = self.disassembler.decompile_cell(cell)?;
        Ok(self.lift_code(&code))
    }

    pub fn decompile_boc(&self, boc: impl AsRef<[u8]>) -> anyhow::Result<String> {
        let cell = Boc::decode(boc.as_ref()).context("Failed to decode BoC bytes")?;
        self.decompile_cell(&cell)
    }

    pub fn decompile_boc_hex(&self, boc_hex: &str) -> anyhow::Result<String> {
        let cell = Boc::decode_hex(boc_hex.trim()).context("Failed to decode hex BoC string")?;
        self.decompile_cell(&cell)
    }

    pub fn decompile_boc_string(&self, boc_data: &str) -> anyhow::Result<String> {
        let data = boc_data.trim();
        if let Ok(cell) = Boc::decode_hex(data) {
            return self.decompile_cell(&cell);
        }
        if let Ok(cell) = Boc::decode_base64(data) {
            return self.decompile_cell(&cell);
        }
        Err(anyhow!("Failed to decode BoC string as hex or base64"))
    }

    #[must_use]
    pub fn lift_code(&self, code: &Code) -> String {
        let mut out = String::new();
        out.push_str(";; Auto-generated by tasm::func_decompile\n");
        out.push_str(";; Pipeline: stack-lift -> structured-flow -> pattern-rewrite -> fallback\n");
        let _ = writeln!(out, "#include \"{}\";", stdlib_include_path());
        out.push_str("#pragma version >=0.4.0;\n\n");

        let Some(dict) = extract_method_dictionary(code) else {
            let signature = super::ast::MethodSignatureAst {
                return_type: "()".to_string(),
                name: "decompiled_entry".to_string(),
                params: Vec::new(),
                qualifiers: vec!["impure".to_string()],
            };
            let leading_comments =
                vec![";; No DICTPUSHCONST method table found; falling back to linear block"
                    .to_string()];
            let mut lift = LiftResult::default();
            let mut state = LiftState::default();
            let lift_ctx = LiftContext::default();
            lift_instructions(&code.instructions, &mut state, &mut lift.stmts, 1, &lift_ctx);
            let mut body = lift.stmts;
            if self.options.include_raw_tasm_fallback {
                body.extend(self.collect_raw_fallback_stmts(&code.instructions));
            }
            let method_ast = MethodAst {
                signature,
                leading_comments,
                body,
            };
            render_method_ast(&method_ast, &mut out);
            return out;
        };

        let mut methods = dict.methods.clone();
        methods.sort_by_key(|m| m.id);
        let pushslice_helpers = collect_pushslice_helpers(&methods);
        let called_targets = collect_call_targets(&methods);
        let empty_ctx = LiftContext::default();
        let mut calldict_arity = BTreeMap::new();
        for method in &methods {
            let patterns = MethodPatterns::analyze(method);
            let kind = classify_method(method, &called_targets, &patterns);
            let mut state = LiftState::default();
            let mut stmts = Vec::new();
            lift_instructions(&method.instructions, &mut state, &mut stmts, 1, &empty_ctx);
            let params = infer_params_for_method(kind, &state);
            calldict_arity.insert(method.id, params.len());
        }
        let lift_ctx = LiftContext {
            calldict_arity,
            ifjmp_unit_return: false,
            pushslice_helpers: pushslice_helpers.clone(),
        };

        let _ = writeln!(out, ";; recovered_methods: {}", methods.len());
        if !called_targets.is_empty() {
            let joined = called_targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, ";; call_dict_targets: {joined}");
        }
        out.push('\n');

        if !pushslice_helpers.is_empty() {
            for (literal, name) in &pushslice_helpers {
                let _ = writeln!(out, "slice {name}() asm \"{literal} PUSHSLICE\";");
            }
            out.push('\n');
        }

        let mut helper_decls = Vec::new();
        for method in &methods {
            let patterns = MethodPatterns::analyze(method);
            let kind = classify_method(method, &called_targets, &patterns);
            if kind != MethodKind::Helper {
                continue;
            }

            let method_lift_ctx = LiftContext {
                calldict_arity: lift_ctx.calldict_arity.clone(),
                ifjmp_unit_return: false,
                pushslice_helpers: lift_ctx.pushslice_helpers.clone(),
            };

            let mut infer_state = LiftState::default();
            let mut infer_stmts = Vec::new();
            lift_instructions(
                &method.instructions,
                &mut infer_state,
                &mut infer_stmts,
                1,
                &method_lift_ctx,
            );
            let params = infer_params_for_method(kind, &infer_state);

            let mut state = LiftState::default();
            let initial_stack = initial_stack_params_for_method(kind, &params);
            state.seed_stack_with_exprs(&initial_stack);
            let mut stmts = Vec::new();
            lift_instructions(
                &method.instructions,
                &mut state,
                &mut stmts,
                1,
                &method_lift_ctx,
            );

            let return_kind = infer_return_kind(kind, &state);
            let param_types = params
                .iter()
                .map(|name| state.param_type(name))
                .collect::<Vec<_>>();
            let sig = render_method_signature(method, kind, &params, &param_types, &return_kind);
            helper_decls.push(format!(
                "{};",
                sig.strip_suffix(" {").unwrap_or(sig.as_str())
            ));
        }
        if !helper_decls.is_empty() {
            for decl in &helper_decls {
                let _ = writeln!(out, "{decl}");
            }
            out.push('\n');
        }

        for method in &methods {
            let patterns = MethodPatterns::analyze(method);
            let kind = classify_method(method, &called_targets, &patterns);

            let mut infer_state = LiftState::default();
            let mut infer_stmts = Vec::new();
            let method_lift_ctx = LiftContext {
                calldict_arity: lift_ctx.calldict_arity.clone(),
                ifjmp_unit_return: kind == MethodKind::RecvInternal,
                pushslice_helpers: lift_ctx.pushslice_helpers.clone(),
            };

            lift_instructions(
                &method.instructions,
                &mut infer_state,
                &mut infer_stmts,
                1,
                &method_lift_ctx,
            );
            let mut params = infer_params_for_method(kind, &infer_state);

            let mut state = LiftState::default();
            let initial_stack = initial_stack_params_for_method(kind, &params);
            state.seed_stack_with_exprs(&initial_stack);
            let mut stmts = Vec::new();
            lift_instructions(
                &method.instructions,
                &mut state,
                &mut stmts,
                1,
                &method_lift_ctx,
            );

            if kind == MethodKind::RecvInternal {
                params = select_recv_internal_params(&stmts);
            }

            let return_kind = infer_return_kind(kind, &state);
            let param_types = params
                .iter()
                .map(|name| state.param_type(name))
                .collect::<Vec<_>>();
            let signature = build_method_signature_ast(
                method,
                kind,
                &params,
                &param_types,
                &return_kind,
            );
            let leading_comments = self.collect_pattern_comments(method, &patterns, kind, &params);

            let mut rewritten = stmts;
            apply_pattern_rewrites(&mut rewritten, &patterns);
            let mut body = rewritten;

            if kind != MethodKind::RecvInternal
                && !state.has_explicit_return()
            {
                let return_values = state.return_expr_asts();
                match return_values.len() {
                    0 => {}
                    1 => {
                        if let Some(ret) = return_values.first() {
                            match return_kind {
                                ReturnKind::Tuple(_) => {
                                    body.push(StmtAst::Return(Some(ExprAst::Tuple(vec![
                                        ret.clone(),
                                    ]))));
                                }
                                ReturnKind::Unit => {}
                                _ => {
                                    body.push(StmtAst::Return(Some(ret.clone())));
                                }
                            }
                        }
                    }
                    _ => {
                        body.push(StmtAst::Return(Some(ExprAst::Tuple(return_values))));
                    }
                }
            }

            if self.options.include_raw_tasm_fallback {
                body.extend(self.collect_raw_fallback_stmts(&method.instructions));
            }

            let method_ast = MethodAst {
                signature,
                leading_comments,
                body,
            };
            render_method_ast(&method_ast, &mut out);
        }

        out
    }

    fn collect_pattern_comments(
        &self,
        method: &Method,
        patterns: &MethodPatterns,
        kind: MethodKind,
        params: &[String],
    ) -> Vec<String> {
        let mut comments = Vec::new();
        comments.push(format!(";; dict_method_id: {}", method.id));

        if kind == MethodKind::RecvInternal {
            comments.push(";; recovered role: recv_internal handler".to_string());
        } else if kind == MethodKind::Getter {
            comments.push(";; recovered role: get-method".to_string());
        } else if kind == MethodKind::Helper {
            comments.push(";; recovered role: helper (called via CALLDICT)".to_string());
        }

        if !params.is_empty() {
            comments.push(format!(";; inferred params: {}", params.join(", ")));
        }

        if patterns.has_bounce_guard {
            comments.push(";; pattern: bounced message guard (msg_flags & 1)".to_string());
        }
        if patterns.has_empty_body_guard {
            comments.push(";; pattern: empty body early return".to_string());
        }
        if patterns.has_op_and_query_id {
            comments.push(";; pattern: body starts with op:uint32 and query_id:uint64".to_string());
        }

        if !patterns.opcodes.is_empty() {
            comments.push(";; recovered opcode dispatch candidates:".to_string());
            for opcode in &patterns.opcodes {
                comments.push(format!(";;   if (op == 0x{opcode:08x}) {{ ... }}"));
            }
        }

        if let Some(layout) = &patterns.storage_load_layout {
            comments.push(format!(";; storage load layout: {}", layout.join(" -> ")));
        }
        if let Some(layout) = &patterns.storage_save_layout {
            comments.push(format!(";; storage save layout: {}", layout.join(" -> ")));
        }

        if !patterns.call_targets.is_empty() {
            let joined = patterns
                .call_targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            comments.push(format!(";; calls helpers: CALLDICT {joined}"));
        }
        if !patterns.getglob_slots.is_empty() {
            let joined = patterns
                .getglob_slots
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            comments.push(format!(";; reads globals: {joined}"));
        }
        if !patterns.setglob_slots.is_empty() {
            let joined = patterns
                .setglob_slots
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            comments.push(format!(";; writes globals: {joined}"));
        }
        if patterns.has_raw_reserve {
            comments.push(";; pattern: RAWRESERVE present".to_string());
        }
        if patterns.has_send_raw_msg {
            comments.push(";; pattern: SENDRAWMSG present".to_string());
        }
        if patterns.has_set_code {
            comments.push(";; pattern: SETCODE present (upgrade branch)".to_string());
        }
        if !patterns.throw_codes.is_empty() {
            let joined = patterns
                .throw_codes
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            comments.push(format!(";; throw codes: {joined}"));
        }
        comments
    }

    fn collect_raw_fallback_stmts(&self, instructions: &[Instruction]) -> Vec<StmtAst> {
        let mut out = Vec::new();
        out.push(StmtAst::comment(";; low-level fallback (TASM)"));

        let max = if self.options.max_raw_tasm_lines_per_method == 0 {
            usize::MAX
        } else {
            self.options.max_raw_tasm_lines_per_method
        };

        for (idx, instruction) in instructions.iter().enumerate() {
            if idx >= max {
                out.push(StmtAst::comment(format!(
                    ";; ... truncated {} instructions",
                    instructions.len().saturating_sub(idx)
                )));
                break;
            }
            out.push(StmtAst::comment(format!(
                ";; {}",
                format_instruction_line(instruction, 1)
            )));
        }
        out
    }
}
