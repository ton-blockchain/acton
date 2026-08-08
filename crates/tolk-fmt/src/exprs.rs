use crate::comments::has_inline_line_comment_in_subtree;
use crate::pretty::RcDoc;
use crate::{Context, comments, common, stmts, types};
use tolk_syntax::{
    ArgumentList, AsCast, Assign, AstNode, Bin, Call, CallArgument, DotAccess, DotAccessField,
    Expr, HasName, Ident, InstanceArg, Instantiation, IsType, Lambda, Lazy, Match, MatchArm,
    MatchArmBody, MatchBody, MatchPattern, NotNull, ObjectLit, Paren, SetAssign, Tensor, Ternary,
    Tuple, Type, Unary, VarDeclLhs, VarDeclPattern,
};
use tree_sitter::Node;

#[must_use]
pub fn print_expression<'a>(ctx: &Context<'_>, expr: &Expr) -> Option<RcDoc<'a>> {
    let node = expr.syntax();
    if !common::should_format_node(ctx, &node) {
        return Some(common::print_original_node_text_inline(ctx, &node));
    }

    // TODO: other literals as well
    if let Expr::NumberLit(lit) = expr {
        let kind = lit.0.parent()?.kind();
        if kind == "tensor_expression"
            || kind == "tuple_expression"
            || kind == "typed_tuple"
            || kind == "annotation_arguments"
        {
            return print_expression_naked(ctx, expr);
        }
    }

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

struct MethodChainDoc<'a> {
    base: RcDoc<'a>,
    links: Vec<MethodChainLink<'a>>,
    has_dot_link: bool,
    base_is_object_lit: bool,
}

struct MethodChainLink<'a> {
    doc: RcDoc<'a>,
    has_leading_comments: bool,
    has_inline_line_comments: bool,
}

fn wrap_doc_with_node_comments<'a>(
    ctx: &Context<'_>,
    node: Node<'_>,
    doc: RcDoc<'a>,
    include_comments: bool,
) -> RcDoc<'a> {
    if !include_comments {
        return doc;
    }

    let comments = ctx.comments.get(&node);
    if comments.is_none() {
        return doc;
    }

    let mut docs = vec![];
    comments::print_leading_comments(ctx, &mut docs, comments);
    docs.push(doc);
    comments::print_inline_comments(ctx, &mut docs, comments);
    RcDoc::concat(docs)
}

fn has_leading_comments_on_node(ctx: &Context<'_>, node: Node<'_>) -> bool {
    ctx.comments.get(&node).is_some_and(|comments| {
        comments.iter().any(|comment| {
            matches!(
                comment.kind,
                comments::CommentKind::Leading | comments::CommentKind::LeadingWithEmptyLine
            )
        })
    })
}

fn has_inline_line_comments_on_node(ctx: &Context<'_>, node: Node<'_>) -> bool {
    ctx.comments.get(&node).is_some_and(|comments| {
        comments.iter().any(|comment| {
            comment.kind == comments::CommentKind::Inline
                && comment.nodes.iter().any(|comment_node| {
                    comment_node
                        .utf8_text(ctx.code.as_ref().as_ref())
                        .ok()
                        .is_some_and(|text| text.trim_start().starts_with("//"))
                })
        })
    })
}

fn print_node_with_detached_leading_comments<'a>(
    ctx: &Context<'_>,
    node: Node<'_>,
) -> Option<(RcDoc<'a>, RcDoc<'a>, bool, bool)> {
    let comments = ctx.comments.get(&node);
    if comments.is_none() {
        return Some((
            RcDoc::nil(),
            common::print_node_text(ctx, &node)?,
            false,
            false,
        ));
    }

    let mut leading_docs = vec![];
    comments::print_leading_comments(ctx, &mut leading_docs, comments);

    let mut body_docs = vec![common::print_node_text(ctx, &node)?];
    comments::print_inline_comments(ctx, &mut body_docs, comments);

    let has_leading_comments = !leading_docs.is_empty();
    let has_inline_line_comments = has_inline_line_comments_on_node(ctx, node);
    Some((
        RcDoc::concat(leading_docs),
        RcDoc::concat(body_docs),
        has_leading_comments,
        has_inline_line_comments,
    ))
}

