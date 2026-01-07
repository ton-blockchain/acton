use crate::{Context, common, stmts, types};
use pretty::RcDoc;
use tolk_ast::*;

pub fn print_expression<'a>(ctx: &Context, expr: &Expression) -> Option<RcDoc<'a>> {
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
        Expression::NumberLiteral(lit) => print_number_literal(ctx, lit),
        Expression::StringLiteral(lit) => print_string_literal(ctx, lit),
        Expression::BooleanLiteral(lit) => print_boolean_literal(ctx, lit),
        Expression::NullLiteral(lit) => print_null_literal(ctx, lit),
        Expression::Underscore(und) => print_underscore(ctx, und),
        Expression::Ident(ident) => print_ident(ctx, ident),
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

pub fn print_unary_operator<'a>(
    ctx: &Context,
    unary: &tolk_ast::UnaryOperator,
) -> Option<RcDoc<'a>> {
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
    let field_text = match field {
        DotAccessField::Ident(i) => i.text(ctx.code.as_ref().as_ref()).to_string(),
        DotAccessField::NumericIndex(n) => n.value(ctx.code.as_ref().as_ref()).to_string(),
    };

    let is_obj_literal = matches!(obj, Expression::ObjectLiteral(_));

    if is_obj_literal {
        Some(RcDoc::group(RcDoc::concat([
            obj_doc,
            RcDoc::text("."),
            RcDoc::text(field_text),
        ])))
    } else {
        Some(RcDoc::group(RcDoc::concat([
            obj_doc,
            RcDoc::concat([
                RcDoc::softline_(),
                RcDoc::text("."),
                RcDoc::text(field_text),
            ])
            .nest(4),
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

    let mut arg_docs = vec![];
    for arg in args {
        arg_docs.push(print_call_argument(ctx, arg)?);
    }

    if args.len() == 1
        && let Some(single) = arg_docs.first()
    {
        return Some(RcDoc::concat([
            RcDoc::text("("),
            single.clone(),
            RcDoc::text(")"),
        ]));
    }

    let (first, rest) = arg_docs.split_first()?;
    let mut tail_docs = vec![];
    for part in rest {
        tail_docs.push(RcDoc::text(","));
        tail_docs.push(RcDoc::line());
        tail_docs.push(part.clone());
    }

    // Add trailing comma for multiline calls
    tail_docs.push(RcDoc::text(",").flat_alt(RcDoc::nil()));

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text("("),
        RcDoc::concat([RcDoc::line_(), first.clone(), RcDoc::concat(tail_docs)]).nest(4),
        RcDoc::line_(),
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

    let mut type_docs = vec![];
    for (i, typ) in types.iter().enumerate() {
        if i > 0 {
            type_docs.push(RcDoc::text(", "));
        }
        type_docs.push(types::print_type(ctx, typ)?);
    }

    Some(RcDoc::concat([
        expr_doc,
        RcDoc::text("<"),
        RcDoc::concat(type_docs),
        RcDoc::text(">"),
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

    let mut arm_docs = vec![RcDoc::hardline()];
    for (i, arm) in arms.iter().enumerate() {
        arm_docs.push(print_match_arm(ctx, arm)?);

        if i != arms.len() - 1 {
            arm_docs.push(RcDoc::hardline());
        }

        // Между arms может быть пустая строка которую мы хотим сохранить
        if let Some(next) = arms.get(i + 1)
            && common::empty_lines_between(&arm.0, &next.0) > 1
        {
            arm_docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat(arm_docs).nest(4),
        RcDoc::hardline(),
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

    let is_multiline = args.len() > 2;
    let separator = if is_multiline {
        RcDoc::hardline()
    } else {
        RcDoc::line()
    };

    let mut arg_docs = vec![separator.clone()];
    for (i, arg) in args.iter().enumerate() {
        let is_last = i == (args.len() - 1);
        arg_docs.push(print_instance_argument(ctx, arg, is_last)?);

        if i != args.len() - 1 {
            arg_docs.push(separator.clone());
        }

        // Между args может быть пустая строка которую мы хотим сохранить
        if let Some(next) = args.get(i + 1)
            && common::empty_lines_between(&arg.0, &next.0) > 1
        {
            arg_docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat(arg_docs).nest(4),
        separator,
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

    // В многострочном литерале мы добавляем запятую к каждому элементу
    // Но в однострочном варианте запятая у последнего элемента не нужна
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
    let mut docs = vec![];
    for el in elements.iter() {
        docs.push(print_expression(ctx, el)?);
    }

    if docs.len() == 1
        && let Some(single) = docs.first()
    {
        return Some(RcDoc::concat([
            RcDoc::text(open_quote),
            single.clone(),
            RcDoc::text(close_quote),
        ]));
    }

    let (first, rest) = docs.split_first()?;
    let mut tail_docs = vec![];
    for part in rest {
        tail_docs.push(RcDoc::text(","));
        tail_docs.push(RcDoc::line());
        tail_docs.push(part.clone());
    }

    // Add trailing comma for multiline tuples/tensors
    tail_docs.push(RcDoc::text(",").flat_alt(RcDoc::nil()));

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text(open_quote),
        RcDoc::concat([RcDoc::line_(), first.clone(), RcDoc::concat(tail_docs)]).nest(4),
        RcDoc::line_(),
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

pub fn print_number_literal<'a>(ctx: &Context, lit: &NumberLiteral) -> Option<RcDoc<'a>> {
    common::print_node_text(ctx, &lit.0)
}

pub fn print_string_literal<'a>(ctx: &Context, lit: &StringLiteral) -> Option<RcDoc<'a>> {
    common::print_node_text(ctx, &lit.0)
}

pub fn print_boolean_literal<'a>(ctx: &Context, lit: &BooleanLiteral) -> Option<RcDoc<'a>> {
    common::print_node_text(ctx, &lit.0)
}

pub fn print_null_literal<'a>(ctx: &Context, lit: &tolk_ast::NullLiteral) -> Option<RcDoc<'a>> {
    common::print_node_text(ctx, &lit.0)
}

pub fn print_underscore<'a>(_ctx: &Context, _und: &Underscore) -> Option<RcDoc<'a>> {
    Some(RcDoc::text("_"))
}

pub fn print_ident<'a>(ctx: &Context, ident: &Ident) -> Option<RcDoc<'a>> {
    common::print_node_text(ctx, &ident.0)
}
