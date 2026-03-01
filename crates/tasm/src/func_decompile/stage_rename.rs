use super::ast::{ExprAst, StmtAst, Var};
use std::collections::{HashMap, HashSet};

pub(crate) fn improve_variable_names(stmts: &mut [StmtAst]) {
    let mut used = HashSet::new();
    collect_used_names_stmt_list(stmts, &mut used);

    let in_msg_slice_bindings = collect_in_msg_slice_bindings(stmts);

    let mut requests = Vec::new();
    collect_rename_requests(stmts, &in_msg_slice_bindings, &mut requests);

    let mut allocator = NameAllocator::new(used);
    let mut rename_map = HashMap::new();
    for request in requests {
        if rename_map.contains_key(&request.old_name) {
            continue;
        }
        if !is_temp_var_name(&request.old_name) || request.old_name == request.base_name {
            continue;
        }
        let fresh = allocator.allocate(&request.base_name);
        rename_map.insert(request.old_name, fresh);
    }

    if rename_map.is_empty() {
        return;
    }
    apply_renames_stmt_list(stmts, &rename_map);
}

#[derive(Debug, Clone)]
struct RenameRequest {
    old_name: String,
    base_name: String,
}

#[derive(Debug)]
struct NameAllocator {
    used: HashSet<String>,
    next_index_by_base: HashMap<String, usize>,
}

impl NameAllocator {
    fn new(used: HashSet<String>) -> Self {
        Self {
            used,
            next_index_by_base: HashMap::new(),
        }
    }

    fn allocate(&mut self, base: &str) -> String {
        if base == "slice" || base == "cell" || base == "builder" || base == "in_msg_slice" {
            let idx = self
                .next_index_by_base
                .entry(base.to_string())
                .or_insert(0_usize);
            loop {
                let candidate = if base == "slice" {
                    format!("slice_{idx}")
                } else if base == "cell" {
                    format!("cell_{idx}")
                } else if base == "builder" {
                    format!("builder_{idx}")
                } else {
                    format!("in_msg_slice_{idx}")
                };
                *idx += 1;
                if !self.used.contains(&candidate) {
                    self.used.insert(candidate.clone());
                    return candidate;
                }
            }
        }

        if !self.used.contains(base) {
            let name = base.to_string();
            self.used.insert(name.clone());
            return name;
        }

        let idx = self
            .next_index_by_base
            .entry(base.to_string())
            .or_insert(2_usize);
        loop {
            let candidate = format!("{base}_{idx}");
            *idx += 1;
            if !self.used.contains(&candidate) {
                self.used.insert(candidate.clone());
                return candidate;
            }
        }
    }
}

fn is_temp_var_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('v') else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())
}

fn collect_in_msg_slice_bindings(stmts: &[StmtAst]) -> HashSet<String> {
    let mut sources = HashMap::new();
    collect_loader_slice_sources_stmt_list(stmts, &mut sources);

    let mut out = HashSet::new();
    let mut memo = HashMap::new();
    for name in sources.keys() {
        if flows_from_in_msg_body(name, &sources, &mut memo, &mut HashSet::new()) {
            out.insert(name.clone());
        }
    }
    out
}

fn collect_loader_slice_sources_stmt_list(
    stmts: &[StmtAst],
    out: &mut HashMap<String, String>,
) {
    for stmt in stmts {
        collect_loader_slice_sources_stmt(stmt, out);
    }
}

fn collect_loader_slice_sources_stmt(stmt: &StmtAst, out: &mut HashMap<String, String>) {
    match stmt {
        StmtAst::VarDecl { binding, expr } => {
            if let Some((next_slice, source)) = loader_next_slice_binding_and_source(binding, expr) {
                out.insert(next_slice, source);
            }
        }
        StmtAst::If {
            then_body,
            else_body,
            ..
        } => {
            collect_loader_slice_sources_stmt_list(then_body, out);
            if let Some(else_body) = else_body {
                collect_loader_slice_sources_stmt_list(else_body, out);
            }
        }
        StmtAst::Repeat { body, .. } | StmtAst::DoUntil { body, .. } => {
            collect_loader_slice_sources_stmt_list(body, out);
        }
        StmtAst::Comment(_)
        | StmtAst::Assign { .. }
        | StmtAst::Return(_)
        | StmtAst::Call { .. } => {}
    }
}

