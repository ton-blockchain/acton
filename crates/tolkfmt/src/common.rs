use tree_sitter::Node;
use pretty::RcDoc;
use crate::Context;

pub fn print_node_text<'a>(ctx: &mut Context, ident: &Node) -> Option<RcDoc<'a>> {
    let text = ident.utf8_text(ctx.code.as_ref().as_ref()).ok()?.to_owned();
    Some(RcDoc::text(text))
}