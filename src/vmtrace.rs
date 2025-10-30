use tolkc::source_map::{DebugLocation, SourceMap};
use vmlogs::parser::VmLine;

pub fn build_vm_trace(vm_logs: &String, source_map: &SourceMap) -> Vec<DebugLocation> {
    let lines = vmlogs::parser::parse_lines(vm_logs.as_str());
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
                .filter(|(mark_offset, _)| return *mark_offset == offset)
                .collect::<Vec<_>>();

            find_locations_by_debug_marks(source_map, debug_pairs)
        })
        .collect::<Vec<_>>()
}

pub fn low_level_loc_to_debug_locations(
    source_map: &SourceMap,
    hash: &str,
    offset: i32,
    allow_approx: bool,
) -> Option<Vec<DebugLocation>> {
    let Some(marks) = source_map.debug_marks.get(hash) else {
        return None;
    };

    let mut debug_pairs = marks
        .iter()
        .filter(|(mark_offset, _)| return *mark_offset == offset)
        .collect::<Vec<_>>();

    if debug_pairs.is_empty() && allow_approx {
        // We can't always find the exact location, so try to find an approximate location
        // For example, to find location where exit code is thrown
        debug_pairs = marks
            .iter()
            .rfind(|(mark_offset, _)| return offset > *mark_offset)
            .iter()
            .map(|pair| *pair)
            .collect::<Vec<_>>();
    }

    let locs = find_locations_by_debug_marks(source_map, debug_pairs);
    if locs.is_empty() {
        return None;
    }

    Some(locs)
}

fn find_locations_by_debug_marks(
    source_map: &SourceMap,
    debug_pairs: Vec<&(i32, i32)>,
) -> Vec<DebugLocation> {
    source_map
        .high_level
        .locations
        .iter()
        .filter(|loc| {
            debug_pairs
                .iter()
                .find(|(_, debug_id)| (*debug_id) as i64 == loc.idx)
                .is_some()
        })
        .filter(|loc| !loc.loc.file.is_empty() && !loc.loc.file.starts_with("@stdlib/"))
        .cloned()
        .collect::<Vec<_>>()
}
