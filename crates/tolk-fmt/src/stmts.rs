use crate::pretty::RcDoc;
use crate::{Context, comments, common, exprs};
use tolk_syntax::{
    Assert, Block, CatchClause, DoWhile, Expr, ExprStmt, If, IfAlt, MatchStmt, Repeat, Return,
    Stmt, Throw, TryCatch, While,
};

#[must_use]
pub fn print_block_statement<'a>(ctx: &Context<'_>, block: &Block) -> Option<RcDoc<'a>> {
    let statements = block
        .stmts()
        .filter(|stmt| !matches!(stmt, Stmt::Unmapped(_) | Stmt::EmptyStmt(_)))
        .collect::<Vec<_>>();

    common::print_list(
        ctx,
        &statements,
        |ctx, stmt| print_statement(ctx, stmt),
        Stmt::syntax,
        |_| {
            block
                .statements_including_comments()
                .iter()
                .filter(|stmt| stmt.syntax().kind() == "comment")
                .map(Stmt::syntax)
                .collect()
        },
        common::ListOptions::curly_bracket_body(),
    )
}

/// Prints block contents and direct match-arm bodies; the enclosing list owns comments.
pub(crate) fn print_statement<'a>(ctx: &Context<'_>, stmt: &Stmt) -> Option<RcDoc<'a>> {
    match stmt {
        Stmt::Block(block) => print_block_statement(ctx, block),
        Stmt::If(if_stmt) => print_if_statement(ctx, if_stmt),
        Stmt::While(while_stmt) => print_while_statement(ctx, while_stmt),
        Stmt::Repeat(repeat_stmt) => print_repeat_statement(ctx, repeat_stmt),
        Stmt::TryCatch(try_catch) => print_try_catch_statement(ctx, try_catch),
        Stmt::Return(return_stmt) => print_return_statement(ctx, return_stmt),
        Stmt::DoWhile(do_while) => print_do_while_statement(ctx, do_while),
        Stmt::Break(node) => Some(RcDoc::text(format!(
            "break{}",
            statement_terminator(node.0)
        ))),
        Stmt::Continue(node) => Some(RcDoc::text(format!(
            "continue{}",
            statement_terminator(node.0)
        ))),
        Stmt::Throw(throw_stmt) => print_throw_statement(ctx, throw_stmt),
        Stmt::Assert(assert_stmt) => print_assert_statement(ctx, assert_stmt),
        Stmt::Match(match_stmt) => print_match_statement(ctx, match_stmt),
        Stmt::EmptyStmt(_) => Some(RcDoc::nil()),
        Stmt::ExprStmt(expr_stmt) => print_expression_statement(ctx, expr_stmt),
        Stmt::Unmapped(node) => {
            if node.0.kind() == "comment" {
                return Some(RcDoc::text(""));
            }
            if node.text(ctx.code.as_ref().as_ref()) == ";" {
                return Some(RcDoc::text(""));
            }
            common::print_node_text(ctx, &node.0)
        }
    }
}

/// A match arm owns its comma; the same statement in a block needs a semicolon.
fn statement_terminator(node: tree_sitter::Node<'_>) -> &'static str {
    if node
        .parent()
        .is_some_and(|parent| parent.kind() == "match_arm")
    {
        ""
    } else {
        ";"
    }
}

fn print_if_statement<'a>(ctx: &Context<'_>, if_stmt: &If) -> Option<RcDoc<'a>> {
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
        let next_node = match &alternative {
            IfAlt::If(next_if) => next_if.0,
            IfAlt::Block(block) => block.0,
        };
        docs.push(print_block_continuation(ctx, &body, Some(next_node)));
        docs.push(RcDoc::text("else "));
        match alternative {
            IfAlt::If(next_if) => {
                docs.push(print_if_statement(ctx, &next_if)?);
            }
            IfAlt::Block(block) => {
                docs.push(print_block_statement(ctx, &block)?);
            }
        }
    }

    Some(RcDoc::concat(docs))
}

fn print_block_continuation<'a>(
    ctx: &Context<'_>,
    block: &Block,
    next_node: Option<tree_sitter::Node<'_>>,
) -> RcDoc<'a> {
    // The enclosing statement owns comments between its branches; the block printer
    // only owns comments inside the braces. Keep continuation keywords out of `//` comments.
    let block_comments = ctx.comments.get(&block.0);
    let mut between = vec![];
    comments::print_trailing_comments(ctx, &mut between, block_comments);
    comments::print_leading_comments(
        ctx,
        &mut between,
        next_node.and_then(|node| ctx.comments.get(&node)),
    );

    let mut docs = vec![];
    comments::print_inline_comments(ctx, &mut docs, block_comments);
    docs.push(
        if !between.is_empty() || comments::has_inline_line_comments_on_node(ctx, block.0) {
            RcDoc::hardline()
        } else {
            RcDoc::space()
        },
    );
    docs.extend(between);
    RcDoc::concat(docs)
}

fn print_while_statement<'a>(ctx: &Context<'_>, while_stmt: &While) -> Option<RcDoc<'a>> {
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

fn print_repeat_statement<'a>(ctx: &Context<'_>, repeat_stmt: &Repeat) -> Option<RcDoc<'a>> {
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

