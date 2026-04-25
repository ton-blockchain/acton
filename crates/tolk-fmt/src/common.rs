use crate::comments::CommentKind;
use crate::pretty::RcDoc;
use crate::{Context, comments};
use tree_sitter::Node;

pub struct ListOptions<'a> {
    pub separator: RcDoc<'a>,
    pub brackets: (RcDoc<'a>, RcDoc<'a>),
    pub multiline_threshold: usize,
    pub single_line_edge_space: bool,
    pub never_break_if_items_lt: usize,
}

impl Default for ListOptions<'_> {
    fn default() -> Self {
        Self {
            separator: RcDoc::text(","),
            brackets: (RcDoc::text("("), RcDoc::text(")")),
            multiline_threshold: 5,
            single_line_edge_space: false,
            never_break_if_items_lt: 0,
        }
    }
}

impl<'a> ListOptions<'a> {
    #[must_use]
    pub fn curly_bracket_body() -> ListOptions<'a> {
        ListOptions {
            brackets: (RcDoc::text("{"), RcDoc::text("}")),
            separator: RcDoc::nil(),
            multiline_threshold: 0, // always break
            ..Default::default()
        }
    }

    #[must_use]
    pub fn triangle_bracket_list() -> ListOptions<'a> {
        ListOptions {
            brackets: (RcDoc::text("<"), RcDoc::text(">")),
            ..Default::default()
        }
    }
}

struct ItemDocInfo<'tree, 'a, 'ctx> {
    doc: RcDoc<'a>,
    comments: Option<&'ctx Vec<comments::Comment<'tree>>>,
    node: Node<'tree>,
    ignored: bool,
    group_max_width: usize,
}