fn loader_next_slice_binding_and_source(binding: &Var, expr: &ExprAst) -> Option<(String, String)> {
    let next_slice = non_modifying_loader_first_binding_from_stmt(binding, expr)?;
    let source = source_ident_for_non_modifying_loader(expr)?;
    Some((next_slice, source))
}

fn source_ident_for_non_modifying_loader(expr: &ExprAst) -> Option<String> {
    match expr {
        ExprAst::Call { callee, args }
            if is_slice_remainder_loader_name(callee.as_str()) && !args.is_empty() =>
        {
            match args.first() {
                Some(ExprAst::Ident(name)) => Some(name.clone()),
                _ => None,
            }
        }
        ExprAst::MethodCall {
            receiver,
            method,
            modifying,
            args,
        } if is_slice_remainder_loader_name(method.as_str()) && !*modifying && args.is_empty() => {
            match receiver.as_ref() {
                ExprAst::Ident(name) => Some(name.clone()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn flows_from_in_msg_body(
    name: &str,
    sources: &HashMap<String, String>,
    memo: &mut HashMap<String, bool>,
    visiting: &mut HashSet<String>,
) -> bool {
    if let Some(cached) = memo.get(name) {
        return *cached;
    }
    if !visiting.insert(name.to_string()) {
        return false;
    }
    let result = match sources.get(name) {
        Some(source) if source == "in_msg_body" => true,
        Some(source) if source.starts_with("in_msg_slice") => true,
        Some(source) if sources.contains_key(source) => {
            flows_from_in_msg_body(source, sources, memo, visiting)
        }
        _ => false,
    };
    visiting.remove(name);
    memo.insert(name.to_string(), result);
    result
}

fn collect_rename_requests(
    stmts: &[StmtAst],
    in_msg_slice_bindings: &HashSet<String>,
    out: &mut Vec<RenameRequest>,
) {
    for stmt in stmts {
        match stmt {
            StmtAst::VarDecl { binding, expr } => {
                if let Some(ds_name) = get_data_begin_parse_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: ds_name,
                        base_name: "ds".to_string(),
                    });
                }
                if let Some(slice_name) = begin_parse_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: slice_name,
                        base_name: "slice".to_string(),
                    });
                }
                if let Some(slice_name) = skip_dict_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: slice_name,
                        base_name: "slice".to_string(),
                    });
                }
                if let Some(slice_name) =
                    non_modifying_loader_first_binding_from_stmt(binding, expr)
                {
                    let base_name = if in_msg_slice_bindings.contains(&slice_name) {
                        "in_msg_slice".to_string()
                    } else {
                        "slice".to_string()
                    };
                    out.push(RenameRequest {
                        old_name: slice_name,
                        base_name,
                    });
                }
                if let Some(builder_name) = builder_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: builder_name,
                        base_name: "builder".to_string(),
                    });
                }
                if let Some(cell_name) = cell_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: cell_name,
                        base_name: "cell".to_string(),
                    });
                }
                if let Some(is_null_name) = null_check_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: is_null_name,
                        base_name: "is_null".to_string(),
                    });
                }
                if let Some(precompiled_gas_name) = precompiled_gas_binding_from_stmt(binding, expr)
                {
                    out.push(RenameRequest {
                        old_name: precompiled_gas_name,
                        base_name: "precompiled_gas".to_string(),
                    });
                }
                if let Some(fwd_fee_name) = fwd_fee_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: fwd_fee_name,
                        base_name: "fwd_fee".to_string(),
                    });
                }
                if let Some(num_name) = loader_value_binding_from_stmt(binding, expr, &["load_int"])
                {
                    out.push(RenameRequest {
                        old_name: num_name,
                        base_name: "num".to_string(),
                    });
                }
                if let Some(addr_name) =
                    loader_value_binding_from_stmt(binding, expr, &["load_msg_addr"])
                {
                    out.push(RenameRequest {
                        old_name: addr_name,
                        base_name: "addr".to_string(),
                    });
                }
                if let Some(coins_name) =
                    loader_value_binding_from_stmt(binding, expr, &["load_grams", "load_coins"])
                {
                    out.push(RenameRequest {
                        old_name: coins_name,
                        base_name: "coins".to_string(),
                    });
                }
                if let Some(ref_name) = loader_value_binding_from_stmt(binding, expr, &["load_ref"])
                {
                    out.push(RenameRequest {
                        old_name: ref_name,
                        base_name: "ref".to_string(),
                    });
                }
                if let Some(value_name) = opcode_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: value_name,
                        base_name: "opcode".to_string(),
                    });
                }
                if let Some((workchain_name, hash_name)) =
                    parse_std_addr_binding_from_stmt(binding, expr)
                {
                    out.push(RenameRequest {
                        old_name: workchain_name,
                        base_name: "workchain".to_string(),
                    });
                    out.push(RenameRequest {
                        old_name: hash_name,
                        base_name: "addr_hash".to_string(),
                    });
                }
                if let Some(msg_flags_name) = msg_flags_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: msg_flags_name,
                        base_name: "msg_flags".to_string(),
                    });
                }
                if let Some((bits_name, refs_name)) = bits_refs_binding_from_stmt(binding, expr) {
                    out.push(RenameRequest {
                        old_name: bits_name,
                        base_name: "bits".to_string(),
                    });
                    out.push(RenameRequest {
                        old_name: refs_name,
                        base_name: "refs".to_string(),
                    });
                }
            }
            StmtAst::If {
                then_body,
                else_body,
                ..
            } => {
                collect_rename_requests(then_body, in_msg_slice_bindings, out);
                if let Some(else_body) = else_body {
                    collect_rename_requests(else_body, in_msg_slice_bindings, out);
                }
            }
            StmtAst::Repeat { body, .. } => {
                collect_rename_requests(body, in_msg_slice_bindings, out)
            }
            StmtAst::DoUntil { body, .. } => {
                collect_rename_requests(body, in_msg_slice_bindings, out)
            }
            StmtAst::Comment(_)
            | StmtAst::Assign { .. }
            | StmtAst::Return(_)
            | StmtAst::Call { .. } => {}
        }
    }
}

