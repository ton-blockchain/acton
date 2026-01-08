use crate::{Context, common};
use pretty::RcDoc;
use std::collections::HashMap;
use tree_sitter::Node;

struct TreeWalker<'a> {
    cursor: tree_sitter::TreeCursor<'a>,
    started: bool,
    finished: bool,
}

impl<'a> TreeWalker<'a> {
    fn new(node: Node<'a>) -> Self {
        Self {
            cursor: node.walk(),
            started: false,
            finished: false,
        }
    }
}

impl<'a> Iterator for TreeWalker<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        if !self.started {
            self.started = true;
            return Some(self.cursor.node());
        }

        if self.cursor.goto_first_child() {
            return Some(self.cursor.node());
        }

        if self.cursor.goto_next_sibling() {
            return Some(self.cursor.node());
        }

        loop {
            if !self.cursor.goto_parent() {
                self.finished = true;
                return None;
            }
            if self.cursor.goto_next_sibling() {
                return Some(self.cursor.node());
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum CommentKind {
    Inline,
    Leading,
    LeadingWithEmptyLine,
    Trailing,
}

#[derive(Clone)]
pub struct Comment<'tree> {
    pub(crate) kind: CommentKind,
    /// A group of comments that follow each other and should be treated as one.
    pub(crate) nodes: Vec<Node<'tree>>,
}

fn next_non_comment_sibling(node: Node) -> Option<Node> {
    let mut cur = node.next_named_sibling();
    while let Some(n) = cur {
        if n.kind() != "comment" {
            return Some(n);
        }
        cur = n.next_named_sibling();
    }
    None
}

fn prev_non_comment_sibling(node: Node) -> Option<Node> {
    let mut cur = node.prev_named_sibling();
    while let Some(n) = cur {
        if n.kind() != "comment" {
            return Some(n);
        }
        cur = n.prev_named_sibling();
    }
    None
}

pub fn collect_comments(root: Node) -> HashMap<Node, Vec<Comment>> {
    let mut comments_map: HashMap<Node, Vec<Comment>> = HashMap::new();

    // First, we find a potential "owner" for each comment.
    // A comment can be attached in several ways:
    //
    // In the same line (inline):
    // ```
    // a: 10 // comment here
    // ```
    //
    // Above the definition (leading):
    // ```
    // // comment here
    // a: 10
    // ```
    //
    // After the definition (trailing):
    // ```
    // a: 10
    // // comment here
    // ```
    let mut comments_with_owner = Vec::new();
    for comment in TreeWalker::new(root) {
        if comment.kind() != "comment" {
            continue;
        }

        // We use "non-comment" versions of siblings to find the actual code node
        // that this comment might belong to, skipping over other comments in a block.
        //
        // Example for `next`:
        // ```
        // // comment 1  <-- when processing this, next_non_comment is `val a`
        // // comment 2
        // val a = 10;
        // ```
        //
        // Example for `prev`:
        // ```
        // val a = 10;
        // // comment 1
        // // comment 2  <-- when processing this, prev_non_comment is `val a`
        // ```
        let next = next_non_comment_sibling(comment);
        let prev = prev_non_comment_sibling(comment);

        if let Some(p) = prev
            && p.end_position().row == comment.start_position().row
        {
            // If the comment is on the same line as the previous node,
            // we always consider this comment as inline relative to that node
            comments_with_owner.push((comment, p, CommentKind::Inline));
        } else if let Some(n) = next {
            // If there is a node after the comment, we prefer to attach it
            // as leading to that node.
            comments_with_owner.push((comment, n, CommentKind::Leading));
        } else if let Some(p) = prev {
            // Otherwise, we consider it trailing for the previous node
            comments_with_owner.push((comment, p, CommentKind::Trailing));
        }
    }

    if comments_with_owner.is_empty() {
        return comments_map;
    }

    // Now we group comments that follow each other and have the same owner/kind.
    // This allows us to handle blocks of comments as a single entity.
    let mut i = 0;
    while i < comments_with_owner.len() {
        let (comment, owner, initial_kind) = comments_with_owner[i];
        let mut group_nodes = vec![comment];

        let mut j = i + 1;
        while j < comments_with_owner.len() {
            let (next_comment, next_owner, next_initial_kind) = comments_with_owner[j];

            // We group comments if they have the same owner, same attachment kind,
            // and follow each other on consecutive lines (or the same line).
            let is_consecutive = group_nodes.last().is_some_and(|last| {
                next_comment.start_position().row == last.end_position().row + 1
                    || next_comment.start_position().row == last.end_position().row
            });

            if next_owner == owner && next_initial_kind == initial_kind && is_consecutive {
                group_nodes.push(next_comment);
                j += 1;
            } else {
                break;
            }
        }

        // For leading comments, we distinguish between comments directly before
        // the node and comments separated by an empty line.
        let final_kind = match initial_kind {
            CommentKind::Leading => {
                let last_comment_row = group_nodes
                    .last()
                    .map(|n| n.end_position().row)
                    .unwrap_or(0);
                if owner.start_position().row == last_comment_row + 1 {
                    CommentKind::Leading
                } else {
                    CommentKind::LeadingWithEmptyLine
                }
            }
            k => k,
        };

        comments_map.entry(owner).or_default().push(Comment {
            kind: final_kind,
            nodes: group_nodes,
        });

        i = j;
    }

    comments_map
}

pub fn print_leading_comments(
    ctx: &Context,
    docs: &mut Vec<RcDoc>,
    comments: Option<&Vec<Comment>>,
) {
    let Some(comments) = comments else {
        return;
    };

    for comment in comments {
        if matches!(
            comment.kind,
            CommentKind::Leading | CommentKind::LeadingWithEmptyLine
        ) {
            for node in &comment.nodes {
                docs.push(common::print_comment_node(ctx, node));
                docs.push(RcDoc::hardline());
            }
            if comment.kind == CommentKind::LeadingWithEmptyLine {
                docs.push(RcDoc::hardline());
            }
        }
    }
}

pub fn print_trailing_comments(
    ctx: &Context,
    docs: &mut Vec<RcDoc>,
    comments: Option<&Vec<Comment>>,
) {
    let Some(comments) = comments else {
        return;
    };

    for comment in comments {
        if comment.kind == CommentKind::Trailing {
            for node in &comment.nodes {
                docs.push(common::print_comment_node(ctx, node));
                docs.push(RcDoc::hardline());
            }
        }
    }
}

pub fn print_inline_comments(
    ctx: &Context,
    docs: &mut Vec<RcDoc>,
    comments: Option<&Vec<Comment>>,
) {
    print_inline_comments_with_alignment(ctx, docs, comments, 0);
}

pub fn print_inline_comments_with_alignment(
    ctx: &Context,
    docs: &mut Vec<RcDoc>,
    comments: Option<&Vec<Comment>>,
    target_width: usize,
) {
    let Some(comments) = comments else {
        return;
    };

    let mut inline_comments = comments.iter().filter(|c| c.kind == CommentKind::Inline).peekable();
    if inline_comments.peek().is_none() {
        return;
    }

    if target_width > 0 {
        docs.push(RcDoc::nesting(move |indent| {
            RcDoc::column(move |col| {
                let current_width = col.saturating_sub(indent);
                if target_width > current_width {
                    RcDoc::text(" ".repeat(target_width - current_width))
                } else {
                    RcDoc::nil()
                }
            })
        }));
    }

    for comment in inline_comments {
        for node in &comment.nodes {
            docs.push(RcDoc::space());
            docs.push(common::print_comment_node(ctx, node));
        }
    }
}

pub fn has_fmt_ignore(ctx: &Context, comments: Option<&Vec<Comment>>) -> bool {
    let Some(comments) = comments else {
        return false;
    };

    comments.iter().any(|c| {
        c.kind == CommentKind::Leading
            && c.nodes.iter().any(|n| {
                let text = n.utf8_text(ctx.code.as_ref().as_bytes()).unwrap_or("");
                text.trim() == "// fmt-ignore"
            })
    })
}
