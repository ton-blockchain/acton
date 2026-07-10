use super::context::TolkCompletionContext;
use crate::{DocumentSnapshot, Range};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub(super) struct WorkspaceCompletionData<'a> {
    pub(super) paths: &'a [PathBuf],
    pub(super) stdlib_path: &'a Path,
    pub(super) mappings: Option<&'a BTreeMap<String, String>>,
    pub(super) contract_ids: &'a [String],
    pub(super) wallet_names: &'a [String],
}

pub(super) fn matches_call(
    context: &TolkCompletionContext,
    expected_function: &str,
    expected_qualifier: Option<&str>,
    expected_argument: usize,
) -> bool {
    context.inside_string()
        && call_at_string(context).is_some_and(|(function, qualifier, argument)| {
            function == expected_function
                && qualifier.as_deref() == expected_qualifier
                && argument == expected_argument
        })
}

fn call_at_string(context: &TolkCompletionContext) -> Option<(String, Option<String>, usize)> {
    let string = context.cursor_node()?;
    let argument = ancestor(string, "call_argument")?;
    let arguments = argument
        .parent()
        .filter(|node| node.kind() == "argument_list")?;
    let call = arguments
        .parent()
        .filter(|node| node.kind() == "function_call")?;
    let callee = call.child_by_field_name("callee")?;
    let (function, qualifier) = if callee.kind() == "dot_access" {
        let function = callee
            .child_by_field_name("field")?
            .utf8_text(context.source().as_bytes())
            .ok()?
            .to_owned();
        let qualifier = callee
            .child_by_field_name("obj")?
            .utf8_text(context.source().as_bytes())
            .ok()?
            .to_owned();
        (function, Some(qualifier))
    } else {
        (
            callee
                .utf8_text(context.source().as_bytes())
                .ok()?
                .to_owned(),
            None,
        )
    };
    let mut cursor = arguments.walk();
    let argument_index = arguments
        .children(&mut cursor)
        .filter(|node| node.kind() == "call_argument")
        .position(|node| node == argument)?;
    Some((function, qualifier, argument_index))
}

fn ancestor<'tree>(
    mut node: tree_sitter::Node<'tree>,
    kind: &str,
) -> Option<tree_sitter::Node<'tree>> {
    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.parent()?;
    }
}

pub(super) fn string_prefix_and_range(
    document: &DocumentSnapshot,
    offset: usize,
) -> Option<(String, Range)> {
    let source = document.text();
    let offset = offset.min(source.len());
    let start = source.as_bytes()[..offset]
        .iter()
        .rposition(|byte| matches!(byte, b'\"' | b'\''))?
        + 1;
    let prefix = source.get(start..offset)?.to_owned();
    let range = document
        .text_index()
        .range_for_offsets(source, start, offset);
    Some((prefix, range))
}
