use crate::{Context, comments, common, stmts, types};
use pretty::RcDoc;
use tolk_ast::*;
use tree_sitter::Node;

pub fn print_expression<'a>(ctx: &Context, expr: &Expression) -> Option<RcDoc<'a>> {
    // TODO: other literals as well
    if let Expression::NumberLiteral(lit) = expr {
        let kind = lit.0.parent()?.kind();
        if kind == "tensor_expression"
            || kind == "tuple_expression"
            || kind == "typed_tuple"
            || kind == "annotation_arguments"
        {
            return print_expression_naked(ctx, expr);
        }
    }

    let node = expr.raw_node();
    let comments = ctx.comments.get(&node);

    if comments.is_none() {
        return print_expression_naked(ctx, expr);
    }

    let mut docs = vec![];
    comments::print_leading_comments(ctx, &mut docs, comments);

    let doc = print_expression_naked(ctx, expr)?;
    docs.push(doc);

    comments::print_inline_comments(ctx, &mut docs, comments);

    Some(RcDoc::concat(docs))
}

fn print_expression_naked<'a>(ctx: &Context, expr: &Expression) -> Option<RcDoc<'a>> {
    match expr {
        Expression::Assignment(assignment) => print_assignment(ctx, assignment),
        Expression::SetAssignment(set_assignment) => print_set_assignment(ctx, set_assignment),
        Expression::TernaryOperator(ternary) => print_ternary_operator(ctx, ternary),
        Expression::BinaryOperator(binary) => print_binary_operator(ctx, binary),
        Expression::UnaryOperator(unary) => print_unary_operator(ctx, unary),
        Expression::LazyExpression(lazy) => print_lazy_expression(ctx, lazy),
        Expression::CastAsOperator(cast) => print_cast_as_operator(ctx, cast),
        Expression::IsTypeOperator(is_type) => print_is_type_operator(ctx, is_type),
        Expression::NotNullOperator(not_null) => print_not_null_operator(ctx, not_null),
        Expression::DotAccess(dot) => print_dot_access(ctx, dot),
        Expression::FunctionCall(call) => print_function_call(ctx, call),
        Expression::GenericInstantiation(r#gen) => print_generic_instantiation(ctx, r#gen),
        Expression::ParenthesizedExpression(paren) => print_parenthesized_expression(ctx, paren),
        Expression::MatchExpression(match_expr) => print_match_expression(ctx, match_expr),
        Expression::ObjectLiteral(obj) => print_object_literal(ctx, obj),
        Expression::TensorExpression(tensor) => print_tensor_expression(ctx, tensor),
        Expression::TypedTuple(tuple) => print_typed_tuple(ctx, tuple),
        Expression::LambdaExpression(lambda) => print_lambda_expression(ctx, lambda),
        Expression::NumberLiteral(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expression::StringLiteral(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expression::BooleanLiteral(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expression::NullLiteral(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expression::Underscore(und) => Some(common::print_node_text(ctx, &und.0)?),
        Expression::Ident(ident) => Some(common::print_node_text(ctx, &ident.0)?),
        Expression::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

pub fn print_assignment<'a>(ctx: &Context, assignment: &Assignment) -> Option<RcDoc<'a>> {
    let left = assignment.left()?;
    let right = assignment.right()?;
    let left_doc = print_expression(ctx, &left)?;
    let right_doc = print_expression(ctx, &right)?;
    Some(RcDoc::concat([left_doc, RcDoc::text(" = "), right_doc]))
}

pub fn print_set_assignment<'a>(
    ctx: &Context,
    set_assignment: &SetAssignment,
) -> Option<RcDoc<'a>> {
    let left = set_assignment.left()?;
    let right = set_assignment.right()?;
    let op = set_assignment
        .operator_name(ctx.code.as_ref().as_ref())
        .to_string();

    let left_doc = print_expression(ctx, &left)?;
    let right_doc = print_expression(ctx, &right)?;

    Some(RcDoc::concat([
        left_doc,
        RcDoc::space(),
        RcDoc::text(op),
        RcDoc::space(),
        right_doc,
    ]))
}

pub fn print_ternary_operator<'a>(ctx: &Context, ternary: &TernaryOperator) -> Option<RcDoc<'a>> {
    let condition = ternary.condition()?;
    let consequence = ternary.consequence()?;
    let alternative = ternary.alternative()?;

    let condition_doc = print_expression(ctx, &condition)?;
    let consequence_doc = print_expression(ctx, &consequence)?;
    let alternative_doc = print_expression(ctx, &alternative)?;

    Some(RcDoc::group(
        condition_doc.append(
            RcDoc::concat([
                RcDoc::line(),
                RcDoc::text("? "),
                consequence_doc,
                RcDoc::line(),
                RcDoc::text(": "),
                alternative_doc,
            ])
            .nest(4),
        ),
    ))
}

pub fn print_binary_operator<'a>(ctx: &Context, binary: &BinaryOperator) -> Option<RcDoc<'a>> {
    let left = binary.left()?;
    let right = binary.right()?;
    let op = binary.operator_name(ctx.code.as_ref().as_ref()).to_string();
    let left_doc = print_expression(ctx, &left)?;
    let right_doc = print_expression(ctx, &right)?;

    Some(RcDoc::group(RcDoc::concat([
        left_doc,
        RcDoc::text(" "),
        RcDoc::text(op),
        RcDoc::line(),
        RcDoc::group(right_doc),
    ])))
}

pub fn print_unary_operator<'a>(ctx: &Context, unary: &UnaryOperator) -> Option<RcDoc<'a>> {
    let op = unary.operator_name(ctx.code.as_ref().as_ref()).to_string();
    let arg = unary.argument()?;
    let arg_doc = print_expression(ctx, &arg)?;
    Some(RcDoc::concat([RcDoc::text(op), arg_doc]))
}

pub fn print_lazy_expression<'a>(ctx: &Context, lazy: &LazyExpression) -> Option<RcDoc<'a>> {
    let expr = lazy.expr()?;
    let expr_doc = print_expression(ctx, &expr)?;
    Some(RcDoc::concat([RcDoc::text("lazy "), expr_doc]))
}

pub fn print_cast_as_operator<'a>(ctx: &Context, cast: &CastAsOperator) -> Option<RcDoc<'a>> {
    let expr = cast.expr()?;
    let typ = cast.casted_to()?;
    let expr_doc = print_expression(ctx, &expr)?;
    let type_doc = types::print_type(ctx, &typ)?;
    Some(RcDoc::concat([expr_doc, RcDoc::text(" as "), type_doc]))
}

pub fn print_is_type_operator<'a>(ctx: &Context, is_type: &IsTypeOperator) -> Option<RcDoc<'a>> {
    let expr = is_type.expr()?;
    let op = is_type
        .operator_name(ctx.code.as_ref().as_ref())
        .to_string();
    let rhs = is_type.rhs_type()?;

    let expr_doc = print_expression(ctx, &expr)?;
    let rhs_doc = types::print_type(ctx, &rhs)?;

    Some(RcDoc::concat([
        expr_doc,
        RcDoc::text(" "),
        RcDoc::text(op),
        RcDoc::text(" "),
        rhs_doc,
    ]))
}

pub fn print_not_null_operator<'a>(ctx: &Context, not_null: &NotNullOperator) -> Option<RcDoc<'a>> {
    let inner = not_null.inner()?;
    let inner_doc = print_expression(ctx, &inner)?;
    Some(RcDoc::concat([inner_doc, RcDoc::text("!")]))
}

pub fn print_dot_access<'a>(ctx: &Context, dot: &DotAccess) -> Option<RcDoc<'a>> {
    let obj = dot.obj()?;
    let field = dot.field()?;
    let obj_doc = print_expression(ctx, &obj)?;

    let field_doc = match field {
        DotAccessField::Ident(i) => print_simple_node(ctx, &i.0)?,
        DotAccessField::NumericIndex(n) => print_simple_node(ctx, &n.0)?,
    };

    let is_obj_literal = matches!(obj, Expression::ObjectLiteral(_));

    if is_obj_literal {
        Some(RcDoc::group(RcDoc::concat([
            obj_doc,
            RcDoc::text("."),
            field_doc,
        ])))
    } else {
        Some(RcDoc::group(RcDoc::concat([
            obj_doc,
            RcDoc::concat([RcDoc::softline_(), RcDoc::text("."), field_doc]).nest(4),
        ])))
    }
}

pub fn print_function_call<'a>(ctx: &Context, call: &FunctionCall) -> Option<RcDoc<'a>> {
    let callee = call.callee()?;
    let callee_doc = print_expression(ctx, &callee)?;
    let args = call.arguments();
    let args_doc = print_argument_list(ctx, &args)?;

    Some(RcDoc::concat([callee_doc, args_doc]))
}

pub fn print_argument_list<'a>(ctx: &Context, args: &[CallArgument]) -> Option<RcDoc<'a>> {
    if args.is_empty() {
        return Some(RcDoc::text("()"));
    }

    // We want to output:
    // ```
    // createMessage({
    //    ...
    // })
    // ```
    // Thus, without breaking the entire { ... } and without adding extra indentation
    // TODO: better way?
    if args.len() == 1
        && let Some(single) = args.first()
        && matches!(single.expr(), Some(Expression::ObjectLiteral(_)))
    {
        return Some(RcDoc::group(RcDoc::concat([
            RcDoc::text("("),
            print_call_argument(ctx, single)?,
            RcDoc::text(")"),
        ])));
    }

    let mut docs = vec![RcDoc::line_()];
    for (i, arg) in args.iter().enumerate() {
        let node = &arg.0;
        let comments = ctx.comments.get(node);
        comments::print_leading_comments(ctx, &mut docs, comments);

        docs.push(print_call_argument(ctx, arg)?);

        let is_last = i == args.len() - 1;
        if !is_last {
            docs.push(RcDoc::text(","));
        } else {
            docs.push(RcDoc::flat_alt(RcDoc::text(","), RcDoc::nil()));
        }

        comments::print_inline_comments(ctx, &mut docs, comments);

        if is_last {
            docs.push(RcDoc::line_());
        } else {
            docs.push(RcDoc::line());
        }

        comments::print_trailing_comments(ctx, &mut docs, comments);

        // There can be an empty line between arguments that we want to preserve
        if let Some(next) = args.get(i + 1)
            && common::empty_lines_between(ctx, node, &next.0) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text("("),
        RcDoc::concat(docs).nest(4),
        RcDoc::text(")"),
    ])))
}

pub fn print_call_argument<'a>(ctx: &Context, arg: &CallArgument) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    if arg.mutate() {
        parts.push(RcDoc::text("mutate "));
    }
    if let Some(expr) = arg.expr() {
        parts.push(print_expression(ctx, &expr)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_generic_instantiation<'a>(
    ctx: &Context,
    instantiation: &GenericInstantiation,
) -> Option<RcDoc<'a>> {
    let expr = instantiation.expr()?;
    let expr_doc = print_expression(ctx, &expr)?;
    let ts = instantiation.instantiation_ts()?;
    let types = ts.types();

    if types.is_empty() {
        return Some(RcDoc::concat([expr_doc, RcDoc::text("<>")]));
    }

    let mut docs = vec![RcDoc::line_()];
    for (i, typ) in types.iter().enumerate() {
        let node = typ.raw_node();
        let comments = ctx.comments.get(&node);
        comments::print_leading_comments(ctx, &mut docs, comments);

        docs.push(types::print_type(ctx, typ)?);

        let is_last = i == types.len() - 1;
        if !is_last {
            docs.push(RcDoc::text(","));
        } else {
            docs.push(RcDoc::flat_alt(RcDoc::text(","), RcDoc::nil()));
        }

        comments::print_inline_comments(ctx, &mut docs, comments);

        if is_last {
            docs.push(RcDoc::line_());
        } else {
            docs.push(RcDoc::line());
        }

        comments::print_trailing_comments(ctx, &mut docs, comments);

        if let Some(next) = types.get(i + 1)
            && common::empty_lines_between(ctx, &node, &next.raw_node()) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat([
        expr_doc,
        RcDoc::group(RcDoc::concat([
            RcDoc::text("<"),
            RcDoc::concat(docs).nest(4),
            RcDoc::text(">"),
        ])),
    ]))
}

pub fn print_parenthesized_expression<'a>(
    ctx: &Context,
    paren: &ParenthesizedExpression,
) -> Option<RcDoc<'a>> {
    let inner = paren.inner()?;
    let inner_doc = print_expression(ctx, &inner)?;
    Some(RcDoc::concat([
        RcDoc::text("("),
        inner_doc,
        RcDoc::text(")"),
    ]))
}