fn print_do_while_statement<'a>(ctx: &Context<'_>, do_while: &DoWhile) -> Option<RcDoc<'a>> {
    let condition = do_while.condition()?;
    let body = do_while.body()?;

    let condition_doc = exprs::print_expression(ctx, &condition)?;
    let body_doc = print_block_statement(ctx, &body)?;

    Some(RcDoc::concat([
        RcDoc::text("do "),
        body_doc,
        print_block_continuation(ctx, &body, None),
        RcDoc::group(RcDoc::concat([
            RcDoc::text("while ("),
            RcDoc::concat([RcDoc::line_(), condition_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(")"),
            RcDoc::text(statement_terminator(do_while.0)),
        ])),
    ]))
}

pub(crate) fn print_return_statement<'a>(ctx: &Context, return_stmt: &Return) -> Option<RcDoc<'a>> {
    let expr = return_stmt.expr();

    let end_semicolon = statement_terminator(return_stmt.0);

    if let Some(expr) = expr {
        let expr_doc = exprs::print_expression(ctx, &expr)?;
        Some(RcDoc::concat([
            RcDoc::text("return "),
            expr_doc,
            RcDoc::text(end_semicolon),
        ]))
    } else {
        Some(RcDoc::text(format!("return{end_semicolon}")))
    }
}

pub(crate) fn print_throw_statement<'a>(ctx: &Context, throw_stmt: &Throw) -> Option<RcDoc<'a>> {
    let expr = throw_stmt.expr()?;

    let end_semicolon = statement_terminator(throw_stmt.0);

    let expr_doc = exprs::print_expression(ctx, &expr)?;
    Some(RcDoc::concat([
        RcDoc::text("throw "),
        expr_doc,
        RcDoc::text(end_semicolon),
    ]))
}

/// The throw form gives the condition its own indented header; the comma form uses call layout.
pub(crate) fn assert_uses_throw_syntax(assert_stmt: &Assert) -> bool {
    assert_stmt
        .0
        .children(&mut assert_stmt.0.walk())
        .any(|child| child.kind() == "throw")
}

fn print_assert_statement<'a>(ctx: &Context<'_>, assert_stmt: &Assert) -> Option<RcDoc<'a>> {
    let condition = assert_stmt.condition()?;
    let exc_no = assert_stmt.expr()?;
    let uses_throw = assert_uses_throw_syntax(assert_stmt);

    // A commented argument list owns its comments so delimiters stay outside `//`.
    // Keep the existing compact layout when no parenthesized argument has attached comments.
    let args = [condition, exc_no];
    let args = if uses_throw { &args[..1] } else { &args[..] };
    let args_doc = if args
        .iter()
        .any(|arg| ctx.comments.contains_key(&arg.syntax()))
    {
        common::print_list(
            ctx,
            args,
            exprs::print_expression_naked,
            Expr::syntax,
            |_| vec![],
            common::ListOptions {
                trailing_separator: false,
                ..Default::default()
            },
        )?
    } else {
        let condition_doc = exprs::print_expression(ctx, &condition)?;
        if uses_throw {
            RcDoc::concat([
                RcDoc::text("("),
                RcDoc::concat([RcDoc::line_(), condition_doc]).nest(4),
                RcDoc::line_(),
                RcDoc::text(")"),
            ])
        } else {
            RcDoc::concat([
                RcDoc::text("("),
                condition_doc,
                RcDoc::text(", "),
                exprs::print_expression(ctx, &exc_no)?,
                RcDoc::text(")"),
            ])
        }
    };

    let mut docs = vec![
        RcDoc::text(if uses_throw { "assert " } else { "assert" }),
        args_doc,
    ];
    if uses_throw {
        docs.push(RcDoc::text(" throw "));
        docs.push(exprs::print_expression(ctx, &exc_no)?);
    }
    docs.push(RcDoc::text(statement_terminator(assert_stmt.0)));
    Some(RcDoc::group(RcDoc::concat(docs)))
}

fn print_try_catch_statement<'a>(ctx: &Context, try_catch: &TryCatch) -> Option<RcDoc<'a>> {
    let body = try_catch.body()?;
    let catch = try_catch.catch()?;

    let body_doc = print_block_statement(ctx, &body)?;
    let catch_doc = print_catch_clause(ctx, &catch)?;

    Some(RcDoc::concat([
        RcDoc::text("try "),
        body_doc,
        print_block_continuation(ctx, &body, Some(catch.0)),
        RcDoc::text("catch "),
        catch_doc,
    ]))
}

fn print_catch_clause<'a>(ctx: &Context<'_>, catch: &CatchClause) -> Option<RcDoc<'a>> {
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

fn print_match_statement<'a>(ctx: &Context<'_>, match_stmt: &MatchStmt) -> Option<RcDoc<'a>> {
    let expr = match_stmt.expr()?;
    exprs::print_match_expression(ctx, &expr)
}

fn print_expression_statement<'a>(ctx: &Context, expr_stmt: &ExprStmt) -> Option<RcDoc<'a>> {
    let expr = expr_stmt.expr()?;
    let expr_doc = exprs::print_expression(ctx, &expr)?;
    Some(RcDoc::concat([expr_doc, RcDoc::text(";")]))
}
