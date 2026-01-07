use crate::{Context, common, exprs};
use pretty::RcDoc;
use tolk_ast::*;

pub fn print_block_statement<'a>(ctx: &mut Context, block: &BlockStatement) -> Option<RcDoc<'a>> {
    let statements = block.statements();
    let statements = statements
        .iter()
        .filter(|stmt| !matches!(stmt, Statement::Unmapped(_) | Statement::EmptyStatement(_)))
        .collect::<Vec<_>>();
    if statements.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    // При печати стейтментов мы должны учитывать что между ними могут быть пустые линии которые
    // мы хотим нормализовать до одной пустой линии, а не убирать их полностью:
    //
    // ```
    // let a = 100;
    //
    // let b = 200;
    // ```
    // Должно оставаться как есть.
    // Чтобы вставлять пустые линии, нам нужно знать были ли в оригинальном коде
    // между двумя стейтментами пустые строки.

    let mut docs = vec![RcDoc::hardline()];

    for (i, stmt) in statements.iter().enumerate() {
        let Some(doc) = print_statement(ctx, stmt) else {
            continue;
        };

        docs.push(doc);
        if i < statements.len() - 1 {
            docs.push(RcDoc::hardline());
        }

        // Если после стейтмента есть другой стейтмент, то есть вероятность, что нам нужна
        // дополнительная пустая строка, чтобы оставить пустые строки по правилам.
        //
        // Если между двумя стейтментами больше одной пустой строки, то добавляем пустую строку.
        if let Some(next_stmt) = statements.get(i + 1)
            && common::empty_lines_between(&stmt.raw_node(), &next_stmt.raw_node()) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    let result = RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat(docs).nest(4),
        RcDoc::hardline(),
        RcDoc::text("}"),
    ]);
    Some(result)
}

fn print_statement<'a>(ctx: &mut Context, stmt: &Statement) -> Option<RcDoc<'a>> {
    match stmt {
        Statement::BlockStatement(block) => print_block_statement(ctx, block),
        Statement::IfStatement(if_stmt) => print_if_statement(ctx, if_stmt),
        Statement::WhileStatement(while_stmt) => print_while_statement(ctx, while_stmt),
        Statement::RepeatStatement(repeat_stmt) => print_repeat_statement(ctx, repeat_stmt),
        Statement::TryCatchStatement(try_catch) => print_try_catch_statement(ctx, try_catch),
        Statement::ReturnStatement(return_stmt) => print_return_statement(ctx, return_stmt),
        Statement::LocalVarsDeclaration(locals) => print_local_variables(ctx, locals),
        Statement::DoWhileStatement(do_while) => print_do_while_statement(ctx, do_while),
        Statement::BreakStatement(_) => Some(RcDoc::text("break;")),
        Statement::ContinueStatement(_) => Some(RcDoc::text("continue;")),
        Statement::ThrowStatement(throw_stmt) => print_throw_statement(ctx, throw_stmt),
        Statement::AssertStatement(assert_stmt) => print_assert_statement(ctx, assert_stmt),
        Statement::MatchStatement(match_stmt) => print_match_statement(ctx, match_stmt),
        Statement::EmptyStatement(_) => Some(RcDoc::nil()),
        Statement::ExpressionStatement(expr_stmt) => print_expression_statement(ctx, expr_stmt),
        Statement::Unmapped(node) => {
            if node.0.kind() == "comment" {
                return Some(RcDoc::text(""));
            }
            if node.text(ctx.code.as_ref().as_ref()) == ";" {
                return Some(RcDoc::text(""));
            }
            crate::common::print_node_text(ctx, &node.0)
        }
    }
}

