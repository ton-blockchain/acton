use crate::comments::CommentKind;
use crate::{Context, comments};
use pretty::RcDoc;
use tree_sitter::Node;

pub struct ListOptions<'a> {
    pub separator: RcDoc<'a>,
    pub brackets: (RcDoc<'a>, RcDoc<'a>),
    pub multiline_threshold: usize,
    pub single_line_edge_space: bool,
}

impl<'a> Default for ListOptions<'a> {
    fn default() -> Self {
        Self {
            separator: RcDoc::text(","),
            brackets: (RcDoc::text("("), RcDoc::text(")")),
            multiline_threshold: 5,
            single_line_edge_space: false,
        }
    }
}

impl<'a> ListOptions<'a> {
    pub fn curly_bracket_body() -> ListOptions<'a> {
        ListOptions {
            brackets: (RcDoc::text("{"), RcDoc::text("}")),
            separator: RcDoc::nil(),
            multiline_threshold: 0, // always break
            ..Default::default()
        }
    }

    pub fn triangle_bracket_list() -> ListOptions<'a> {
        ListOptions {
            brackets: (RcDoc::text("<"), RcDoc::text(">")),
            ..Default::default()
        }
    }
}

pub fn print_list<'a, 'tree, T, F, N>(
    ctx: &Context<'tree>,
    items: &[T],
    item_printer: F,
    node_extractor: N,
    options: ListOptions<'a>,
) -> Option<RcDoc<'a>>
where
    F: Fn(&Context<'tree>, &T) -> Option<RcDoc<'a>>,
    N: Fn(&T) -> Node<'tree>,
{
    if items.is_empty() {
        return Some(RcDoc::concat([
            options.brackets.0.clone(),
            options.brackets.1.clone(),
        ]));
    }

    let has_comments = items.iter().any(|item| {
        ctx.comments
            .get(&node_extractor(item))
            .is_some_and(|cs| !cs.is_empty())
    });

    let is_multiline = items.len() > options.multiline_threshold || has_comments;
    let (separator, item_separator) = if is_multiline {
        (RcDoc::hardline(), RcDoc::hardline())
    } else {
        (RcDoc::line(), RcDoc::line())
    };

    let mut item_docs_with_info = Vec::with_capacity(items.len());
    let mut max_width = 0;

    let sep_width = doc_width(&options.separator);

    for (i, item) in items.iter().enumerate() {
        let node = node_extractor(item);
        let comments = ctx.comments.get(&node);

        if comments::has_fmt_ignore(ctx, comments) {
            let doc = print_original_node_text(ctx, &node);
            item_docs_with_info.push((doc, comments, node));
            continue;
        }

        let doc = item_printer(ctx, item)?;
        let mut width = doc_width(&doc);

        let is_last = i == items.len() - 1;
        if !is_last {
            width += sep_width;
        } else {
            // we don't know if it will be multiline or not here for width calculation purposes
            // but alignment is only for multiline, so we should assume trailing comma width
            width += sep_width;
        }

        let has_inline =
            comments.is_some_and(|cs| cs.iter().any(|c| c.kind == CommentKind::Inline));

        if has_inline && is_multiline {
            max_width = max_width.max(width);
        }
        item_docs_with_info.push((doc, comments, node));
    }

    let mut docs = vec![if is_multiline {
        separator.clone()
    } else if options.single_line_edge_space {
        RcDoc::line()
    } else {
        RcDoc::line_()
    }];

    let len = item_docs_with_info.len();
    for (i, (item_doc, comments, node)) in item_docs_with_info.into_iter().enumerate() {
        let is_last = i == len - 1;

        comments::print_leading_comments(ctx, &mut docs, comments);

        docs.push(item_doc);

        if !is_last {
            docs.push(options.separator.clone());
        } else {
            docs.push(RcDoc::flat_alt(options.separator.clone(), RcDoc::nil()));
        }

        if is_multiline {
            comments::print_inline_comments_with_alignment(ctx, &mut docs, comments, max_width);
        } else {
            comments::print_inline_comments(ctx, &mut docs, comments);
        }

        if is_last {
            if is_multiline {
                docs.push(RcDoc::hardline());
            } else if options.single_line_edge_space {
                docs.push(RcDoc::line());
            } else {
                docs.push(RcDoc::line_());
            }
        } else {
            docs.push(item_separator.clone());
        }

        comments::print_trailing_comments(ctx, &mut docs, comments);

        // Preserve empty lines between items
        if let Some(next) = items.get(i + 1)
            && empty_lines_between(ctx, &node, &node_extractor(next)) > 1
        {
            docs.push(RcDoc::hardline());
        }
    }

    Some(RcDoc::group(RcDoc::concat([
        options.brackets.0,
        RcDoc::concat(docs).nest(4),
        options.brackets.1,
    ])))
}

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

    // final new line
    final_docs.push(RcDoc::hardline());

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