fn collect_method_chain_doc<'a>(
    ctx: &Context<'_>,
    expr: &Expr,
    include_expr_comments: bool,
) -> Option<MethodChainDoc<'a>> {
    match expr {
        Expr::DotAccess(dot) => {
            let obj = dot.obj()?;
            let mut chain = collect_method_chain_doc(ctx, &obj, true)?;

            let (
                field_leading_comments_doc,
                field_doc,
                field_has_leading_comments,
                field_has_inline_line_comments,
            ) = match dot.field()? {
                DotAccessField::Ident(i) => print_node_with_detached_leading_comments(ctx, i.0)?,
                DotAccessField::NumericIndex(n) => {
                    print_node_with_detached_leading_comments(ctx, n.0)?
                }
            };

            let dot_has_leading_comments =
                include_expr_comments && has_leading_comments_on_node(ctx, dot.syntax());
            let dot_has_inline_line_comments =
                include_expr_comments && has_inline_line_comments_on_node(ctx, dot.syntax());
            let has_leading_comments = field_has_leading_comments || dot_has_leading_comments;
            let has_inline_line_comments =
                field_has_inline_line_comments || dot_has_inline_line_comments;

            let link_doc = if field_has_leading_comments {
                RcDoc::concat([field_leading_comments_doc, RcDoc::text("."), field_doc])
            } else {
                RcDoc::concat([RcDoc::text("."), field_doc])
            };

            let dot_doc =
                wrap_doc_with_node_comments(ctx, dot.syntax(), link_doc, include_expr_comments);

            chain.links.push(MethodChainLink {
                doc: dot_doc,
                has_leading_comments,
                has_inline_line_comments,
            });
            chain.has_dot_link = true;
            Some(chain)
        }
        Expr::Call(call) => {
            let callee = call.callee()?;
            let mut chain = collect_method_chain_doc(ctx, &callee, true)?;
            let args: Vec<_> = call.arguments().collect();
            let args_doc = print_argument_list(ctx, &args, call.0.field("arguments"))?;

            if chain.has_dot_link {
                if let Some(last) = chain.links.last_mut() {
                    last.doc = wrap_doc_with_node_comments(
                        ctx,
                        call.syntax(),
                        last.doc.clone().append(args_doc),
                        include_expr_comments,
                    );
                    let call_has_leading_comments =
                        include_expr_comments && has_leading_comments_on_node(ctx, call.syntax());
                    last.has_leading_comments |= call_has_leading_comments;
                    let call_has_inline_line_comments = include_expr_comments
                        && has_inline_line_comments_on_node(ctx, call.syntax());
                    last.has_inline_line_comments |= call_has_inline_line_comments;
                } else {
                    chain.base = chain.base.append(args_doc);
                }
                Some(chain)
            } else {
                let callee_doc = print_expression(ctx, &callee)?;
                let call_doc = wrap_doc_with_node_comments(
                    ctx,
                    call.syntax(),
                    RcDoc::concat([callee_doc, args_doc]),
                    include_expr_comments,
                );
                Some(MethodChainDoc {
                    base: call_doc,
                    links: vec![],
                    has_dot_link: false,
                    base_is_object_lit: false,
                })
            }
        }
        Expr::Instantiation(instantiation) => {
            let expr = instantiation.expr()?;
            let mut chain = collect_method_chain_doc(ctx, &expr, true)?;
            let types_doc = print_instantiation_types(ctx, instantiation)?;

            if chain.has_dot_link {
                if let Some(last) = chain.links.last_mut() {
                    last.doc = wrap_doc_with_node_comments(
                        ctx,
                        instantiation.syntax(),
                        last.doc.clone().append(types_doc),
                        include_expr_comments,
                    );
                    let instantiation_has_leading_comments = include_expr_comments
                        && has_leading_comments_on_node(ctx, instantiation.syntax());
                    last.has_leading_comments |= instantiation_has_leading_comments;
                    let instantiation_has_inline_line_comments = include_expr_comments
                        && has_inline_line_comments_on_node(ctx, instantiation.syntax());
                    last.has_inline_line_comments |= instantiation_has_inline_line_comments;
                } else {
                    chain.base = chain.base.append(types_doc);
                }
                Some(chain)
            } else {
                let expr_doc = print_expression(ctx, &expr)?;
                let instantiation_doc = wrap_doc_with_node_comments(
                    ctx,
                    instantiation.syntax(),
                    RcDoc::concat([expr_doc, types_doc]),
                    include_expr_comments,
                );
                Some(MethodChainDoc {
                    base: instantiation_doc,
                    links: vec![],
                    has_dot_link: false,
                    base_is_object_lit: matches!(expr, Expr::ObjectLit(_)),
                })
            }
        }
        Expr::NotNull(not_null) => {
            let inner = not_null.inner()?;
            let mut chain = collect_method_chain_doc(ctx, &inner, true)?;

            if chain.has_dot_link {
                if let Some(last) = chain.links.last_mut() {
                    last.doc = wrap_doc_with_node_comments(
                        ctx,
                        not_null.syntax(),
                        last.doc.clone().append(RcDoc::text("!")),
                        include_expr_comments,
                    );
                    let not_null_has_leading_comments = include_expr_comments
                        && has_leading_comments_on_node(ctx, not_null.syntax());
                    last.has_leading_comments |= not_null_has_leading_comments;
                    let not_null_has_inline_line_comments = include_expr_comments
                        && has_inline_line_comments_on_node(ctx, not_null.syntax());
                    last.has_inline_line_comments |= not_null_has_inline_line_comments;
                } else {
                    chain.base = chain.base.append(RcDoc::text("!"));
                }
                Some(chain)
            } else {
                let inner_doc = print_expression(ctx, &inner)?;
                let not_null_doc = wrap_doc_with_node_comments(
                    ctx,
                    not_null.syntax(),
                    RcDoc::concat([inner_doc, RcDoc::text("!")]),
                    include_expr_comments,
                );
                Some(MethodChainDoc {
                    base: not_null_doc,
                    links: vec![],
                    has_dot_link: false,
                    base_is_object_lit: false,
                })
            }
        }
        _ => Some(MethodChainDoc {
            base: print_expression(ctx, expr)?,
            links: vec![],
            has_dot_link: false,
            base_is_object_lit: matches!(expr, Expr::ObjectLit(_)),
        }),
    }
}

