use tolkc::source_map::{DebugLocation, EntryContextDescription, SourceMap};
use vmlogs::parser::VmLine;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkipBlocksMode {
    None = 0,
    Before = 1,
    After = 2,
}

pub fn build_vm_trace(vm_logs: &str, source_map: &SourceMap) -> Vec<DebugLocation> {
    let lines = vmlogs::parser::parse_lines(vm_logs);
    build_vm_trace_from_lines(lines, source_map, SkipBlocksMode::After)
}

pub fn build_vm_trace_from_lines(
    lines: Vec<Result<VmLine, String>>,
    source_map: &SourceMap,
    skip_block_mode: SkipBlocksMode,
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

            find_locations_by_debug_marks(source_map, debug_pairs, skip_block_mode)
        })
        .collect::<Vec<_>>()
}

pub fn low_level_loc_to_debug_locations(
    source_map: &SourceMap,
    hash: &str,
    offset: i32,
    skip_block_statements: SkipBlocksMode,
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
    skip_block_mode: SkipBlocksMode,
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

    if skip_block_mode != SkipBlocksMode::None
        && locs.iter().any(|loc| {
            matches!(
                &loc.context.description,
                EntryContextDescription::Basic { ast_kind } if ast_kind == "ast_block_statement"
            )
        })
    {
        let actual_locs = if skip_block_mode == SkipBlocksMode::Before {
            locs.iter()
                .take_while(|el| {
                    !matches!(
                    &el.context.description,
                    EntryContextDescription::Basic { ast_kind } if ast_kind == "ast_block_statement"
                )
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            locs.iter()
                .rev()
                .take_while(|el| {
                    !matches!(
                    &el.context.description,
                    EntryContextDescription::Basic { ast_kind } if ast_kind == "ast_block_statement"
                )
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        return actual_locs;
    }

    locs
}

#[derive(Debug)]
pub enum TraceStep {
    Mapped(TraceStepMapped),
    Unmapped(TraceStepUnmapped),
}

#[derive(Debug)]
pub struct TraceStepMapped {
    pub instr: String,
    pub locs: Vec<DebugLocation>,
    pub gas: usize,
}

#[derive(Debug)]
pub struct TraceStepUnmapped {
    pub instr: String,
    pub gas: usize,
}

pub fn build_extended_vm_trace(vm_logs: &str, source_map: &SourceMap) -> Vec<TraceStep> {
    let lines = vmlogs::parser::parse_lines(vm_logs);
    build_extended_vm_trace_from_lines(lines, source_map)
}

pub fn build_extended_vm_trace_from_lines(
    lines: Vec<Result<VmLine, String>>,
    source_map: &SourceMap,
) -> Vec<TraceStep> {
    let mut gas_remaining = 1_000_000;

    let mut trace = Vec::<TraceStep>::new();
    let mut current_hash: Option<String> = None;
    let mut current_offset: Option<String> = None;
    let mut current_instr: Option<String> = None;

    for line_result in lines {
        let Ok(line) = line_result else { continue };

        match line {
            VmLine::VmLoc { hash, offset } => {
                current_hash = Some(hash.to_string());
                current_offset = Some(offset.to_string());
            }
            VmLine::VmExecute { instr } => {
                current_instr = Some(instr.to_string());
            }
            VmLine::VmGasRemaining { gas } => {
                let new_gas = gas.parse::<usize>().unwrap_or(gas_remaining);
                let gas_cost = gas_remaining.saturating_sub(new_gas);
                gas_remaining = new_gas;

                let instr = current_instr.take().unwrap_or_default();

                if let (Some(hash), Some(offset_str)) = (current_hash.take(), current_offset.take())
                {
                    let offset = offset_str.parse().unwrap_or(0);

                    if let Some(marks) = source_map.debug_marks.get(&hash) {
                        let debug_pairs = marks
                            .iter()
                            .filter(|(mark_offset, _)| *mark_offset == offset)
                            .collect::<Vec<_>>();

                        let locs = find_locations_by_debug_marks(
                            source_map,
                            debug_pairs,
                            SkipBlocksMode::After,
                        );

                        if !locs.is_empty() {
                            trace.push(TraceStep::Mapped(TraceStepMapped {
                                instr,
                                locs,
                                gas: gas_cost,
                            }));
                            continue;
                        }
                    }
                }

                trace.push(TraceStep::Unmapped(TraceStepUnmapped {
                    instr,
                    gas: gas_cost,
                }));
            }
            VmLine::VmLimitChanged { limit } => {
                gas_remaining = limit.parse().unwrap_or(gas_remaining);
            }
            _ => {}
        }
    }

    trace
}
