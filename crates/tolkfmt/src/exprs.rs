use crate::comments::has_inline_line_comment_in_subtree;
use crate::pretty::RcDoc;
use crate::{Context, comments, common, stmts, types};
use tolk_syntax::*;
use tree_sitter::Node;

#[must_use]
pub fn print_expression<'a>(ctx: &Context<'_>, expr: &Expr) -> Option<RcDoc<'a>> {
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

    let node = expr.syntax();
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
}

fn collect_method_chain_doc<'a>(ctx: &Context<'_>, expr: &Expr) -> Option<MethodChainDoc<'a>> {
    match expr {
        Expr::DotAccess(dot) => {
            let obj = dot.obj()?;
            let mut chain = collect_method_chain_doc(ctx, &obj)?;

            let field_doc = match dot.field()? {
                DotAccessField::Ident(i) => print_simple_node(ctx, &i.0)?,
                DotAccessField::NumericIndex(n) => print_simple_node(ctx, &n.0)?,
            };

            chain.links.push(MethodChainLink {
                doc: RcDoc::concat([RcDoc::text("."), field_doc]),
            });
            chain.has_dot_link = true;
            Some(chain)
        }
        Expr::Call(call) => {
            let callee = call.callee()?;
            let mut chain = collect_method_chain_doc(ctx, &callee)?;
            let args: Vec<_> = call.arguments().collect();
            let args_doc = print_argument_list(ctx, &args)?;

            if chain.has_dot_link {
                if let Some(last) = chain.links.last_mut() {
                    last.doc = last.doc.clone().append(args_doc);
                } else {
                    chain.base = chain.base.append(args_doc);
                }
                Some(chain)
            } else {
                let callee_doc = print_expression(ctx, &callee)?;
                Some(MethodChainDoc {
                    base: RcDoc::concat([callee_doc, args_doc]),
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
    let chain = collect_method_chain_doc(ctx, expr)?;
    if !chain.has_dot_link {
        return Some(chain.base);
    }

    let keep_single_link_attached = chain.links.len() == 1;

    let mut tail = Vec::with_capacity(chain.links.len() * 3);
    for (index, link) in chain.links.into_iter().enumerate() {
        let separator = if index == 0 && (chain.base_is_object_lit || keep_single_link_attached)
        {
            RcDoc::nil()
        } else {
            RcDoc::line_()
        };
        tail.push(separator);
        // If a part of the chain breaks, force parent groups to break too.
        tail.push(RcDoc::break_parent().flat_alt(RcDoc::nil()));
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
        |v| v.syntax(),
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
    let args_doc = print_argument_list(ctx, &args)?;

    Some(RcDoc::concat([callee_doc, args_doc]))
}

pub fn print_argument_list<'a>(ctx: &Context<'_>, args: &[CallArgument]) -> Option<RcDoc<'a>> {
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
        && matches!(single.expr(), Some(Expr::ObjectLit(_)))
    {
        return Some(RcDoc::group(RcDoc::concat([
            RcDoc::text("("),
            print_call_argument(ctx, single)?,
            RcDoc::text(")"),
        ])));
    }

    common::print_list(
        ctx,
        args,
        print_call_argument,
        |arg| arg.0,
        |_| vec![],
        common::ListOptions::default(),
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

pub fn print_generic_instantiation<'a>(
    ctx: &Context,
    instantiation: &Instantiation,
) -> Option<RcDoc<'a>> {
    let expr = instantiation.expr()?;
    let expr_doc = print_expression(ctx, &expr)?;
    let ts = instantiation.instantiation_ts()?;
    let types: Vec<_> = ts.types().collect();

    let types_doc = common::print_list(
        ctx,
        &types,
        types::print_type,
        Type::syntax,
        |_| vec![],
        common::ListOptions::triangle_bracket_list(),
    )?;

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
    let mut docs = vec![];
    if let Some(typ) = typ {
        docs.push(types::print_type(ctx, &typ)?);
        docs.push(RcDoc::space());
    }

    let args: Vec<_> = obj.arguments().collect();
    let args_doc = print_object_literal_body(ctx, &args)?;
    docs.push(args_doc);

    Some(RcDoc::group(RcDoc::concat(docs)))
}

pub fn print_object_literal_body<'a>(ctx: &Context, args: &[InstanceArg]) -> Option<RcDoc<'a>> {
    common::print_list(
        ctx,
        args,
        print_instance_argument,
        |arg| arg.0,
        |_| vec![],
        common::ListOptions {
            brackets: (RcDoc::text("{"), RcDoc::text("}")),
            multiline_threshold: 2,
            single_line_edge_space: true,
            ..Default::default()
        },
    )
}

#[must_use]
pub fn print_instance_argument<'a>(ctx: &Context<'_>, arg: &InstanceArg) -> Option<RcDoc<'a>> {
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