pub fn print_match_expression<'a>(
    ctx: &Context,
    match_expr: &MatchExpression,
) -> Option<RcDoc<'a>> {
    let expr = match_expr.expr()?;
    let body = match_expr.body()?;

    let expr_doc = match expr {
        MatchExpr::Expression(e) => print_expression(ctx, &e)?,
        MatchExpr::LocalVarsDeclaration(l) => stmts::print_local_variables(ctx, &l)?,
    };

    let body_doc = print_match_body(ctx, &body)?;

    Some(RcDoc::concat([
        RcDoc::group(RcDoc::concat([
            RcDoc::text("match ("),
            RcDoc::concat([RcDoc::line_(), expr_doc]).nest(4),
            RcDoc::line_(),
            RcDoc::text(") "),
        ])),
        body_doc,
    ]))
}

pub fn print_match_body<'a>(ctx: &Context, body: &MatchBody) -> Option<RcDoc<'a>> {
    let arms = body.arms();
    if arms.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let mut arm_docs_with_info = Vec::with_capacity(arms.len());
    let mut max_width = 0;

    for arm in arms.iter() {
        let doc = print_match_arm(ctx, arm)?;
        let width = common::doc_width(&doc);

        let comments = ctx.comments.get(&arm.0);
        let has_inline =
            comments.is_some_and(|cs| cs.iter().any(|c| c.kind == comments::CommentKind::Inline));

        if has_inline {
            max_width = max_width.max(width);
        }
        arm_docs_with_info.push((doc, comments));
    }

    let mut docs = vec![RcDoc::hardline()];
    for (i, (arm_doc, comments)) in arm_docs_with_info.into_iter().enumerate() {
        let arm = &arms[i];
        comments::print_leading_comments(ctx, &mut docs, comments);

        docs.push(arm_doc);

        comments::print_inline_comments_with_alignment(ctx, &mut docs, comments, max_width);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);

        // There can be an empty line between arms that we want to preserve
        if let Some(next) = arms.get(i + 1)
            && common::empty_lines_between(ctx, &arm.0, &next.0) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat(docs).nest(4),
        RcDoc::text("}"),
    ]))
}

