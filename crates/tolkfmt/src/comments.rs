use crate::TreeWalker;
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

        // Комментарий может быть привязан в нескольких вариантах
        //
        // В той же строке (inline):
        // ```
        // a: 10 // comment here
        // ```
        //
        // Над определением (leading):
        // ```
        // // comment here
        // a: 10
        // ```
        //
        // После определения (trailing):
        // ```
        // a: 10
        // // comment here
        // ```
        //
        // Но этот вариант существует ТОЛЬКО если этот комментарий не привязан к другому
        // узлу как leading.

        let prev_sibling = comment.prev_named_sibling();
        let next_sibling = comment.next_named_sibling();

        if let Some(prev_sibling) = prev_sibling {
            // Если комментарий находится на той же строке, что и узел до него,
            // то мы всегда считаем что этот комментарий inline по отношению к
            // этому узлу
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

        // Если после комментария есть узел, то комментарий может быть
        // привязан к этому узла.
        if let Some(next_sibling) = next_sibling {
            // если комментарий находится прямо перед узлами, то мы считаем что они связаны
            if next_sibling.start_position().row == comment.end_position().row + 1 {
                comments_map.entry(next_sibling).or_default().push(Comment {
                    kind: CommentKind::Leading,
                    comment,
                });
                continue;
            }
        }

        // иначе мы считаем что он trailing для предыдущего узла
        if let Some(prev_sibling) = prev_sibling {
            // Если перед комментарием идет другой комментарий, то это может быть
            // как группа, так и разные группы комментариев:
            //
            // ```
            // a: 10, // comment
            // // comment 2
            // b: 20,
            // ```
            //
            // В этом примере мы можем считать два комментария единой группой,
            // а можем как две отдельные. И это сложный вопрос, потому что пользователь
            // мог хотеть написать что-то типа:
            //
            // ```
            // a: 10, // comment
            //        // comment 2
            // b: 20,
            // ```
            //
            // Но при этом мог и не хотеть, поэтому связывание этих двух комментариев задача
            // нетривиальная. Возможно мы можем смотреть на расположение комментария относительно
            // следующего узла и если комментарий сильно смещен вбок, то считать что комментарии
            // предполагается держать вместе.
            if prev_sibling.kind() == "comment" {
                // Если комментарии идут друг за другом пока что считаем их единой группой
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
            // если предыдущего узла нет, то нам не остается ничего кроме как
            // все же считать узел как leading даже если они не идут друг за другом.
            comments_map.entry(next_sibling).or_default().push(Comment {
                kind: CommentKind::LeadingWithEmptyLine,
                comment,
            });
        }
    }
    comments_map
}
