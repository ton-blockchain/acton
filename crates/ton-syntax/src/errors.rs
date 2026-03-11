use std::cmp::Ordering;
use std::sync::Arc;
use tree_sitter::{Language, Point, Tree};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    pub start: Point,
    pub end: Point,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParseErrorKind {
    Unexpected,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
    pub message: String,
    pub expected: Vec<String>,
}

/// Collects parser errors for ERROR/MISSING nodes.
pub fn collect_errors(source: &Arc<str>, tree: &Tree, language: &Language) -> Vec<ParseError> {
    let root = tree.root_node();
    if !root.has_error() {
        return vec![];
    }

    let mut out = Vec::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        for i in (0..node.child_count()).rev() {
            if let Some(child) = node.child(i) {
                stack.push(child);
            }
        }

        if node.is_error() {
            let expected = expected_symbols(language, node.parse_state());
            let text = node
                .utf8_text(source.as_bytes())
                .ok()
                .map(str::trim)
                .unwrap_or("");
            let mut message = if text.is_empty() {
                "syntax error: unexpected fragment.".to_string()
            } else {
                format!("syntax error: unexpected `{}`.", truncate(text, 60))
            };

            if !expected.is_empty() {
                message.push_str(" Expected: ");
                message.push_str(&expected.join(", "));
                message.push('.');
            }

            out.push(ParseError {
                kind: ParseErrorKind::Unexpected,
                span: Span {
                    start: node.start_position(),
                    end: node.end_position(),
                },
                message,
                expected,
            });
        }

        if node.is_missing() {
            let expected = expected_symbols(language, node.parse_state());
            let missing_kind = node.kind().to_string();
            let mut message = format!("syntax error: missing `{}`.", missing_kind);
            if !expected.is_empty() {
                message.push_str(" Valid here: ");
                message.push_str(&expected.join(", "));
                message.push('.');
            }

            out.push(ParseError {
                kind: ParseErrorKind::Missing,
                span: Span {
                    start: node.start_position(),
                    end: node.end_position(),
                },
                message,
                expected,
            });
        }
    }

    out.sort_by(|a, b| {
        a.span
            .cmp(&b.span)
            .then_with(|| a.kind.cmp(&b.kind))
            .then_with(|| compare_message_len(&a.message, &b.message))
    });
    out.dedup_by(|a, b| a.kind == b.kind && a.span == b.span && a.message == b.message);
    out
}

fn expected_symbols(language: &Language, state: u16) -> Vec<String> {
    let Some(mut it) = language.lookahead_iterator(state) else {
        return vec![];
    };

    let mut out = Vec::new();
    for name in it.iter_names().take(16) {
        let name = name.trim();
        if name.is_empty() || name == "ERROR" {
            continue;
        }
        if out.iter().any(|existing: &String| existing == name) {
            continue;
        }
        out.push(name.to_string());
    }
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }

    let mut t = s[..max].to_string();
    t.push('…');
    t
}

fn compare_message_len(a: &str, b: &str) -> Ordering {
    a.len().cmp(&b.len())
}
