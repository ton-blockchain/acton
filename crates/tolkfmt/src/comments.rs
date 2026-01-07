use crate::{Context, TreeWalker, common};
use pretty::RcDoc;
use std::collections::HashMap;
use tree_sitter::Node;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) enum CommentKind {
    Inline,
    Leading,
    LeadingWithEmptyLine,
    Trailing,
}

#[derive(Clone, Copy)]
pub(crate) struct Comment<'tree> {
    pub(crate) kind: CommentKind,
    pub(crate) comment: Node<'tree>,
}

pub fn collect_comments(root: Node) -> HashMap<Node, Vec<Comment>> {
    let mut comments_map: HashMap<Node, Vec<Comment>> = HashMap::new();

    for comment in TreeWalker::new(root) {
        if comment.kind() != "comment" {
            continue;
        }

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
        //
        // But this option exists ONLY if this comment is not attached to another
        // node as leading.

        let prev_sibling = comment.prev_named_sibling();
        let next_sibling = comment.next_named_sibling();

        if let Some(prev_sibling) = prev_sibling {
            // If the comment is on the same line as the previous node,
            // we always consider this comment as inline relative to
            // that node
            if prev_sibling.end_position().row == comment.start_position().row {
                // on the same line
                let entry = comments_map.entry(prev_sibling).or_default();
                entry.push(Comment {
                    kind: CommentKind::Inline,
                    comment,
                });
                continue;
            }
        }

        // If there is a node after the comment, the comment can be
        // attached to that node.
        if let Some(next_sibling) = next_sibling {
            // if the comment is directly before nodes, we consider them connected
            if next_sibling.start_position().row == comment.end_position().row + 1 {
                comments_map.entry(next_sibling).or_default().push(Comment {
                    kind: CommentKind::Leading,
                    comment,
                });
                continue;
            }
        }

        // otherwise we consider it trailing for the previous node
        if let Some(prev_sibling) = prev_sibling {
            // If another comment comes before this comment, it could be
            // either a group or different groups of comments:
            //
            // ```
            // a: 10, // comment
            // // comment 2
            // b: 20,
            // ```
            //
            // In this example, we can consider the two comments as one group,
            // or as two separate ones. And this is a complex issue because the user
            // might have wanted to write something like:
            //
            // ```
            // a: 10, // comment
            //        // comment 2
            // b: 20,
            // ```
            //
            // But might not have wanted to, so linking these two comments is
            // a non-trivial task. Perhaps we can look at the comment's position relative to
            // the next node and if the comment is significantly offset, consider that comments
            // are meant to be kept together.
            if prev_sibling.kind() == "comment" {
                // If comments follow each other, for now we consider them as one group
                // if prev_sibling.end_position().row + 1 == comment.start_position().row {
                //     comments_map.entry(prev_sibling).or_default().push(Comment {
                //         kind: CommentKind::Continuation,
                //         comment,
                //         attach_to: prev_sibling,
                //     });
                //
                //     continue;
                // }
            } else {
                comments_map.entry(prev_sibling).or_default().push(Comment {
                    kind: CommentKind::Trailing,
                    comment,
                });
                continue;
            }
        }

        if let Some(next_sibling) = next_sibling {
            // if there is no previous node, we have no choice but to
            // consider the node as leading even if they don't follow each other.
            comments_map.entry(next_sibling).or_default().push(Comment {
                kind: CommentKind::LeadingWithEmptyLine,
                comment,
            });
        }
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
            docs.push(common::print_comment(ctx, comment));
            docs.push(RcDoc::hardline());
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
            docs.push(common::print_comment(ctx, comment));
            docs.push(RcDoc::hardline());
        }
    }
}

pub fn print_inline_comments(
    ctx: &Context,
    docs: &mut Vec<RcDoc>,
    comments: Option<&Vec<Comment>>,
) {
    let Some(comments) = comments else {
        return;
    };

    for inline_comment in comments.iter().filter(|c| c.kind == CommentKind::Inline) {
        docs.push(RcDoc::space());
        docs.push(common::print_comment(ctx, inline_comment));
    }
}