pub fn print_match_arm<'a>(ctx: &Context, arm: &MatchArm) -> Option<RcDoc<'a>> {
    let pattern = arm.pattern();
    let body = arm.body()?;

    let pattern_doc = match pattern {
        MatchPattern::Type(t) => types::print_type(ctx, &t)?,
        MatchPattern::Expression(e) => print_expression(ctx, &e)?,
        MatchPattern::Else => RcDoc::text("else"),
    };

    let (body_doc, is_block) = match body {
        MatchArmBody::BlockStatement(b) => (stmts::print_block_statement(ctx, &b)?, true),
        MatchArmBody::ReturnStatement(r) => (stmts::print_return_statement(ctx, &r)?, false),
        MatchArmBody::ThrowStatement(t) => (stmts::print_throw_statement(ctx, &t)?, false),
        MatchArmBody::Expression(e) => (print_expression(ctx, &e)?, false),
    };

    Some(RcDoc::concat([
        pattern_doc,
        RcDoc::text(" => "),
        body_doc,
        if is_block {
            RcDoc::nil()
        } else {
            RcDoc::text(",")
        },
    ]))
}

pub fn print_object_literal<'a>(ctx: &Context, obj: &ObjectLiteral) -> Option<RcDoc<'a>> {
    let typ = obj.typ();
    let mut docs = vec![];
    if let Some(typ) = typ {
        docs.push(types::print_type(ctx, &typ)?);
        docs.push(RcDoc::space());
    }

    let args = obj.arguments();
    let args_doc = print_object_literal_body(ctx, &args)?;
    docs.push(args_doc);

    Some(RcDoc::group(RcDoc::concat(docs)))
}

