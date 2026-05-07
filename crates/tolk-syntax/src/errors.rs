use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use tree_sitter::{Language, Node, Tree, TreeCursor};

use ton_syntax::errors::{ParseError, ParseErrorKind, Span};

/// Collects errors for ERROR/MISSING nodes.
pub(crate) fn collect_errors(
    source: &Arc<str>,
    tree: &Tree,
    language: &Language,
) -> Vec<ParseError> {
    let root = tree.root_node();
    if !root.has_error() {
        return vec![];
    }

    let bytes = source.as_bytes();

    // raw candidates (nested ERROR and duplicates possible).
    let mut raw = Vec::new();
    walk_tree(root, |n| {
        if n.is_error() {
            raw.push(build_unexpected_error(n, bytes, language));
        } else if n.is_missing() {
            raw.push(build_missing_error(root, n, language));
        }
    });

    let mut out = coalesce_errors(raw);
    out.sort_by(|a, b| a.span.cmp(&b.span).then_with(|| a.kind.cmp(&b.kind)));
    out
}

/// Iterative DFS preorder traversal.
fn walk_tree(node: Node<'_>, mut f: impl FnMut(Node<'_>)) {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        f(n);
        let child_count = n.child_count();
        for i in (0..child_count).rev() {
            if let Some(ch) = n.child(i) {
                stack.push(ch);
            }
        }
    }
}

