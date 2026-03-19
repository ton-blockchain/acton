use crate::replayer::{CallFrameInfo, ExceptionBreakMode, StepMode, TolkReplayer};
use crate::vmtrace;
use crate::vmtrace::SkipBlocksMode;
pub use retrace::trace::{
    ExecutedAction, ExecutedActions, InstalledAction, InstalledActions, InvalidAction,
};
use ton_source_map::{DebugLocation, SourceLocation, SourceMap};
use vmlogs::parser::VmLine;

#[derive(Debug)]
pub struct ExceptionInfo {
    pub description: String,
    pub loc: Option<SourceLocation>,
    pub backtrace: Vec<DebugLocation>,
}

#[derive(Debug, Clone)]
pub struct TolkBacktraceFrame {
    pub function_name: String,
    pub loc: SourceLocation,
}

#[derive(Debug, Clone)]
pub struct TolkTraceInfo {
    pub loc: SourceLocation,
    pub backtrace: Vec<TolkBacktraceFrame>,
}

#[derive(Debug, Clone)]
pub struct TolkExceptionInfo {
    pub errno: String,
    pub description: String,
    pub loc: SourceLocation,
    pub backtrace: Vec<TolkBacktraceFrame>,
}

#[must_use]
pub fn find_exception_info(vm_logs: &str, source_map: &SourceMap) -> Option<ExceptionInfo> {
    let lines = vmlogs::parser::parse_lines(vm_logs);

    let exception = lines
        .iter()
        .rfind(|line| matches!(line, Ok(VmLine::VmException { .. })));
    let description = match exception {
        Some(Ok(VmLine::VmException { message, .. })) => (*message).to_string(),
        _ => String::new(),
    };

    let location = lines
        .iter()
        .rfind(|line| matches!(line, Ok(VmLine::VmLoc { .. })));

    let (hash, offset) = match location {
        Some(Ok(VmLine::VmLoc { hash, offset })) => {
            ((*hash).to_string(), offset.parse().unwrap_or(0))
        }
        _ => (String::new(), 0),
    };

    let loc = find_source_loc(source_map, &hash, offset);

    let backtrace = find_backtrace(source_map, lines);

    Some(ExceptionInfo {
        description,
        loc,
        backtrace,
    })
}

#[must_use]
pub fn find_tolk_exception_info(
    vm_logs: &str,
    source_map: Option<&tolkc::SourceMap>,
    code_boc: &[u8],
    marks_boc: Option<&[u8]>,
) -> Option<TolkExceptionInfo> {
    let source_map = source_map?;
    let marks_boc = marks_boc?;
    if marks_boc.is_empty() {
        return None;
    }

    let marks_dict = tolkc::debug_marks_dict::parse_debug_marks(marks_boc, code_boc);
    let vm_lines = vmlogs::parser::parse_lines(vm_logs);
    let description = vm_lines
        .iter()
        .rfind(|line| matches!(line, Ok(VmLine::VmException { .. })))
        .and_then(|line| match line {
            Ok(VmLine::VmException { message, .. }) => Some((*message).to_string()),
            _ => None,
        })
        .unwrap_or_default();
    let mut replayer = TolkReplayer::new(source_map.clone(), &marks_dict, &vm_lines);
    replayer.set_exception_breakpoints(ExceptionBreakMode::Uncaught);

    while !replayer.is_finished() {
        replayer.step(StepMode::StepInto);

        let Some(exception) = replayer.last_exception() else {
            continue;
        };
        if !exception.is_uncaught {
            continue;
        }

        let loc = to_tolk_source_location(
            source_map,
            replayer.current_file_id(),
            replayer.current_line(),
            replayer.current_column(),
        );

        return Some(TolkExceptionInfo {
            errno: exception.errno.clone(),
            description,
            backtrace: find_tolk_backtrace(source_map, &replayer.call_stack(), &loc),
            loc,
        });
    }

    None
}

