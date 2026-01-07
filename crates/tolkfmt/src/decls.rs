use crate::{Context, exprs, stmts};
use pretty::RcDoc;
use tolk_ast::{Function, FunctionBody, SourceFile, TopLevel};

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
        TopLevel::TolkRequiredVersion(_) => todo!(),
        TopLevel::Import(_) => todo!(),
        TopLevel::GlobalVarDeclaration(_) => todo!(),
        TopLevel::ConstantDeclaration(_) => todo!(),
        TopLevel::TypeAliasDeclaration(_) => todo!(),
        TopLevel::StructDeclaration(_) => todo!(),
        TopLevel::EnumDeclaration(_) => todo!(),
        TopLevel::Function(func) => print_function(ctx, func),
        TopLevel::MethodDeclaration(_) => todo!(),
        TopLevel::GetMethodDeclaration(_) => todo!(),
        TopLevel::EmptyStatement(_) => todo!(),
        TopLevel::Unmapped(_) => todo!(),
    }
}

pub fn print_function<'a>(ctx: &mut Context, func: &Function) -> Option<RcDoc<'a>> {
    let name = func.name()?;
    let name_doc = exprs::print_ident(ctx, &name)?;

    let parameters = func.parameters();
    let parameters_doc = print_parameter_list(ctx, &parameters)?;

    let body = func.body()?;
    let body_doc = print_function_body(ctx, &body)?;

    let result = RcDoc::concat([
        RcDoc::text("fun "),
        name_doc,
        parameters_doc,
        RcDoc::space(),
        body_doc,
    ]);

    Some(result)
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

impl<'tree> ParameterTrait for tolk_ast::Parameter<'tree> {
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
        parts.push(crate::types::print_type(ctx, &typ)?);
    }

    if let Some(default) = param.default() {
        parts.push(RcDoc::text(" = "));
        parts.push(exprs::print_expression(ctx, &default)?);
    }

    Some(RcDoc::concat(parts))
}

fn print_function_body<'a>(ctx: &mut Context, body: &FunctionBody) -> Option<RcDoc<'a>> {
    match body {
        FunctionBody::BlockStatement(block) => stmts::print_block_statement(ctx, block),
        FunctionBody::AsmBody(_) => todo!(),
        FunctionBody::BuiltinSpecifier(_) => todo!(),
        FunctionBody::Unmapped(_) => todo!(),
    }
}
