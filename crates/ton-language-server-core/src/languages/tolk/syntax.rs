use tolk_syntax::{Call, SourceFile, TryFromNode};

/// Finds the nearest call whose argument list contains the byte offset.
///
/// Calls that syntactically contain the cursor but whose parentheses do not
/// contain it are skipped, allowing the search to continue to an enclosing
/// call expression.
pub(super) fn find_call_at_offset(source_file: &SourceFile, offset: usize) -> Option<Call<'_>> {
    find_node_at_offset(source_file, offset, |call: &Call<'_>, offset| {
        call.argument_list()
            .is_some_and(|arguments| arguments.contains_offset(offset))
    })
}

/// Finds the nearest typed AST node at a byte offset that satisfies a filter.
///
/// The search starts at the smallest tree-sitter node containing `offset` and
/// walks toward the root. Each ancestor is converted with [`TryFromNode`]; the
/// first successfully converted node accepted by `filter` is returned. The
/// offset is clamped to the source length and passed to the filter so callers
/// can apply position-sensitive checks without repeating boundary handling.
pub(super) fn find_node_at_offset<'tree, N, F>(
    source_file: &'tree SourceFile,
    offset: usize,
    mut filter: F,
) -> Option<N>
where
    N: TryFromNode<'tree>,
    F: FnMut(&N, usize) -> bool,
{
    let offset = offset.min(source_file.source.len());
    let mut node = source_file
        .tree
        .root_node()
        .descendant_for_byte_range(offset, offset)?;

    loop {
        if let Ok(typed) = N::try_from_node(node)
            && filter(&typed, offset)
        {
            return Some(typed);
        }
        node = node.parent()?;
    }
}
