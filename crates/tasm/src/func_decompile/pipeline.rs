use super::ast::{ExprAst, MethodAst, StmtAst, Var, render_method_ast};
use super::method_model::{
    MethodKind, ReturnKind, build_method_signature_ast, classify_method, collect_call_targets,
    extract_method_dictionary, infer_params_for_method, infer_return_kind, render_method_signature,
};
use super::render::format_cell_literal;
use super::stage_patterns::{MethodPatterns, apply_pattern_rewrites};
use super::stage_rename::improve_variable_names;
use super::stage_simplify::simplify_method_body;
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
    fn walk(
        instructions: &[Instruction],
        out: &mut BTreeMap<String, String>,
        next_idx: &mut usize,
    ) {
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

fn initial_stack_params_for_method(kind: MethodKind, params: &[String]) -> Vec<ExprAst> {
    if kind == MethodKind::RecvInternal {
        return vec![
            ExprAst::Ident("balance".to_string()),
            ExprAst::Ident("msg_value".to_string()),
            ExprAst::Ident("in_msg_full".to_string()),
            ExprAst::Ident("in_msg_body".to_string()),
        ];
    }
    params
        .iter()
        .map(|name| ExprAst::Ident(name.clone()))
        .collect()
}

fn select_recv_internal_params(stmts: &[StmtAst]) -> Vec<String> {
    if stmts
        .iter()
        .any(|stmt| stmt_contains_ident(stmt, "balance"))
    {
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
        StmtAst::Comment(_) => false,
        StmtAst::VarDecl { binding, expr } => {
            tensor_contains_ident(binding, ident) || expr_contains_ident(expr, ident)
        }
        StmtAst::Assign { target, expr } => target == ident || expr_contains_ident(expr, ident),
        StmtAst::Return(Some(expr)) => expr_contains_ident(expr, ident),
        StmtAst::Return(None) => false,
        StmtAst::Call { callee, args } => {
            callee == ident || args.iter().any(|a| expr_contains_ident(a, ident))
        }
        StmtAst::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            expr_contains_ident(condition, ident)
                || stmt_list_contains_ident(then_body, ident)
                || else_body
                    .as_ref()
                    .is_some_and(|body| stmt_list_contains_ident(body, ident))
        }
        StmtAst::Repeat { count, body } => {
            expr_contains_ident(count, ident) || stmt_list_contains_ident(body, ident)
        }
        StmtAst::DoUntil { body, condition } => {
            expr_contains_ident(condition, ident) || stmt_list_contains_ident(body, ident)
        }
    }
}

fn stmt_list_contains_ident(stmts: &[StmtAst], ident: &str) -> bool {
    stmts.iter().any(|stmt| stmt_contains_ident(stmt, ident))
}

fn expr_contains_ident(expr: &ExprAst, ident: &str) -> bool {
    match expr {
        ExprAst::Ident(text) => text == ident,
        ExprAst::Number(_) => false,
        ExprAst::StringLiteral(_) => false,
        ExprAst::CellLiteral(_) => false,
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
            callee == ident || args.iter().any(|arg| expr_contains_ident(arg, ident))
        }
        ExprAst::MethodCall { receiver, args, .. } => {
            expr_contains_ident(receiver, ident)
                || args.iter().any(|arg| expr_contains_ident(arg, ident))
        }
    }
}

fn tensor_contains_ident(tensor: &Var, ident: &str) -> bool {
    match tensor {
        Var::Name(name) => name == ident,
        Var::Tensor(items) => items.iter().any(|item| tensor_contains_ident(item, ident)),
    }
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
        let _ = options;
        Self {
            disassembler: Disassembler::new(),
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
        let _ = writeln!(out, "#include \"{}\";", stdlib_include_path());
        out.push_str("#pragma version >=0.4.0;\n\n");

        let Some(dict) = extract_method_dictionary(code) else {
            let signature = super::ast::MethodSignatureAst {
                return_type: "()".to_string(),
                name: "decompiled_entry".to_string(),
                params: Vec::new(),
                qualifiers: vec!["impure".to_string()],
            };
            let leading_comments = Vec::new();
            let mut lift = LiftResult::default();
            let mut state = LiftState::default();
            let lift_ctx = LiftContext::default();
            lift_instructions(
                &code.instructions,
                &mut state,
                &mut lift.stmts,
                1,
                &lift_ctx,
            );
            let mut body = lift.stmts;
            simplify_method_body(&mut body);
            improve_variable_names(&mut body);
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
            let signature =
                build_method_signature_ast(method, kind, &params, &param_types, &return_kind);
            let leading_comments = Vec::new();

            let mut rewritten = stmts;
            apply_pattern_rewrites(&mut rewritten, &patterns);
            let mut body = rewritten;

            if kind != MethodKind::RecvInternal && !state.has_explicit_return() {
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
            simplify_method_body(&mut body);
            improve_variable_names(&mut body);

            let method_ast = MethodAst {
                signature,
                leading_comments,
                body,
            };
            render_method_ast(&method_ast, &mut out);
        }

        out
    }
}