pub fn print_object_literal_body<'a>(
    ctx: &Context,
    args: &[InstanceArgument],
) -> Option<RcDoc<'a>> {
    if args.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let has_comments = args.iter().any(|arg| {
        ctx.comments
            .get(&arg.0)
            .is_some_and(|cs| !cs.is_empty())
    });

    let is_multiline = args.len() > 2 || has_comments;
    let separator = if is_multiline {
        RcDoc::hardline()
    } else {
        RcDoc::line()
    };

    let mut arg_docs_with_info = Vec::with_capacity(args.len());
    let mut max_width = 0;

    for (i, arg) in args.iter().enumerate() {
        let is_last = i == (args.len() - 1);
        let doc = print_instance_argument(ctx, arg, is_last)?;
        let width = common::doc_width(&doc);

        let comments = ctx.comments.get(&arg.0);
        let has_inline =
            comments.is_some_and(|cs| cs.iter().any(|c| c.kind == comments::CommentKind::Inline));

        if has_inline && is_multiline {
            max_width = max_width.max(width);
        }
        arg_docs_with_info.push((doc, comments));
    }

    let mut arg_docs = vec![separator.clone()];
    for (i, (arg_doc, comments)) in arg_docs_with_info.into_iter().enumerate() {
        let arg = &args[i];

        comments::print_leading_comments(ctx, &mut arg_docs, comments);

        arg_docs.push(arg_doc);

        if is_multiline {
            comments::print_inline_comments_with_alignment(ctx, &mut arg_docs, comments, max_width);
        } else {
            comments::print_inline_comments(ctx, &mut arg_docs, comments);
        }

        arg_docs.push(separator.clone());
        comments::print_trailing_comments(ctx, &mut arg_docs, comments);

        // There can be an empty line between args that we want to preserve
        if let Some(next) = args.get(i + 1)
            && common::empty_lines_between(ctx, &arg.0, &next.0) > 1
        {
            arg_docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat(arg_docs).nest(4),
        RcDoc::text("}"),
    ]))
}

