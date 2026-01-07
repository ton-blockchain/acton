use crate::{Context, common, exprs, stmts, types};
use pretty::RcDoc;
use tolk_ast::{
    Annotation, AnnotationArguments, AnnotationList, AsmBody, ConstantDeclaration, EnumBody,
    EnumDeclaration, EnumMemberDeclaration, Function, FunctionBody, GetMethodDeclaration,
    GlobalVarDeclaration, Import, MethodDeclaration, MethodReceiver, Parameter, SourceFile,
    StructBody, StructDeclaration, StructFieldDeclaration, TolkRequiredVersion, TopLevel,
    TypeAliasDeclaration, TypeAliasUnderlyingType, TypeParameter, TypeParameters,
};

pub fn print_source_file<'a>(ctx: &mut Context, file: &SourceFile) -> Option<RcDoc<'a>> {
    let mut docs = vec![];

    let mut top_levels_iter = file.top_levels_iter().peekable();
    while let Some(top_level) = top_levels_iter.next() {
        let Some(doc) = print_decl(ctx, &top_level) else {
            continue;
        };
        docs.push(doc);

        if top_levels_iter.peek().is_some() {
            docs.push(RcDoc::hardline());
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::concat(docs))
}

pub fn print_decl<'a>(ctx: &mut Context, decl: &TopLevel) -> Option<RcDoc<'a>> {
    match decl {
        TopLevel::TolkRequiredVersion(v) => print_tolk_required_version(ctx, v),
        TopLevel::Import(i) => print_import(ctx, i),
        TopLevel::GlobalVarDeclaration(g) => print_global_var_declaration(ctx, g),
        TopLevel::ConstantDeclaration(constant) => print_constant_declaration(ctx, constant),
        TopLevel::TypeAliasDeclaration(t) => print_type_alias_declaration(ctx, t),
        TopLevel::StructDeclaration(s) => print_struct_declaration(ctx, s),
        TopLevel::EnumDeclaration(e) => print_enum_declaration(ctx, e),
        TopLevel::Function(func) => print_function(ctx, func),
        TopLevel::MethodDeclaration(m) => print_method_declaration(ctx, m),
        TopLevel::GetMethodDeclaration(g) => print_get_method_declaration(ctx, g),
        TopLevel::EmptyStatement(_) => Some(RcDoc::text(";")),
        TopLevel::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

pub fn print_tolk_required_version<'a>(
    ctx: &mut Context,
    v: &TolkRequiredVersion,
) -> Option<RcDoc<'a>> {
    let value = v.value()?;
    let value_doc = common::print_node_text(ctx, &value.0)?;
    Some(RcDoc::concat([RcDoc::text("tolk "), value_doc]))
}

pub fn print_import<'a>(ctx: &mut Context, i: &Import) -> Option<RcDoc<'a>> {
    let path = i.path()?;
    let path_doc = common::print_node_text(ctx, &path.0)?;
    Some(RcDoc::concat([RcDoc::text("import "), path_doc]))
}

pub fn print_global_var_declaration<'a>(
    ctx: &mut Context,
    g: &GlobalVarDeclaration,
) -> Option<RcDoc<'a>> {
    let name = g.name()?;
    let typ = g.typ()?;

    let mut parts = vec![];
    if let Some(annotations) = g.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("global "));
    parts.push(exprs::print_ident(ctx, &name)?);
    parts.push(RcDoc::text(": "));
    parts.push(types::print_type(ctx, &typ)?);
    parts.push(RcDoc::text(";"));

    Some(RcDoc::concat(parts))
}

pub fn print_constant_declaration<'a>(
    ctx: &mut Context,
    constant: &ConstantDeclaration,
) -> Option<RcDoc<'a>> {
    let name = constant.name()?;

    let mut parts = vec![];
    if let Some(annotations) = constant.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }
    parts.push(RcDoc::text("const "));

    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(typ) = constant.typ() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &typ)?);
        parts.push(RcDoc::space());
    } else {
        parts.push(RcDoc::space());
    }

    if let Some(value) = constant.value() {
        parts.push(RcDoc::text("= "));
        parts.push(exprs::print_expression(ctx, &value)?);
    }

    parts.push(RcDoc::text(";"));

    Some(RcDoc::concat(parts))
}

