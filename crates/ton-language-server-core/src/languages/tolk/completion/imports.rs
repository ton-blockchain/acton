use super::context::TolkCompletionContext;
use crate::{DocumentSnapshot, Range};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tolk_syntax::{AstNode, Call, Expr};

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
    let cursor = context.cursor_node()?;
    let call = context.ancestor_as::<Call>()?;
    let argument_index = call.arguments().position(|argument| {
        let syntax = argument.syntax();
        syntax.start_byte() <= cursor.start_byte() && cursor.end_byte() <= syntax.end_byte()
    })?;
    let callee = call.callee()?;
    let (function, qualifier) = match callee {
        Expr::DotAccess(dot_access) => {
            let function = dot_access.field()?;
            let qualifier = dot_access.obj()?;
            (
                context.text_of(function).to_owned(),
                Some(context.text_of(qualifier).to_owned()),
            )
        }
        _ => (context.text_of(callee).to_owned(), None),
    };
    Some((function, qualifier, argument_index))
}

pub(super) fn string_prefix_and_range(
    context: &TolkCompletionContext,
    document: &DocumentSnapshot,
) -> Option<(String, Range)> {
    let literal = context.string_literal()?;
    let literal_node = literal.syntax();
    let start = literal_node.start_byte() + 1;
    let offset = context
        .offset
        .min(literal_node.end_byte().saturating_sub(1));
    let content = literal.content(context.source());
    let prefix_length = offset.checked_sub(start)?;
    let prefix = content.get(..prefix_length)?.to_owned();
    let range = document
        .text_index()
        .range_for_offsets(context.source(), start, offset);
    Some((prefix, range))
}
