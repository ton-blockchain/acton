use crate::Context;
use crate::comments::CommentKind;
use pretty::RcDoc;
use tree_sitter::Node;

pub fn print_comment_node<'a>(ctx: &Context, comment: &Node) -> RcDoc<'a> {
    let text = comment
        .utf8_text(ctx.code.as_ref().as_ref())
        .unwrap_or("");
    RcDoc::text(text.to_owned())
}

pub fn print_node_text<'a>(ctx: &Context, ident: &Node) -> Option<RcDoc<'a>> {
    let text = ident.utf8_text(ctx.code.as_ref().as_ref()).ok()?.to_owned();
    Some(RcDoc::text(text))
}

pub fn empty_lines_between(ctx: &Context, top: &Node, bottom: &Node) -> usize {
    // [
    //
    // ] <- end   position of top
    // ( <- start position of botton
    //
    // )

    let botton_line = start_line(ctx, bottom);
    let top_line = top.end_position().row;
    botton_line.saturating_sub(top_line)
}

fn start_line(ctx: &Context, node: &Node) -> usize {
    if let Some(comments) = ctx.comments.get(node) {
        let leading = comments.iter().find(|c| {
            matches!(
                c.kind,
                CommentKind::Leading | CommentKind::LeadingWithEmptyLine
            )
        });
        if let Some(leading) = leading {
            // there is some leading comment so return its line
            if let Some(first_node) = leading.nodes.first() {
                return first_node.start_position().row;
            }
        }
        // no leading comments so start line is start line of node itself
        node.start_position().row
    } else {
        // no comments so start line is start line of node itself
        node.start_position().row
    }
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