fn build_unexpected_error(error_node: Node<'_>, bytes: &[u8], language: &Language) -> ParseError {
    // For ERROR: use parse_state of first "significant" leaf inside ERROR.
    // Recommendation: "first leaf node state" for ERROR.
    let leaf = first_non_extra_leaf(error_node)
        .unwrap_or_else(|| first_leaf(error_node).unwrap_or(error_node));
    let state = leaf.parse_state();

    let expected = expected_symbols(language, state);

    // Message: unexpected `<token/snippet>`, plus "expected: ..."
    let unexpected_text = leaf
        .utf8_text(bytes)
        .ok()
        .map(str::to_string)
        .unwrap_or_default();
    let span = node_range_for_display(leaf, error_node);

    let mut message = if unexpected_text.trim().is_empty() {
        "syntax error: unexpected fragment.".to_string()
    } else {
        format!(
            "syntax error: unexpected `{}`.",
            truncate(&unexpected_text, 60)
        )
    };

    if !expected.is_empty() {
        message.push_str(" Expected: ");
        message.push_str(
            &expected
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        if expected.len() > 8 {
            message.push_str(", …");
        }
        message.push('.');
    }

    ParseError {
        kind: ParseErrorKind::Unexpected,
        span,
        message,
        expected,
    }
}

fn build_missing_error(root: Node<'_>, missing_node: Node<'_>, language: &Language) -> ParseError {
    // For MISSING: "previous non-extra leaf node may be appropriate".
    let prev_leaf = previous_non_extra_leaf(root, missing_node);

    // State for expected:
    // - if found prev_leaf: use next_parse_state() (after previous token)
    // - otherwise fallback to missing_node.parse_state()
    let state = if let Some(p) = prev_leaf {
        p.next_parse_state()
    } else {
        missing_node.parse_state()
    };

    let mut expected = expected_symbols(language, state);

    // kind() of missing node usually contains "what is missing" (literal or symbol).
    let missing_kind = missing_node.kind().to_string();

    // Move missing_kind to the beginning of expected (as "most likely"), if not already there.
    if !missing_kind.is_empty() && !expected.iter().any(|x| x == &missing_kind) {
        expected.insert(0, missing_kind.clone());
        expected = dedup_preserve(expected);
    }

    // Range for MISSING:
    // - if missing_node has zero-width (often), calculate "insertion range" between prev and next leaf:
    //   * start = end = position of next leaf start (or prev leaf end)
    // - if range is not empty, use it.
    let span = missing_insertion_range(root, missing_node, prev_leaf);

    let mut message = format!("syntax error: missing `{missing_kind}`.");
    if !expected.is_empty() {
        message.push_str(" Valid here: ");
        message.push_str(
            &expected
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        );
        if expected.len() > 8 {
            message.push_str(", …");
        }
        message.push('.');
    }

    ParseError {
        kind: ParseErrorKind::Missing,
        span,
        message,
        expected,
    }
}

fn expected_symbols(language: &Language, state: u16) -> Vec<String> {
    let mut out = Vec::new();
    let Some(mut it) = language.lookahead_iterator(state) else {
        return out;
    };

    // Iterates symbol names valid at the given parse state.
    // Length limit — protection from noisy grammars/states.
    for name in it.iter_names().take(128) {
        let s = normalize_symbol_name(name);
        if s.is_empty() || s == "ERROR" {
            continue;
        }
        out.push(s);
    }

    out.sort_by(|a, b| compare_expected(a, b));
    dedup_preserve(out)
}

fn normalize_symbol_name(s: &str) -> String {
    s.trim().to_string()
}

fn compare_expected(a: &str, b: &str) -> Ordering {
    let sa = expected_priority(a);
    let sb = expected_priority(b);
    sa.cmp(&sb).then_with(|| a.cmp(b))
}

/// Prioritization of expected symbols, tuned for your grammar (brackets/delimiters/operators/keywords).
fn expected_priority(s: &str) -> u8 {
    match s {
        // closing brackets
        "}" | ")" | "]" | ">" => 0,
        // delimiters
        ";" | "," => 1,
        // important connectors
        "=>" | "->" | ":" | "|" => 2,
        // keywords and common top-level/statement tokens
        "fun" | "get" | "if" | "else" | "while" | "do" | "try" | "catch" | "match" | "return"
        | "throw" | "assert" | "struct" | "enum" | "type" | "const" | "global" | "import"
        | "true" | "false" | "null" | "lazy" | "mutate" | "var" | "val" | "asm" | "builtin"
        | "tolk" => 3,
        // literals/names — as "last" option
        "identifier" | "type_identifier" | "number_literal" | "string_literal" => 7,
        _ => 5,
    }
}

/// First leaf in subtree (via `first_child` until exhausted).
fn first_leaf(mut n: Node<'_>) -> Option<Node<'_>> {
    loop {
        let c = n.child(0)?;
        n = c;
        if n.child_count() == 0 {
            return Some(n);
        }
    }
}

/// First non-extra leaf (skip extras: whitespace/comment).
fn first_non_extra_leaf(n: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = n.walk();
    // Go down to the leftmost leaf, then find first non-extra leaf in document order
    while cursor.goto_first_child() {}
    // Now on the left leaf.
    loop {
        let cur = cursor.node();
        if cur.child_count() == 0 && !cur.is_extra() {
            return Some(cur);
        }
        if cursor.goto_next_sibling() {
            while cursor.goto_first_child() {}
        } else {
            // go up and find next
            while cursor.goto_parent() {
                if cursor.goto_next_sibling() {
                    while cursor.goto_first_child() {}
                    break;
                }
            }
            if cursor.node().id() == n.id() {
                // reached subtree root and nowhere else to go
                return None;
            }
        }
    }
}

/// Previous leaf in document order, skipping extra.
/// Important: cursor must be created from root, and reset to start.
fn previous_non_extra_leaf<'a>(root: Node<'a>, start: Node<'a>) -> Option<Node<'a>> {
    let mut cursor = root.walk();
    cursor.reset(start);

    loop {
        let prev = prev_leaf(&mut cursor)?;
        if !prev.is_extra() {
            return Some(prev);
        }
    }
}

/// Next leaf in document order, skipping extra.
/// Useful for calculating MISSING insertion point between tokens.
fn next_non_extra_leaf<'a>(root: Node<'a>, start: Node<'a>) -> Option<Node<'a>> {
    let mut cursor = root.walk();
    cursor.reset(start);

    loop {
        let nxt = next_leaf(&mut cursor)?;
        if !nxt.is_extra() {
            return Some(nxt);
        }
    }
}

/// Step backward through leaves.
fn prev_leaf<'a>(cursor: &mut TreeCursor<'a>) -> Option<Node<'a>> {
    if cursor.goto_previous_sibling() {
        while cursor.goto_last_child() {}
        return Some(cursor.node());
    }
    while cursor.goto_parent() {
        if cursor.goto_previous_sibling() {
            while cursor.goto_last_child() {}
            return Some(cursor.node());
        }
    }
    None
}