fn null_check_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    match expr {
        ExprAst::Call { callee, args } if callee == "null?" && args.len() == 1 => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn precompiled_gas_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    match expr {
        ExprAst::Call { callee, args }
            if callee == "get_precompiled_gas_consumption" && args.is_empty() =>
        {
            Some(name.clone())
        }
        _ => None,
    }
}

fn fwd_fee_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    match expr {
        ExprAst::Call { callee, args } if callee == "get_original_fwd_fee" && args.len() == 2 => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn cell_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    match expr {
        ExprAst::Call { callee, args } if callee == "end_cell" && args.len() == 1 => {
            Some(name.clone())
        }
        ExprAst::MethodCall {
            method,
            modifying,
            args,
            ..
        } if method == "end_cell" && !*modifying && args.is_empty() => Some(name.clone()),
        _ => None,
    }
}

fn begin_parse_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    if get_data_begin_parse_expr(expr) {
        return None;
    }
    match expr {
        ExprAst::Call { callee, args } if callee == "begin_parse" && args.len() == 1 => {
            Some(name.clone())
        }
        ExprAst::MethodCall {
            method,
            modifying,
            args,
            ..
        } if method == "begin_parse" && !*modifying && args.is_empty() => Some(name.clone()),
        _ => None,
    }
}

fn get_data_begin_parse_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    if get_data_begin_parse_expr(expr) {
        return Some(name.clone());
    }
    None
}

fn get_data_begin_parse_expr(expr: &ExprAst) -> bool {
    match expr {
        ExprAst::Call { callee, args } if callee == "begin_parse" && args.len() == 1 => {
            matches!(
                args.first(),
                Some(ExprAst::Call { callee, args }) if callee == "get_data" && args.is_empty()
            )
        }
        ExprAst::MethodCall {
            receiver,
            method,
            modifying,
            args,
        } if method == "begin_parse" && !*modifying && args.is_empty() => {
            matches!(
                receiver.as_ref(),
                ExprAst::Call { callee, args } if callee == "get_data" && args.is_empty()
            )
        }
        _ => false,
    }
}

fn skip_dict_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    match expr {
        ExprAst::Call { callee, args } if callee == "skip_dict" && args.len() == 1 => {
            Some(name.clone())
        }
        ExprAst::MethodCall {
            method,
            modifying,
            args,
            ..
        } if method == "skip_dict" && !*modifying && args.is_empty() => Some(name.clone()),
        _ => None,
    }
}

