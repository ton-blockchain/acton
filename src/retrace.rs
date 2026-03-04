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