fn print_if_statement<'a>(ctx: &mut Context, if_stmt: &IfStatement) -> Option<RcDoc<'a>> {
    let condition = if_stmt.condition()?;
    let body = if_stmt.body()?;
    let alternative = if_stmt.alternative();

    let condition_doc = exprs::print_expression(ctx, &condition)?;
    let body_doc = print_block_statement(ctx, &body)?;

    let mut docs = vec![
        RcDoc::group(RcDoc::concat([
            RcDoc::text("if ("),
            RcDoc::concat([RcDoc::line_(), condition_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(") "),
        ])),
        body_doc,
    ];

    if let Some(alternative) = alternative {
        docs.push(RcDoc::text(" else "));
        match alternative {
            IfStatementAlternative::IfStatement(next_if) => {
                docs.push(print_if_statement(ctx, &next_if)?);
            }
            IfStatementAlternative::BlockStatement(block) => {
                docs.push(print_block_statement(ctx, &block)?);
            }
        }
    }

    Some(RcDoc::concat(docs))
}

fn print_while_statement<'a>(ctx: &mut Context, while_stmt: &WhileStatement) -> Option<RcDoc<'a>> {
    let condition = while_stmt.condition()?;
    let body = while_stmt.body()?;

    let condition_doc = exprs::print_expression(ctx, &condition)?;
    let body_doc = print_block_statement(ctx, &body)?;

    Some(RcDoc::concat([
        RcDoc::group(RcDoc::concat([
            RcDoc::text("while ("),
            RcDoc::concat([RcDoc::line_(), condition_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(") "),
        ])),
        body_doc,
    ]))
}

fn print_repeat_statement<'a>(
    ctx: &mut Context,
    repeat_stmt: &RepeatStatement,
) -> Option<RcDoc<'a>> {
    let count = repeat_stmt.count()?;
    let body = repeat_stmt.body()?;

    let count_doc = exprs::print_expression(ctx, &count)?;
    let body_doc = print_block_statement(ctx, &body)?;

    Some(RcDoc::concat([
        RcDoc::group(RcDoc::concat([
            RcDoc::text("repeat ("),
            RcDoc::concat([RcDoc::line_(), count_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(") "),
        ])),
        body_doc,
    ]))
}

fn print_do_while_statement<'a>(
    ctx: &mut Context,
    do_while: &DoWhileStatement,
) -> Option<RcDoc<'a>> {
    let condition = do_while.condition()?;
    let body = do_while.body()?;

    let condition_doc = exprs::print_expression(ctx, &condition)?;
    let body_doc = print_block_statement(ctx, &body)?;

    Some(RcDoc::concat([
        RcDoc::text("do "),
        body_doc,
        RcDoc::group(RcDoc::concat([
            RcDoc::text(" while ("),
            RcDoc::concat([RcDoc::line_(), condition_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(");"),
        ])),
    ]))
}

pub(crate) fn print_return_statement<'a>(
    ctx: &mut Context,
    return_stmt: &ReturnStatement,
) -> Option<RcDoc<'a>> {
    let expr = return_stmt.expr();

    // 10 => return 10,
    let in_match_arm = return_stmt
        .0
        .parent()
        .map(|p| p.kind() == "match_arm")
        .unwrap_or(false);
    let end_semicolon = if in_match_arm { "" } else { ";" };

    if let Some(expr) = expr {
        let expr_doc = exprs::print_expression(ctx, &expr)?;
        Some(RcDoc::concat([
            RcDoc::text("return "),
            expr_doc,
            RcDoc::text(end_semicolon),
        ]))
    } else {
        Some(RcDoc::text(format!("return{}", end_semicolon)))
    }
}

pub(crate) fn print_throw_statement<'a>(
    ctx: &mut Context,
    throw_stmt: &ThrowStatement,
) -> Option<RcDoc<'a>> {
    let expr = throw_stmt.expression()?;

    // 10 => throw 10,
    let in_match_arm = throw_stmt
        .0
        .parent()
        .map(|p| p.kind() == "match_arm")
        .unwrap_or(false);
    let end_semicolon = if in_match_arm { "" } else { ";" };

    let expr_doc = exprs::print_expression(ctx, &expr)?;
    Some(RcDoc::concat([
        RcDoc::text("throw "),
        expr_doc,
        RcDoc::text(end_semicolon),
    ]))
}

fn print_assert_statement<'a>(
    ctx: &mut Context,
    assert_stmt: &AssertStatement,
) -> Option<RcDoc<'a>> {
    let condition = assert_stmt.condition()?;
    let exc_no = assert_stmt.expression()?;

    let condition_doc = exprs::print_expression(ctx, &condition)?;
    let exc_no_doc = exprs::print_expression(ctx, &exc_no)?;

    // TODO: better way?
    // Check if it's the throw form: assert(...) throw ...
    let has_throw = assert_stmt
        .0
        .children(&mut assert_stmt.0.walk())
        .any(|child| child.kind() == "throw");

    if has_throw {
        Some(RcDoc::group(RcDoc::concat([
            RcDoc::text("assert ("),
            RcDoc::concat([RcDoc::line_(), condition_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(") throw "),
            exc_no_doc,
            RcDoc::text(";"),
        ])))
    } else {
        Some(RcDoc::group(RcDoc::concat([
            RcDoc::text("assert("),
            condition_doc,
            RcDoc::text(", "),
            exc_no_doc,
            RcDoc::text(");"),
        ])))
    }
}

fn print_try_catch_statement<'a>(
    ctx: &mut Context,
    try_catch: &TryCatchStatement,
) -> Option<RcDoc<'a>> {
    let body = try_catch.body()?;
    let catch = try_catch.catch()?;

    let body_doc = print_block_statement(ctx, &body)?;
    let catch_doc = print_catch_clause(ctx, &catch)?;

    Some(RcDoc::concat([
        RcDoc::text("try "),
        body_doc,
        RcDoc::text(" catch "),
        catch_doc,
    ]))
}