fn print_method_chain_expression<'a>(ctx: &Context<'_>, expr: &Expr) -> Option<RcDoc<'a>> {
    // Root expression comments are handled by `print_expression` wrapper.
    let chain = collect_method_chain_doc(ctx, expr, false)?;
    if !chain.has_dot_link {
        return Some(chain.base);
    }

    let keep_single_link_attached = chain.links.len() == 1;

    let mut tail = Vec::with_capacity(chain.links.len() * 3);
    let mut previous_has_inline_line_comments = false;
    for (index, link) in chain.links.into_iter().enumerate() {
        let separator = if previous_has_inline_line_comments || link.has_leading_comments {
            RcDoc::hardline()
        } else if index == 0 && (chain.base_is_object_lit || keep_single_link_attached) {
            RcDoc::nil()
        } else {
            RcDoc::line_()
        };
        tail.push(separator);
        // If a part of the chain breaks, force parent groups to break too.
        tail.push(RcDoc::break_parent().flat_alt(RcDoc::nil()));
        previous_has_inline_line_comments = link.has_inline_line_comments;
        tail.push(link.doc);
    }

    let tail_doc = if keep_single_link_attached {
        RcDoc::concat(tail)
    } else {
        RcDoc::concat(tail).nest(4)
    };

    Some(RcDoc::group(RcDoc::concat([chain.base, tail_doc])))
}

