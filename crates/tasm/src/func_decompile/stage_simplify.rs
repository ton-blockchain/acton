use super::ast::{ExprAst, StmtAst, UnaryOp, Var};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone)]
struct UseOccurrence {
    stmt_idx: usize,
    ident: String,
}

#[derive(Debug, Default)]
struct BlockDefUseIndex {
    uses: Vec<UseOccurrence>,
    def_ident_to_uses: BTreeMap<(usize, String), Vec<usize>>,
    use_to_def: Vec<Option<usize>>,
}

pub(crate) fn simplify_method_body(stmts: &mut Vec<StmtAst>) {
    simplify_stmt_list(stmts);
}

fn simplify_stmt_list(stmts: &mut Vec<StmtAst>) {
    for stmt in stmts.iter_mut() {
        simplify_stmt(stmt);
    }

    loop {
        let index = build_block_def_use_index(stmts);
        if rewrite_tuple_modifying_chain_once(stmts, &index) {
            continue;
        }
        if inline_condition_temp_once(stmts, &index) {
            continue;
        }
        break;
    }

    rewrite_store_calls_stmt_list(stmts);
    collapse_store_chain_stmt_list(stmts);

    loop {
        let index = build_block_def_use_index(stmts);
        if rewrite_tuple_modifying_chain_once(stmts, &index) {
            continue;
        }
        if !inline_single_use_value_once(stmts, &index) {
            break;
        }
    }
}

fn inline_condition_temp_once(stmts: &mut Vec<StmtAst>, index: &BlockDefUseIndex) -> bool {
    for def_idx in 0..stmts.len().saturating_sub(1) {
        let (var_name, init_expr) = match &stmts[def_idx] {
            StmtAst::VarDecl {
                binding: Var::Name(name),
                expr,
            } => (name.clone(), expr.clone()),
            _ => continue,
        };

        if !has_single_immediate_use(index, def_idx, &var_name) {
            continue;
        }

        let next_stmt = match stmts.get_mut(def_idx + 1) {
            Some(stmt) => stmt,
            None => continue,
        };
        if !rewrite_next_stmt_condition_site(next_stmt, &var_name, &init_expr) {
            continue;
        }

        stmts.remove(def_idx);
        return true;
    }

    false
}

fn inline_single_use_value_once(stmts: &mut Vec<StmtAst>, index: &BlockDefUseIndex) -> bool {
    for def_idx in 0..stmts.len().saturating_sub(1) {
        let (var_name, init_expr) = match &stmts[def_idx] {
            StmtAst::VarDecl {
                binding: Var::Name(name),
                expr,
            } => (name.clone(), expr.clone()),
            _ => continue,
        };

        if !has_single_immediate_use(index, def_idx, &var_name) {
            continue;
        }

        let next_stmt = match stmts.get_mut(def_idx + 1) {
            Some(stmt) => stmt,
            None => continue,
        };
        if !rewrite_next_stmt_value_site(next_stmt, &var_name, &init_expr) {
            continue;
        }

        stmts.remove(def_idx);
        return true;
    }

    false
}

fn rewrite_next_stmt_value_site(stmt: &mut StmtAst, ident: &str, replacement: &ExprAst) -> bool {
    let mut replaced = 0_usize;
    match stmt {
        StmtAst::VarDecl { expr, .. }
        | StmtAst::Assign { expr, .. }
        | StmtAst::Return(Some(expr)) => {
            replace_ident_in_expr(expr, ident, replacement, &mut replaced);
        }
        StmtAst::Call { args, .. } => {
            for arg in args {
                replace_ident_in_expr(arg, ident, replacement, &mut replaced);
            }
        }
        StmtAst::If { condition, .. } => {
            replace_ident_in_expr(condition, ident, replacement, &mut replaced);
        }
        StmtAst::Repeat { count, .. } => {
            replace_ident_in_expr(count, ident, replacement, &mut replaced);
        }
        StmtAst::DoUntil { condition, .. } => {
            replace_ident_in_expr(condition, ident, replacement, &mut replaced);
        }
        StmtAst::Comment(_) | StmtAst::Return(None) => {}
    }
    replaced == 1
}