fn print_catch_clause<'a>(ctx: &mut Context, catch: &CatchClause) -> Option<RcDoc<'a>> {
    let body = catch.body()?;
    let var1 = catch.catch_var1();
    let var2 = catch.catch_var2();

    let body_doc = print_block_statement(ctx, &body)?;

    let mut vars_doc = RcDoc::nil();
    if let Some(v1) = var1 {
        let v1_doc = exprs::print_ident(ctx, &v1)?;
        if let Some(v2) = var2 {
            let v2_doc = exprs::print_ident(ctx, &v2)?;
            vars_doc = RcDoc::concat([
                RcDoc::text("("),
                v1_doc,
                RcDoc::text(", "),
                v2_doc,
                RcDoc::text(") "),
            ]);
        } else {
            vars_doc = RcDoc::concat([RcDoc::text("("), v1_doc, RcDoc::text(") ")]);
        }
    }

    Some(RcDoc::concat([vars_doc, body_doc]))
}

fn print_match_statement<'a>(ctx: &mut Context, match_stmt: &MatchStatement) -> Option<RcDoc<'a>> {
    let expr = match_stmt.expression()?;
    exprs::print_match_expression(ctx, &expr)
}

fn print_expression_statement<'a>(
    ctx: &mut Context,
    expr_stmt: &ExpressionStatement,
) -> Option<RcDoc<'a>> {
    let expr = expr_stmt.expression()?;
    let expr_doc = exprs::print_expression(ctx, &expr)?;
    Some(RcDoc::concat([expr_doc, RcDoc::text(";")]))
}

pub(crate) fn print_local_variables<'a>(
    ctx: &mut Context,
    locals: &LocalVarsDeclaration,
) -> Option<RcDoc<'a>> {
    let kind = locals.kind();
    let lhs = locals.lhs()?;
    let assigned_val = locals.assigned_val();

    let lhs_doc = print_var_declaration_lhs(ctx, &lhs)?;

    // Check if parent is match_expression like:
    // match (val a = 100) { ... }
    let is_match_expression = locals
        .0
        .parent()
        .map(|p| p.kind() == "match_expression")
        .unwrap_or(false);

    if let Some(assigned_val) = assigned_val {
        let assigned_val_doc = exprs::print_expression(ctx, &assigned_val)?;

        let result = RcDoc::concat([
            RcDoc::text(kind.as_str()),
            RcDoc::space(),
            lhs_doc,
            RcDoc::text(" = "),
            assigned_val_doc,
            if is_match_expression {
                RcDoc::nil()
            } else {
                RcDoc::text(";")
            },
        ]);
        return Some(result);
    }

    let result = RcDoc::concat([
        RcDoc::text(kind.as_str()),
        RcDoc::space(),
        lhs_doc,
        RcDoc::text(";"),
    ]);
    Some(result)
}

fn print_var_declaration_lhs<'a>(ctx: &mut Context, lhs: &VarDeclarationLhs) -> Option<RcDoc<'a>> {
    match lhs {
        VarDeclarationLhs::TupleVarsDeclaration(tuple) => {
            let vars = tuple.vars();
            print_tensor_tuple_lhs(ctx, vars, "[", "]")
        }
        VarDeclarationLhs::TensorVarsDeclaration(tensor) => {
            let vars = tensor.vars();
            print_tensor_tuple_lhs(ctx, vars, "(", ")")
        }
        VarDeclarationLhs::VarDeclaration(var) => {
            let name = var.name()?;
            let typ = var.typ();
            let is_redefinition = var.is_redefinition();

            let name_doc = exprs::print_ident(ctx, &name)?;
            if is_redefinition {
                Some(RcDoc::concat([name_doc, RcDoc::text(" redef")]))
            } else if let Some(typ) = typ {
                let type_doc = crate::types::print_type(ctx, &typ)?;
                Some(RcDoc::concat([name_doc, RcDoc::text(": "), type_doc]))
            } else {
                Some(name_doc)
            }
        }
    }
}

fn print_tensor_tuple_lhs<'a>(
    ctx: &mut Context,
    vars: Vec<VarDeclarationLhs>,
    open_quote: &'a str,
    close_quote: &'a str,
) -> Option<RcDoc<'a>> {
    let mut parts = vec![];

    for (i, var) in vars.iter().enumerate() {
        if i > 0 {
            parts.push(RcDoc::text(", "));
        }
        parts.push(print_var_declaration_lhs(ctx, var)?);
    }

    if parts.len() == 1 {
        return Some(RcDoc::concat([
            RcDoc::text(open_quote),
            parts[0].clone(),
            RcDoc::text(close_quote),
        ]));
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text(open_quote),
        RcDoc::concat([RcDoc::line_(), RcDoc::concat(parts)]).nest(4),
        RcDoc::line_(),
        RcDoc::text(close_quote),
    ])))
}
