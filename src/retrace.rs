pub use ::ton_retrace::trace::{
    ExecutedAction, ExecutedActions, InstalledAction, InstalledActions, InvalidAction,
};
use acton_debug::replayer::{CallFrameInfo, ExceptionBreakMode, StepMode, TolkReplayer};
use tolk_compiler::SourceMap;
use ton_source_map::SourceLocation;
use tvm_logs::parser::VmLine;

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
pub fn find_exception_info(vm_logs: &str, source_map: &SourceMap) -> Option<TolkExceptionInfo> {
    let description = exception_description(vm_logs);
    let mut replayer = TolkReplayer::new(source_map, vm_logs).ok()?;
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
pub fn find_execution_trace(vm_logs: &str, source_map: &SourceMap) -> Option<TolkTraceInfo> {
    let mut replayer = TolkReplayer::new(source_map, vm_logs).ok()?;

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

fn exception_description(vm_logs: &str) -> String {
    tvm_logs::parser::parse_lines(vm_logs)
        .filter_map(Result::ok)
        .filter_map(|line| match line {
            VmLine::VmException { message, .. } => Some(message.to_string()),
            _ => None,
        })
        .last()
        .unwrap_or_default()
}

fn find_backtrace(
    source_map: &SourceMap,
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
    source_map: &SourceMap,
    range: &tolk_compiler::source_map::SrcRange,
) -> SourceLocation {
    to_source_location(
        source_map,
        range.file_id(),
        range.start_line(),
        range.start_col(),
    )
}

fn to_source_location(
    source_map: &SourceMap,
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
    if source_map.is_empty() {
        return None;
    }

    source_map.find_source_loc(hash, offset)
}

#[must_use]
pub fn find_installed_actions(vm_logs: &str) -> InstalledActions {
    ::ton_retrace::trace::Trace::new(vm_logs, None).actions()
}
