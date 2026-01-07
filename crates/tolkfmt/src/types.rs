use crate::{Context, common};
use pretty::RcDoc;
use tolk_ast::Type;

pub fn print_type<'a>(ctx: &mut Context, typ: &Type) -> Option<RcDoc<'a>> {
    match typ {
        Type::TypeIdentifier(ident) => common::print_node_text(ctx, &ident.0),
        Type::TypeInstantiatedTs(_) => todo!(),
        Type::TensorType(_) => todo!(),
        Type::TupleType(_) => todo!(),
        Type::ParenthesizedType(_) => todo!(),
        Type::FunCallableType(_) => todo!(),
        Type::NullableType(_) => todo!(),
        Type::UnionType(_) => todo!(),
        Type::NullLiteral(_) => todo!(),
        Type::Unmapped(_) => todo!(),
    }
}
