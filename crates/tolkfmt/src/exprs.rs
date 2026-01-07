use pretty::RcDoc;
use tolk_ast::{
    Expression, Ident, MatchArm, MatchArmBody, MatchBody, MatchExpr, MatchExpression, MatchPattern,
    TernaryOperator,
};
use crate::Context;

pub fn print_expression<'a>(ctx: &mut Context, expr: &Expression) -> Option<RcDoc<'a>> {
    match expr {
        Expression::Assignment(assignment) => {
            let left = assignment.left()?;
            let right = assignment.right()?;
            let left_doc = print_expression(ctx, &left)?;
            let right_doc = print_expression(ctx, &right)?;
            Some(RcDoc::concat([left_doc, RcDoc::text(" = "), right_doc]))
        }
        Expression::SetAssignment(set_assignment) => {
            let left = set_assignment.left()?;
            let right = set_assignment.right()?;
            let op = set_assignment.operator_name(ctx.code.as_ref().as_ref()).to_owned();
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
        Expression::TernaryOperator(ternary) => print_ternary_operator(ctx, ternary),
        Expression::BinaryOperator(binary) => {
            let left = binary.left()?;
            let right = binary.right()?;
            let op = binary.operator_name(ctx.code.as_ref().as_ref()).to_owned();
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
        Expression::UnaryOperator(unary) => {
            let op = unary.operator_name(ctx.code.as_ref().as_ref()).to_owned();
            let arg = unary.argument()?;
            let arg_doc = print_expression(ctx, &arg)?;
            Some(RcDoc::concat([RcDoc::text(op), arg_doc]))
        }
        Expression::LazyExpression(lazy) => {
            let expr = lazy.expr()?;
            let expr_doc = print_expression(ctx, &expr)?;
            Some(RcDoc::concat([RcDoc::text("lazy "), expr_doc]))
        }
        Expression::CastAsOperator(cast) => {
            let expr = cast.expr()?;
            let typ = cast.casted_to()?;
            let expr_doc = print_expression(ctx, &expr)?;
            let type_doc = crate::types::print_type(ctx, &typ)?;
            Some(RcDoc::concat([
                expr_doc,
                RcDoc::text(" as "),
                type_doc,
            ]))
        }
        Expression::IsTypeOperator(is_type) => {
            let expr = is_type.expr()?;
            let op = is_type.operator_name(ctx.code.as_ref().as_ref()).to_owned();
            let rhs = is_type.rhs_type()?;
            let expr_doc = print_expression(ctx, &expr)?;
            let rhs_doc = crate::types::print_type(ctx, &rhs)?;
            Some(RcDoc::concat([
                expr_doc,
                RcDoc::text(" "),
                RcDoc::text(op),
                RcDoc::text(" "),
                rhs_doc,
            ]))
        }
        Expression::NotNullOperator(not_null) => {
            let inner = not_null.inner()?;
            let inner_doc = print_expression(ctx, &inner)?;
            Some(RcDoc::concat([inner_doc, RcDoc::text("!")]))
        }
        Expression::DotAccess(dot) => {
            let obj = dot.obj()?;
            let field = dot.field()?;
            let obj_doc = print_expression(ctx, &obj)?;
            let field_text = match field {
                tolk_ast::DotAccessField::Ident(i) => i.text(ctx.code.as_ref().as_ref()).to_string(),
                tolk_ast::DotAccessField::NumericIndex(n) => {
                    n.value(ctx.code.as_ref().as_ref()).to_string()
                }
            };
            Some(RcDoc::concat([obj_doc, RcDoc::text("."), RcDoc::text(field_text)]))
        }
        Expression::FunctionCall(call) => {
            let callee = call.callee()?;
            let callee_doc = print_expression(ctx, &callee)?;
            let args = call.arguments();
            let mut arg_docs = vec![];
            for (i, arg) in args.iter().enumerate() {
                if i > 0 {
                    arg_docs.push(RcDoc::text(", "));
                }
                if arg.mutate() {
                    arg_docs.push(RcDoc::text("mutate "));
                }
                if let Some(expr) = arg.expr() {
                    arg_docs.push(print_expression(ctx, &expr)?);
                }
            }
            Some(RcDoc::concat([
                callee_doc,
                RcDoc::text("("),
                RcDoc::group(RcDoc::concat([
                    RcDoc::line_(),
                    RcDoc::concat(arg_docs),
                ])).nest(4),
                RcDoc::line_(),
                RcDoc::text(")"),
            ]))
        }
        Expression::GenericInstantiation(r#gen) => {
            let expr = r#gen.expr()?;
            let expr_doc = print_expression(ctx, &expr)?;
            let ts = r#gen.instantiation_ts()?;
            let types = ts.types();
            let mut type_docs = vec![];
            for (i, typ) in types.iter().enumerate() {
                if i > 0 {
                    type_docs.push(RcDoc::text(", "));
                }
                type_docs.push(crate::types::print_type(ctx, typ)?);
            }
            Some(RcDoc::concat([
                expr_doc,
                RcDoc::text("<"),
                RcDoc::concat(type_docs),
                RcDoc::text(">"),
            ]))
        }
        Expression::ParenthesizedExpression(paren) => {
            let inner = paren.inner()?;
            let inner_doc = print_expression(ctx, &inner)?;
            Some(RcDoc::concat([RcDoc::text("("), inner_doc, RcDoc::text(")")]))
        }
        Expression::MatchExpression(match_expr) => print_match_expression(ctx, match_expr),
        Expression::ObjectLiteral(obj) => {
            let typ = obj.typ();
            let mut docs = vec![];
            if let Some(typ) = typ {
                docs.push(crate::types::print_type(ctx, &typ)?);
                docs.push(RcDoc::space());
            }
            docs.push(RcDoc::text("{"));
            let args = obj.arguments();
            if args.is_empty() {
                docs.push(RcDoc::text("}"));
            } else {
                let mut arg_docs = vec![];
                for (i, arg) in args.iter().enumerate() {
                    let name = arg.name()?;
                    let name_doc = print_ident(ctx, &name)?;
                    let mut parts = vec![name_doc];
                    if let Some(val) = arg.value() {
                        let val_doc = print_expression(ctx, &val)?;
                        parts.push(RcDoc::text(": "));
                        parts.push(val_doc);
                    }
                    parts.push(RcDoc::text(","));
                    arg_docs.push(RcDoc::concat([
                        RcDoc::hardline(),
                        RcDoc::concat(parts),
                    ]));
                }
                docs.push(RcDoc::concat(arg_docs).nest(4));
                docs.push(RcDoc::hardline());
                docs.push(RcDoc::text("}"));
            }
            Some(RcDoc::concat(docs))
        }
        Expression::TensorExpression(tensor) => {
            let elements = tensor.elements();
            let mut docs = vec![];
            for (i, el) in elements.iter().enumerate() {
                if i > 0 {
                    docs.push(RcDoc::text(", "));
                }
                docs.push(print_expression(ctx, el)?);
            }
            if docs.is_empty() {
                return Some(RcDoc::text("()"));
            }
            if docs.len() == 1 {
                return Some(RcDoc::concat([RcDoc::text("("), docs[0].clone(), RcDoc::text(")")]));
            }
            Some(RcDoc::group(RcDoc::concat([
                RcDoc::text("("),
                RcDoc::concat([RcDoc::line_(), RcDoc::concat(docs)]).nest(4),
                RcDoc::line_(),
                RcDoc::text(")"),
            ])))
        }
        Expression::TypedTuple(tuple) => {
            let elements = tuple.elements();
            let mut docs = vec![];
            for (i, el) in elements.iter().enumerate() {
                if i > 0 {
                    docs.push(RcDoc::text(", "));
                }
                docs.push(print_expression(ctx, el)?);
            }
            if docs.is_empty() {
                return Some(RcDoc::text("[]"));
            }
            if docs.len() == 1 {
                return Some(RcDoc::concat([RcDoc::text("["), docs[0].clone(), RcDoc::text("]")]));
            }
            Some(RcDoc::group(RcDoc::concat([
                RcDoc::text("["),
                RcDoc::concat([RcDoc::line_(), RcDoc::concat(docs)]).nest(4),
                RcDoc::line_(),
                RcDoc::text("]"),
            ])))
        }
        Expression::LambdaExpression(lambda) => {
            let params = lambda.parameters();
            let mut param_docs = vec![];
            for (i, p) in params.iter().enumerate() {
                if i > 0 {
                    param_docs.push(RcDoc::text(", "));
                }
                if p.mutate() {
                    param_docs.push(RcDoc::text("mutate "));
                }
                let name = p.name()?;
                param_docs.push(print_ident(ctx, &name)?);
                if let Some(typ) = p.typ() {
                    param_docs.push(RcDoc::text(": "));
                    param_docs.push(crate::types::print_type(ctx, &typ)?);
                }
            }
            let mut docs = vec![
                RcDoc::text("fun("),
                RcDoc::concat(param_docs),
                RcDoc::text(")"),
            ];
            if let Some(ret) = lambda.return_type() {
                docs.push(RcDoc::text(": "));
                docs.push(crate::types::print_type(ctx, &ret)?);
            }
            docs.push(RcDoc::space());
            if let Some(body) = lambda.body() {
                docs.push(crate::stmts::print_block_statement(ctx, &body)?);
            }
            Some(RcDoc::concat(docs))
        }
        Expression::NumberLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::StringLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::BooleanLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::NullLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::Underscore(_) => Some(RcDoc::text("_")),
        Expression::Ident(ident) => print_ident(ctx, ident),
        Expression::NumericIndex(index) => crate::common::print_node_text(ctx, &index.0),
        Expression::Unmapped(node) => crate::common::print_node_text(ctx, &node.0),
    }
}

pub fn print_match_expression<'a>(
    ctx: &mut Context,
    match_expr: &MatchExpression,
) -> Option<RcDoc<'a>> {
    let expr = match_expr.expr()?;
    let body = match_expr.body()?;

    let expr_doc = match expr {
        MatchExpr::Expression(e) => print_expression(ctx, &e)?,
        MatchExpr::LocalVarsDeclaration(l) => crate::stmts::print_local_variables(ctx, &l)?,
    };

    let body_doc = print_match_body(ctx, &body)?;

    Some(RcDoc::concat([
        RcDoc::text("match ("),
        expr_doc,
        RcDoc::text(") "),
        body_doc,
    ]))
}

