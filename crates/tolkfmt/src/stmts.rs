use pretty::RcDoc;
use tolk_ast::{BlockStatement, LocalVarsDeclaration, Statement, VarDeclarationLhs};
use crate::{exprs, Context};

pub fn print_block_statement<'a>(ctx: &mut Context, block: &BlockStatement) -> Option<RcDoc<'a>> {
    let statements = block.statements();
    let statements_doc = statements
        .iter()
        .flat_map(|stmt| print_statement(ctx, stmt));

    let result = RcDoc::concat([
        RcDoc::text("{"),
        RcDoc::concat([RcDoc::hardline(), RcDoc::concat(statements_doc)]).nest(4),
        RcDoc::hardline(),
        RcDoc::text("}"),
    ]);
    Some(result)
}

fn print_statement<'a>(ctx: &mut Context, stmt: &Statement) -> Option<RcDoc<'a>> {
    match stmt {
        Statement::BlockStatement(block) => print_block_statement(ctx, block),
        Statement::IfStatement(_) => todo!(),
        Statement::WhileStatement(_) => todo!(),
        Statement::RepeatStatement(_) => todo!(),
        Statement::TryCatchStatement(_) => todo!(),
        Statement::ReturnStatement(_) => todo!(),
        Statement::LocalVarsDeclaration(locals) => print_local_variables(ctx, locals),
        Statement::DoWhileStatement(_) => todo!(),
        Statement::BreakStatement(_) => todo!(),
        Statement::ContinueStatement(_) => todo!(),
        Statement::ThrowStatement(_) => todo!(),
        Statement::AssertStatement(_) => todo!(),
        Statement::MatchStatement(_) => todo!(),
        Statement::EmptyStatement(_) => todo!(),
        Statement::ExpressionStatement(_) => todo!(),
        Statement::Unmapped(node) => {
            if node.0.kind() == "comment" {
                return Some(RcDoc::text(""));
            }
            if node.text(ctx.code.as_ref().as_ref()) == ";" {
                return Some(RcDoc::text(""));
            }
            crate::common::print_node_text(ctx, &node.0)
        }
    }
}

fn print_local_variables<'a>(
    ctx: &mut Context,
    locals: &LocalVarsDeclaration,
) -> Option<RcDoc<'a>> {
    let kind = locals.kind();
    let lhs = locals.lhs()?;
    let assigned_val = locals.assigned_val();

    let lhs_doc = print_var_declaration_lhs(ctx, &lhs)?;

    if let Some(assigned_val) = assigned_val {
        let assigned_val_doc = exprs::print_expression(ctx, &assigned_val)?;

        let result = RcDoc::concat([
            RcDoc::text(kind.as_str()),
            RcDoc::space(),
            lhs_doc,
            RcDoc::text(" = "),
            assigned_val_doc,
            RcDoc::text(";"), // TODO: match expression
        ]);
        return Some(result);
    }

    let result = RcDoc::concat([
        RcDoc::text(kind.as_str()),
        RcDoc::space(),
        lhs_doc,
        RcDoc::text(";"), // TODO: match expression
    ]);
    Some(result)
}

fn print_var_declaration_lhs<'a>(ctx: &mut Context, lhs: &VarDeclarationLhs) -> Option<RcDoc<'a>> {
    match lhs {
        VarDeclarationLhs::TupleVarsDeclaration(_) => todo!(),
        VarDeclarationLhs::TensorVarsDeclaration(_) => todo!(),
        VarDeclarationLhs::VarDeclaration(var) => {
            let name = var.name()?;
            let typ = var.typ();
            let is_redefinition = var.is_redefinition();

            let name_doc = exprs::print_ident(ctx, &name)?;
            if is_redefinition {
                Some(RcDoc::concat([name_doc, RcDoc::text(" redef")]))
            } else if let Some(typ) = typ {
                let type_doc = crate::types::print_type(ctx, &typ)?;
                Some(RcDoc::concat([name_doc, RcDoc::text(": "), type_doc]))
            } else {
                Some(name_doc)
            }
        }
    }
}