fn print_expression_naked<'a>(ctx: &Context<'_>, expr: &Expr) -> Option<RcDoc<'a>> {
    match expr {
        Expr::VarDeclLhs(node) => print_var_declaration_lhs(ctx, node),
        Expr::Assign(assignment) => print_assignment(ctx, assignment),
        Expr::SetAssign(set_assignment) => print_set_assignment(ctx, set_assignment),
        Expr::Ternary(ternary) => print_ternary_operator(ctx, ternary),
        Expr::Bin(binary) => print_binary_operator(ctx, binary),
        Expr::Unary(unary) => print_unary_operator(ctx, unary),
        Expr::Lazy(lazy) => print_lazy_expression(ctx, lazy),
        Expr::AsCast(cast) => print_cast_as_operator(ctx, cast),
        Expr::IsType(is_type) => print_is_type_operator(ctx, is_type),
        Expr::NotNull(not_null) => print_not_null_operator(ctx, not_null),
        Expr::DotAccess(_) | Expr::Call(_) => print_method_chain_expression(ctx, expr),
        Expr::Instantiation(r#gen) => print_generic_instantiation(ctx, r#gen),
        Expr::Paren(paren) => print_parenthesized_expression(ctx, paren),
        Expr::Match(match_expr) => print_match_expression(ctx, match_expr),
        Expr::ObjectLit(obj) => print_object_literal(ctx, obj),
        Expr::Tensor(tensor) => print_tensor_expression(ctx, tensor),
        Expr::Tuple(tuple) => print_typed_tuple(ctx, tuple),
        Expr::Lambda(lambda) => print_lambda_expression(ctx, lambda),
        Expr::NumberLit(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expr::StringLit(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expr::BoolLit(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expr::NullLit(lit) => Some(common::print_node_text(ctx, &lit.0)?),
        Expr::Underscore(und) => Some(common::print_node_text(ctx, &und.0)?),
        Expr::Ident(ident) => Some(common::print_node_text(ctx, &ident.0)?),
        Expr::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

#[must_use]
pub fn print_var_declaration_lhs<'a>(ctx: &Context<'_>, node: &VarDeclLhs) -> Option<RcDoc<'a>> {
    let kind = node.kind();
    let pattern = node.pattern()?;
    let pattern_doc = print_var_declaration_pattern(ctx, &pattern)?;

    Some(RcDoc::concat([
        RcDoc::text(kind.as_str()),
        RcDoc::space(),
        pattern_doc,
    ]))
}

fn print_var_declaration_pattern<'a>(
    ctx: &Context<'_>,
    pattern: &VarDeclPattern,
) -> Option<RcDoc<'a>> {
    match pattern {
        VarDeclPattern::TupleVars(tuple) => {
            let vars: Vec<_> = tuple.vars().collect();
            print_tensor_tuple_pattern(ctx, &vars, "[", "]")
        }
        VarDeclPattern::TensorVars(tensor) => {
            let vars: Vec<_> = tensor.vars().collect();
            print_tensor_tuple_pattern(ctx, &vars, "(", ")")
        }
        VarDeclPattern::VarDecl(var) => {
            let name = var.name()?;
            let typ = var.typ();
            let is_redefinition = var.is_redefinition();

            let name_doc = print_ident(ctx, &name)?;
            if is_redefinition {
                Some(RcDoc::concat([name_doc, RcDoc::text(" redef")]))
            } else if let Some(typ) = typ {
                let type_doc = types::print_type(ctx, &typ)?;
                Some(RcDoc::concat([name_doc, RcDoc::text(": "), type_doc]))
            } else {
                Some(name_doc)
            }
        }
    }
}

fn print_tensor_tuple_pattern<'a>(
    ctx: &Context,
    vars: &[VarDeclPattern],
    open_quote: &'a str,
    close_quote: &'a str,
) -> Option<RcDoc<'a>> {
    common::print_list(
        ctx,
        vars,
        print_var_declaration_pattern,
        AstNode::syntax,
        |_| vec![],
        common::ListOptions {
            brackets: (RcDoc::text(open_quote), RcDoc::text(close_quote)),
            ..Default::default()
        },
    )
}