fn builder_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Name(name) = binding else {
        return None;
    };
    match expr {
        ExprAst::Call { callee, args } if callee == "begin_cell" && args.is_empty() => {
            Some(name.clone())
        }
        ExprAst::Call { callee, args } if callee.starts_with("store_") && !args.is_empty() => {
            Some(name.clone())
        }
        ExprAst::MethodCall {
            method, modifying, ..
        } if method.starts_with("store_") && !*modifying => Some(name.clone()),
        _ => None,
    }
}

fn msg_flags_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Tensor(items) = binding else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    let value_name = match &items[1] {
        Var::Name(name) => name.clone(),
        Var::Tensor(_) => return None,
    };

    match expr {
        ExprAst::Call { callee, args } if callee == "load_uint" && args.len() == 2 => {
            let src = args.first()?;
            let bits = args.get(1)?;
            if matches!(src, ExprAst::MethodCall { receiver, method, modifying, args } if matches!(receiver.as_ref(), ExprAst::Ident(name) if name == "in_msg_full") && method == "begin_parse" && !*modifying && args.is_empty())
                && matches!(bits, ExprAst::Number(bits) if bits == "4")
            {
                return Some(value_name);
            }
            None
        }
        _ => None,
    }
}

fn bits_refs_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<(String, String)> {
    let Var::Tensor(items) = binding else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    let bits_name = match &items[0] {
        Var::Name(name) => name.clone(),
        Var::Tensor(_) => return None,
    };
    let refs_name = match &items[1] {
        Var::Name(name) => name.clone(),
        Var::Tensor(_) => return None,
    };
    match expr {
        ExprAst::Call { callee, args } if callee == "slice_bits_refs" && args.len() == 1 => {
            Some((bits_name, refs_name))
        }
        _ => None,
    }
}

fn non_modifying_loader_first_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let load_name = match expr {
        ExprAst::Call { callee, .. } => callee.as_str(),
        ExprAst::MethodCall {
            method,
            modifying,
            args,
            ..
        } => {
            if *modifying || !args.is_empty() {
                return None;
            }
            method.as_str()
        }
        _ => return None,
    };
    if !is_slice_remainder_loader_name(load_name) {
        return None;
    }

    let Var::Tensor(items) = binding else {
        return None;
    };
    let first = items.first()?;
    match first {
        Var::Name(name) => Some(name.clone()),
        Var::Tensor(_) => None,
    }
}

fn is_slice_remainder_loader_name(name: &str) -> bool {
    matches!(
        name,
        "load_int"
            | "load_uint"
            | "load_grams"
            | "load_coins"
            | "load_ref"
            | "load_bits"
            | "load_dict"
            | "load_maybe_ref"
            | "load_msg_addr"
            | "load_bool"
            | "load_op"
            | "load_query_id"
            | "load_op_and_query_id"
    )
}

fn loader_value_binding_from_stmt(
    binding: &Var,
    expr: &ExprAst,
    loader_names: &[&str],
) -> Option<String> {
    let is_loader_expr = match expr {
        ExprAst::Call { callee, args } => {
            loader_names.contains(&callee.as_str()) && !args.is_empty()
        }
        ExprAst::MethodCall { method, args, .. } => {
            loader_names.contains(&method.as_str()) && args.is_empty()
        }
        _ => false,
    };
    if !is_loader_expr {
        return None;
    }

    match binding {
        Var::Name(name) => Some(name.clone()),
        Var::Tensor(items) if items.len() == 2 => match &items[1] {
            Var::Name(name) => Some(name.clone()),
            Var::Tensor(_) => None,
        },
        Var::Tensor(_) => None,
    }
}

fn opcode_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<String> {
    let Var::Tensor(items) = binding else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    let value_name = match &items[1] {
        Var::Name(name) => name.clone(),
        Var::Tensor(_) => return None,
    };

    match expr {
        ExprAst::Call { callee, args } if callee == "load_uint" && args.len() == 2 => {
            let src = args.first()?;
            let bits = args.get(1)?;
            if matches!(src, ExprAst::Ident(name) if name == "in_msg_body")
                && matches!(bits, ExprAst::Number(bits) if bits == "32")
            {
                return Some(value_name);
            }
            None
        }
        _ => None,
    }
}

