use crate::comments::CommentKind;
use crate::{Context, comments};
use pretty::RcDoc;
use tree_sitter::Node;

pub fn print_comment_node<'a>(ctx: &Context, comment: &Node) -> RcDoc<'a> {
    let text = comment.utf8_text(ctx.code.as_ref().as_ref()).unwrap_or("");
    RcDoc::text(text.to_owned())
}

pub fn print_original_node_text<'a>(ctx: &Context, node: &Node) -> RcDoc<'a> {
    let mut docs = vec![];
    let comments = ctx.comments.get(node);

    comments::print_leading_comments(ctx, &mut docs, comments);

    let text = node.utf8_text(ctx.code.as_ref().as_ref()).unwrap_or("");
    let mut text = text.to_owned();

    // semicolon is not a part of some nodes in the CST, so we need to add it manually if missing
    let need_semicolon = matches!(
        node.kind(),
        "local_vars_declaration"
            | "return_statement"
            | "do_while_statement"
            | "break_statement"
            | "continue_statement"
            | "throw_statement"
            | "assert_statement"
            | "expression_statement"
    );

    if need_semicolon && !text.ends_with(';') {
        text.push(';');
    }

    docs.push(RcDoc::text(text));

    comments::print_inline_comments(ctx, &mut docs, comments);
    docs.push(RcDoc::hardline());
    comments::print_trailing_comments(ctx, &mut docs, comments);

    RcDoc::concat(docs)
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

pub fn doc_width(doc: &RcDoc) -> usize {
    struct MeasureWriter {
        last_line_len: usize,
    }

    impl pretty::Render for MeasureWriter {
        type Error = std::fmt::Error;

        fn write_str(&mut self, s: &str) -> Result<usize, Self::Error> {
            for c in s.chars() {
                if c == '\n' {
                    self.last_line_len = 0;
                } else {
                    self.last_line_len += 1;
                }
            }
            Ok(s.len())
        }

        fn fail_doc(&self) -> Self::Error {
            std::fmt::Error
        }
    }

    impl<'a, A> pretty::RenderAnnotated<'a, A> for MeasureWriter {
        fn push_annotation(&mut self, _: &'a A) -> Result<(), Self::Error> {
            Ok(())
        }

        fn pop_annotation(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let mut writer = MeasureWriter { last_line_len: 0 };
    doc.render_raw(usize::MAX, &mut writer).ok();
    writer.last_line_len
}