/// Step forward through leaves.
fn next_leaf<'a>(cursor: &mut TreeCursor<'a>) -> Option<Node<'a>> {
    // 1) If there is child — go into it and then to the leftmost leaf.
    if cursor.goto_first_child() {
        while cursor.goto_first_child() {}
        return Some(cursor.node());
    }

    // 2) Otherwise try to go to next sibling; if none — go up.
    if cursor.goto_next_sibling() {
        while cursor.goto_first_child() {}
        return Some(cursor.node());
    }

    while cursor.goto_parent() {
        if cursor.goto_next_sibling() {
            while cursor.goto_first_child() {}
            return Some(cursor.node());
        }
    }

    None
}

fn node_range(n: Node<'_>) -> Span {
    Span {
        start: n.start_position(),
        end: n.end_position(),
    }
}

/// For ERROR:
/// - if leaf has non-zero range, use leaf
/// - otherwise use `error_node`
fn node_range_for_display(leaf: Node<'_>, error_node: Node<'_>) -> Span {
    if leaf.start_byte() == leaf.end_byte() {
        node_range(error_node)
    } else {
        node_range(leaf)
    }
}

/// For MISSING:
/// Build insertion point so that highlighting is stable and “between tokens”.
///
/// Logic:
/// - if `missing_node` has non-zero range: use it
/// - else:
///    * if there is next non-extra leaf (starting from `missing_node)`: position = `start(next_leaf)`
///    * otherwise if there is prev leaf: position = `end(prev_leaf)`
///    * otherwise: `start_position(missing_node)`
fn missing_insertion_range(
    root: Node<'_>,
    missing_node: Node<'_>,
    prev_leaf: Option<Node<'_>>,
) -> Span {
    if missing_node.start_byte() != missing_node.end_byte() {
        return node_range(missing_node);
    }

    // Try to find the "next" token — this is usually more correct for highlighting “insert here”.
    if let Some(next_leaf) = next_non_extra_leaf(root, missing_node) {
        let p = next_leaf.start_position();
        return Span { start: p, end: p };
    }

    if let Some(prev) = prev_leaf {
        let p = prev.end_position();
        return Span { start: p, end: p };
    }

    let p = missing_node.start_position();
    Span { start: p, end: p }
}

fn dedup_preserve(v: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(v.len());
    for x in v {
        if seen.insert(x.clone()) {
            out.push(x);
        }
    }
    out
}

fn coalesce_errors(errors: Vec<ParseError>) -> Vec<ParseError> {
    // Group by (kind, range). Inside combine expected/fix_its and choose best message.
    let mut map: BTreeMap<(ParseErrorKind, Span), ParseError> = BTreeMap::new();

    for d in errors {
        let key = (d.kind.clone(), d.span);
        map.entry(key)
            .and_modify(|acc| {
                if d.message.len() > acc.message.len() {
                    acc.message.clone_from(&d.message);
                }
                let mut exp = acc.expected.clone();
                exp.extend(d.expected.clone());
                acc.expected = dedup_preserve(exp);
            })
            .or_insert(d);
    }

    map.into_values().collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut t = s[..max].to_string();
    t.push('…');
    t
}