fn parse_std_addr_binding_from_stmt(binding: &Var, expr: &ExprAst) -> Option<(String, String)> {
    let Var::Tensor(items) = binding else {
        return None;
    };
    if items.len() != 2 {
        return None;
    }
    let workchain_name = match &items[0] {
        Var::Name(name) => name.clone(),
        Var::Tensor(_) => return None,
    };
    let hash_name = match &items[1] {
        Var::Name(name) => name.clone(),
        Var::Tensor(_) => return None,
    };

    match expr {
        ExprAst::Call { callee, .. } if callee == "parse_std_addr" => {
            Some((workchain_name, hash_name))
        }
        _ => None,
    }
}

fn collect_used_names_stmt_list(stmts: &[StmtAst], out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            StmtAst::Comment(_) => {}
            StmtAst::VarDecl { binding, expr } => {
                collect_binding_names(binding, out);
                collect_expr_ident_names(expr, out);
            }
            StmtAst::Assign { target, expr } => {
                out.insert(target.clone());
                collect_expr_ident_names(expr, out);
            }
            StmtAst::Return(Some(expr)) => collect_expr_ident_names(expr, out),
            StmtAst::Return(None) => {}
            StmtAst::Call { args, .. } => {
                for arg in args {
                    collect_expr_ident_names(arg, out);
                }
            }
            StmtAst::If {
                condition,
                then_body,
                else_body,
                ..
            } => {
                collect_expr_ident_names(condition, out);
                collect_used_names_stmt_list(then_body, out);
                if let Some(else_body) = else_body {
                    collect_used_names_stmt_list(else_body, out);
                }
            }
            StmtAst::Repeat { count, body } => {
                collect_expr_ident_names(count, out);
                collect_used_names_stmt_list(body, out);
            }
            StmtAst::DoUntil { body, condition } => {
                collect_used_names_stmt_list(body, out);
                collect_expr_ident_names(condition, out);
            }
        }
    }
}

fn collect_binding_names(binding: &Var, out: &mut HashSet<String>) {
    match binding {
        Var::Name(name) => {
            out.insert(name.clone());
        }
        Var::Tensor(items) => {
            for item in items {
                collect_binding_names(item, out);
            }
        }
    }
}

fn collect_expr_ident_names(expr: &ExprAst, out: &mut HashSet<String>) {
    match expr {
        ExprAst::Ident(name) => {
            out.insert(name.clone());
        }
        ExprAst::Unary { expr, .. } => collect_expr_ident_names(expr, out),
        ExprAst::Binary { lhs, rhs, .. } => {
            collect_expr_ident_names(lhs, out);
            collect_expr_ident_names(rhs, out);
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            collect_expr_ident_names(condition, out);
            collect_expr_ident_names(then_expr, out);
            collect_expr_ident_names(else_expr, out);
        }
        ExprAst::Tuple(items) => {
            for item in items {
                collect_expr_ident_names(item, out);
            }
        }
        ExprAst::Call { args, .. } => {
            for arg in args {
                collect_expr_ident_names(arg, out);
            }
        }
        ExprAst::MethodCall { receiver, args, .. } => {
            collect_expr_ident_names(receiver, out);
            for arg in args {
                collect_expr_ident_names(arg, out);
            }
        }
        ExprAst::Number(_)
        | ExprAst::StringLiteral(_)
        | ExprAst::CellLiteral(_)
        | ExprAst::NullLiteral => {}
    }
}

fn apply_renames_stmt_list(stmts: &mut [StmtAst], rename_map: &HashMap<String, String>) {
    for stmt in stmts {
        apply_renames_stmt(stmt, rename_map);
    }
}

fn apply_renames_stmt(stmt: &mut StmtAst, rename_map: &HashMap<String, String>) {
    match stmt {
        StmtAst::Comment(_) => {}
        StmtAst::VarDecl { binding, expr } => {
            apply_renames_binding(binding, rename_map);
            apply_renames_expr(expr, rename_map);
        }
        StmtAst::Assign { target, expr } => {
            if let Some(new_name) = rename_map.get(target) {
                *target = new_name.clone();
            }
            apply_renames_expr(expr, rename_map);
        }
        StmtAst::Return(Some(expr)) => apply_renames_expr(expr, rename_map),
        StmtAst::Return(None) => {}
        StmtAst::Call { args, .. } => {
            for arg in args {
                apply_renames_expr(arg, rename_map);
            }
        }
        StmtAst::If {
            condition,
            then_body,
            else_body,
            ..
        } => {
            apply_renames_expr(condition, rename_map);
            apply_renames_stmt_list(then_body, rename_map);
            if let Some(else_body) = else_body {
                apply_renames_stmt_list(else_body, rename_map);
            }
        }
        StmtAst::Repeat { count, body } => {
            apply_renames_expr(count, rename_map);
            apply_renames_stmt_list(body, rename_map);
        }
        StmtAst::DoUntil { body, condition } => {
            apply_renames_stmt_list(body, rename_map);
            apply_renames_expr(condition, rename_map);
        }
    }
}