fn rewrite_tuple_modifying_chain_once(stmts: &mut [StmtAst], index: &BlockDefUseIndex) -> bool {
    for def_idx in 0..stmts.len().saturating_sub(1) {
        let Some((next_name, value_name, method, receiver_name, method_args)) =
            extract_tuple_call_parts(&stmts[def_idx])
        else {
            continue;
        };

        if !is_temp_ident(&receiver_name) || !has_single_immediate_use(index, def_idx, &next_name) {
            continue;
        }

        let next_stmt = match stmts.get_mut(def_idx + 1) {
            Some(stmt) => stmt,
            None => continue,
        };
        if !replace_first_call_input_in_stmt(
            next_stmt,
            &next_name,
            ExprAst::Ident(receiver_name.clone()),
        ) {
            continue;
        }

        stmts[def_idx] = StmtAst::VarDecl {
            binding: Var::name(value_name),
            expr: ExprAst::MethodCall {
                receiver: Box::new(ExprAst::Ident(receiver_name)),
                method,
                modifying: true,
                args: method_args,
            },
        };
        return true;
    }

    false
}

fn extract_tuple_call_parts(
    stmt: &StmtAst,
) -> Option<(String, String, String, String, Vec<ExprAst>)> {
    let StmtAst::VarDecl { binding, expr } = stmt else {
        return None;
    };
    let (next_name, value_name) = two_name_tensor(binding)?;

    match expr {
        ExprAst::Call { callee, args } => {
            let (receiver_name, method_args) = split_receiver_ident(args)?;
            Some((
                next_name,
                value_name,
                callee.clone(),
                receiver_name,
                method_args,
            ))
        }
        ExprAst::MethodCall {
            receiver,
            method,
            modifying,
            args,
        } if !*modifying => {
            let ExprAst::Ident(receiver_name) = receiver.as_ref() else {
                return None;
            };
            Some((
                next_name,
                value_name,
                method.clone(),
                receiver_name.clone(),
                args.clone(),
            ))
        }
        _ => None,
    }
}

fn split_receiver_ident(args: &[ExprAst]) -> Option<(String, Vec<ExprAst>)> {
    let receiver = args.first()?;
    let ExprAst::Ident(receiver_name) = receiver else {
        return None;
    };
    Some((
        receiver_name.clone(),
        args.iter().skip(1).cloned().collect(),
    ))
}

fn two_name_tensor(binding: &Var) -> Option<(String, String)> {
    let Var::Tensor(items) = binding else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    match (&items[0], &items[1]) {
        (Var::Name(a), Var::Name(b)) => Some((a.clone(), b.clone())),
        _ => None,
    }
}