fn print_match_body<'a>(ctx: &mut Context, body: &MatchBody) -> Option<RcDoc<'a>> {
    let arms = body.arms();
    if arms.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let mut arm_docs = vec![];
    for arm in arms {
        arm_docs.push(RcDoc::hardline());
        arm_docs.push(print_match_arm(ctx, &arm)?);
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat(arm_docs).nest(4),
        RcDoc::hardline(),
        RcDoc::text("}"),
    ]))
}

fn print_match_arm<'a>(ctx: &mut Context, arm: &MatchArm) -> Option<RcDoc<'a>> {
    let pattern = arm.pattern();
    let body = arm.body()?;

    let pattern_doc = match pattern {
        MatchPattern::Type(t) => crate::types::print_type(ctx, &t)?,
        MatchPattern::Expression(e) => print_expression(ctx, &e)?,
        MatchPattern::Else => RcDoc::text("else"),
    };

    let (body_doc, is_block) = match body {
        MatchArmBody::BlockStatement(b) => (crate::stmts::print_block_statement(ctx, &b)?, true),
        MatchArmBody::ReturnStatement(r) => (crate::stmts::print_return_statement(ctx, &r)?, false),
        MatchArmBody::ThrowStatement(t) => (crate::stmts::print_throw_statement(ctx, &t)?, false),
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

fn print_ternary_operator<'a>(ctx: &mut Context, ternary: &TernaryOperator) -> Option<RcDoc<'a>> {
    let condition = ternary.condition()?;
    let consequence = ternary.consequence()?;
    let alternative = ternary.alternative()?;

    let condition_doc = print_expression(ctx, &condition)?;
    let consequence_doc = print_expression(ctx, &consequence)?;
    let alternative_doc = print_expression(ctx, &alternative)?;

    Some(RcDoc::concat([
        condition_doc,
        RcDoc::text(" ? "),
        consequence_doc,
        RcDoc::text(" : "),
        alternative_doc,
    ]))
}

pub fn print_ident<'a>(ctx: &mut Context, ident: &Ident) -> Option<RcDoc<'a>> {
    crate::common::print_node_text(ctx, &ident.0)
}