pub fn print_type_alias_declaration<'a>(
    ctx: &mut Context,
    t: &TypeAliasDeclaration,
) -> Option<RcDoc<'a>> {
    let name = t.name()?;

    let mut parts = vec![];
    if let Some(annotations) = t.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }
    parts.push(RcDoc::text("type "));
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(tp) = t.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    let underlying = t.underlying_type()?;
    let is_union = matches!(
        underlying,
        TypeAliasUnderlyingType::Type(tolk_ast::Type::UnionType(_))
    );

    parts.push(RcDoc::text(" ="));
    if is_union {
        parts.push(RcDoc::flat_alt(RcDoc::nil(), RcDoc::space()));
    } else {
        parts.push(RcDoc::space());
    }

    parts.push(match underlying {
        TypeAliasUnderlyingType::Type(typ) => types::print_type(ctx, &typ)?,
        TypeAliasUnderlyingType::BuiltinSpecifier(_) => RcDoc::text("builtin"),
    });
    parts.push(RcDoc::text(";"));

    Some(RcDoc::group(RcDoc::concat(parts)))
}

pub fn print_struct_declaration<'a>(ctx: &mut Context, s: &StructDeclaration) -> Option<RcDoc<'a>> {
    let name = s.name()?;

    let mut parts = vec![];
    if let Some(annotations) = s.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("struct "));
    if let Some(prefix) = s.pack_prefix() {
        parts.push(RcDoc::text("("));
        parts.push(common::print_node_text(ctx, &prefix.0)?);
        parts.push(RcDoc::text(") "));
    }

    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(tp) = s.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    parts.push(RcDoc::space());

    if let Some(body) = s.body() {
        parts.push(print_struct_body(ctx, &body)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_struct_body<'a>(ctx: &mut Context, body: &StructBody) -> Option<RcDoc<'a>> {
    let fields = body.fields();
    if fields.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let mut parts = vec![];
    for field in fields {
        parts.push(print_struct_field_declaration(ctx, &field)?);
    }

    let (first, rest) = parts.split_first()?;
    let mut tail_docs = vec![];
    for doc in rest {
        tail_docs.push(RcDoc::hardline());
        tail_docs.push(doc.clone());
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat([RcDoc::hardline(), first.clone(), RcDoc::concat(tail_docs)]).nest(4),
        RcDoc::hardline(),
        RcDoc::text("}"),
    ]))
}

pub fn print_struct_field_declaration<'a>(
    ctx: &mut Context,
    f: &StructFieldDeclaration,
) -> Option<RcDoc<'a>> {
    let name = f.name()?;
    let typ = f.typ()?;

    let mut parts = vec![];
    if let Some(modifiers) = f.modifiers() {
        for modifier in modifiers.modifiers() {
            parts.push(RcDoc::text(modifier.as_str()));
            parts.push(RcDoc::space());
        }
    }

    parts.push(exprs::print_ident(ctx, &name)?);
    parts.push(RcDoc::text(": "));
    parts.push(types::print_type(ctx, &typ)?);

    if let Some(default) = f.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }

    Some(RcDoc::concat(parts))
}

