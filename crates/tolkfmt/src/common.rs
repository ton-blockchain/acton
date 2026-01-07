use crate::Context;
use pretty::RcDoc;
use tree_sitter::Node;

pub fn print_node_text<'a>(ctx: &mut Context, ident: &Node) -> Option<RcDoc<'a>> {
    let text = ident.utf8_text(ctx.code.as_ref().as_ref()).ok()?.to_owned();
    Some(RcDoc::text(text))
}

pub fn empty_lines_between(top: &Node, bottom: &Node) -> usize {
    // [
    //
    // ] <- end   position of top
    // ( <- start position of botton
    //
    // )

    let botton_line = bottom.start_position().row;
    let top_line = top.end_position().row;
    botton_line.saturating_sub(top_line)
}

pub fn print_sections(sections: Vec<Vec<RcDoc>>) -> RcDoc {
    let mut final_docs = Vec::with_capacity(sections.len());
    for (i, section) in sections.iter().enumerate() {
        if section.is_empty() {
            continue;
        }

        let doc = RcDoc::concat(section.clone());
        final_docs.push(doc);

        if i < sections.len() - 1 && !sections[i + 1].is_empty() {
            final_docs.push(RcDoc::hardline());
        }
    }

    RcDoc::concat(final_docs)
}
