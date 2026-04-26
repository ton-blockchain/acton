//! Standalone DAP (Debug Adapter Protocol) server for a single `TolkReplayer`.
//! Mirrors `tolk-replay` DAP as closely as possible, but:
//! 1. uses tolerant request parsing for custom VS Code messages
//! 2. attaches to an already prepared `TolkReplayer`

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;

use dap::prelude::*;
use dap::responses::{
    ContinueResponse, EvaluateResponse, ExceptionInfoResponse, ScopesResponse,
    SetBreakpointsResponse, SetExceptionBreakpointsResponse, StackTraceResponse, ThreadsResponse,
    VariablesResponse,
};
use dap::types::{
    Breakpoint, ExceptionBreakMode, ExceptionBreakpointsFilter, Scope, ScopePresentationhint,
    Source, StackFrame, StackFramePresentationhint, StoppedEventReason, Variable,
};

use crate::core::evaluate::{evaluate_condition_expression, evaluate_expression};
use crate::core::exception_format::{build_exception_details, exception_overview};
use crate::replayer::{self, StepMode, TolkReplayer};
use crate::transport::{DapConnection, IncomingRequest};
use crate::types_render::RenderedValue;

const THREAD_ID: i64 = 1;

fn make_capabilities() -> types::Capabilities {
    types::Capabilities {
        supports_configuration_done_request: Some(true),
        supports_step_back: Some(false),
        supports_exception_info_request: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_conditional_breakpoints: Some(true),
        exception_breakpoint_filters: Some(vec![
            ExceptionBreakpointsFilter {
                filter: "uncaught".to_string(),
                label: "Uncaught Exceptions".to_string(),
                description: Some("Break when an exception terminates the contract".to_string()),
                default: Some(true),
                supports_condition: None,
                condition_description: None,
            },
            ExceptionBreakpointsFilter {
                filter: "all".to_string(),
                label: "All Exceptions".to_string(),
                description: Some(
                    "Break on any exception, including caught by try/catch".to_string(),
                ),
                default: Some(false),
                supports_condition: None,
                condition_description: None,
            },
        ]),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

type PendingBreakpoints = HashMap<String, Vec<SourceBreakpointInfo>>;

#[derive(Debug, Clone)]
struct SourceBreakpointInfo {
    id: i64,
    line: i64,
    condition: Option<String>,
}

#[derive(Debug, Clone)]
struct BreakpointStopInfo {
    ids: Vec<i64>,
    description: String,
}

enum BreakpointCheck {
    None,
    Skip,
    Hit(BreakpointStopInfo),
}

enum AdvanceStop {
    Terminated,
    Exception { description: String, text: String },
    Breakpoint(BreakpointStopInfo),
    Step,
}

struct DapState {
    replayer: Option<TolkReplayer>,
    pending_breakpoints: PendingBreakpoints,
    pending_exception_mode: replayer::ExceptionBreakMode,
    config_done: bool,
    next_breakpoint_id: i64,
    resolved_breakpoints: HashMap<(usize, usize), Vec<SourceBreakpointInfo>>,

    next_req_id: i64,
    /// Maps frame `ref_id` (returned in `StackTrace`) → `depth_from_top` (0 = innermost).
    /// Rebuilt on every `StackTrace` request.
    frame_to_depth: HashMap<i64, usize>,
    /// Maps variable `req_id` → `RenderedValue` for structured drill-down.
    /// Rebuilt on every `StackTrace` request (old values are stale after stepping).
    vars_debug_values: HashMap<i64, RenderedValue>,
    runtime_register_scope_requests: HashSet<i64>,
}

impl DapState {
    fn new() -> Self {
        Self {
            replayer: None,
            pending_breakpoints: HashMap::new(),
            pending_exception_mode: replayer::ExceptionBreakMode::Uncaught,
            config_done: false,
            next_breakpoint_id: 1,
            resolved_breakpoints: HashMap::new(),
            // Keep structured variable refs in a separate numeric range so they
            // never collide with stable frame ids derived from stack depth.
            next_req_id: 1_000_000,
            frame_to_depth: HashMap::new(),
            vars_debug_values: HashMap::new(),
            runtime_register_scope_requests: HashSet::new(),
        }
    }

    fn apply_pending_breakpoints(&mut self) {
        if let Some(ref mut r) = self.replayer {
            r.clear_all_breakpoints();
            self.resolved_breakpoints.clear();

            for (path, breakpoints) in &self.pending_breakpoints {
                if let Some(file_id) = r.file_id_by_path(path) {
                    let requested_lines = breakpoints
                        .iter()
                        .map(|bp| bp.line.max(1) as usize)
                        .collect::<Vec<_>>();
                    let resolved_lines = r.resolve_breakpoint_lines(file_id, &requested_lines);
                    r.set_breakpoints(file_id, &requested_lines);

                    for (bp, resolved_line) in breakpoints.iter().zip(resolved_lines) {
                        self.resolved_breakpoints
                            .entry((file_id, resolved_line))
                            .or_default()
                            .push(bp.clone());
                    }
                }
            }
        }
    }

    const fn alloc_req_id(&mut self) -> i64 {
        let id = self.next_req_id;
        self.next_req_id += 1;
        id
    }

    fn alloc_frame_id(&mut self, depth_from_top: usize) -> i64 {
        // Frame ids must remain stable across repeated `stackTrace` requests,
        // otherwise a later `scopes`/`variables` request can refer to a frame
        // id from a previous response that we already invalidated.
        let id = depth_from_top as i64 + 1;
        self.frame_to_depth.insert(id, depth_from_top);
        id
    }

    /// Store a `RenderedValue` and return its `req_id` for DAP drill-down.
    fn store_debug_value(&mut self, dv: RenderedValue) -> i64 {
        let id = self.alloc_req_id();
        self.vars_debug_values.insert(id, dv);
        id
    }

    fn do_step(&mut self, step_mode: StepMode) -> bool {
        if let Some(ref mut r) = self.replayer {
            r.step(step_mode);
            r.is_finished()
        } else {
            true
        }
    }

    fn current_breakpoint_check(&self) -> BreakpointCheck {
        let Some(r) = self.replayer.as_ref() else {
            return BreakpointCheck::None;
        };
        let file_id = r.current_file_id();
        let line = r.current_line();
        let Some(breakpoints) = self.resolved_breakpoints.get(&(file_id, line)) else {
            return BreakpointCheck::None;
        };

        evaluate_breakpoint_conditions(&r.locals_for_frame(0), breakpoints)
    }

    fn resolve_breakpoint_lines_for_path(
        &self,
        path: &str,
        requested_lines: &[usize],
    ) -> Option<Vec<usize>> {
        let r = self.replayer.as_ref()?;
        let file_id = r.file_id_by_path(path)?;
        Some(r.resolve_breakpoint_lines(file_id, requested_lines))
    }

    fn evaluate_on_frame(
        &self,
        frame_id: Option<i64>,
        expression: &str,
    ) -> anyhow::Result<RenderedValue> {
        let Some(replayer) = self.replayer.as_ref() else {
            return evaluate_expression(&[], None, expression);
        };

        let locals = match frame_id {
            Some(frame_id) => {
                let depth = self
                    .frame_to_depth
                    .get(&frame_id)
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("Unknown frame id {frame_id}"))?;
                replayer.locals_for_frame(depth)
            }
            None => replayer.locals_for_frame(0),
        };

        evaluate_expression(&locals, Some(replayer.source_map()), expression)
    }
}

fn evaluate_breakpoint_conditions(
    locals: &[replayer::LocalVarRendered],
    breakpoints: &[SourceBreakpointInfo],
) -> BreakpointCheck {
    let mut hit_ids = Vec::new();
    let mut error_ids = Vec::new();
    let mut first_error = None;

    for breakpoint in breakpoints {
        let Some(condition) = breakpoint.condition.as_deref() else {
            hit_ids.push(breakpoint.id);
            continue;
        };

        match evaluate_condition_expression(locals, condition) {
            Ok(true) => hit_ids.push(breakpoint.id),
            Ok(false) => {}
            Err(err) => {
                error_ids.push(breakpoint.id);
                if first_error.is_none() {
                    first_error = Some(err.to_string());
                }
            }
        }
    }

    if !hit_ids.is_empty() {
        BreakpointCheck::Hit(BreakpointStopInfo {
            ids: hit_ids,
            description: "Breakpoint hit".to_string(),
        })
    } else if let Some(err) = first_error {
        BreakpointCheck::Hit(BreakpointStopInfo {
            ids: error_ids,
            description: format!("Conditional breakpoint evaluation failed: {err}"),
        })
    } else {
        BreakpointCheck::Skip
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_source(replayer: &TolkReplayer, file_id: usize) -> Source {
    let full_path = replayer.file_full_path(file_id);
    let short_name = replayer.file_display_name(file_id);
    Source {
        name: Some(short_name.to_string()),
        path: full_path.map(ToString::to_string),
        source_reference: None,
        presentation_hint: None,
        origin: None,
        sources: None,
        adapter_data: None,
        checksums: None,
    }
}

/// Emit the standard DAP `stopped` event once the replayer has already advanced
/// to the location that should be shown to the client.
fn send_stopped(
    server: &mut DapConnection<impl BufRead, impl Write>,
    reason: StoppedEventReason,
    description: Option<String>,
    hit_breakpoint_ids: Option<Vec<i64>>,
) -> anyhow::Result<()> {
    server.send_event(Event::Stopped(events::StoppedEventBody {
        reason,
        description,
        thread_id: Some(THREAD_ID),
        preserve_focus_hint: None,
        text: None,
        all_threads_stopped: Some(true),
        hit_breakpoint_ids,
    }))?;
    Ok(())
}

/// Finish the debuggee from DAP's point of view: first `exited`, then `terminated`.
fn send_terminated(server: &mut DapConnection<impl BufRead, impl Write>) -> anyhow::Result<()> {
    server.send_event(Event::Exited(events::ExitedEventBody { exit_code: 0 }))?;
    server.send_event(Event::Terminated(Some(
        events::TerminatedEventBody::default(),
    )))?;
    Ok(())
}

/// Run one logical debugger action and translate the resulting stop reason
/// (termination / exception / breakpoint / plain step) into DAP events.
fn step_and_notify(
    state: &mut DapState,
    step_mode: StepMode,
    server: &mut DapConnection<impl BufRead, impl Write>,
) -> anyhow::Result<()> {
    match advance_to_next_stop(state, step_mode) {
        AdvanceStop::Terminated => send_terminated(server)?,
        AdvanceStop::Exception { description, text } => {
            send_stopped_exception(server, &description, &text)?;
        }
        AdvanceStop::Breakpoint(stop) => {
            send_stopped(
                server,
                StoppedEventReason::Breakpoint,
                Some(stop.description),
                Some(stop.ids),
            )?;
        }
        AdvanceStop::Step => {
            send_stopped(server, StoppedEventReason::Step, None, None)?;
        }
    }
    Ok(())
}

fn advance_to_next_stop(state: &mut DapState, step_mode: StepMode) -> AdvanceStop {
    loop {
        if state.do_step(step_mode) {
            return AdvanceStop::Terminated;
        }

        if let Some(exc) = state.replayer.as_ref().and_then(|r| r.last_exception()) {
            let overview = exception_overview(exc);
            return AdvanceStop::Exception {
                description: overview.stop_description,
                text: overview.stop_text,
            };
        }

        match state.current_breakpoint_check() {
            BreakpointCheck::Hit(stop) => return AdvanceStop::Breakpoint(stop),
            BreakpointCheck::Skip if matches!(step_mode, StepMode::RunUntilBreakpoint) => {
                continue;
            }
            BreakpointCheck::Skip | BreakpointCheck::None => return AdvanceStop::Step,
        }
    }
}

fn send_stopped_exception(
    server: &mut DapConnection<impl BufRead, impl Write>,
    description: &str,
    text: &str,
) -> anyhow::Result<()> {
    server.send_event(Event::Stopped(events::StoppedEventBody {
        reason: StoppedEventReason::Exception,
        description: Some(description.to_string()),
        thread_id: Some(THREAD_ID),
        preserve_focus_hint: None,
        text: Some(text.to_string()),
        all_threads_stopped: Some(true),
        hit_breakpoint_ids: None,
    }))?;
    Ok(())
}

const fn resolve_step_mode(
    granularity: Option<&types::SteppingGranularity>,
    default: StepMode,
) -> StepMode {
    match granularity {
        Some(types::SteppingGranularity::Instruction) => StepMode::EachAsmInstruction,
        _ => default,
    }
}

fn format_frame_name(f: &replayer::CallFrameInfo) -> String {
    if f.is_builtin {
        format!("{} (built-in)", f.f_name)
    } else if f.is_inlined {
        format!("{} (inlined)", f.f_name)
    } else {
        f.f_name.clone()
    }
}

fn debug_value_to_variable(state: &mut DapState, name: String, dv: &RenderedValue) -> Variable {
    let (value, type_field) = dv.dap_parts_for_client(Some(&name));
    let (value, var_ref) = if dv.has_children() {
        (value, state.store_debug_value(dv.clone()))
    } else {
        (value, 0)
    };
    Variable {
        name,
        value,
        type_field,
        presentation_hint: None,
        evaluate_name: None,
        variables_reference: var_ref,
        named_variables: None,
        indexed_variables: None,
        memory_reference: None,
    }
}

fn evaluate_response_from_value(state: &mut DapState, value: RenderedValue) -> EvaluateResponse {
    let (result, type_field) = value.dap_parts_for_client(None);
    let variables_reference = if value.has_children() {
        state.store_debug_value(value)
    } else {
        0
    };

    EvaluateResponse {
        result,
        type_field,
        presentation_hint: None,
        variables_reference,
        named_variables: None,
        indexed_variables: None,
        memory_reference: None,
    }
}

// ---------------------------------------------------------------------------
// Request handling
// ---------------------------------------------------------------------------

fn handle_request(
    state: &mut DapState,
    req: Request,
    server: &mut DapConnection<impl BufRead, impl Write>,
) -> anyhow::Result<()> {
    let response = match req.command.clone() {
        Command::Initialize(_) => {
            let resp = req.success(ResponseBody::Initialize(make_capabilities()));
            server.respond(resp)?;
            server.send_event(Event::Initialized)?;
            return Ok(());
        }
        Command::Launch(args) => handle_launch(state, &args, req)?,
        Command::Attach(args) => handle_attach(state, &args, req)?,
        Command::SetBreakpoints(args) => handle_set_breakpoints(state, &args, req),
        Command::SetExceptionBreakpoints(ref args) => {
            handle_set_exception_breakpoints(state, args, req)
        }
        Command::ExceptionInfo(_) => handle_exception_info(state, req),
        Command::ConfigurationDone => handle_configuration_done(state, server, req)?,
        Command::Threads => {
            let threads = vec![types::Thread {
                id: THREAD_ID,
                name: "main".to_string(),
            }];
            req.success(ResponseBody::Threads(ThreadsResponse { threads }))
        }
        Command::Continue(_) => {
            step_and_notify(state, StepMode::RunUntilBreakpoint, server)?;
            req.success(ResponseBody::Continue(ContinueResponse {
                all_threads_continued: Some(true),
            }))
        }
        Command::Next(ref args) => {
            let mode = resolve_step_mode(args.granularity.as_ref(), StepMode::StepOver);
            step_and_notify(state, mode, server)?;
            req.success(ResponseBody::Next)
        }
        Command::StepIn(ref args) => {
            let mode = resolve_step_mode(args.granularity.as_ref(), StepMode::StepInto);
            step_and_notify(state, mode, server)?;
            req.success(ResponseBody::StepIn)
        }
        Command::StepOut(_) => {
            step_and_notify(state, StepMode::StepOut, server)?;
            req.success(ResponseBody::StepOut)
        }
        Command::StackTrace(_) => handle_stack_trace(state, req),
        Command::Scopes(ref args) => handle_scopes(state, args, req),
        Command::Variables(ref args) => handle_variables(state, args, req),
        Command::Disconnect(_) => {
            state.replayer = None;
            req.success(ResponseBody::Disconnect)
        }
        Command::Evaluate(args) => match state.evaluate_on_frame(args.frame_id, &args.expression) {
            Ok(value) => req.success(ResponseBody::Evaluate(evaluate_response_from_value(
                state, value,
            ))),
            Err(err) => req.error(&err.to_string()),
        },
        _ => req.error("Unsupported command"),
    };

    server.respond(response)?;
    Ok(())
}

fn handle_launch(
    state: &mut DapState,
    _args: &requests::LaunchRequestArguments,
    req: Request,
) -> anyhow::Result<Response> {
    if let Some(ref mut r) = state.replayer {
        r.set_exception_breakpoints(state.pending_exception_mode);
        state.apply_pending_breakpoints();
        state.config_done = false;
        Ok(req.success(ResponseBody::Launch))
    } else {
        Ok(req.error("Debugger is not initialized"))
    }
}

fn handle_attach(
    state: &mut DapState,
    _args: &requests::AttachRequestArguments,
    req: Request,
) -> anyhow::Result<Response> {
    if let Some(ref mut r) = state.replayer {
        r.set_exception_breakpoints(state.pending_exception_mode);
        state.apply_pending_breakpoints();
        state.config_done = false;
        Ok(req.success(ResponseBody::Attach))
    } else {
        Ok(req.error("Debugger is not initialized"))
    }
}

fn handle_set_breakpoints(
    state: &mut DapState,
    args: &requests::SetBreakpointsArguments,
    req: Request,
) -> Response {
    let path = args
        .source
        .path
        .clone()
        .or_else(|| args.source.name.clone())
        .unwrap_or_default();
    let requested_lines = args
        .breakpoints
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|bp| bp.line.max(1) as usize)
        .collect::<Vec<_>>();
    let resolved_lines = state.resolve_breakpoint_lines_for_path(&path, &requested_lines);
    let mut source_breakpoints = Vec::new();
    let mut breakpoints = Vec::new();
    for (idx, bp) in args
        .breakpoints
        .as_deref()
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        let id = state.next_breakpoint_id;
        state.next_breakpoint_id += 1;

        source_breakpoints.push(SourceBreakpointInfo {
            id,
            line: bp.line,
            condition: bp.condition.clone(),
        });
        breakpoints.push(Breakpoint {
            id: Some(id),
            verified: true,
            source: Some(args.source.clone()),
            line: Some(
                resolved_lines
                    .as_ref()
                    .and_then(|lines| lines.get(idx).copied())
                    .map_or(bp.line, |line| line as i64),
            ),
            column: bp.column,
            ..Default::default()
        });
    }

    if source_breakpoints.is_empty() {
        state.pending_breakpoints.remove(&path);
    } else {
        state.pending_breakpoints.insert(path, source_breakpoints);
    }
    state.apply_pending_breakpoints();

    req.success(ResponseBody::SetBreakpoints(SetBreakpointsResponse {
        breakpoints,
    }))
}

fn handle_set_exception_breakpoints(
    state: &mut DapState,
    args: &requests::SetExceptionBreakpointsArguments,
    req: Request,
) -> Response {
    let mode = if args.filters.iter().any(|f| f == "all") {
        replayer::ExceptionBreakMode::All
    } else if args.filters.iter().any(|f| f == "uncaught") {
        replayer::ExceptionBreakMode::Uncaught
    } else {
        replayer::ExceptionBreakMode::Never
    };
    if let Some(ref mut r) = state.replayer {
        r.set_exception_breakpoints(mode);
    }
    state.pending_exception_mode = mode;
    req.success(ResponseBody::SetExceptionBreakpoints(
        SetExceptionBreakpointsResponse { breakpoints: None },
    ))
}

fn handle_exception_info(state: &DapState, req: Request) -> Response {
    let replayer = state.replayer.as_ref();
    let exc = replayer.and_then(|r| r.last_exception());
    match exc {
        Some(info) => {
            let break_mode = if info.is_uncaught {
                ExceptionBreakMode::Unhandled
            } else {
                ExceptionBreakMode::Always
            };
            let overview = exception_overview(info);
            req.success(ResponseBody::ExceptionInfo(ExceptionInfoResponse {
                exception_id: info.errno.clone(),
                description: Some(overview.info_description),
                break_mode,
                details: Some(build_exception_details(info)),
            }))
        }
        _ => req.error("No exception"),
    }
}

fn handle_configuration_done(
    state: &mut DapState,
    server: &mut DapConnection<impl BufRead, impl Write>,
    req: Request,
) -> anyhow::Result<Response> {
    state.config_done = true;
    let has_breakpoints = state
        .pending_breakpoints
        .values()
        .any(|breakpoints| !breakpoints.is_empty());
    // Match VS Code's startup flow: after configuration completes the adapter itself
    // must advance to the first interesting stop and report it as Entry unless a
    // stronger reason (breakpoint / exception / termination) wins first.
    let step_mode = if has_breakpoints {
        StepMode::RunUntilBreakpoint
    } else {
        StepMode::StepOver
    };
    match advance_to_next_stop(state, step_mode) {
        AdvanceStop::Terminated => send_terminated(server)?,
        AdvanceStop::Exception { description, text } => {
            send_stopped_exception(server, &description, &text)?;
        }
        AdvanceStop::Breakpoint(stop) => {
            send_stopped(
                server,
                StoppedEventReason::Breakpoint,
                Some(stop.description),
                Some(stop.ids),
            )?;
        }
        AdvanceStop::Step => send_stopped(server, StoppedEventReason::Entry, None, None)?,
    }
    Ok(req.success(ResponseBody::ConfigurationDone))
}

/// Collects all data from the replayer, then allocates frame IDs (separate borrows).
fn handle_stack_trace(state: &mut DapState, req: Request) -> Response {
    // Collect all needed data from replayer (immutable borrow)
    let collected = state.replayer.as_ref().map(|r| {
        let call_stack = r.call_stack();
        let file_id = r.current_file_id();
        let line = r.current_line();
        let column = r.current_column();
        let end_line = r.current_end_line();
        let end_column = r.current_end_column();
        let top_source = build_source(r, file_id);
        let top_name = call_stack
            .last()
            .map_or_else(|| r.current_file_name().to_string(), format_frame_name);
        let top_is_builtin = call_stack.last().is_some_and(|f| f.is_builtin);
        let stopped_on_exception = r.last_exception().is_some();

        struct ParentData {
            name: String,
            is_builtin: bool,
            source: Option<Source>,
            line: i64,
            col: i64,
            end_line: i64,
            end_column: i64,
        }

        let n = call_stack.len();
        let mut parents = Vec::new();
        for depth in 1..n {
            let frame_idx = n - 1 - depth;
            let frame = &call_stack[frame_idx];
            let child_frame = &call_stack[frame_idx + 1];
            let (source, line, col, end_line, end_column) = match &child_frame.call_site_loc {
                Some(loc) => (
                    Some(build_source(r, loc.file_id())),
                    loc.start_line() as i64,
                    loc.start_col() as i64,
                    loc.end_line() as i64,
                    loc.end_col() as i64,
                ),
                None => (None, 0, 0, 0, 0),
            };
            parents.push(ParentData {
                name: format_frame_name(frame),
                is_builtin: frame.is_builtin,
                source,
                line,
                col,
                end_line,
                end_column,
            });
        }

        (
            line,
            column,
            end_line,
            end_column,
            top_name,
            top_is_builtin,
            top_source,
            parents,
            stopped_on_exception,
        )
    });

    let Some((
        line,
        column,
        end_line,
        end_column,
        top_name,
        top_is_builtin,
        top_source,
        parents,
        stopped_on_exception,
    )) = collected
    else {
        return req.success(ResponseBody::StackTrace(StackTraceResponse {
            stack_frames: Vec::new(),
            total_frames: Some(0),
        }));
    };

    // Now allocate frame IDs (mutable borrow of state — no conflict with replayer)
    state.frame_to_depth.clear();
    state.vars_debug_values.clear();
    state.runtime_register_scope_requests.clear();
    let total = 1 + parents.len();
    let mut frames: Vec<StackFrame> = Vec::with_capacity(total);

    let top_hint = if top_is_builtin && !stopped_on_exception {
        Some(StackFramePresentationhint::Subtle)
    } else {
        None
    };
    let top_id = state.alloc_frame_id(0);
    frames.push(StackFrame {
        id: top_id,
        name: top_name,
        source: Some(top_source),
        line: line as i64,
        column: column as i64,
        end_line: (end_line > 0).then_some(end_line as i64),
        end_column: (end_column > 0).then_some(end_column as i64),
        can_restart: None,
        module_id: None,
        presentation_hint: top_hint,
        instruction_pointer_reference: None,
    });

    for (i, p) in parents.into_iter().enumerate() {
        let hint = if p.is_builtin {
            Some(StackFramePresentationhint::Subtle)
        } else {
            None
        };
        let id = state.alloc_frame_id(i + 1);
        frames.push(StackFrame {
            id,
            name: p.name,
            source: p.source,
            line: p.line,
            column: p.col,
            end_line: (p.end_line > 0).then_some(p.end_line),
            end_column: (p.end_column > 0).then_some(p.end_column),
            can_restart: None,
            module_id: None,
            presentation_hint: hint,
            instruction_pointer_reference: None,
        });
    }

    req.success(ResponseBody::StackTrace(StackTraceResponse {
        stack_frames: frames,
        total_frames: Some(total as i64),
    }))
}

fn handle_scopes(state: &mut DapState, args: &requests::ScopesArguments, req: Request) -> Response {
    let mut scopes = vec![Scope {
        name: "Locals".to_string(),
        variables_reference: args.frame_id,
        named_variables: None,
        indexed_variables: None,
        expensive: false,
        source: None,
        line: None,
        column: None,
        end_line: None,
        end_column: None,
        presentation_hint: Some(ScopePresentationhint::Locals),
    }];

    if state
        .replayer
        .as_ref()
        .is_some_and(|r| r.runtime_backend_kind() == replayer::RuntimeBackendKind::LiveVm)
    {
        let registers_ref = state.alloc_req_id();
        state.runtime_register_scope_requests.insert(registers_ref);
        scopes.push(Scope {
            name: "Registers".to_string(),
            variables_reference: registers_ref,
            named_variables: None,
            indexed_variables: None,
            expensive: false,
            source: None,
            line: None,
            column: None,
            end_line: None,
            end_column: None,
            presentation_hint: Some(ScopePresentationhint::Registers),
        });
    }

    req.success(ResponseBody::Scopes(ScopesResponse { scopes }))
}

fn handle_variables(
    state: &mut DapState,
    args: &requests::VariablesArguments,
    req: Request,
) -> Response {
    let req_id = args.variables_reference;

    // Path A: frame-level request — return top-level locals
    if let Some(&depth) = state.frame_to_depth.get(&req_id) {
        let locals = state
            .replayer
            .as_ref()
            .map(|r| r.locals_for_frame(depth))
            .unwrap_or_default();
        let variables: Vec<Variable> = locals
            .into_iter()
            .map(|lv| debug_value_to_variable(state, lv.var_name, &lv.value))
            .collect();
        return req.success(ResponseBody::Variables(VariablesResponse { variables }));
    }

    if state.runtime_register_scope_requests.contains(&req_id) {
        let variables = state
            .replayer
            .as_ref()
            .map(TolkReplayer::runtime_registers)
            .unwrap_or_default()
            .into_iter()
            .map(|lv| debug_value_to_variable(state, lv.var_name, &lv.value))
            .collect();
        return req.success(ResponseBody::Variables(VariablesResponse { variables }));
    }

    // Path B: drill-down into a structured RenderedValue
    if let Some(dv) = state.vars_debug_values.get(&req_id).cloned() {
        let variables = expand_debug_value(state, &dv);
        return req.success(ResponseBody::Variables(VariablesResponse { variables }));
    }

    req.success(ResponseBody::Variables(VariablesResponse {
        variables: Vec::new(),
    }))
}

fn expand_debug_value(state: &mut DapState, dv: &RenderedValue) -> Vec<Variable> {
    match dv {
        RenderedValue::Struct { fields, .. }
        | RenderedValue::Address { fields, .. }
        | RenderedValue::CellLike { fields, .. }
        | RenderedValue::EnumValue { fields, .. }
        | RenderedValue::UnionCase { fields, .. }
        | RenderedValue::CellOf { fields, .. } => fields
            .iter()
            .map(|(name, val)| debug_value_to_variable(state, name.clone(), val))
            .collect(),
        RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => items
            .iter()
            .enumerate()
            .map(|(i, val)| debug_value_to_variable(state, i.to_string(), val))
            .collect(),
        RenderedValue::LastSeen { inner } => expand_debug_value(state, inner),
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn serve_single_replayer_dap(replayer: TolkReplayer, port: u16) -> anyhow::Result<()> {
    let address = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&address)?;

    println!("Debugger listening on {address}");

    let (stream, remote_addr) = listener.accept()?;
    println!("DAP client connected from {remote_addr}");

    let input = BufReader::new(stream.try_clone()?);
    let output = stream;
    let mut server = DapConnection::new(input, output);
    let mut state = DapState::new();
    state.replayer = Some(replayer);

    if let Some(req) = server.poll_request()? {
        match req {
            IncomingRequest::Known(req) => {
                if let Command::Initialize(_) = req.command {
                    server.respond(req.success(ResponseBody::Initialize(make_capabilities())))?;
                    server.send_event(Event::Initialized)?;
                } else {
                    handle_request(&mut state, req, &mut server)?;
                }
            }
            IncomingRequest::Unsupported { seq, command } => {
                server.respond_custom_success(seq, &command)?;
            }
        }
    }

    loop {
        match server.poll_request()? {
            Some(IncomingRequest::Known(req)) => handle_request(&mut state, req, &mut server)?,
            Some(IncomingRequest::Unsupported { seq, command }) => {
                server.respond_custom_success(seq, &command)?;
            }
            None => break,
        }
    }

    println!("DAP client disconnected");
    Ok(())
}
