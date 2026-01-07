use crate::{Context, comments, common, exprs, stmts, types};
use pretty::RcDoc;
use tolk_ast::*;

pub fn print_source_file<'a>(ctx: &Context, file: &SourceFile) -> Option<RcDoc<'a>> {
    let mut sections = vec![];

    // tolk required version section
    let mut docs = vec![];

    // В файле по идее может быть несколько версий Толка, но мы оставляем только одну
    let required_version = file
        .top_levels_iter()
        .filter_map(|decl| match decl {
            TopLevel::TolkRequiredVersion(decl) => Some(decl),
            _ => None,
        })
        .next();

    // Версия Толка всегда печатается в начале файла, до импортов
    if let Some(required_version) = required_version {
        let doc = print_tolk_required_version(ctx, &required_version);
        if let Some(doc) = doc {
            docs.push(doc);
            docs.push(RcDoc::hardline());
        }
    }

    if !docs.is_empty() {
        sections.push(docs);
    }

    // imports section
    let mut docs = vec![];

    let imports = file
        .top_levels_iter()
        .filter_map(|decl| match decl {
            TopLevel::Import(decl) => Some(decl),
            _ => None,
        })
        .collect::<Vec<_>>();

    // После опциональной версии Толка идут импорты.
    // Импорты печатаются без пустых строк как остальные декларации, но как и стейтменты
    // они могут быть разделены одной пустой строкой если так было в оригинальном коде
    for (i, import) in imports.iter().enumerate() {
        let comments = ctx.comments.get(&import.0);
        comments::print_leading_comments(ctx, &mut docs, comments);

        let Some(doc) = print_import(ctx, import) else {
            continue;
        };
        docs.push(doc);

        comments::print_inline_comments(ctx, &mut docs, comments);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);

        if let Some(next_import) = imports.get(i + 1)
            && common::empty_lines_between(ctx, &import.0, &next_import.0) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    if !docs.is_empty() {
        sections.push(docs);
    }

    // declarations section
    let mut docs = vec![];

    let mut top_levels_iter = file
        .top_levels_iter()
        .filter(|decl| {
            !matches!(
                decl,
                TopLevel::TolkRequiredVersion(_)
                    | TopLevel::Import(_)
                    | TopLevel::EmptyStatement(_)
                    | TopLevel::Unmapped(_)
            )
        })
        .peekable();

    while let Some(top_level) = top_levels_iter.next() {
        let node = top_level.raw_node();
        let comments = ctx.comments.get(&node);

        comments::print_leading_comments(ctx, &mut docs, comments);

        let Some(doc) = print_decl(ctx, &top_level) else {
            continue;
        };
        docs.push(doc);

        comments::print_inline_comments(ctx, &mut docs, comments);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);

        // Добавляем пустую строку между декларациями
        if top_levels_iter.peek().is_some() {
            docs.push(RcDoc::hardline());
        }
    }

    if !docs.is_empty() {
        sections.push(docs);
    }

    Some(common::print_sections(sections))
}

pub fn print_decl<'a>(ctx: &Context, decl: &TopLevel) -> Option<RcDoc<'a>> {
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
    ctx: &Context,
    v: &TolkRequiredVersion,
) -> Option<RcDoc<'a>> {
    let value = v.value()?;
    let value_doc = common::print_node_text(ctx, &value.0)?;
    Some(RcDoc::concat([RcDoc::text("tolk "), value_doc]))
}

pub fn print_import<'a>(ctx: &Context, i: &Import) -> Option<RcDoc<'a>> {
    let path = i.path()?;
    let path_doc = common::print_node_text(ctx, &path.0)?;
    Some(RcDoc::concat([RcDoc::text("import "), path_doc]))
}

pub fn print_global_var_declaration<'a>(
    ctx: &Context,
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
    ctx: &Context,
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
    ctx: &Context,
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
        parts.push(RcDoc::line());
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

