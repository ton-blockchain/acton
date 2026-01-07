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
    let parameters_doc = RcDoc::text("()"); // TODO

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

fn print_function_body<'a>(ctx: &mut Context, body: &FunctionBody) -> Option<RcDoc<'a>> {
    match body {
        FunctionBody::BlockStatement(block) => stmts::print_block_statement(ctx, block),
        FunctionBody::AsmBody(_) => todo!(),
        FunctionBody::BuiltinSpecifier(_) => todo!(),
        FunctionBody::Unmapped(_) => todo!(),
    }
}