pub fn print_enum_declaration<'a>(ctx: &mut Context, e: &EnumDeclaration) -> Option<RcDoc<'a>> {
    let name = e.name()?;

    let mut parts = vec![];
    if let Some(annotations) = e.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("enum "));
    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(typ) = e.backed_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &typ)?);
    }

    if let Some(body) = e.body() {
        parts.push(RcDoc::space());
        parts.push(print_enum_body(ctx, &body)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_enum_body<'a>(ctx: &mut Context, body: &EnumBody) -> Option<RcDoc<'a>> {
    let members = body.members();
    if members.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let mut parts = vec![];
    for member in members {
        parts.push(print_enum_member_declaration(ctx, &member)?);
    }

    if parts.len() == 1
        && let Some(single) = parts.first()
    {
        return Some(RcDoc::concat([
            RcDoc::text("{"),
            RcDoc::space(),
            single.clone(),
            RcDoc::space(),
            RcDoc::text("}"),
        ]));
    }

    let (first, rest) = parts.split_first()?;
    let mut tail_docs = vec![];
    for doc in rest {
        tail_docs.push(RcDoc::text(","));
        tail_docs.push(RcDoc::hardline());
        tail_docs.push(doc.clone());
    }

    Some(RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat([
            RcDoc::hardline(),
            first.clone(),
            RcDoc::concat(tail_docs),
            RcDoc::text(","),
            RcDoc::hardline(),
        ])
        .nest(4),
        RcDoc::text("}"),
    ]))
}

pub fn print_enum_member_declaration<'a>(
    ctx: &mut Context,
    m: &EnumMemberDeclaration,
) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    let name = m.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(default) = m.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_function<'a>(ctx: &mut Context, func: &Function) -> Option<RcDoc<'a>> {
    let name = func.name()?;
    let parameters = func.parameters();
    let body = func.body()?;

    let mut parts = vec![];
    if let Some(annotations) = func.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("fun "));
    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(tp) = func.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    parts.push(print_parameter_list(ctx, &parameters)?);

    if let Some(ret) = func.return_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &ret)?);
    }

    let is_special = !matches!(body, FunctionBody::BlockStatement(_));
    if is_special {
        parts.push(RcDoc::concat([RcDoc::hardline(), print_function_body(ctx, &body)?]).nest(4));
    } else {
        parts.push(RcDoc::space());
        parts.push(print_function_body(ctx, &body)?);
    }

    Some(RcDoc::concat(parts))
}

pub fn print_method_declaration<'a>(ctx: &mut Context, m: &MethodDeclaration) -> Option<RcDoc<'a>> {
    let name = m.name()?;
    let parameters = m.parameters(ctx.code.as_ref().as_ref(), false);
    let body = m.body()?;

    let mut parts = vec![];
    if let Some(annotations) = m.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("fun "));

    if let Some(receiver) = m.receiver() {
        parts.push(print_method_receiver(ctx, &receiver)?);
    }

    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(tp) = m.type_parameters() {
        parts.push(print_type_parameters(ctx, &tp)?);
    }

    parts.push(print_parameter_list(ctx, &parameters)?);

    if let Some(ret) = m.return_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &ret)?);
    }

    let is_special = !matches!(body, FunctionBody::BlockStatement(_));
    if is_special {
        parts.push(RcDoc::concat([RcDoc::hardline(), print_function_body(ctx, &body)?]).nest(4));
    } else {
        parts.push(RcDoc::space());
        parts.push(print_function_body(ctx, &body)?);
    }

    Some(RcDoc::concat(parts))
}

pub fn print_get_method_declaration<'a>(
    ctx: &mut Context,
    g: &GetMethodDeclaration,
) -> Option<RcDoc<'a>> {
    let name = g.name()?;
    let parameters = g.parameters();
    let body = g.body()?;

    let mut parts = vec![];
    if let Some(annotations) = g.annotations() {
        parts.push(print_annotation_list(ctx, &annotations)?);
    }

    parts.push(RcDoc::text("get fun "));
    parts.push(exprs::print_ident(ctx, &name)?);

    parts.push(print_parameter_list(ctx, &parameters)?);

    if let Some(ret) = g.return_type() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &ret)?);
    }

    let is_special = !matches!(body, FunctionBody::BlockStatement(_));
    if is_special {
        parts.push(RcDoc::concat([RcDoc::hardline(), print_function_body(ctx, &body)?]).nest(4));
    } else {
        parts.push(RcDoc::space());
        parts.push(print_function_body(ctx, &body)?);
    }

    Some(RcDoc::concat(parts))
}