#[must_use]
pub fn print_assignment<'a>(ctx: &Context<'_>, assignment: &Assign) -> Option<RcDoc<'a>> {
    let left = assignment.left()?;
    let right = assignment.right()?;
    let left_doc = print_expression(ctx, &left)?;
    let right_doc = print_expression(ctx, &right)?;
    Some(RcDoc::concat([left_doc, RcDoc::text(" = "), right_doc]))
}

#[must_use]
pub fn print_set_assignment<'a>(ctx: &Context, set_assignment: &SetAssign) -> Option<RcDoc<'a>> {
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

#[must_use]
pub fn print_ternary_operator<'a>(ctx: &Context<'_>, ternary: &Ternary) -> Option<RcDoc<'a>> {
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

#[must_use]
pub fn print_binary_operator<'a>(ctx: &Context<'_>, binary: &Bin) -> Option<RcDoc<'a>> {
    let left = binary.left()?;
    let right = binary.right()?;
    let op = binary.operator_name(ctx.code.as_ref().as_ref()).to_string();
    let left_doc = print_expression(ctx, &left)?;
    let right_doc = print_expression(ctx, &right)?;

    if has_inline_line_comment_in_subtree(ctx, left.syntax()) {
        return Some(RcDoc::group(RcDoc::concat([
            left_doc,
            RcDoc::hardline(),
            RcDoc::text(op),
            RcDoc::space(),
            RcDoc::group(right_doc),
        ])));
    }

    Some(RcDoc::group(RcDoc::concat([
        left_doc,
        RcDoc::text(" "),
        RcDoc::text(op),
        RcDoc::line(),
        RcDoc::group(right_doc),
    ])))
}

#[must_use]
pub fn print_unary_operator<'a>(ctx: &Context<'_>, unary: &Unary) -> Option<RcDoc<'a>> {
    let op = unary.operator_name(ctx.code.as_ref().as_ref()).to_string();
    let arg = unary.argument()?;
    let arg_doc = print_expression(ctx, &arg)?;
    Some(RcDoc::concat([RcDoc::text(op), arg_doc]))
}

#[must_use]
pub fn print_lazy_expression<'a>(ctx: &Context<'_>, lazy: &Lazy) -> Option<RcDoc<'a>> {
    let expr = lazy.expr()?;
    let expr_doc = print_expression(ctx, &expr)?;
    Some(RcDoc::concat([RcDoc::text("lazy "), expr_doc]))
}

#[must_use]
pub fn print_cast_as_operator<'a>(ctx: &Context<'_>, cast: &AsCast) -> Option<RcDoc<'a>> {
    let expr = cast.expr()?;
    let typ = cast.casted_to()?;
    let expr_doc = print_expression(ctx, &expr)?;
    let type_doc = types::print_type(ctx, &typ)?;
    Some(RcDoc::concat([expr_doc, RcDoc::text(" as "), type_doc]))
}

#[must_use]
pub fn print_is_type_operator<'a>(ctx: &Context<'_>, is_type: &IsType) -> Option<RcDoc<'a>> {
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

#[must_use]
pub fn print_not_null_operator<'a>(ctx: &Context<'_>, not_null: &NotNull) -> Option<RcDoc<'a>> {
    let inner = not_null.inner()?;
    let inner_doc = print_expression(ctx, &inner)?;
    Some(RcDoc::concat([inner_doc, RcDoc::text("!")]))
}

#[must_use]
pub fn print_dot_access<'a>(ctx: &Context<'_>, dot: &DotAccess) -> Option<RcDoc<'a>> {
    let obj = dot.obj()?;
    let field = dot.field()?;
    let obj_doc = print_expression(ctx, &obj)?;

    let field_doc = match field {
        DotAccessField::Ident(i) => print_simple_node(ctx, &i.0)?,
        DotAccessField::NumericIndex(n) => print_simple_node(ctx, &n.0)?,
    };

    let is_obj_literal = matches!(obj, Expr::ObjectLit(_));

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

