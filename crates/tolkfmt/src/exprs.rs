use pretty::RcDoc;
use tolk_ast::{Expression, Ident, TernaryOperator};
use crate::Context;

pub fn print_expression<'a>(ctx: &mut Context, expr: &Expression) -> Option<RcDoc<'a>> {
    match expr {
        Expression::Assignment(_) => todo!(),
        Expression::SetAssignment(_) => todo!(),
        Expression::TernaryOperator(ternary) => print_ternary_operator(ctx, ternary),
        Expression::BinaryOperator(_) => todo!(),
        Expression::UnaryOperator(_) => todo!(),
        Expression::LazyExpression(_) => todo!(),
        Expression::CastAsOperator(_) => todo!(),
        Expression::IsTypeOperator(_) => todo!(),
        Expression::NotNullOperator(_) => todo!(),
        Expression::DotAccess(_) => todo!(),
        Expression::FunctionCall(_) => todo!(),
        Expression::GenericInstantiation(_) => todo!(),
        Expression::ParenthesizedExpression(_) => todo!(),
        Expression::MatchExpression(_) => todo!(),
        Expression::ObjectLiteral(_) => todo!(),
        Expression::TensorExpression(_) => todo!(),
        Expression::TypedTuple(_) => todo!(),
        Expression::LambdaExpression(_) => todo!(),
        Expression::NumberLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::StringLiteral(_) => todo!(),
        Expression::BooleanLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::NullLiteral(lit) => crate::common::print_node_text(ctx, &lit.0),
        Expression::Underscore(_) => Some(RcDoc::text("_")),
        Expression::Ident(ident) => print_ident(ctx, ident),
        Expression::NumericIndex(index) => crate::common::print_node_text(ctx, &index.0),
        Expression::Unmapped(node) => crate::common::print_node_text(ctx, &node.0),
    }
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