fn is_temp_ident(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

fn has_single_immediate_use(index: &BlockDefUseIndex, def_idx: usize, ident: &str) -> bool {
    let use_ids = match index.def_ident_to_uses.get(&(def_idx, ident.to_string())) {
        Some(ids) if ids.len() == 1 => ids,
        _ => return false,
    };
    let use_id = use_ids[0];
    let use_occurrence = match index.uses.get(use_id) {
        Some(occ) => occ,
        None => return false,
    };

    use_occurrence.stmt_idx == def_idx + 1
        && use_occurrence.ident == ident
        && index.use_to_def.get(use_id).copied().flatten() == Some(def_idx)
}

fn condition_inline_replacement(
    condition: &ExprAst,
    var_name: &str,
    init_expr: &ExprAst,
) -> Option<ExprAst> {
    match condition {
        ExprAst::Ident(name) if name == var_name => Some(init_expr.clone()),
        ExprAst::Unary {
            op: UnaryOp::BitNot,
            expr,
        } => match expr.as_ref() {
            ExprAst::Ident(name) if name == var_name => Some(ExprAst::Unary {
                op: UnaryOp::BitNot,
                expr: Box::new(init_expr.clone()),
            }),
            _ => None,
        },
        _ => None,
    }
}

fn rewrite_next_stmt_condition_site(
    stmt: &mut StmtAst,
    var_name: &str,
    init_expr: &ExprAst,
) -> bool {
    match stmt {
        StmtAst::If { condition, .. } => {
            if let Some(replacement) = condition_inline_replacement(condition, var_name, init_expr)
            {
                *condition = replacement;
                return true;
            }
            false
        }
        StmtAst::VarDecl {
            expr: ExprAst::Ternary { condition, .. },
            ..
        }
        | StmtAst::Assign {
            expr: ExprAst::Ternary { condition, .. },
            ..
        } => {
            if let Some(replacement) = condition_inline_replacement(condition, var_name, init_expr)
            {
                *condition = Box::new(replacement);
                return true;
            }
            false
        }
        _ => false,
    }
}

fn replace_first_call_input_in_stmt(stmt: &mut StmtAst, ident: &str, replacement: ExprAst) -> bool {
    match stmt {
        StmtAst::VarDecl { expr, .. }
        | StmtAst::Assign { expr, .. }
        | StmtAst::Return(Some(expr)) => replace_first_call_input_in_expr(expr, ident, replacement),
        StmtAst::Call { args, .. } => replace_ident_in_first_arg(args, ident, replacement),
        StmtAst::Comment(_)
        | StmtAst::Return(None)
        | StmtAst::If { .. }
        | StmtAst::Repeat { .. }
        | StmtAst::DoUntil { .. } => false,
    }
}

fn replace_first_call_input_in_expr(expr: &mut ExprAst, ident: &str, replacement: ExprAst) -> bool {
    match expr {
        ExprAst::Call { args, .. } => replace_ident_in_first_arg(args, ident, replacement),
        ExprAst::MethodCall { receiver, .. } => {
            if matches!(receiver.as_ref(), ExprAst::Ident(name) if name == ident) {
                *receiver = Box::new(replacement);
                return true;
            }
            false
        }
        _ => false,
    }
}

fn replace_ident_in_first_arg(args: &mut [ExprAst], ident: &str, replacement: ExprAst) -> bool {
    let Some(first) = args.first_mut() else {
        return false;
    };
    if matches!(first, ExprAst::Ident(name) if name == ident) {
        *first = replacement;
        return true;
    }
    false
}

fn rewrite_store_calls_stmt_list(stmts: &mut [StmtAst]) {
    for stmt in stmts {
        rewrite_store_calls_stmt(stmt);
    }
}

fn collapse_store_chain_stmt_list(stmts: &mut Vec<StmtAst>) {
    loop {
        let index = build_block_def_use_index(stmts);
        let mut changed = false;

        for def_idx in 0..stmts.len().saturating_sub(1) {
            let (var_name, init_expr) = match &stmts[def_idx] {
                StmtAst::VarDecl {
                    binding: Var::Name(name),
                    expr,
                } => (name.clone(), expr.clone()),
                _ => continue,
            };
            if !is_store_chain_expr(&init_expr) {
                continue;
            }
            if !has_single_immediate_use(&index, def_idx, &var_name) {
                continue;
            }

            let next_stmt = match stmts.get_mut(def_idx + 1) {
                Some(stmt) => stmt,
                None => continue,
            };
            if !replace_ident_in_stmt_once(next_stmt, &var_name, &init_expr) {
                continue;
            }

            stmts.remove(def_idx);
            changed = true;
            break;
        }

        if !changed {
            break;
        }
    }
}

fn is_store_chain_expr(expr: &ExprAst) -> bool {
    match expr {
        ExprAst::Call { callee, args } if callee == "begin_cell" && args.is_empty() => true,
        ExprAst::MethodCall {
            method, modifying, ..
        } => !*modifying && method.starts_with("store_"),
        ExprAst::Call { callee, args } if callee == "end_cell" && args.len() == 1 => true,
        _ => false,
    }
}

fn replace_ident_in_stmt_once(stmt: &mut StmtAst, ident: &str, replacement: &ExprAst) -> bool {
    let mut replaced = 0_usize;
    match stmt {
        StmtAst::VarDecl { expr, .. }
        | StmtAst::Assign { expr, .. }
        | StmtAst::Return(Some(expr)) => {
            replace_ident_in_expr(expr, ident, replacement, &mut replaced);
        }
        StmtAst::Call { args, .. } => {
            for arg in args {
                replace_ident_in_expr(arg, ident, replacement, &mut replaced);
            }
        }
        StmtAst::Comment(_)
        | StmtAst::Return(None)
        | StmtAst::If { .. }
        | StmtAst::Repeat { .. }
        | StmtAst::DoUntil { .. } => {}
    }
    replaced == 1
}

fn replace_ident_in_expr(
    expr: &mut ExprAst,
    ident: &str,
    replacement: &ExprAst,
    replaced: &mut usize,
) {
    if matches!(expr, ExprAst::Ident(name) if name == ident) {
        *expr = replacement.clone();
        *replaced += 1;
        return;
    }

    match expr {
        ExprAst::Unary { expr, .. } => replace_ident_in_expr(expr, ident, replacement, replaced),
        ExprAst::Binary { lhs, rhs, .. } => {
            replace_ident_in_expr(lhs, ident, replacement, replaced);
            replace_ident_in_expr(rhs, ident, replacement, replaced);
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            replace_ident_in_expr(condition, ident, replacement, replaced);
            replace_ident_in_expr(then_expr, ident, replacement, replaced);
            replace_ident_in_expr(else_expr, ident, replacement, replaced);
        }
        ExprAst::Tuple(items) => {
            for item in items {
                replace_ident_in_expr(item, ident, replacement, replaced);
            }
        }
        ExprAst::Call { args, .. } => {
            for arg in args {
                replace_ident_in_expr(arg, ident, replacement, replaced);
            }
        }
        ExprAst::MethodCall { receiver, args, .. } => {
            replace_ident_in_expr(receiver, ident, replacement, replaced);
            for arg in args {
                replace_ident_in_expr(arg, ident, replacement, replaced);
            }
        }
        ExprAst::Ident(_)
        | ExprAst::Number(_)
        | ExprAst::StringLiteral(_)
        | ExprAst::CellLiteral(_)
        | ExprAst::NullLiteral => {}
    }
}

fn rewrite_store_calls_stmt(stmt: &mut StmtAst) {
    match stmt {
        StmtAst::Comment(_) | StmtAst::Return(None) => {}
        StmtAst::VarDecl { expr, .. }
        | StmtAst::Assign { expr, .. }
        | StmtAst::Return(Some(expr)) => {
            rewrite_store_calls_expr(expr);
        }
        StmtAst::Call { args, .. } => {
            for arg in args {
                rewrite_store_calls_expr(arg);
            }
        }
        StmtAst::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            rewrite_store_calls_expr(condition);
            rewrite_store_calls_stmt_list(then_body);
            if let Some(else_body) = else_body.as_mut() {
                rewrite_store_calls_stmt_list(else_body);
            }
        }
        StmtAst::Repeat { count, body } => {
            rewrite_store_calls_expr(count);
            rewrite_store_calls_stmt_list(body);
        }
        StmtAst::DoUntil { body, condition } => {
            rewrite_store_calls_stmt_list(body);
            rewrite_store_calls_expr(condition);
        }
    }
}