#[must_use]
pub fn print_function_call<'a>(ctx: &Context<'_>, call: &Call) -> Option<RcDoc<'a>> {
    let callee = call.callee()?;
    let callee_doc = print_expression(ctx, &callee)?;
    let args: Vec<_> = call.arguments().collect();
    let args_doc = print_argument_list(ctx, &args, call.0.field("arguments"))?;

    Some(RcDoc::concat([callee_doc, args_doc]))
}

pub fn print_argument_list<'a>(
    ctx: &Context<'_>,
    args: &[CallArgument],
    argument_list: Option<ArgumentList<'_>>,
) -> Option<RcDoc<'a>> {
    // Respect only explicit top-level line breaks in `(...)`.
    // Newlines inside a single object/lambda argument should not force the whole call to break.
    let has_top_level_newline = argument_list
        .is_some_and(|argument_list| argument_list_has_top_level_newline(ctx, &argument_list));

    // We want to output:
    // ```
    // createMessage({
    //    ...
    // })
    // ```
    // Thus, without breaking the entire { ... } and without adding extra indentation
    // Keep this compact form unless the user already split the argument list itself.
    // TODO: better way?
    if !has_top_level_newline
        && args.len() == 1
        && let Some(single) = args.first()
        && matches!(
            single.expr(),
            Some(Expr::ObjectLit(_) | Expr::StringLit(_) | Expr::Lambda(_))
        )
    {
        return Some(RcDoc::group(RcDoc::concat([
            RcDoc::text("("),
            print_call_argument(ctx, single)?,
            RcDoc::text(")"),
        ])));
    }

    let list_options = if has_top_level_newline
        || (args.len() > 1 && args.iter().any(call_argument_contains_lambda))
    {
        common::ListOptions {
            multiline_threshold: 0,
            ..Default::default()
        }
    } else {
        common::ListOptions {
            never_break_if_items_lt: if matches!(args, [arg] if single_scalar_call_argument(arg)) {
                2
            } else {
                0
            },
            ..Default::default()
        }
    };

    common::print_list(
        ctx,
        args,
        print_call_argument,
        |arg| arg.0,
        |_| vec![],
        list_options,
    )
}

fn argument_list_has_top_level_newline(
    ctx: &Context<'_>,
    argument_list: &ArgumentList<'_>,
) -> bool {
    let source = ctx.code.as_ref().as_bytes();
    let node = argument_list.0;
    let close_paren_start = node.end_byte().saturating_sub(1);
    let mut previous_end = node.start_byte().saturating_add(1);

    // Scan only the gaps between top-level argument nodes and the parens.
    // This ignores newlines nested inside an argument expression itself.
    for argument in argument_list.arguments() {
        if source[previous_end..argument.0.start_byte()].contains(&b'\n') {
            return true;
        }
        previous_end = argument.0.end_byte();
    }

    source[previous_end..close_paren_start].contains(&b'\n')
}

fn call_argument_contains_lambda(arg: &CallArgument) -> bool {
    matches!(arg.expr(), Some(Expr::Lambda(_)))
}

fn single_scalar_call_argument(arg: &CallArgument) -> bool {
    matches!(
        arg.expr(),
        Some(Expr::NumberLit(_) | Expr::BoolLit(_) | Expr::NullLit(_))
    )
}

fn print_instantiation_types<'a>(
    ctx: &Context<'_>,
    instantiation: &Instantiation,
) -> Option<RcDoc<'a>> {
    let ts = instantiation.instantiation_ts()?;
    let types: Vec<_> = ts.types().collect();

    if let [single_type] = types.as_slice()
        && types::single_type_argument_should_stay_inline(single_type)
    {
        let single_type_doc = types::print_type(ctx, single_type)?;
        return Some(RcDoc::concat([
            RcDoc::text("<"),
            single_type_doc,
            RcDoc::text(">"),
        ]));
    }

    common::print_list(
        ctx,
        &types,
        types::print_type,
        Type::syntax,
        |_| vec![],
        common::ListOptions::triangle_bracket_list(),
    )
}