pub fn print_list<'a, 'tree, T, F, N, P>(
    ctx: &Context<'tree>,
    items: &[T],
    item_printer: F,
    node_extractor: N,
    lonely_comments_extractor: P,
    options: ListOptions<'a>,
) -> Option<RcDoc<'a>>
where
    F: Fn(&Context<'tree>, &T) -> Option<RcDoc<'a>>,
    N: Fn(&T) -> Node<'tree>,
    P: FnOnce(&Context<'tree>) -> Vec<Node<'tree>>,
{
    if items.is_empty() {
        let lonely_comments = lonely_comments_extractor(ctx);

        if lonely_comments.is_empty() {
            return Some(RcDoc::concat([options.brackets.0, options.brackets.1]));
        }

        let mut docs = vec![RcDoc::hardline()];
        for comment in lonely_comments {
            docs.push(print_comment_node(ctx, &comment));
            docs.push(RcDoc::hardline());
        }

        return Some(RcDoc::concat([
            options.brackets.0,
            RcDoc::concat(docs).nest(4),
            options.brackets.1,
        ]));
    }

    let has_comments = items.iter().any(|item| {
        ctx.comments
            .get(&node_extractor(item))
            .is_some_and(|cs| !cs.is_empty())
    });

    let force_single_line = options.never_break_if_items_lt > 0
        && items.len() < options.never_break_if_items_lt
        && !has_comments;

    let is_multiline =
        !force_single_line && (items.len() > options.multiline_threshold || has_comments);
    let (separator, item_separator) = if is_multiline {
        (RcDoc::hardline(), RcDoc::hardline())
    } else if force_single_line {
        (RcDoc::space(), RcDoc::space())
    } else {
        (RcDoc::line(), RcDoc::line())
    };

    let mut item_docs_with_info: Vec<ItemDocInfo<'tree, 'a, '_>> = Vec::with_capacity(items.len());
    let mut current_group_start = 0;
    let mut current_group_max_width = 0;

    let sep_width = doc_width(&options.separator);

    for (i, item) in items.iter().enumerate() {
        let node = node_extractor(item);
        let comments = ctx.comments.get(&node);

        // check if we need to start a new group for alignment
        if i > 0 {
            let prev_node = node_extractor(&items[i - 1]);
            if empty_lines_between(ctx, &prev_node, &node) > 1 {
                // new group started, backfill previous group with their max width
                for info in item_docs_with_info
                    .iter_mut()
                    .take(i)
                    .skip(current_group_start)
                {
                    info.group_max_width = current_group_max_width;
                }
                current_group_start = i;
                current_group_max_width = 0;
            }
        }

        if comments::has_fmt_ignore(ctx, comments) {
            let doc = print_original_node_text(ctx, &node);
            item_docs_with_info.push(ItemDocInfo {
                doc,
                comments: None,
                node,
                ignored: true,
                group_max_width: 0,
            });
            continue;
        }

        let doc = item_printer(ctx, item)?;
        let width = doc_width(&doc) + sep_width;

        let has_inline =
            comments.is_some_and(|cs| cs.iter().any(|c| c.kind == CommentKind::Inline));

        if has_inline && is_multiline {
            current_group_max_width = current_group_max_width.max(width);
        }
        item_docs_with_info.push(ItemDocInfo {
            doc,
            comments,
            node,
            ignored: false,
            group_max_width: 0,
        });
    }

    // fill max width for the last group
    let len = item_docs_with_info.len();
    for info in item_docs_with_info
        .iter_mut()
        .take(len)
        .skip(current_group_start)
    {
        info.group_max_width = current_group_max_width;
    }

    let mut docs = vec![if is_multiline {
        separator.clone()
    } else if force_single_line {
        if options.single_line_edge_space {
            RcDoc::space()
        } else {
            RcDoc::nil()
        }
    } else if options.single_line_edge_space {
        RcDoc::line()
    } else {
        RcDoc::line_()
    }];

    for (i, info) in item_docs_with_info.into_iter().enumerate() {
        let is_last = i == len - 1;

        if !info.ignored {
            comments::print_leading_comments(ctx, &mut docs, info.comments);
        }

        docs.push(info.doc);

        if !info.ignored {
            if is_last {
                if force_single_line {
                    docs.push(RcDoc::nil());
                } else {
                    docs.push(RcDoc::flat_alt(options.separator.clone(), RcDoc::nil()));
                }
            } else {
                docs.push(options.separator.clone());
            }

            if is_multiline {
                comments::print_inline_comments_with_alignment(
                    ctx,
                    &mut docs,
                    info.comments,
                    info.group_max_width,
                );
            } else {
                comments::print_inline_comments(ctx, &mut docs, info.comments);
            }
        }

        if is_last {
            if is_multiline {
                if !info.ignored {
                    docs.push(RcDoc::hardline());
                }
            } else if force_single_line {
                if options.single_line_edge_space {
                    docs.push(RcDoc::space());
                }
            } else if options.single_line_edge_space {
                docs.push(RcDoc::line());
            } else {
                docs.push(RcDoc::line_());
            }
        } else if !info.ignored {
            docs.push(item_separator.clone());
        }

        if !info.ignored {
            comments::print_trailing_comments(ctx, &mut docs, info.comments);
        }

        // Preserve empty lines between items
        if let Some(next) = items.get(i + 1)
            && empty_lines_between(ctx, &info.node, &node_extractor(next)) > 1
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

#[must_use]
pub fn print_comment_node<'a>(ctx: &Context<'_>, comment: &Node) -> RcDoc<'a> {
    let text = comment.utf8_text(ctx.code.as_ref().as_ref()).unwrap_or("");
    RcDoc::text(text.to_owned())
}

#[must_use]
pub fn print_original_node_text<'a>(ctx: &Context<'_>, node: &Node) -> RcDoc<'a> {
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

#[must_use]
pub fn print_node_text<'a>(ctx: &Context<'_>, ident: &Node) -> Option<RcDoc<'a>> {
    let text = ident.utf8_text(ctx.code.as_ref().as_ref()).ok()?.to_owned();
    Some(RcDoc::text(text))
}

#[must_use]
pub fn empty_lines_between(ctx: &Context<'_>, top: &Node, bottom: &Node) -> usize {
    // [
    //
    // ] <- end   position of top
    // ( <- start position of bottom
    //
    // )

    let bottom_line = start_line(ctx, bottom);
    let top_line = top.end_position().row;
    bottom_line.saturating_sub(top_line)
}

#[allow(clippy::branches_sharing_code)] // for readability
fn start_line(ctx: &Context<'_>, node: &Node) -> usize {
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

#[must_use]
#[allow(clippy::needless_pass_by_value)]
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

#[must_use]
pub fn doc_width(doc: &RcDoc) -> usize {
    struct MeasureWriter {
        last_line_len: usize,
    }

    impl crate::pretty::Render for MeasureWriter {
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

    impl<'a, A> crate::pretty::RenderAnnotated<'a, A> for MeasureWriter {
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
