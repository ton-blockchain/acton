use crate::replayer::{CallFrameInfo, ExceptionBreakMode, StepMode, TolkReplayer};
pub use retrace::trace::{
    ExecutedAction, ExecutedActions, InstalledAction, InstalledActions, InvalidAction,
};
use tolkc::TolkSourceMap;
use ton_source_map::{DebugLocation, SourceLocation};
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
pub struct TolkTraceLine {
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
pub fn find_exception_info(
    vm_logs: &str,
    tolk_source_map: &TolkSourceMap,
) -> Option<TolkExceptionInfo> {
    let source_map = &tolk_source_map.source_map;
    let (mut replayer, description) = create_tolk_replayer(vm_logs, tolk_source_map)?;
    replayer.set_exception_breakpoints(ExceptionBreakMode::Uncaught);

    while !replayer.is_finished() {
        replayer.step(StepMode::StepInto);

        let Some(exception) = replayer.last_exception() else {
            continue;
        };
        if !exception.is_uncaught {
            continue;
        }

        let loc = to_source_location(
            source_map,
            replayer.current_file_id(),
            replayer.current_line(),
            replayer.current_column(),
        );

        return Some(TolkExceptionInfo {
            errno: exception.errno.clone(),
            description,
            backtrace: find_backtrace(source_map, &replayer.call_stack(), &loc),
            loc,
        });
    }

    None
}

#[must_use]
pub fn find_execution_trace(
    vm_logs: &str,
    tolk_source_map: &TolkSourceMap,
) -> Option<TolkTraceInfo> {
    let source_map = &tolk_source_map.source_map;
    let (mut replayer, _) = create_tolk_replayer(vm_logs, tolk_source_map)?;

    while !replayer.is_finished() {
        replayer.step(StepMode::StepInto);
    }

    let loc = to_source_location(
        source_map,
        replayer.current_file_id(),
        replayer.current_line(),
        replayer.current_column(),
    );
    if loc.line == 0 && loc.column == 0 && replayer.call_stack().is_empty() {
        return None;
    }

    Some(TolkTraceInfo {
        backtrace: find_backtrace(source_map, &replayer.call_stack(), &loc),
        loc,
    })
}

#[must_use]
pub fn collect_tolk_line_trace(
    vm_logs: &str,
    tolk_source_map: &TolkSourceMap,
) -> Option<Vec<TolkTraceLine>> {
    let source_map = &tolk_source_map.source_map;
    let (mut replayer, _) = create_tolk_replayer(vm_logs, tolk_source_map)?;
    let mut trace = Vec::new();
    let mut last_key = None;

    while !replayer.is_finished() {
        replayer.step(StepMode::StepInto);

        let loc = to_source_location(
            source_map,
            replayer.current_file_id(),
            replayer.current_line(),
            replayer.current_column(),
        );
        if loc.line == 0 && loc.column == 0 {
            continue;
        }

        let function_name = replayer
            .call_stack()
            .last()
            .map(|frame| frame.f_name.clone())
            .unwrap_or_default();
        let key = (
            loc.file.clone(),
            loc.line,
            loc.column,
            function_name.clone(),
        );
        if last_key.as_ref() == Some(&key) {
            continue;
        }

        trace.push(TolkTraceLine { function_name, loc });
        last_key = Some(key);
    }

    if trace.is_empty() { None } else { Some(trace) }
}

#[must_use]
pub fn build_tolk_replayer(vm_logs: &str, tolk_source_map: &TolkSourceMap) -> Option<TolkReplayer> {
    create_tolk_replayer(vm_logs, tolk_source_map).map(|(replayer, _)| replayer)
}

fn create_tolk_replayer(
    vm_logs: &str,
    tolk_source_map: &TolkSourceMap,
) -> Option<(TolkReplayer, String)> {
    let vm_lines = vmlogs::parser::parse_lines(vm_logs);
    let description = exception_description(&vm_lines);
    let marks_dict = tolk_source_map.marks_dict.as_deref()?;

    Some((
        TolkReplayer::new(tolk_source_map.source_map.clone(), marks_dict, &vm_lines),
        description,
    ))
}

fn exception_description(vm_lines: &[Result<VmLine<'_>, String>]) -> String {
    vm_lines
        .iter()
        .rfind(|line| matches!(line, Ok(VmLine::VmException { .. })))
        .and_then(|line| match line {
            Ok(VmLine::VmException { message, .. }) => Some((*message).to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

fn find_backtrace(
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
    to_source_location(
        source_map,
        range.file_id(),
        range.start_line(),
        range.start_col(),
    )
}

fn to_source_location(
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
pub fn find_source_loc(
    tolk_source_map: &TolkSourceMap,
    hash: &str,
    offset: u16,
) -> Option<SourceLocation> {
    if tolk_source_map.source_map.is_empty() {
        // `--backtrace full` is not enabled
        return None;
    }

    tolk_source_map.find_source_loc(hash, offset)
}

#[must_use]
pub fn find_installed_actions(vm_logs: &str) -> InstalledActions {
    retrace::trace::Trace::new(vm_logs, None).actions()
}