fn apply_renames_binding(binding: &mut Var, rename_map: &HashMap<String, String>) {
    match binding {
        Var::Name(name) => {
            if let Some(new_name) = rename_map.get(name) {
                *name = new_name.clone();
            }
        }
        Var::Tensor(items) => {
            for item in items {
                apply_renames_binding(item, rename_map);
            }
        }
    }
}

fn apply_renames_expr(expr: &mut ExprAst, rename_map: &HashMap<String, String>) {
    match expr {
        ExprAst::Ident(name) => {
            if let Some(new_name) = rename_map.get(name) {
                *name = new_name.clone();
            }
        }
        ExprAst::Unary { expr, .. } => apply_renames_expr(expr, rename_map),
        ExprAst::Binary { lhs, rhs, .. } => {
            apply_renames_expr(lhs, rename_map);
            apply_renames_expr(rhs, rename_map);
        }
        ExprAst::Ternary {
            condition,
            then_expr,
            else_expr,
        } => {
            apply_renames_expr(condition, rename_map);
            apply_renames_expr(then_expr, rename_map);
            apply_renames_expr(else_expr, rename_map);
        }
        ExprAst::Tuple(items) => {
            for item in items {
                apply_renames_expr(item, rename_map);
            }
        }
        ExprAst::Call { args, .. } => {
            for arg in args {
                apply_renames_expr(arg, rename_map);
            }
        }
        ExprAst::MethodCall { receiver, args, .. } => {
            apply_renames_expr(receiver, rename_map);
            for arg in args {
                apply_renames_expr(arg, rename_map);
            }
        }
        ExprAst::Number(_)
        | ExprAst::StringLiteral(_)
        | ExprAst::CellLiteral(_)
        | ExprAst::NullLiteral => {}
    }
}

#[cfg(test)]
mod tests {
    use super::improve_variable_names;
    use crate::func_decompile::ast::{ExprAst, StmtAst, Var};

    fn ident(name: &str) -> ExprAst {
        ExprAst::Ident(name.to_string())
    }

