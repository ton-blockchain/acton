use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use tolk_ast::SourceFile;
use tree_sitter::Node;

mod common;
mod decls;
mod exprs;
mod stmts;
mod types;

#[cfg(test)]
mod exprs_tests;
#[cfg(test)]
mod stmts_tests;
#[cfg(test)]
mod types_tests;

#[derive(Debug)]
#[allow(dead_code)]
enum CommentKind {
    Inline,
    Continuation,
    Leading,
    Trailing,
}

#[allow(dead_code)]
struct Comment<'tree> {
    kind: CommentKind,
    node: Node<'tree>,
    attach_to: Node<'tree>,
}

struct CommentGroup<'tree> {
    comments: Vec<Node<'tree>>,
}

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

//  const foo = Foo { // comment 0
//         a: 10, // comment 1
//         // comment 2
//         b: 20,
//         // comment 3
//     }

struct Context<'tree> {
    code: Rc<str>,
    #[allow(dead_code)]
    comments: HashMap<Node<'tree>, Comment<'tree>>,
}

fn main() {
    let code = "
    fun foo() {
        val a = 100;
    }

    fun main() {
        val a = true
            ? 1 // if true
            // comment 0
            : 0; // if false
    }
    ";
    let tree = tolk_parser::parser::parse(code).unwrap();

    let root = tree.root_node();

    let mut comments = TreeWalker::new(root)
        .filter(|n| n.kind() == "comment")
        .collect::<VecDeque<_>>();

    let mut groups = vec![];
    let mut current_group = CommentGroup {
        comments: Vec::new(),
    };
    let mut previous_comment: Option<Node> = None;
    while let Some(comment) = comments.pop_front() {
        let Some(prev) = previous_comment else {
            previous_comment = Some(comment);
            current_group.comments.push(comment);
            continue;
        };

        println!("prev {}", prev.utf8_text(code.as_bytes()).unwrap(),);
        println!("process {}", comment.utf8_text(code.as_bytes()).unwrap(),);

        // if current is right after previous
        let next_s = prev.next_named_sibling();
        if let Some(next_s) = next_s {
            println!("next_s {}", next_s.utf8_text(code.as_bytes()).unwrap(),);
        }

        // comment after comment with single new line
        let in_row = prev.end_position().row + 1 == comment.end_position().row;
        if next_s == Some(comment) && in_row {
            current_group.comments.push(comment);
        } else {
            // new group
            groups.push(current_group);
            current_group = CommentGroup {
                comments: vec![comment],
            };
        }

        previous_comment = Some(comment)
    }

    if !current_group.comments.is_empty() {
        groups.push(current_group);
    }

    let mut comments_map = HashMap::new();

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
                comments_map.insert(
                    comment,
                    Comment {
                        kind: CommentKind::Inline,
                        node: comment,
                        attach_to: prev_sibling,
                    },
                );

                continue;
            }
        }

        // Если после комментария есть узел, то комментарий может быть
        // привязан к этому узла.
        if let Some(next_sibling) = next_sibling {
            // если комментарий находится прямо перед узлами, то мы считаем что они связаны
            if next_sibling.start_position().row == comment.end_position().row + 1 {
                comments_map.insert(
                    comment,
                    Comment {
                        kind: CommentKind::Leading,
                        node: comment,
                        attach_to: next_sibling,
                    },
                );
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
                if prev_sibling.end_position().row + 1 == comment.start_position().row {
                    comments_map.insert(
                        comment,
                        Comment {
                            kind: CommentKind::Continuation,
                            node: comment,
                            attach_to: prev_sibling,
                        },
                    );

                    continue;
                }
            }

            comments_map.insert(
                comment,
                Comment {
                    kind: CommentKind::Trailing,
                    node: comment,
                    attach_to: prev_sibling,
                },
            );
            continue;
        }

        if let Some(next_sibling) = next_sibling {
            // если предыдущего узла нет, то нам не остается ничего кроме как
            // все же считать узел как leading даже если они не идут друг за другом.
            comments_map.insert(
                comment,
                Comment {
                    kind: CommentKind::Leading,
                    node: comment,
                    attach_to: next_sibling,
                },
            );
        }
    }

    // for (comment, to) in &comments_map {
    //     println!(
    //         "{} -- {:?} -> {}",
    //         comment.utf8_text(code.as_bytes()).unwrap(),
    //         to.kind,
    //         to.attach_to.utf8_text(code.as_bytes()).unwrap(),
    //     );
    // }

    let source_file = SourceFile {
        tree: tree.clone(),
        source: code.into(),
    };
    let mut ctx = Context {
        code: code.into(),
        comments: comments_map,
    };
    let doc = decls::print_source_file(&mut ctx, &source_file).unwrap();

    let mut out = Vec::new();
    doc.render(80, &mut out).unwrap();
    println!("{}", String::from_utf8(out).unwrap());
}