pub fn print_method_receiver<'a>(ctx: &mut Context, r: &MethodReceiver) -> Option<RcDoc<'a>> {
    let typ = r.typ()?;
    let typ_doc = types::print_type(ctx, &typ)?;
    Some(RcDoc::concat([typ_doc, RcDoc::text(".")]))
}

pub fn print_parameter_list<'a, P>(ctx: &mut Context, params: &[P]) -> Option<RcDoc<'a>>
where
    P: ParameterTrait,
{
    if params.is_empty() {
        return Some(RcDoc::text("()"));
    }

    let mut parts = vec![];
    for p in params {
        parts.push(print_parameter_declaration(ctx, p)?);
    }

    if parts.len() == 1 {
        return Some(RcDoc::concat([
            RcDoc::text("("),
            parts.into_iter().next().unwrap(),
            RcDoc::text(")"),
        ]));
    }

    let (first, rest) = parts.split_first().unwrap();
    let mut tail_docs = vec![];
    for part in rest {
        tail_docs.push(RcDoc::text(","));
        tail_docs.push(RcDoc::line());
        tail_docs.push(part.clone());
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text("("),
        RcDoc::concat([
            RcDoc::softline_(),
            first.clone(),
            RcDoc::concat(tail_docs),
            RcDoc::flat_alt(RcDoc::text(","), RcDoc::nil()),
        ])
        .nest(4),
        RcDoc::softline_(),
        RcDoc::text(")"),
    ])))
}

pub trait ParameterTrait {
    fn mutate(&self) -> bool;
    fn name(&self) -> Option<tolk_ast::Ident<'_>>;
    fn typ(&self) -> Option<tolk_ast::Type<'_>>;
    fn default(&self) -> Option<tolk_ast::Expression<'_>>;
}

impl<'tree> ParameterTrait for Parameter<'tree> {
    fn mutate(&self) -> bool {
        self.mutate()
    }
    fn name(&self) -> Option<tolk_ast::Ident<'tree>> {
        self.name()
    }
    fn typ(&self) -> Option<tolk_ast::Type<'tree>> {
        self.typ()
    }
    fn default(&self) -> Option<tolk_ast::Expression<'tree>> {
        self.default()
    }
}

impl<'tree> ParameterTrait for tolk_ast::LambdaParameter<'tree> {
    fn mutate(&self) -> bool {
        self.mutate()
    }
    fn name(&self) -> Option<tolk_ast::Ident<'tree>> {
        self.name()
    }
    fn typ(&self) -> Option<tolk_ast::Type<'tree>> {
        self.typ()
    }
    fn default(&self) -> Option<tolk_ast::Expression<'tree>> {
        None
    }
}

pub fn print_parameter_declaration<'a, P>(ctx: &mut Context, param: &P) -> Option<RcDoc<'a>>
where
    P: ParameterTrait,
{
    let mut parts = vec![];
    if param.mutate() {
        parts.push(RcDoc::text("mutate "));
    }
    let name = param.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);

    if let Some(typ) = param.typ() {
        parts.push(RcDoc::text(": "));
        parts.push(types::print_type(ctx, &typ)?);
    }

    if let Some(default) = param.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }

    Some(RcDoc::concat(parts))
}

pub fn print_annotation_list<'a>(ctx: &mut Context, a: &AnnotationList) -> Option<RcDoc<'a>> {
    let annotations = a.annotations();

    let mut parts = vec![];
    for annotation in annotations {
        parts.push(print_annotation(ctx, &annotation)?);
        parts.push(RcDoc::hardline());
    }

    Some(RcDoc::concat(parts))
}