pub fn print_struct_declaration<'a>(ctx: &Context, s: &StructDeclaration) -> Option<RcDoc<'a>> {
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

pub fn print_struct_body<'a>(ctx: &Context, body: &StructBody) -> Option<RcDoc<'a>> {
    let fields = body.fields();
    if fields.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let mut docs = vec![RcDoc::hardline()];
    for (i, field) in fields.iter().enumerate() {
        let comments = ctx.comments.get(&field.0);
        comments::print_leading_comments(ctx, &mut docs, comments);

        docs.push(print_struct_field_declaration(ctx, field)?);

        comments::print_inline_comments(ctx, &mut docs, comments);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);

        // Между полями может быть пустая строка которую мы хотим сохранить
        if let Some(next) = fields.get(i + 1)
            && common::empty_lines_between(ctx, &field.0, &next.0) > 1
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

pub fn print_struct_field_declaration<'a>(
    ctx: &Context,
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

pub fn print_enum_declaration<'a>(ctx: &Context, e: &EnumDeclaration) -> Option<RcDoc<'a>> {
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

pub fn print_enum_body<'a>(ctx: &Context, body: &EnumBody) -> Option<RcDoc<'a>> {
    let members = body.members();
    if members.is_empty() {
        return Some(RcDoc::text("{}"));
    }

    let mut docs = vec![RcDoc::hardline()];
    for (i, member) in members.iter().enumerate() {
        let comments = ctx.comments.get(&member.0);
        comments::print_leading_comments(ctx, &mut docs, comments);

        docs.push(print_enum_member_declaration(ctx, member)?);

        comments::print_inline_comments(ctx, &mut docs, comments);
        docs.push(RcDoc::hardline());
        comments::print_trailing_comments(ctx, &mut docs, comments);

        // Между полями может быть пустая строка которую мы хотим сохранить
        if let Some(next) = members.get(i + 1)
            && common::empty_lines_between(ctx, &member.0, &next.0) > 1
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

pub fn print_enum_member_declaration<'a>(
    ctx: &Context,
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

pub fn print_function<'a>(ctx: &Context, func: &Function) -> Option<RcDoc<'a>> {
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

pub fn print_method_declaration<'a>(ctx: &Context, m: &MethodDeclaration) -> Option<RcDoc<'a>> {
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
    ctx: &Context,
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

pub fn print_method_receiver<'a>(ctx: &Context, r: &MethodReceiver) -> Option<RcDoc<'a>> {
    let typ = r.typ()?;
    let typ_doc = types::print_type(ctx, &typ)?;
    Some(RcDoc::concat([typ_doc, RcDoc::text(".")]))
}

pub fn print_parameter_list<'a, P>(ctx: &Context, params: &[P]) -> Option<RcDoc<'a>>
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

pub fn print_parameter_declaration<'a, P>(ctx: &Context, param: &P) -> Option<RcDoc<'a>>
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

pub fn print_annotation_list<'a>(ctx: &Context, a: &AnnotationList) -> Option<RcDoc<'a>> {
    let annotations = a.annotations();

    let mut parts = vec![];
    for annotation in annotations {
        parts.push(print_annotation(ctx, &annotation)?);
        parts.push(RcDoc::hardline());
    }

    Some(RcDoc::concat(parts))
}

pub fn print_annotation<'a>(ctx: &Context, a: &Annotation) -> Option<RcDoc<'a>> {
    let mut parts = vec![RcDoc::text("@")];
    if let Some(name) = a.name() {
        parts.push(exprs::print_ident(ctx, &name)?);
    }
    if let Some(args) = a.arguments() {
        parts.push(print_annotation_arguments(ctx, &args)?);
    }
    Some(RcDoc::concat(parts))
}

pub fn print_annotation_arguments<'a>(ctx: &Context, a: &AnnotationArguments) -> Option<RcDoc<'a>> {
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

pub fn print_type_parameters<'a>(ctx: &Context, tp: &TypeParameters) -> Option<RcDoc<'a>> {
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

pub fn print_type_parameter<'a>(ctx: &Context, tp: &TypeParameter) -> Option<RcDoc<'a>> {
    let mut parts = vec![];
    let name = tp.name()?;
    parts.push(exprs::print_ident(ctx, &name)?);
    if let Some(default) = tp.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(types::print_type(ctx, &default)?);
    }
    Some(RcDoc::concat(parts))
}

fn print_function_body<'a>(ctx: &Context, body: &FunctionBody) -> Option<RcDoc<'a>> {
    match body {
        FunctionBody::BlockStatement(block) => stmts::print_block_statement(ctx, block),
        FunctionBody::AsmBody(asm) => print_asm_body(ctx, asm),
        FunctionBody::BuiltinSpecifier(_) => Some(RcDoc::text("builtin")),
        FunctionBody::Unmapped(node) => common::print_node_text(ctx, &node.0),
    }
}

pub fn print_asm_body<'a>(ctx: &Context, asm: &AsmBody) -> Option<RcDoc<'a>> {
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
