use comments::Comment;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::time::Instant;
use tolk_ast::SourceFile;
use tree_sitter::Node;

mod common;
mod decls;
mod exprs;
mod stmts;
mod types;

mod comments;
#[cfg(test)]
mod decls_tests;
#[cfg(test)]
mod exprs_tests;
#[cfg(test)]
mod stmts_tests;
#[cfg(test)]
mod types_tests;

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
    comments: HashMap<Node<'tree>, Vec<Comment<'tree>>>,
}

fn main() {
    // let code = fs::read_to_string("/Users/petrmakhnev/emulator-rs/.jetton/tests/wallet.test.tolk")
    //     .unwrap();
    // let code = fs::read_to_string(
    //     "/Users/petrmakhnev/emulator-rs/.jetton/contracts/jetton-wallet-contract.tolk",
    // )
    // .unwrap();
    // let code = code.as_str();

    let code = "
    fun main() {
        foo(
            // first comment
            a,
        );
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

    let comments_map = comments::collect_comments(root);

    for (node, comments) in &comments_map {
        for comment in comments {
            println!(
                "{} -- {:?} -> {} {}",
                comment.comment.utf8_text(code.as_bytes()).unwrap(),
                comment.kind,
                node.utf8_text(code.as_bytes()).unwrap(),
                node.id(),
            );
        }
    }

    let now = Instant::now();
    let source_file = SourceFile {
        tree: tree.clone(),
        source: code.into(),
    };
    let comments_map = comments::collect_comments(root);
    let ctx = Context {
        code: code.into(),
        comments: comments_map,
    };
    let doc = decls::print_source_file(&ctx, &source_file).unwrap();

    let mut out = Vec::with_capacity(code.len());
    doc.render(100, &mut out).unwrap();
    println!("{}", String::from_utf8(out).unwrap());
    println!("tolkfmt took {:?}", now.elapsed());
}