pub fn print_instance_argument<'a>(
    ctx: &Context,
    arg: &InstanceArgument,
    is_last: bool,
) -> Option<RcDoc<'a>> {
    let name = arg.name()?;
    let name_text = name.text(ctx.code.as_ref().as_ref()).to_string();
    let name_doc = print_ident(ctx, &name)?;

    // TODO: check logic with TS
    let mut parts = vec![name_doc];
    if let Some(val) = arg.value() {
        let val_text = val.text(ctx.code.as_ref().as_ref());
        if val_text != name_text {
            let val_doc = print_expression(ctx, &val)?;
            parts.push(RcDoc::text(": "));
            parts.push(val_doc);
        }
    }

    // In multiline literals we add a comma to each element
    // But in the single-line version, the comma is not needed for the last element
    parts.push(RcDoc::flat_alt(
        RcDoc::text(","),
        if is_last {
            RcDoc::nil()
        } else {
            RcDoc::text(",")
        },
    ));

    Some(RcDoc::concat(parts))
}

pub fn print_tensor_expression<'a>(ctx: &Context, tensor: &TensorExpression) -> Option<RcDoc<'a>> {
    let elements = tensor.elements();
    if elements.is_empty() {
        return Some(RcDoc::text("()"));
    }

    print_tuple_tensor(ctx, elements, "(", ")")
}