#[must_use]
pub fn print_call_argument<'a>(ctx: &Context<'_>, arg: &CallArgument) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    if arg.mutate() {
        parts.push(RcDoc::text("mutate "));
    }
    if let Some(expr) = arg.expr() {
        parts.push(print_expression(ctx, &expr)?);
    }
    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_generic_instantiation<'a>(
    ctx: &Context,
    instantiation: &Instantiation,
) -> Option<RcDoc<'a>> {
    let expr = instantiation.expr()?;
    let expr_doc = print_expression(ctx, &expr)?;
    let types_doc = print_instantiation_types(ctx, instantiation)?;

    Some(RcDoc::concat([expr_doc, types_doc]))
}

#[must_use]
pub fn print_parenthesized_expression<'a>(ctx: &Context, paren: &Paren) -> Option<RcDoc<'a>> {
    let inner = paren.inner()?;
    let inner_doc = print_expression(ctx, &inner)?;
    Some(RcDoc::concat([
        RcDoc::text("("),
        inner_doc,
        RcDoc::text(")"),
    ]))
}

#[must_use]
pub fn print_match_expression<'a>(ctx: &Context, match_expr: &Match) -> Option<RcDoc<'a>> {
    let expr = match_expr.expr()?;
    let body = match_expr.body()?;

    let expr_doc = print_expression(ctx, &expr)?;
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

pub fn print_match_body<'a>(ctx: &Context<'_>, body: &MatchBody) -> Option<RcDoc<'a>> {
    let arms: Vec<_> = body.arms().collect();
    common::print_list(
        ctx,
        &arms,
        print_match_arm,
        |arm| arm.0,
        |_| vec![],
        common::ListOptions {
            brackets: (RcDoc::text("{"), RcDoc::text("}")),
            separator: RcDoc::nil(), // handled by print_match_arm itself
            multiline_threshold: 0,  // always break
            ..Default::default()
        },
    )
}

#[must_use]
pub fn print_match_arm<'a>(ctx: &Context<'_>, arm: &MatchArm) -> Option<RcDoc<'a>> {
    let pattern = arm.pattern();
    let body = arm.body()?;

    let pattern_doc = match pattern {
        MatchPattern::Type(t) => types::print_type(ctx, &t)?,
        MatchPattern::Expr(e) => print_expression(ctx, &e)?,
        MatchPattern::Else => RcDoc::text("else"),
    };

    let (body_doc, is_block) = match body {
        MatchArmBody::Block(b) => (stmts::print_block_statement(ctx, &b)?, true),
        MatchArmBody::Return(r) => (stmts::print_return_statement(ctx, &r)?, false),
        MatchArmBody::Throw(t) => (stmts::print_throw_statement(ctx, &t)?, false),
        MatchArmBody::Expr(e) => (print_expression(ctx, &e)?, false),
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

#[must_use]
pub fn print_object_literal<'a>(ctx: &Context<'_>, obj: &ObjectLit) -> Option<RcDoc<'a>> {
    let typ = obj.typ();
    let has_type_name = typ.is_some();
    let mut docs = vec![];
    if let Some(typ) = typ {
        docs.push(types::print_type(ctx, &typ)?);
        docs.push(RcDoc::space());
    }

    let args_doc = print_object_literal_body(ctx, obj, has_type_name)?;
    docs.push(args_doc);

    Some(RcDoc::group(RcDoc::concat(docs)))
}