fn rewrite_store_calls_expr(expr: &mut ExprAst) {
    match expr {
        ExprAst::Unary { expr, .. } => rewrite_store_calls_expr(expr),
        ExprAst::Binary { lhs, rhs, .. } => {
            rewrite_store_calls_expr(lhs);
            rewrite_store_calls_expr(rhs);
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            rewrite_store_calls_expr(condition);
            rewrite_store_calls_expr(then_expr);
            rewrite_store_calls_expr(else_expr);
        }
        ExprAst::Tuple(items) => {
            for item in items {
                rewrite_store_calls_expr(item);
            }
        }
        ExprAst::Call { callee, args } => {
            for arg in args.iter_mut() {
                rewrite_store_calls_expr(arg);
            }
            if callee.starts_with("store_") && !args.is_empty() {
                let mut moved_args = std::mem::take(args);
                let receiver = moved_args.remove(0);
                let method = std::mem::take(callee);
                *expr = ExprAst::MethodCall {
                    receiver: Box::new(receiver),
                    method,
                    modifying: false,
                    args: moved_args,
                };
            } else if callee == "end_cell" && args.len() == 1 {
                let mut moved_args = std::mem::take(args);
                let receiver = moved_args.remove(0);
                let method = std::mem::take(callee);
                *expr = ExprAst::MethodCall {
                    receiver: Box::new(receiver),
                    method,
                    modifying: false,
                    args: Vec::new(),
                };
            }
        }
        ExprAst::MethodCall { receiver, args, .. } => {
            rewrite_store_calls_expr(receiver);
            for arg in args {
                rewrite_store_calls_expr(arg);
            }
        }
        ExprAst::Ident(_)
        | ExprAst::Number(_)
        | ExprAst::StringLiteral(_)
        | ExprAst::CellLiteral(_)
        | ExprAst::NullLiteral => {}
    }
}

