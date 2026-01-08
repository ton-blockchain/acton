use crate::{Context, comments, common};
use pretty::RcDoc;
use tolk_ast::*;

pub fn print_type<'a>(ctx: &Context, typ: &Type) -> Option<RcDoc<'a>> {
    let node = typ.raw_node();
    let comments = ctx.comments.get(&node);

    if comments.is_none() {
        return print_type_naked(ctx, typ);
    }

    let mut docs = vec![];
    comments::print_leading_comments(ctx, &mut docs, comments);

    let doc = print_type_naked(ctx, typ)?;
    docs.push(doc);

    comments::print_inline_comments(ctx, &mut docs, comments);

    Some(RcDoc::concat(docs))
}

fn print_type_naked<'a>(ctx: &Context, typ: &Type) -> Option<RcDoc<'a>> {
    match typ {
        Type::TypeIdentifier(ident) => Some(common::print_node_text(ctx, &ident.0)?),
        Type::TypeInstantiatedTs(inst) => print_type_instantiated_ts(ctx, inst),
        Type::TensorType(tensor) => print_tensor_type(ctx, tensor),
        Type::TupleType(tuple) => print_tuple_type(ctx, tuple),
        Type::ParenthesizedType(paren) => print_parenthesized_type(ctx, paren),
        Type::FunCallableType(fun) => print_fun_callable_type(ctx, fun),
        Type::NullableType(nullable) => print_nullable_type(ctx, nullable),
        Type::UnionType(union) => print_union_type(ctx, union),
        Type::NullLiteral(null) => Some(common::print_node_text(ctx, &null.0)?),
        Type::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

pub fn print_union_type<'a>(ctx: &Context, union: &UnionType) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    collect_union_parts(union, &mut parts);

    let mut parts_docs = vec![];
    for part in parts {
        parts_docs.push(print_type(ctx, &part)?);
    }

    let (first, rest) = parts_docs.split_first()?;

    let in_type_alias = union.0.parent().is_some_and(|p| {
        let kind = p.kind();
        kind == "type_alias_declaration"
    });

    // we add `|` only for
    // ```
    // type Foo =
    //     | Bar
    //     | Baz
    //     | Boo
    // ```
    //
    // but
    // ```
    // dest: int
    //     | slice
    //     | address
    // ```
    let multiline_prefix = if in_type_alias { "    | " } else { "" };

    let first_doc = RcDoc::concat([
        RcDoc::flat_alt(RcDoc::text(multiline_prefix), RcDoc::nil()),
        first.clone(),
    ]);

    let mut tail_docs = vec![];
    for doc in rest {
        tail_docs.push(RcDoc::line());
        tail_docs.push(RcDoc::text("| "));
        tail_docs.push(doc.clone());
    }

    Some(RcDoc::group(
        RcDoc::concat([RcDoc::softline_(), first_doc, RcDoc::concat(tail_docs)]).nest(4),
    ))
}

fn collect_union_parts<'tree>(union: &UnionType<'tree>, parts: &mut Vec<Type<'tree>>) {
    if let Some(lhs) = union.lhs() {
        parts.push(lhs);
    }
    if let Some(rhs) = union.rhs() {
        match rhs {
            Type::UnionType(inner_union) => collect_union_parts(&inner_union, parts),
            _ => parts.push(rhs),
        }
    }
}

pub fn print_nullable_type<'a>(ctx: &Context, nullable: &NullableType) -> Option<RcDoc<'a>> {
    let inner = nullable.inner()?;
    let inner_doc = print_type(ctx, &inner)?;
    Some(inner_doc.append(RcDoc::text("?")))
}

pub fn print_parenthesized_type<'a>(ctx: &Context, paren: &ParenthesizedType) -> Option<RcDoc<'a>> {
    let inner = paren.inner()?;
    let inner_doc = print_type(ctx, &inner)?;
    Some(RcDoc::concat([
        RcDoc::text("("),
        inner_doc,
        RcDoc::text(")"),
    ]))
}

pub fn print_tensor_type<'a>(ctx: &Context, tensor: &TensorType) -> Option<RcDoc<'a>> {
    let elements = tensor.element_types();
    print_tuple_tensor_type(ctx, elements, "(", ")")
}

pub fn print_tuple_type<'a>(ctx: &Context, tuple: &TupleType) -> Option<RcDoc<'a>> {
    let elements = tuple.element_types();
    print_tuple_tensor_type(ctx, elements, "[", "]")
}

fn print_tuple_tensor_type<'a>(
    ctx: &Context,
    elements: Vec<Type>,
    open_quote: &'a str,
    close_quote: &'a str,
) -> Option<RcDoc<'a>> {
    common::print_list(
        ctx,
        &elements,
        print_type,
        Type::raw_node,
        common::ListOptions {
            brackets: (RcDoc::text(open_quote), RcDoc::text(close_quote)),
            ..Default::default()
        },
    )
}

pub fn print_fun_callable_type<'a>(ctx: &Context, fun: &FunCallableType) -> Option<RcDoc<'a>> {
    let params = fun.param_types()?;
    let ret = fun.return_type()?;

    let params_doc = print_type(ctx, &params)?;
    let ret_doc = print_type(ctx, &ret)?;

    Some(RcDoc::concat([params_doc, RcDoc::text(" -> "), ret_doc]))
}

pub fn print_type_instantiated_ts<'a>(
    ctx: &Context,
    inst: &TypeInstantiatedTs,
) -> Option<RcDoc<'a>> {
    let name = inst.name()?;
    let name_doc = common::print_node_text(ctx, &name.0)?;
    let args = inst.arguments()?;
    let types = args.types();

    let types_doc = common::print_list(
        ctx,
        &types,
        print_type,
        Type::raw_node,
        common::ListOptions::triangle_bracket_list(),
    )?;

    Some(RcDoc::concat([name_doc, types_doc]))
}