#[must_use]
pub fn find_tolk_execution_trace(
    vm_logs: &str,
    source_map: Option<&tolkc::SourceMap>,
    code_boc: &[u8],
    marks_boc: Option<&[u8]>,
) -> Option<TolkTraceInfo> {
    let source_map = source_map?;
    let marks_boc = marks_boc?;
    if marks_boc.is_empty() {
        return None;
    }

    let marks_dict = tolkc::debug_marks_dict::parse_debug_marks(marks_boc, code_boc);
    let vm_lines = vmlogs::parser::parse_lines(vm_logs);
    let mut replayer = TolkReplayer::new(source_map.clone(), &marks_dict, &vm_lines);

    while !replayer.is_finished() {
        replayer.step(StepMode::StepInto);
    }

    let loc = to_tolk_source_location(
        source_map,
        replayer.current_file_id(),
        replayer.current_line(),
        replayer.current_column(),
    );
    if loc.line == 0 && loc.column == 0 && replayer.call_stack().is_empty() {
        return None;
    }

    Some(TolkTraceInfo {
        backtrace: find_tolk_backtrace(source_map, &replayer.call_stack(), &loc),
        loc,
    })
}

fn find_backtrace(
    source_map: &SourceMap,
    lines: Vec<Result<VmLine, String>>,
) -> Vec<DebugLocation> {
    let execution_path =
        vmtrace::build_vm_trace_from_lines(lines, source_map, SkipBlocksMode::None);

    let mut stack = vec![];

    for step in &execution_path {
        if step.context.event == Some("EnterFunction".to_string())
            || step.context.event == Some("EnterInlinedFunction".to_string())
        {
            if step.context.event_function.is_none() {
                continue;
            }

            stack.push(step);
        }
        if step.context.event == Some("AfterFunctionCall".to_string())
            || step.context.event == Some("LeaveInlinedFunction".to_string())
        {
            let event_function = &step.context.event_function;

            let Some(last) = stack.last() else {
                continue;
            };

            if last.context.event_function == *event_function {
                stack.pop();
            }
        }
    }
    stack.iter().map(|loc| (**loc).clone()).collect::<Vec<_>>()
}

fn find_tolk_backtrace(
    source_map: &tolkc::SourceMap,
    call_stack: &[CallFrameInfo],
    current_loc: &SourceLocation,
) -> Vec<TolkBacktraceFrame> {
    let mut frames = Vec::new();

    for idx in (0..call_stack.len()).rev() {
        let frame = &call_stack[idx];
        let loc = if idx + 1 == call_stack.len() {
            Some(current_loc.clone())
        } else {
            call_stack[idx + 1]
                .call_site_loc
                .as_ref()
                .map(|range| src_range_to_source_location(source_map, range))
        };

        if let Some(loc) = loc {
            frames.push(TolkBacktraceFrame {
                function_name: frame.f_name.clone(),
                loc,
            });
        }
    }

    frames
}

fn src_range_to_source_location(
    source_map: &tolkc::SourceMap,
    range: &tolkc::source_map::SrcRange,
) -> SourceLocation {
    to_tolk_source_location(
        source_map,
        range.file_id(),
        range.start_line(),
        range.start_col(),
    )
}

fn to_tolk_source_location(
    source_map: &tolkc::SourceMap,
    file_id: usize,
    line: usize,
    column: usize,
) -> SourceLocation {
    let file = source_map
        .resolve_file_full_path(file_id)
        .unwrap_or_else(|| source_map.resolve_file_name(file_id))
        .to_owned();

    SourceLocation {
        file,
        line: line as i64,
        column: column as i64,
        end_line: line as i64,
        end_column: column as i64,
        length: 0,
    }
}

#[must_use]
pub fn find_source_loc(source_map: &SourceMap, hash: &str, offset: u16) -> Option<SourceLocation> {
    if source_map.high_level.locations.is_empty() {
        // `--backtrace full` is not enabled
        return None;
    }

    let locs = vmtrace::low_level_loc_to_debug_locations(
        source_map,
        hash,
        offset,
        SkipBlocksMode::None,
        true,
    )?;
    locs.last().map(|l| l.loc.clone())
}

#[must_use]
pub fn find_installed_actions(vm_logs: &str) -> InstalledActions {
    retrace::trace::Trace::new(vm_logs, None).actions()
}