pub fn print_annotation<'a>(ctx: &mut Context, a: &Annotation) -> Option<RcDoc<'a>> {
    let mut parts = vec![RcDoc::text("@")];
    if let Some(name) = a.name() {
        parts.push(exprs::print_ident(ctx, &name)?);
    }
    if let Some(args) = a.arguments() {
        parts.push(print_annotation_arguments(ctx, &args)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_annotation_arguments<'a>(
    ctx: &mut Context,
    a: &AnnotationArguments,
) -> Option<RcDoc<'a>> {
    let args = a.arguments();
    if args.is_empty() {
        return Some(RcDoc::text("()"));
    }

    let mut parts = vec![];
    for arg in args {
        parts.push(exprs::print_expression(ctx, &arg)?);
    }

    if parts.len() == 1
        && let Some(single) = parts.first()
    {
        return Some(RcDoc::concat([
            RcDoc::text("("),
            single.clone(),
            RcDoc::text(")"),
        ]));
    }

    let (first, rest) = parts.split_first()?;
    let mut tail_docs = vec![];
    for doc in rest {
        tail_docs.push(RcDoc::text(", "));
        tail_docs.push(doc.clone());
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text("("),
        RcDoc::concat([RcDoc::softline_(), first.clone(), RcDoc::concat(tail_docs)]).nest(4),
        RcDoc::softline_(),
        RcDoc::text(")"),
    ])))
}

pub fn print_type_parameters<'a>(ctx: &mut Context, tp: &TypeParameters) -> Option<RcDoc<'a>> {
    let parameters = tp.parameters();
    if parameters.is_empty() {
        return Some(RcDoc::text("<>"));
    }

    let mut parts = vec![];
    for p in parameters {
        parts.push(print_type_parameter(ctx, &p)?);
    }

    if parts.len() == 1
        && let Some(single) = parts.first()
    {
        return Some(RcDoc::concat([
            RcDoc::text("<"),
            single.clone(),
            RcDoc::text(">"),
        ]));
    }

    let (first, rest) = parts.split_first()?;
    let mut tail_docs = vec![];
    for doc in rest {
        tail_docs.push(RcDoc::text(", "));
        tail_docs.push(doc.clone());
    }

    Some(RcDoc::group(RcDoc::concat([
        RcDoc::text("<"),
        first.clone(),
        RcDoc::concat(tail_docs),
        RcDoc::text(">"),
    ])))
}

pub fn print_type_parameter<'a>(ctx: &mut Context, tp: &TypeParameter) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    let name = tp.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(default) = tp.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(types::print_type(ctx, &default)?);
    }
    Some(RcDoc::concat(parts))
}

fn print_function_body<'a>(ctx: &mut Context, body: &FunctionBody) -> Option<RcDoc<'a>> {
    match body {
        FunctionBody::BlockStatement(block) => stmts::print_block_statement(ctx, block),
        FunctionBody::AsmBody(asm) => print_asm_body(ctx, asm),
        FunctionBody::BuiltinSpecifier(_) => Some(RcDoc::text("builtin")),
        FunctionBody::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

pub fn print_asm_body<'a>(ctx: &mut Context, asm: &AsmBody) -> Option<RcDoc<'a>> {
    let mut parts = vec![RcDoc::text("asm")];

    let params = asm.params();
    let returns = asm.return_values();

    if !params.is_empty() || !returns.is_empty() {
        parts.push(RcDoc::text("("));
        for (i, p) in params.iter().enumerate() {
            if i > 0 {
                parts.push(RcDoc::space());
            }
            parts.push(exprs::print_ident(ctx, p)?);
        }
        if !returns.is_empty() {
            parts.push(RcDoc::text(" -> "));
            for (i, r) in returns.iter().enumerate() {
                if i > 0 {
                    parts.push(RcDoc::space());
                }
                parts.push(common::print_node_text(ctx, &r.0)?);
            }
        }
        parts.push(RcDoc::text(")"));
    }

    let instructions = asm.instructions();
    for inst in instructions {
        parts.push(RcDoc::line());
        parts.push(common::print_node_text(ctx, &inst.0)?);
    }

    Some(RcDoc::group(RcDoc::concat(parts)))
}
