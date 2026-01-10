use retrace::trace::Trace;
use ton_source_map::{DebugLocation, EntryContextDescription, OffsetAndId, SourceMap};
use vmlogs::parser::VmLine;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkipBlocksMode {
    None = 0,
    Before = 1,
    After = 2,
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
                .filter(|OffsetAndId(mark_offset, _)| *mark_offset == offset)
                .collect::<Vec<_>>();

            find_locations_by_debug_marks(source_map, debug_pairs, skip_block_mode)
        })
        .collect::<Vec<_>>()
}

pub fn low_level_loc_to_debug_locations(
    source_map: &SourceMap,
    hash: &str,
    offset: u16,
    skip_block_statements: SkipBlocksMode,
    allow_approx: bool,
) -> Option<Vec<DebugLocation>> {
    let marks = source_map.debug_marks.get(hash)?;

    let mut debug_pairs = marks
        .iter()
        .filter(|OffsetAndId(mark_offset, _)| mark_offset == &offset)
        .collect::<Vec<_>>();

    if debug_pairs.is_empty() && !marks.is_empty() && allow_approx {
        // We can't always find the exact location, so try to find an approximate location
        // For example, to find location where exit code is thrown
        debug_pairs = marks
            .iter()
            .rfind(|OffsetAndId(mark_offset, _)| offset > *mark_offset)
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
    debug_pairs: Vec<&OffsetAndId>,
    skip_block_mode: SkipBlocksMode,
) -> Vec<DebugLocation> {
    let locs = source_map
        .high_level
        .locations
        .iter()
        .filter(|loc| {
            debug_pairs
                .iter()
                .any(|OffsetAndId(_, debug_id)| debug_id == &loc.idx)
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

pub struct HighLevelTrace {
    pub steps: Vec<HighLevelTraceStep>,
}

pub enum HighLevelTraceStep {
    Mapped(HighLevelTraceStepMapped),
    Unmapped(HighLevelTraceStepUnmapped),
}

pub struct HighLevelTraceStepMapped {
    pub inner: retrace::trace::TraceStep,
    pub locs: Vec<DebugLocation>,
}

pub struct HighLevelTraceStepUnmapped {
    pub inner: retrace::trace::TraceStep,
}

impl HighLevelTrace {
    pub fn new(trace: Trace, source_map: &SourceMap) -> HighLevelTrace {
        let steps = trace.steps.iter().map(|step| {
            match step {
                retrace::trace::TraceStep::Execute {
                    hash,
                    offset,
                    instr,
                    ..
                } => {
                    let Some(marks) = source_map.debug_marks.get(hash) else {
                        // Если у нас нет информации для текущей ячейки, то мы считаем
                        // все шаги в ней незамапленными.
                        return HighLevelTraceStep::Unmapped(HighLevelTraceStepUnmapped {
                            inner: step.clone(),
                        });
                    };

                    let debug_marks = marks
                        .iter()
                        .filter(|OffsetAndId(mark_offset, _)| mark_offset == offset)
                        .collect::<Vec<_>>();

                    let locs = find_locations_by_debug_marks(
                        source_map,
                        debug_marks,
                        SkipBlocksMode::After,
                    );

                    if !locs.is_empty() {
                        return HighLevelTraceStep::Mapped(HighLevelTraceStepMapped {
                            inner: step.clone(),
                            locs,
                        });
                    }

                    // Если мы не нашли ни одной high-level локации, что довольно подозрительно,
                    // так как мы нашли debug mark, считаем что данная инструкция не имеет прямого
                    // кода на языке высокого уровня.
                    HighLevelTraceStep::Unmapped(HighLevelTraceStepUnmapped {
                        inner: step.clone(),
                    })
                }
                _ => {
                    // Другие виды шагов нас не интересуют, но могут быть полезны, поэтому
                    // оставляем их незамапленными.
                    // TODO: специальный вид шага для exception с локацией где он был выброшен?
                    HighLevelTraceStep::Unmapped(HighLevelTraceStepUnmapped {
                        inner: step.clone(),
                    })
                }
            }
        });

        HighLevelTrace {
            steps: steps.collect(),
        }
    }
}