fn simplify_stmt(stmt: &mut StmtAst) {
    match stmt {
        StmtAst::If {
            then_body,
            else_body,
            ..
        } => {
            simplify_stmt_list(then_body);
            if let Some(else_body) = else_body.as_mut() {
                simplify_stmt_list(else_body);
            }
        }
        StmtAst::Repeat { body, .. } => simplify_stmt_list(body),
        StmtAst::DoUntil { body, .. } => simplify_stmt_list(body),
        StmtAst::Comment(_)
        | StmtAst::VarDecl { .. }
        | StmtAst::Assign { .. }
        | StmtAst::Return(_)
        | StmtAst::Call { .. } => {}
    }
}

fn build_block_def_use_index(stmts: &[StmtAst]) -> BlockDefUseIndex {
    let mut defs_by_ident: HashMap<String, Vec<usize>> = HashMap::new();
    for (stmt_idx, stmt) in stmts.iter().enumerate() {
        if let StmtAst::VarDecl { binding, .. } = stmt {
            for name in collect_binding_names(binding) {
                defs_by_ident.entry(name).or_default().push(stmt_idx);
            }
        }
    }

    let mut index = BlockDefUseIndex::default();
    for (stmt_idx, stmt) in stmts.iter().enumerate() {
        let mut idents = Vec::new();
        collect_stmt_idents(stmt, &mut idents);
        for ident in idents {
            let use_id = index.uses.len();
            index.uses.push(UseOccurrence {
                stmt_idx,
                ident: ident.clone(),
            });

            let def = defs_by_ident
                .get(&ident)
                .and_then(|defs| defs.iter().copied().filter(|d| *d < stmt_idx).max());
            index.use_to_def.push(def);
            if let Some(def_idx) = def {
                index
                    .def_ident_to_uses
                    .entry((def_idx, ident.clone()))
                    .or_default()
                    .push(use_id);
            }
        }
    }

    index
}

fn collect_binding_names(binding: &Var) -> Vec<String> {
    let mut out = Vec::new();
    collect_binding_names_inner(binding, &mut out);
    out
}

fn collect_binding_names_inner(binding: &Var, out: &mut Vec<String>) {
    match binding {
        Var::Name(name) => out.push(name.clone()),
        Var::Tensor(items) => {
            for item in items {
                collect_binding_names_inner(item, out);
            }
        }
    }
}

