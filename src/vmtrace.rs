use tolkc::source_map::{DebugLocation, EntryContextDescription, SourceMap};
use vmlogs::parser::VmLine;

pub fn build_vm_trace(vm_logs: &str, source_map: &SourceMap) -> Vec<DebugLocation> {
    let lines = vmlogs::parser::parse_lines(vm_logs);
    build_vm_trace_from_lines(lines, source_map)
}

pub fn build_vm_trace_from_lines(
    lines: Vec<Result<VmLine, String>>,
    source_map: &SourceMap,
) -> Vec<DebugLocation> {
    lines
        .iter()
        .filter_map(|line| match line {
            Ok(VmLine::VmLoc { hash, offset }) => Some((hash, offset.parse().unwrap_or(0))),
            _ => None,
        })
        .flat_map(|(hash, offset)| {
            let Some(marks) = source_map.debug_marks.get(*hash) else {
                return vec![];
            };

            let debug_pairs = marks
                .iter()
                .filter(|(mark_offset, _)| *mark_offset == offset)
                .collect::<Vec<_>>();

            find_locations_by_debug_marks(source_map, debug_pairs, false)
        })
        .collect::<Vec<_>>()
}

pub fn low_level_loc_to_debug_locations(
    source_map: &SourceMap,
    hash: &str,
    offset: i32,
    skip_block_statements: bool,
    allow_approx: bool,
) -> Option<Vec<DebugLocation>> {
    let marks = source_map.debug_marks.get(hash)?;

    let mut debug_pairs = marks
        .iter()
        .filter(|(mark_offset, _)| *mark_offset == offset)
        .collect::<Vec<_>>();

    if debug_pairs.is_empty() && !marks.is_empty() && allow_approx {
        // We can't always find the exact location, so try to find an approximate location
        // For example, to find location where exit code is thrown
        debug_pairs = marks
            .iter()
            .rfind(|(mark_offset, _)| offset > *mark_offset)
            .iter()
            .copied()
            .collect::<Vec<_>>();

        if debug_pairs.is_empty()
            && let Some(first_mark) = marks.first()
        {
            // If we don't find approx info but have marks info,
            // use first one to show at least some location to user
            debug_pairs = vec![first_mark]
        }
    }

    let locs = find_locations_by_debug_marks(source_map, debug_pairs, skip_block_statements);
    if locs.is_empty() {
        return None;
    }

    Some(locs)
}

fn find_locations_by_debug_marks(
    source_map: &SourceMap,
    debug_pairs: Vec<&(i32, i32)>,
    skip_block_statements: bool,
) -> Vec<DebugLocation> {
    let locs = source_map
        .high_level
        .locations
        .iter()
        .filter(|loc| {
            debug_pairs
                .iter()
                .any(|(_, debug_id)| (*debug_id) as i64 == loc.idx)
        })
        .filter(|loc| {
            loc.loc.column != -1
                && !loc.loc.file.is_empty()
                && !loc.loc.file.starts_with("@stdlib/")
        })
        .cloned()
        .collect::<Vec<_>>();

    if skip_block_statements
        && locs.iter().any(|loc| {
            matches!(
                &loc.context.description,
                EntryContextDescription::Basic { ast_kind } if ast_kind == "ast_block_statement"
            )
        })
    {
        let actual_locs = locs
            .iter()
            .rev()
            .take_while(|el| {
                !matches!(
                    &el.context.description,
                    EntryContextDescription::Basic { ast_kind } if ast_kind == "ast_block_statement"
                )
            })
            .cloned()
            .collect::<Vec<_>>();

        return actual_locs;
    }

    locs
}