    #[test]
    fn renames_loaded_in_msg_body_32_value_to_opcode() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v41"), Var::name("v42")]),
                expr: ExprAst::Call {
                    callee: "load_uint".to_string(),
                    args: vec![ident("in_msg_body"), ExprAst::Number("32".to_string())],
                },
            },
            StmtAst::If {
                negated: false,
                condition: ident("v42"),
                then_body: vec![StmtAst::Return(None)],
                else_body: None,
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(items[1], Var::Name(ref name) if name == "opcode"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::If { condition, .. } => {
                assert!(matches!(condition, ExprAst::Ident(name) if name == "opcode"));
            }
            _ => panic!("expected if"),
        }
    }

    #[test]
    fn renames_in_msg_body_loader_chain_first_binding_to_in_msg_slice() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v41"), Var::name("v42")]),
                expr: ExprAst::Call {
                    callee: "load_uint".to_string(),
                    args: vec![ident("in_msg_body"), ExprAst::Number("32".to_string())],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v43"), Var::name("v44")]),
                expr: ExprAst::Call {
                    callee: "load_grams".to_string(),
                    args: vec![ident("v41")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v45"), Var::name("v46")]),
                expr: ExprAst::Call {
                    callee: "load_ref".to_string(),
                    args: vec![ident("v43")],
                },
            },
            StmtAst::Call {
                callee: "touch_many".to_string(),
                args: vec![ident("v41"), ident("v43"), ident("v45")],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(&items[0], Var::Name(name) if name == "in_msg_slice_0"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(&items[0], Var::Name(name) if name == "in_msg_slice_1"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(&items[0], Var::Name(name) if name == "in_msg_slice_2"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[3] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "in_msg_slice_0"));
                assert!(matches!(args[1], ExprAst::Ident(ref name) if name == "in_msg_slice_1"));
                assert!(matches!(args[2], ExprAst::Ident(ref name) if name == "in_msg_slice_2"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn picks_fresh_opcode_name_on_conflict() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("opcode"),
                expr: ExprAst::Number("0".to_string()),
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v41"), Var::name("v42")]),
                expr: ExprAst::Call {
                    callee: "load_uint".to_string(),
                    args: vec![ident("in_msg_body"), ExprAst::Number("32".to_string())],
                },
            },
            StmtAst::Call {
                callee: "touch".to_string(),
                args: vec![ident("v42")],
            },
        ];

        improve_variable_names(&mut body);

        match &body[1] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(items[1], Var::Name(ref name) if name == "opcode_2"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "opcode_2"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn renames_parse_std_addr_outputs_and_uses_global_index() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v60"), Var::name("v61")]),
                expr: ExprAst::Call {
                    callee: "parse_std_addr".to_string(),
                    args: vec![ident("v59")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v62"), Var::name("v63")]),
                expr: ExprAst::Call {
                    callee: "parse_std_addr".to_string(),
                    args: vec![ident("v58")],
                },
            },
            StmtAst::Call {
                callee: "touch_many".to_string(),
                args: vec![ident("v60"), ident("v61"), ident("v62"), ident("v63")],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(items[0], Var::Name(ref name) if name == "workchain"));
                    assert!(matches!(items[1], Var::Name(ref name) if name == "addr_hash"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(items[0], Var::Name(ref name) if name == "workchain_2"));
                    assert!(matches!(items[1], Var::Name(ref name) if name == "addr_hash_2"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "workchain"));
                assert!(matches!(args[1], ExprAst::Ident(ref name) if name == "addr_hash"));
                assert!(matches!(args[2], ExprAst::Ident(ref name) if name == "workchain_2"));
                assert!(matches!(args[3], ExprAst::Ident(ref name) if name == "addr_hash_2"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn renames_loader_values_to_addr_coins_ref() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v30"),
                expr: ExprAst::MethodCall {
                    receiver: Box::new(ident("v1")),
                    method: "load_msg_addr".to_string(),
                    modifying: true,
                    args: vec![],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v34"),
                expr: ExprAst::MethodCall {
                    receiver: Box::new(ident("v1")),
                    method: "load_grams".to_string(),
                    modifying: true,
                    args: vec![],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v19"),
                expr: ExprAst::MethodCall {
                    receiver: Box::new(ident("v1")),
                    method: "load_ref".to_string(),
                    modifying: true,
                    args: vec![],
                },
            },
            StmtAst::Call {
                callee: "touch_many".to_string(),
                args: vec![ident("v30"), ident("v34"), ident("v19")],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "addr"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "coins"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "ref"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[3] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "addr"));
                assert!(matches!(args[1], ExprAst::Ident(ref name) if name == "coins"));
                assert!(matches!(args[2], ExprAst::Ident(ref name) if name == "ref"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn renames_slice_builder_and_num_patterns() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v0"),
                expr: ExprAst::MethodCall {
                    receiver: Box::new(ExprAst::Call {
                        callee: "get_data".to_string(),
                        args: vec![],
                    }),
                    method: "begin_parse".to_string(),
                    modifying: false,
                    args: vec![],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v1"), Var::name("v2")]),
                expr: ExprAst::Call {
                    callee: "load_int".to_string(),
                    args: vec![ident("v0"), ExprAst::Number("8".to_string())],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v3"),
                expr: ExprAst::Call {
                    callee: "begin_cell".to_string(),
                    args: vec![],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v4"),
                expr: ExprAst::MethodCall {
                    receiver: Box::new(ident("v3")),
                    method: "store_uint".to_string(),
                    modifying: false,
                    args: vec![
                        ExprAst::Number("0".to_string()),
                        ExprAst::Number("4".to_string()),
                    ],
                },
            },
            StmtAst::Call {
                callee: "touch_many".to_string(),
                args: vec![
                    ident("v0"),
                    ident("v1"),
                    ident("v2"),
                    ident("v3"),
                    ident("v4"),
                ],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "ds"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(&items[0], Var::Name(name) if name == "slice_0"));
                    assert!(matches!(&items[1], Var::Name(name) if name == "num"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "builder_0"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[3] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "builder_1"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[4] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "ds"));
                assert!(matches!(args[1], ExprAst::Ident(ref name) if name == "slice_0"));
                assert!(matches!(args[2], ExprAst::Ident(ref name) if name == "num"));
                assert!(matches!(args[3], ExprAst::Ident(ref name) if name == "builder_0"));
                assert!(matches!(args[4], ExprAst::Ident(ref name) if name == "builder_1"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn renames_skip_dict_result_to_slice() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v34"),
                expr: ExprAst::Call {
                    callee: "skip_dict".to_string(),
                    args: vec![ident("v0")],
                },
            },
            StmtAst::Call {
                callee: "touch".to_string(),
                args: vec![ident("v34")],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "slice_0"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "slice_0"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn renames_null_check_and_precompiled_gas_calls() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v10"),
                expr: ExprAst::Call {
                    callee: "get_precompiled_gas_consumption".to_string(),
                    args: vec![],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v11"),
                expr: ExprAst::Call {
                    callee: "null?".to_string(),
                    args: vec![ident("v10")],
                },
            },
            StmtAst::Call {
                callee: "touch_many".to_string(),
                args: vec![ident("v10"), ident("v11")],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "precompiled_gas"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "is_null"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "precompiled_gas"));
                assert!(matches!(args[1], ExprAst::Ident(ref name) if name == "is_null"));
            }
            _ => panic!("expected call"),
        }
    }

    #[test]
    fn renames_cell_bits_refs_fwd_fee_and_msg_flags() {
        let mut body = vec![
            StmtAst::VarDecl {
                binding: Var::name("v1"),
                expr: ExprAst::Call {
                    callee: "get_original_fwd_fee".to_string(),
                    args: vec![ExprAst::Number("0".to_string()), ident("v39")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::name("v2"),
                expr: ExprAst::MethodCall {
                    receiver: Box::new(ident("v111")),
                    method: "end_cell".to_string(),
                    modifying: false,
                    args: vec![],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v3"), Var::name("v4")]),
                expr: ExprAst::Call {
                    callee: "slice_bits_refs".to_string(),
                    args: vec![ident("v78")],
                },
            },
            StmtAst::VarDecl {
                binding: Var::tensor(vec![Var::name("v5"), Var::name("v6")]),
                expr: ExprAst::Call {
                    callee: "load_uint".to_string(),
                    args: vec![
                        ExprAst::MethodCall {
                            receiver: Box::new(ident("in_msg_full")),
                            method: "begin_parse".to_string(),
                            modifying: false,
                            args: vec![],
                        },
                        ExprAst::Number("4".to_string()),
                    ],
                },
            },
            StmtAst::Call {
                callee: "touch_many".to_string(),
                args: vec![
                    ident("v1"),
                    ident("v2"),
                    ident("v3"),
                    ident("v4"),
                    ident("v6"),
                ],
            },
        ];

        improve_variable_names(&mut body);

        match &body[0] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "fwd_fee"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[1] {
            StmtAst::VarDecl { binding, .. } => {
                assert!(matches!(binding, Var::Name(name) if name == "cell_0"));
            }
            _ => panic!("expected var decl"),
        }
        match &body[2] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(&items[0], Var::Name(name) if name == "bits"));
                    assert!(matches!(&items[1], Var::Name(name) if name == "refs"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[3] {
            StmtAst::VarDecl { binding, .. } => match binding {
                Var::Tensor(items) => {
                    assert!(matches!(&items[1], Var::Name(name) if name == "msg_flags"));
                }
                _ => panic!("expected tensor binding"),
            },
            _ => panic!("expected var decl"),
        }
        match &body[4] {
            StmtAst::Call { args, .. } => {
                assert!(matches!(args[0], ExprAst::Ident(ref name) if name == "fwd_fee"));
                assert!(matches!(args[1], ExprAst::Ident(ref name) if name == "cell_0"));
                assert!(matches!(args[2], ExprAst::Ident(ref name) if name == "bits"));
                assert!(matches!(args[3], ExprAst::Ident(ref name) if name == "refs"));
                assert!(matches!(args[4], ExprAst::Ident(ref name) if name == "msg_flags"));
            }
            _ => panic!("expected call"),
        }
    }
}
