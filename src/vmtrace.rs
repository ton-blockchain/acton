use retrace::trace::Trace;
use ton_source_map::{DebugLocation, EntryContextDescription, OffsetAndId, SourceMap};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SkipBlocksMode {
    None = 0,
    Before = 1,
    After = 2,
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
    #[must_use]
    pub fn new(trace: Trace, source_map: &SourceMap) -> HighLevelTrace {
        let steps = trace.steps.iter().map(|step| {
            match step {
                retrace::trace::TraceStep::Execute { hash, offset, .. } => {
                    let Some(marks) = source_map.debug_marks.get(hash) else {
                        // If we don't have information for the current cell, then we consider
                        // all steps in it unmapped.
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

                    HighLevelTraceStep::Unmapped(HighLevelTraceStepUnmapped {
                        inner: step.clone(),
                    })
                }
                _ => {
                    // Other types of steps don't interest us, but they might be useful, so
                    // we leave them unmapped.
                    // TODO: special step type for exception with location where it was thrown?
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