fn collect_stmt_idents(stmt: &StmtAst, out: &mut Vec<String>) {
    match stmt {
        StmtAst::Comment(_) => {}
        StmtAst::VarDecl { expr, .. } => collect_expr_idents(expr, out),
        StmtAst::Assign { target, expr } => {
            out.push(target.clone());
            collect_expr_idents(expr, out);
        }
        StmtAst::Return(Some(expr)) => collect_expr_idents(expr, out),
        StmtAst::Return(None) => {}
        StmtAst::Call { args, .. } => {
            for arg in args {
                collect_expr_idents(arg, out);
            }
        }
        StmtAst::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            collect_expr_idents(condition, out);
            for nested in then_body {
                collect_stmt_idents(nested, out);
            }
            if let Some(else_body) = else_body {
                for nested in else_body {
                    collect_stmt_idents(nested, out);
                }
            }
        }
        StmtAst::Repeat { count, body } => {
            collect_expr_idents(count, out);
            for nested in body {
                collect_stmt_idents(nested, out);
            }
        }
        StmtAst::DoUntil { body, condition } => {
            for nested in body {
                collect_stmt_idents(nested, out);
            }
            collect_expr_idents(condition, out);
        }
    }
}

fn collect_expr_idents(expr: &ExprAst, out: &mut Vec<String>) {
    match expr {
        ExprAst::Ident(name) => out.push(name.clone()),
        ExprAst::Number(_)
        | ExprAst::StringLiteral(_)
        | ExprAst::CellLiteral(_)
        | ExprAst::NullLiteral => {}
        ExprAst::Unary { expr, .. } => collect_expr_idents(expr, out),
        ExprAst::Binary { lhs, rhs, .. } => {
            collect_expr_idents(lhs, out);
            collect_expr_idents(rhs, out);
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_idents(condition, out);
            collect_expr_idents(then_expr, out);
            collect_expr_idents(else_expr, out);
        }
        ExprAst::Tuple(items) => {
            for item in items {
                collect_expr_idents(item, out);
            }
        }
        ExprAst::Call { args, .. } => {
            for arg in args {
                collect_expr_idents(arg, out);
            }
        }
        ExprAst::MethodCall { receiver, args, .. } => {
            collect_expr_idents(receiver, out);
            for arg in args {
                collect_expr_idents(arg, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::simplify_method_body;
    use crate::func_decompile::ast::{BinaryOp, ExprAst, StmtAst, Var};

    fn ident(name: &str) -> ExprAst {
        ExprAst::Ident(name.to_string())
    }

    fn num(n: &str) -> ExprAst {
        ExprAst::Number(n.to_string())
    }

    #[test]
    fn inlines_single_use_if_condition() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v247"),
                expr: ExprAst::Binary {
                    lhs: Box::new(ident("v42")),
                    op: BinaryOp::Equal,
                    rhs: Box::new(num("621336170")),
                    wrap_lhs: true,
                    wrap_rhs: true,
                },
            },
            StmtAst::If {
                negated: false,
                condition: ident("v247"),
                then_body: vec![StmtAst::Return(None)],
                else_body: None,
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 1);
        match &body[0] {
            StmtAst::If { condition, .. } => {
                assert_ne!(condition, &ident("v247"));
            }
            _ => panic!("expected if"),
        }
    }

    #[test]
    fn does_not_inline_when_multiple_uses() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v247"),
                expr: ExprAst::Binary {
                    lhs: Box::new(ident("v42")),
                    op: BinaryOp::Equal,
                    rhs: Box::new(num("621336170")),
                    wrap_lhs: true,
                    wrap_rhs: true,
                },
            },
            StmtAst::If {
                negated: false,
                condition: ident("v247"),
                then_body: vec![],
                else_body: None,
            },
            StmtAst::Call {
                callee: "touch".to_string(),
                args: vec![ident("v247")],
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 3);
        match &body[1] {
            StmtAst::If { condition, .. } => assert_eq!(condition, &ident("v247")),
            _ => panic!("expected if"),
        }
    }

    #[test]
    fn inlines_single_use_into_var_decl_expr() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v0"),
                expr: num("7"),
            },
            StmtAst::VarDecl {
                binding: Var::name("v1"),
                expr: ExprAst::Binary {
                    lhs: Box::new(ident("v0")),
                    op: BinaryOp::Add,
                    rhs: Box::new(num("1")),
                    wrap_lhs: false,
                    wrap_rhs: false,
                },
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 1);
        match &body[0] {
            StmtAst::VarDecl { expr, .. } => match expr {
                ExprAst::Binary { lhs, .. } => assert_eq!(lhs.as_ref(), &num("7")),
                _ => panic!("expected binary expr"),
            },
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn inlines_single_use_into_call_arg() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v0"),
                expr: ExprAst::Call {
                    callee: "null?".to_string(),
                    args: vec![ident("v87")],
                },
            },
            StmtAst::Call {
                callee: "touch".to_string(),
                args: vec![ident("v0")],
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 1);
        match &body[0] {
            StmtAst::Call { args, .. } => {
                assert_eq!(args.len(), 1);
                assert_ne!(args[0], ident("v0"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn inlines_single_use_if_condition_under_bitnot() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v165"),
                expr: ExprAst::Binary {
                    lhs: Box::new(ident("v164")),
                    op: BinaryOp::Equal,
                    rhs: Box::new(num("0")),
                    wrap_lhs: true,
                    wrap_rhs: true,
                },
            },
            StmtAst::If {
                negated: false,
                condition: ExprAst::Unary {
                    op: crate::func_decompile::ast::UnaryOp::BitNot,
                    expr: Box::new(ident("v165")),
                },
                then_body: vec![StmtAst::Return(None)],
                else_body: None,
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 1);
        match &body[0] {
            StmtAst::If { condition, .. } => match condition {
                ExprAst::Unary { op, expr } => {
                    assert_eq!(*op, crate::func_decompile::ast::UnaryOp::BitNot);
                    assert_ne!(expr.as_ref(), &ident("v165"));
                }
                _ => panic!("expected unary bitnot condition"),
            },
            _ => panic!("expected if"),
        }
    }

    #[test]
    fn inlines_single_use_ternary_condition() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v88"),
                expr: ExprAst::Call {
                    callee: "null?".to_string(),
                    args: vec![ident("v87")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v89"),
                expr: ExprAst::Ternary {
                    condition: Box::new(ident("v88")),
                    then_expr: Box::new(num("10065")),
                    else_expr: Box::new(ident("v87")),
                },
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 1);
        match &body[0] {
            StmtAst::VarDecl { expr, .. } => match expr {
                ExprAst::Ternary { condition, .. } => {
                    assert_ne!(condition.as_ref(), &ident("v88"));
                }
                _ => panic!("expected ternary expr"),
            },
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn inlines_each_adjacent_ternary_condition_pair() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v88"),
                expr: ExprAst::Call {
                    callee: "null?".to_string(),
                    args: vec![ident("v87")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v89"),
                expr: ExprAst::Ternary {
                    condition: Box::new(ident("v88")),
                    then_expr: Box::new(num("10065")),
                    else_expr: Box::new(ident("v87")),
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v90"),
                expr: ExprAst::Call {
                    callee: "null?".to_string(),
                    args: vec![ident("v87")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v91"),
                expr: ExprAst::Ternary {
                    condition: Box::new(ident("v90")),
                    then_expr: Box::new(num("10435")),
                    else_expr: Box::new(ident("v87")),
                },
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 2);
        match &body[0] {
            StmtAst::VarDecl { expr, .. } => match expr {
                ExprAst::Ternary { condition, .. } => {
                    assert_ne!(condition.as_ref(), &ident("v88"));
                }
                _ => panic!("expected ternary expr"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { expr, .. } => match expr {
                ExprAst::Ternary { condition, .. } => {
                    assert_ne!(condition.as_ref(), &ident("v90"));
                }
                _ => panic!("expected ternary expr"),
            },
            _ => panic!("expected var decl"),
        }
    }

    #[test]
    fn folds_tuple_chain_into_modifying_method_calls() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v12"), Var::name("v13")]),
                expr: ExprAst::Call {
                    callee: "load_grams".to_string(),
                    args: vec![ident("v11")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v14"), Var::name("v15")]),
                expr: ExprAst::Call {
                    callee: "load_msg_addr".to_string(),
                    args: vec![ident("v12")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v16"), Var::name("v17")]),
                expr: ExprAst::Call {
                    callee: "load_ref".to_string(),
                    args: vec![ident("v14")],
                },
            },
            StmtAst::Call {
                callee: "end_parse".to_string(),
                args: vec![ident("v16")],
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 4);
        for idx in [0_usize, 1, 2] {
            match &body[idx] {
                StmtAst::VarDecl { binding, expr } => {
                    assert!(matches!(binding, Var::Name(_)));
                    match expr {
                        ExprAst::MethodCall {
                            receiver,
                            modifying,
                            ..
                        } => {
                            assert!(
                                matches!(receiver.as_ref(), ExprAst::Ident(name) if name == "v11")
                            );
                            assert!(*modifying);
                        }
                        _ => panic!("expected method call"),
                    }
                }
                _ => panic!("expected var decl"),
            }
        }
        match &body[3] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args.first(), Some(ExprAst::Ident(name)) if name == "v11"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn rewrites_store_calls_to_non_modifying_method_calls() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v23"),
                expr: ExprAst::Call {
                    callee: "store_grams".to_string(),
                    args: vec![
                        ExprAst::Call {
                            callee: "begin_cell".to_string(),
                            args: vec![],
                        },
                        ident("v22"),
                    ],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v24"),
                expr: ExprAst::Call {
                    callee: "store_slice".to_string(),
                    args: vec![ident("v23"), ident("v15")],
                },
            },
        ];

        simplify_method_body(&mut body);

        for stmt in &body {
            let StmtAst::VarDecl { expr, .. } = stmt else {
                panic!("expected var decl");
            };
            match expr {
                ExprAst::MethodCall { modifying, .. } => {
                    assert!(!*modifying);
                }
                _ => panic!("expected non-modifying method call"),
            }
        }
    }

    #[test]
    fn collapses_linear_store_chain_into_final_use() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v23"),
                expr: ExprAst::Call {
                    callee: "store_grams".to_string(),
                    args: vec![
                        ExprAst::Call {
                            callee: "begin_cell".to_string(),
                            args: vec![],
                        },
                        ident("v22"),
                    ],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v24"),
                expr: ExprAst::Call {
                    callee: "store_slice".to_string(),
                    args: vec![ident("v23"), ident("v15")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v25"),
                expr: ExprAst::Call {
                    callee: "store_slice".to_string(),
                    args: vec![ident("v24"), ident("v17")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v26"),
                expr: ExprAst::Call {
                    callee: "store_ref".to_string(),
                    args: vec![ident("v25"), ident("v19")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v27"),
                expr: ExprAst::Call {
                    callee: "store_ref".to_string(),
                    args: vec![ident("v26"), ident("v21")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v28"),
                expr: ExprAst::Call {
                    callee: "end_cell".to_string(),
                    args: vec![ident("v27")],
                },
            },
            StmtAst::Call {
                callee: "set_data".to_string(),
                args: vec![ident("v28")],
            },
        ];

        simplify_method_body(&mut body);

        assert_eq!(body.len(), 1);
        match &body[0] {
            StmtAst::Call { callee, args } => {
                assert_eq!(callee, "set_data");
                assert_eq!(args.len(), 1);
                assert!(!matches!(args[0], ExprAst::Ident(_)));
                match &args[0] {
                    ExprAst::MethodCall {
                        method,
                        modifying,
                        receiver,
                        args: method_args,
                    } => {
                        assert_eq!(method, "end_cell");
                        assert!(!*modifying);
                        assert!(method_args.is_empty());
                        assert!(matches!(receiver.as_ref(), ExprAst::MethodCall { .. }));
                    }
                    _ => panic!("expected end_cell method call"),
                }
            }
            _ => panic!("expected set_data call"),
        }
    }
}