pub fn print_typed_tuple<'a>(ctx: &Context, tuple: &TypedTuple) -> Option<RcDoc<'a>> {
    let elements = tuple.elements();
    if elements.is_empty() {
        return Some(RcDoc::text("[]"));
    }

    print_tuple_tensor(ctx, elements, "[", "]")
}

fn print_tuple_tensor<'a>(
    ctx: &Context,
    elements: Vec<Expression>,
    open_quote: &'a str,
    close_quote: &'a str,
) -> Option<RcDoc<'a>> {
    if elements.is_empty() {
        return Some(RcDoc::concat([
            RcDoc::text(open_quote),
            RcDoc::text(close_quote),
        ]));
    }

    let mut docs = vec![RcDoc::line_()];
    for (i, el) in elements.iter().enumerate() {
        let node = el.raw_node();
        let comments = ctx.comments.get(&node);

        if comments::has_fmt_ignore(ctx, comments) {
            docs.push(common::print_original_node_text(ctx, &node));
        } else {
            comments::print_leading_comments(ctx, &mut docs, comments);

            docs.push(print_expression(ctx, el)?);

            let is_last = i == elements.len() - 1;
            if !is_last {
                docs.push(RcDoc::text(","));
            } else {
                docs.push(RcDoc::flat_alt(RcDoc::text(","), RcDoc::nil()));
            }

            comments::print_inline_comments(ctx, &mut docs, comments);

            if is_last {
                docs.push(RcDoc::line_());
            } else {
                docs.push(RcDoc::line());
            }

            comments::print_trailing_comments(ctx, &mut docs, comments);
        }

        if let Some(next) = elements.get(i + 1)
            && common::empty_lines_between(ctx, &node, &next.raw_node()) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text(open_quote),
        RcDoc::concat(docs).nest(4),
        RcDoc::text(close_quote),
    ])))
}

pub fn print_lambda_expression<'a>(ctx: &Context, lambda: &LambdaExpression) -> Option<RcDoc<'a>> {
    let params = lambda.parameters();
    let params_doc = crate::decls::print_parameter_list(ctx, &params)?;

    let mut docs = vec![RcDoc::text("fun"), params_doc];
    if let Some(ret) = lambda.return_type() {
        docs.push(RcDoc::text(": "));
        docs.push(types::print_type(ctx, &ret)?);
    }
    docs.push(RcDoc::space());
    if let Some(body) = lambda.body() {
        docs.push(stmts::print_block_statement(ctx, &body)?);
    }
    Some(RcDoc::concat(docs))
}

pub fn print_simple_node<'a>(ctx: &Context, node: &Node) -> Option<RcDoc<'a>> {
    let comments = ctx.comments.get(node);
    if comments.is_none() {
        // fast path for most of the identifier
        return common::print_node_text(ctx, node);
    }

    let mut docs = vec![];

    comments::print_leading_comments(ctx, &mut docs, comments);
    docs.push(common::print_node_text(ctx, node)?);
    comments::print_inline_comments(ctx, &mut docs, comments);

    Some(RcDoc::concat(docs))
}

pub fn print_ident<'a>(ctx: &Context, ident: &Ident) -> Option<RcDoc<'a>> {
    print_simple_node(ctx, &ident.0)
}