pub fn print_object_literal_body<'a>(
    ctx: &Context,
    obj: &ObjectLit,
    has_type_name: bool,
) -> Option<RcDoc<'a>> {
    let node = obj.syntax();
    let args: Vec<_> = obj.arguments().collect();

    let (multiline_threshold, never_break_if_items_lt) =
        if is_single_typeless_object_call_argument(node, has_type_name) {
            if node.start_position().row < node.end_position().row {
                (0, 0)
            } else {
                (usize::MAX, args.len() + 1)
            }
        } else {
            (object_literal_multiline_threshold(&args, has_type_name), 0)
        };

    common::print_list(
        ctx,
        &args,
        print_instance_argument,
        |arg| arg.0,
        |_| vec![],
        common::ListOptions {
            brackets: (RcDoc::text("{"), RcDoc::text("}")),
            multiline_threshold,
            single_line_edge_space: true,
            never_break_if_items_lt,
            ..Default::default()
        },
    )
}

fn object_literal_multiline_threshold(args: &[InstanceArg], has_type_name: bool) -> usize {
    if !has_type_name {
        return 2;
    }

    if args.len() < 2 {
        return 2;
    }

    if args.iter().all(is_shorthand_instance_argument) {
        // Keep compact form when it fits into line width.
        usize::MAX
    } else {
        // For 2+ args, only all-shorthand literals may stay one-line.
        0
    }
}

fn is_single_typeless_object_call_argument(node: Node<'_>, has_type_name: bool) -> bool {
    if has_type_name {
        return false;
    }

    node.parent()
        .filter(|node| node.kind() == "call_argument")
        .and_then(|node| node.parent())
        .filter(|node| node.kind() == "argument_list")
        .map(ArgumentList)
        .is_some_and(|args| args.arguments().count() == 1)
}

fn is_shorthand_instance_argument(arg: &InstanceArg) -> bool {
    arg.value().is_none()
}

#[must_use]
pub fn print_instance_argument<'a>(ctx: &Context<'_>, arg: &InstanceArg) -> Option<RcDoc<'a>> {
    let name = arg.name()?;
    let name_doc = print_ident(ctx, &name)?;

    let mut parts = vec![name_doc];

    if !is_shorthand_instance_argument(arg)
        && let Some(val) = arg.value()
    {
        let val_doc = print_expression(ctx, &val)?;
        parts.push(RcDoc::text(": "));
        parts.push(val_doc);
    }

    Some(RcDoc::concat(parts))
}

#[must_use]
pub fn print_tensor_expression<'a>(ctx: &Context<'_>, tensor: &Tensor) -> Option<RcDoc<'a>> {
    let elements: Vec<_> = tensor.elements().collect();
    if elements.is_empty() {
        return Some(RcDoc::text("()"));
    }

    print_tuple_tensor(ctx, &elements, "(", ")")
}

#[must_use]
pub fn print_typed_tuple<'a>(ctx: &Context<'_>, tuple: &Tuple) -> Option<RcDoc<'a>> {
    let tuple_type = tuple.typ();
    let elements: Vec<_> = tuple.elements().collect();
    let tuple_doc = print_tuple_tensor(ctx, &elements, "[", "]")?;

    let mut docs = vec![];
    if let Some(typ) = tuple_type {
        docs.push(types::print_type(ctx, &typ)?);
        docs.push(RcDoc::space());
    }
    docs.push(tuple_doc);
    Some(RcDoc::concat(docs))
}

fn print_tuple_tensor<'a>(
    ctx: &Context,
    elements: &[Expr],
    open_quote: &'a str,
    close_quote: &'a str,
) -> Option<RcDoc<'a>> {
    common::print_list(
        ctx,
        elements,
        print_expression,
        Expr::syntax,
        |_| vec![],
        common::ListOptions {
            brackets: (RcDoc::text(open_quote), RcDoc::text(close_quote)),
            ..Default::default()
        },
    )
}

#[must_use]
pub fn print_lambda_expression<'a>(ctx: &Context<'_>, lambda: &Lambda) -> Option<RcDoc<'a>> {
    let params: Vec<_> = lambda.parameters().collect();
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

#[must_use]
pub fn print_simple_node<'a>(ctx: &Context<'_>, node: &Node) -> Option<RcDoc<'a>> {
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

#[must_use]
pub fn print_ident<'a>(ctx: &Context<'_>, ident: &Ident) -> Option<RcDoc<'a>> {
    print_simple_node(ctx, &ident.0)
}
