//! `ReplayerDebugSession` exposes one DAP session over a stack of `TolkReplayer`s.
//! The root context debugs the current script/test, while nested runtime operations
//! (`send_message`, `run_get_method`) temporarily push child contexts backed by
//! live executors and later pop back to the parent.

use crate::core::evaluate::{evaluate_condition_expression, evaluate_expression};
use crate::multi::dap_transport::{DapMessage, DapTransport};
use crate::multi::session::ChildDebugContextSpec;
use crate::replayer::{
    self, CallFrameInfo, ExceptionInfo, LocalVarRendered, StepMode, TolkReplayer,
};
use crate::types_render::RenderedValue;
use crate::{
    core::exception_format::build_exception_details, core::exception_format::exception_overview,
};
use anyhow::anyhow;
use dap::events::{Event, ExitedEventBody, StoppedEventBody, TerminatedEventBody};
use dap::prelude::{Command, Request, Response, ResponseBody};
use dap::requests::{
    AttachRequestArguments, ScopesArguments, SetBreakpointsArguments,
    SetExceptionBreakpointsArguments, VariablesArguments,
};
use dap::responses::{
    ContinueResponse, EvaluateResponse, ExceptionInfoResponse, ScopesResponse,
    SetBreakpointsResponse, SetExceptionBreakpointsResponse, StackTraceResponse, ThreadsResponse,
    VariablesResponse,
};
use dap::types::{
    Breakpoint, ExceptionBreakMode, ExceptionBreakpointsFilter, Scope, ScopePresentationhint,
    Source, StackFrame, StackFramePresentationhint, StoppedEventReason, Thread, Variable,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

const THREAD_ID: i64 = 1;

const fn resolve_step_mode(
    granularity: Option<&dap::types::SteppingGranularity>,
    default: StepMode,
) -> StepMode {
    match granularity {
        Some(dap::types::SteppingGranularity::Instruction) => StepMode::EachAsmInstruction,
        _ => default,
    }
}

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

enum AdvanceOutcome {
    Terminated,
    Stopped(StopReason),
}

#[derive(Debug, Clone)]
struct FrameLocator {
    context_idx: usize,
    depth_from_top: usize,
    /// Parent contexts are still borrowed while a child VM message is stopped;
    /// frozen locals keep those outer stack frames inspectable in DAP.
    snapshot_locals: Option<Vec<LocalVarRendered>>,
}

#[derive(Debug, Clone, Copy)]
struct StepFramePosition {
    context_idx: usize,
    file_id: usize,
    line: usize,
}

struct ReplayerContext {
    label: Arc<str>,
    replayer: TolkReplayer,
    /// Breakpoints resolve against the child replayer's own source map, which may
    /// differ from the parent contract when a nested call crosses contract boundaries.
    resolved_breakpoints: HashMap<(usize, usize), Vec<SourceBreakpointInfo>>,
    /// Snapshot of the parent-visible frames captured when this child context starts.
    /// Appended after child frames so stack traces preserve the runtime call chain.
    outer_frames: Vec<CollectedFrame>,
}

impl ReplayerContext {
    fn new(label: Arc<str>, replayer: TolkReplayer) -> Self {
        Self {
            label,
            replayer,
            resolved_breakpoints: HashMap::new(),
            outer_frames: Vec::new(),
        }
    }

    fn with_outer_frames(
        label: Arc<str>,
        replayer: TolkReplayer,
        outer_frames: Vec<CollectedFrame>,
    ) -> Self {
        Self {
            label,
            replayer,
            resolved_breakpoints: HashMap::new(),
            outer_frames,
        }
    }
}

pub struct ReplayerDebugSession {
    transport: DapTransport,
    /// Context stack: the active debugger always talks to the last replayer here.
    contexts: Vec<Rc<RefCell<ReplayerContext>>>,
    breakpoints: HashMap<PathBuf, Vec<SourceBreakpointInfo>>,
    next_breakpoint_id: i64,
    exception_mode: replayer::ExceptionBreakMode,
    performing_step: Option<StepMode>,
    /// Last visible frame snapshot captured while stepping the parent. Reused when a
    /// child context starts so it can keep showing where the nested call came from.
    cached_visible_frames: RefCell<Vec<CollectedFrame>>,
    capture_outer_frame_locals: bool,
    frame_to_depth: HashMap<i64, FrameLocator>,
    vars_debug_values: HashMap<i64, RenderedValue>,
    runtime_register_scope_requests: HashMap<i64, usize>,
    next_req_id: i64,
    stop_requested: bool,
    /// Parent step state must survive nested child VM stepping.
    active_step_start: Option<StepFramePosition>,
    /// Set after child VM returns when the parent may first land on `active_step_start`.
    skip_active_step_start_once: bool,
}

impl ReplayerDebugSession {
    /// Runtime shims and generated helpers are not user code. When Step Into
    /// lands there, keep stepping until we either enter a nested child context
    /// or reach a genuinely user-visible stop.
    fn is_transparent_step_into_function(function_name: &str) -> bool {
        fn strip_generic_suffix(name: &str) -> &str {
            name.split_once('<').map_or(name, |(base, _)| base)
        }

        let (receiver_name, leaf_function_name) = function_name.rsplit_once('.').map_or_else(
            || (None, strip_generic_suffix(function_name)),
            |(receiver, leaf)| {
                (
                    Some(strip_generic_suffix(receiver)),
                    strip_generic_suffix(leaf),
                )
            },
        );

        if leaf_function_name.starts_with("__") {
            return true;
        }

        if receiver_name == Some("impl") {
            // skip any impl.foo functions
            return true;
        }

        if leaf_function_name.contains("runGetMethod") {
            // runGetMethod or runGetMethodById
            return true;
        }

        matches!(
            (receiver_name, leaf_function_name),
            (Some("net"), "send")
                | (Some("net"), "sendExternal")
                | (Some("testing"), "processSingleTraceStep")
                | (Some("testing"), "createTraceIterationCursor")
                | (Some("TxCursor"), "isDone")
                | (Some("TxCursor"), "close")
                | (Some("TxCursor"), "executeN")
                | (Some("TxCursor"), "executeTill")
                | (Some("TxCursor"), "executeAllRemaining")
                | (Some("testing"), "isDeployed")
        )
    }

    #[must_use]
    pub fn new(transport: DapTransport, replayer: TolkReplayer, root_name: Arc<str>) -> Self {
        Self {
            transport,
            contexts: vec![Rc::new(RefCell::new(ReplayerContext::new(
                root_name, replayer,
            )))],
            breakpoints: HashMap::new(),
            next_breakpoint_id: 1,
            exception_mode: replayer::ExceptionBreakMode::Uncaught,
            performing_step: None,
            cached_visible_frames: RefCell::new(Vec::new()),
            capture_outer_frame_locals: true,
            frame_to_depth: HashMap::new(),
            vars_debug_values: HashMap::new(),
            runtime_register_scope_requests: HashMap::new(),
            next_req_id: 1_000_000,
            stop_requested: false,
            active_step_start: None,
            skip_active_step_start_once: false,
        }
    }

    #[must_use]
    pub const fn with_outer_frame_local_snapshots(mut self, enabled: bool) -> Self {
        self.capture_outer_frame_locals = enabled;
        self
    }

    fn active_context(&self) -> Option<Rc<RefCell<ReplayerContext>>> {
        self.contexts.last().cloned()
    }

    fn send_response(&self, response: Response) -> anyhow::Result<()> {
        self.transport
            .dap_sender
            .send(DapMessage::Response(response))?;
        Ok(())
    }

    fn send_event(&self, event: Event) -> anyhow::Result<()> {
        self.transport.dap_sender.send(DapMessage::Event(event))?;
        Ok(())
    }

    fn send_stopped(
        &self,
        reason: StoppedEventReason,
        description: Option<String>,
        hit_breakpoint_ids: Option<Vec<i64>>,
    ) -> anyhow::Result<()> {
        self.send_event(Event::Stopped(StoppedEventBody {
            reason,
            description,
            thread_id: Some(THREAD_ID),
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: Some(true),
            hit_breakpoint_ids,
        }))
    }

    fn send_exception_stop(&self, exc: &ExceptionInfo) -> anyhow::Result<()> {
        let overview = exception_overview(exc);
        self.send_event(Event::Stopped(StoppedEventBody {
            reason: StoppedEventReason::Exception,
            description: Some(overview.stop_description),
            thread_id: Some(THREAD_ID),
            preserve_focus_hint: None,
            text: Some(overview.stop_text),
            all_threads_stopped: Some(true),
            hit_breakpoint_ids: None,
        }))
    }

    fn send_terminated(&self) -> anyhow::Result<()> {
        self.send_event(Event::Exited(ExitedEventBody { exit_code: 0 }))?;
        self.send_event(Event::Terminated(Some(TerminatedEventBody::default())))
    }

    const fn alloc_req_id(&mut self) -> i64 {
        let id = self.next_req_id;
        self.next_req_id += 1;
        id
    }

    fn store_debug_value(&mut self, dv: RenderedValue) -> i64 {
        let id = self.alloc_req_id();
        self.vars_debug_values.insert(id, dv);
        id
    }

    fn evaluate_on_frame(
        &self,
        frame_id: Option<i64>,
        expression: &str,
    ) -> anyhow::Result<RenderedValue> {
        if let Some(frame_id) = frame_id {
            let locator = self
                .frame_to_depth
                .get(&frame_id)
                .cloned()
                .ok_or_else(|| anyhow!("Unknown frame id {frame_id}"))?;
            if let Some(locals) = locator.snapshot_locals {
                return evaluate_expression(&locals, expression);
            }
            let Some(ctx) = self.contexts.get(locator.context_idx) else {
                return evaluate_expression(&[], expression);
            };
            let Ok(ctx) = ctx.try_borrow() else {
                return evaluate_expression(&[], expression);
            };
            let locals = ctx.replayer.locals_for_frame(locator.depth_from_top);
            evaluate_expression(&locals, expression)
        } else {
            let Some(ctx) = self.active_context() else {
                return evaluate_expression(&[], expression);
            };
            let Ok(ctx) = ctx.try_borrow() else {
                return evaluate_expression(&[], expression);
            };
            let locals = ctx.replayer.locals_for_frame(0);
            evaluate_expression(&locals, expression)
        }
    }

    fn alloc_frame_id(&mut self, locator: FrameLocator) -> i64 {
        let id = self.frame_to_depth.len() as i64 + 1;
        self.frame_to_depth.insert(id, locator);
        id
    }

    fn apply_breakpoints_to_context(&self, ctx_index: usize) {
        let stored = self.breakpoints.clone();
        let Some(ctx) = self.contexts.get(ctx_index) else {
            return;
        };
        let Ok(mut ctx) = ctx.try_borrow_mut() else {
            return;
        };

        ctx.replayer.clear_all_breakpoints();
        ctx.resolved_breakpoints.clear();

        for (path, breakpoints) in stored {
            let path_str = path.to_string_lossy();
            let Some(file_id) = ctx.replayer.file_id_by_path(path_str.as_ref()) else {
                continue;
            };

            let requested_lines = breakpoints
                .iter()
                .map(|bp| bp.line.max(1) as usize)
                .collect::<Vec<_>>();
            let resolved_lines = ctx
                .replayer
                .resolve_breakpoint_lines(file_id, &requested_lines);
            ctx.replayer.set_breakpoints(file_id, &requested_lines);

            for (bp, resolved_line) in breakpoints.into_iter().zip(resolved_lines) {
                ctx.resolved_breakpoints
                    .entry((file_id, resolved_line))
                    .or_default()
                    .push(bp);
            }
        }
    }

    fn apply_breakpoints_to_all_contexts(&self) {
        for idx in 0..self.contexts.len() {
            self.apply_breakpoints_to_context(idx);
        }
    }

    fn set_exception_mode(&mut self, mode: replayer::ExceptionBreakMode) {
        self.exception_mode = mode;
        for ctx in &mut self.contexts {
            if let Ok(mut ctx) = ctx.try_borrow_mut() {
                ctx.replayer.set_exception_breakpoints(mode);
            }
        }
    }

    fn current_breakpoint_check(&self) -> BreakpointCheck {
        let Some(ctx) = self.active_context() else {
            return BreakpointCheck::None;
        };
        let Ok(ctx) = ctx.try_borrow() else {
            return BreakpointCheck::None;
        };
        let file_id = ctx.replayer.current_file_id();
        let line = ctx.replayer.current_line();
        let Some(breakpoints) = ctx.resolved_breakpoints.get(&(file_id, line)) else {
            return BreakpointCheck::None;
        };

        evaluate_breakpoint_conditions(&ctx.replayer.locals_for_frame(0), breakpoints)
    }

    fn resolve_breakpoint_lines_for_path(
        &self,
        path: &Path,
        requested_lines: &[usize],
    ) -> Option<Vec<usize>> {
        self.contexts.iter().rev().find_map(|ctx| {
            let ctx = ctx.try_borrow().ok()?;
            let path_str = path.to_string_lossy();
            let file_id = ctx.replayer.file_id_by_path(path_str.as_ref())?;
            Some(
                ctx.replayer
                    .resolve_breakpoint_lines(file_id, requested_lines),
            )
        })
    }

    fn step_active_context(&self, mode: StepMode) -> bool {
        let Some(ctx) = self.active_context() else {
            return true;
        };
        let visible_frames_cache = &self.cached_visible_frames;
        let mut ctx = ctx.borrow_mut();

        let context_idx = self.contexts.len().saturating_sub(1);
        let label = Arc::clone(&ctx.label);
        let outer_frames = ctx.outer_frames.clone();
        ctx.replayer.step_with_callback(mode, |_tick, replayer| {
            *visible_frames_cache.borrow_mut() = Self::build_visible_frames_for(
                context_idx,
                label.as_ref(),
                &outer_frames,
                replayer,
                self.capture_outer_frame_locals,
            );
        });
        ctx.replayer.is_finished()
    }

    fn advance_active_context(&mut self, mode: StepMode) -> AdvanceOutcome {
        let track_step_start = matches!(
            mode,
            StepMode::StepOver | StepMode::StepInto | StepMode::StepOut
        );
        // Nested child contexts run their own steps, so restore the parent state
        // when this logical debugger action completes.
        let previous_step = self.performing_step;
        self.performing_step = Some(mode);
        let previous_step_start = self.active_step_start;
        self.active_step_start = if track_step_start {
            self.active_context().and_then(|ctx| {
                let context_idx = self.contexts.len().checked_sub(1)?;
                let ctx = ctx.try_borrow().ok()?;
                Some(StepFramePosition {
                    context_idx,
                    file_id: ctx.replayer.current_file_id(),
                    line: ctx.replayer.current_line(),
                })
            })
        } else {
            None
        };

        let outcome = loop {
            let is_end = self.step_active_context(mode);
            let skip_active_step_start = std::mem::take(&mut self.skip_active_step_start_once);
            if is_end {
                break AdvanceOutcome::Terminated;
            }

            if let Some(exc) = self
                .active_context()
                .and_then(|ctx| ctx.try_borrow().ok()?.replayer.last_exception().cloned())
            {
                break AdvanceOutcome::Stopped(StopReason::Exception(exc));
            }

            // Child VM return can surface the original parent call line once.
            // Skip it before breakpoint handling so that line does not re-stop.
            if skip_active_step_start
                && self.active_step_start.is_some_and(|start| {
                    let current_context_idx = self.contexts.len().saturating_sub(1);
                    current_context_idx == start.context_idx
                        && self.active_context().is_some_and(|ctx| {
                            ctx.try_borrow().is_ok_and(|ctx| {
                                ctx.replayer.current_file_id() == start.file_id
                                    && ctx.replayer.current_line() == start.line
                            })
                        })
                })
            {
                continue;
            }

            match self.current_breakpoint_check() {
                BreakpointCheck::Hit(stop) => {
                    break AdvanceOutcome::Stopped(StopReason::Breakpoint(stop));
                }
                BreakpointCheck::Skip if matches!(mode, StepMode::RunUntilBreakpoint) => {
                    continue;
                }
                BreakpointCheck::Skip | BreakpointCheck::None => {
                    break AdvanceOutcome::Stopped(StopReason::Step);
                }
            }
        };

        self.active_step_start = previous_step_start;
        self.performing_step = previous_step;
        outcome
    }

    fn stop_reason_for_active_context(&self) -> StopReason {
        if let Some(exc) = self
            .active_context()
            .and_then(|ctx| ctx.try_borrow().ok()?.replayer.last_exception().cloned())
        {
            return StopReason::Exception(exc);
        }

        if let BreakpointCheck::Hit(stop) = self.current_breakpoint_check() {
            return StopReason::Breakpoint(stop);
        }

        StopReason::Step
    }

    fn send_stop_reason(&self, reason: StopReason) -> anyhow::Result<()> {
        match reason {
            StopReason::Step => self.send_stopped(StoppedEventReason::Step, None, None),
            StopReason::Breakpoint(stop) => self.send_stopped(
                StoppedEventReason::Breakpoint,
                Some(stop.description),
                Some(stop.ids),
            ),
            StopReason::Exception(exc) => self.send_exception_stop(&exc),
        }
    }

    fn has_breakpoints(&self) -> bool {
        self.breakpoints
            .values()
            .any(|breakpoints| !breakpoints.is_empty())
    }

    fn active_context_uses_live_backend(&self) -> bool {
        self.active_context().and_then(|ctx| {
            ctx.try_borrow()
                .ok()
                .map(|ctx| ctx.replayer.runtime_backend_kind())
        }) == Some(replayer::RuntimeBackendKind::LiveVm)
    }

    const fn child_stop_on_entry_step_mode(&self) -> StepMode {
        match self.performing_step {
            Some(StepMode::EachAsmInstruction) => StepMode::EachAsmInstruction,
            _ => StepMode::StepInto,
        }
    }

    fn should_skip_step_into_stop(&self) -> bool {
        if !matches!(self.stop_reason_for_active_context(), StopReason::Step) {
            return false;
        }

        let Some(ctx) = self.active_context() else {
            return false;
        };
        let Ok(ctx) = ctx.try_borrow() else {
            return false;
        };
        let call_stack = ctx.replayer.call_stack();
        let Some(top_frame) = call_stack.last() else {
            return false;
        };
        if top_frame.definition_loc.is_none() {
            return true;
        }

        Self::is_transparent_step_into_function(top_frame.f_name.as_str())
    }

    fn step_into_until_user_visible_stop(&mut self) -> bool {
        loop {
            let is_end = self.step(StepMode::StepInto);
            if is_end || !self.should_skip_step_into_stop() {
                return is_end;
            }
        }
    }

    fn reapply_pending_debug_state(&mut self) {
        self.set_exception_mode(self.exception_mode);
        self.apply_breakpoints_to_all_contexts();
    }

    fn format_frame_name(
        context_label: &str,
        frame: Option<&CallFrameInfo>,
        fallback: &str,
    ) -> String {
        match frame {
            Some(f) if f.is_builtin => format!("{} (built-in)", f.f_name),
            Some(f) if f.is_inlined => format!("{} (inlined)", f.f_name),
            Some(f) => f.f_name.clone(),
            None => {
                if context_label.is_empty() {
                    fallback.to_string()
                } else {
                    format!("{fallback} [{context_label}]")
                }
            }
        }
    }

    fn build_source(replayer: &TolkReplayer, file_id: usize) -> Option<Source> {
        let path = replayer.file_full_path(file_id).map(str::to_owned);
        let name = replayer.file_display_name(file_id);

        if path.is_none() && name == "unknown-file" {
            return None;
        }

        Some(Source {
            name: Some(name.to_string()),
            path,
            ..Default::default()
        })
    }

    fn build_visible_frames_for(
        ctx_index: usize,
        context_label: &str,
        outer_frames: &[CollectedFrame],
        replayer: &TolkReplayer,
        capture_locals: bool,
    ) -> Vec<CollectedFrame> {
        let call_stack = replayer.call_stack();
        // Only runtime boundary helpers can open a child VM context while the
        // parent replayer is still borrowed, so snapshot locals only there.
        let capture_locals = capture_locals
            && call_stack
                .iter()
                .any(|frame| Self::is_transparent_step_into_function(frame.f_name.as_str()));
        let file_id = replayer.current_file_id();
        let line = replayer.current_line();
        let column = replayer.current_column();
        let end_line = replayer.current_end_line();
        let end_column = replayer.current_end_column();
        let top_frame = call_stack.last();
        let top_source = Self::build_source(replayer, file_id).or_else(|| {
            top_frame
                .and_then(|frame| frame.definition_loc.as_ref())
                .and_then(|loc| Self::build_source(replayer, loc.file_id()))
        });
        let top_name =
            Self::format_frame_name(context_label, top_frame, replayer.current_file_name());
        let top_is_builtin = top_frame.is_some_and(|f| f.is_builtin);
        let stopped_on_exception = replayer.last_exception().is_some();

        let mut frames = Vec::new();
        frames.push(CollectedFrame {
            context_idx: ctx_index,
            depth_from_top: 0,
            snapshot_locals: capture_locals.then(|| replayer.locals_for_frame(0)),
            name: top_name,
            source: top_source,
            line: line as i64,
            column: column as i64,
            end_line: end_line as i64,
            end_column: end_column as i64,
            is_builtin: top_is_builtin && !stopped_on_exception,
        });

        let n = call_stack.len();
        for depth in 1..n {
            let frame_idx = n - 1 - depth;
            let frame = &call_stack[frame_idx];
            let child_frame = &call_stack[frame_idx + 1];
            let (source, line, col, end_line, end_column) = match &child_frame.call_site_loc {
                Some(loc) => (
                    Self::build_source(replayer, loc.file_id()),
                    loc.start_line() as i64,
                    loc.start_col() as i64,
                    loc.end_line() as i64,
                    loc.end_col() as i64,
                ),
                None => (None, 0, 0, 0, 0),
            };
            frames.push(CollectedFrame {
                context_idx: ctx_index,
                depth_from_top: depth,
                snapshot_locals: capture_locals.then(|| replayer.locals_for_frame(depth)),
                name: Self::format_frame_name(context_label, Some(frame), frame.f_name.as_str()),
                source,
                line,
                column: col,
                end_line,
                end_column,
                is_builtin: frame.is_builtin,
            });
        }

        frames.extend(outer_frames.iter().cloned());
        frames
    }

    fn build_replayer_frames(
        &self,
        ctx_index: usize,
        ctx: &ReplayerContext,
    ) -> Vec<CollectedFrame> {
        Self::build_visible_frames_for(
            ctx_index,
            ctx.label.as_ref(),
            &ctx.outer_frames,
            &ctx.replayer,
            false,
        )
    }

    fn collect_visible_frames_snapshot(&self) -> Vec<CollectedFrame> {
        let Some(active_ctx) = self.active_context() else {
            return self.cached_visible_frames.borrow().clone();
        };
        let Ok(active_ctx) = active_ctx.try_borrow() else {
            return self.cached_visible_frames.borrow().clone();
        };

        let active_idx = self.contexts.len().saturating_sub(1);
        self.build_replayer_frames(active_idx, &active_ctx)
    }

    fn handle_variables(&mut self, args: &VariablesArguments) -> ResponseBody {
        let req_id = args.variables_reference;

        if let Some(locator) = self.frame_to_depth.get(&req_id).cloned() {
            let locals = if let Some(snapshot_locals) = locator.snapshot_locals {
                // Parent frames shown under a child VM stop cannot be read live.
                snapshot_locals
            } else {
                self.contexts
                    .get(locator.context_idx)
                    .and_then(|ctx| {
                        ctx.try_borrow()
                            .ok()
                            .map(|ctx| ctx.replayer.locals_for_frame(locator.depth_from_top))
                    })
                    .unwrap_or_default()
            };
            let variables = locals
                .into_iter()
                .map(|lv| self.debug_value_to_variable(lv))
                .collect();
            return ResponseBody::Variables(VariablesResponse { variables });
        }

        if let Some(&context_idx) = self.runtime_register_scope_requests.get(&req_id) {
            let variables = self
                .contexts
                .get(context_idx)
                .and_then(|ctx| {
                    ctx.try_borrow()
                        .ok()
                        .map(|ctx| ctx.replayer.runtime_registers())
                })
                .unwrap_or_default()
                .into_iter()
                .map(|lv| self.debug_value_to_named_variable(lv.var_name, &lv.value))
                .collect();
            return ResponseBody::Variables(VariablesResponse { variables });
        }

        if let Some(dv) = self.vars_debug_values.get(&req_id).cloned() {
            let variables = self.expand_debug_value(&dv);
            return ResponseBody::Variables(VariablesResponse { variables });
        }

        ResponseBody::Variables(VariablesResponse {
            variables: Vec::new(),
        })
    }

    fn debug_value_to_variable(&mut self, local: LocalVarRendered) -> Variable {
        self.debug_value_to_named_variable(local.var_name, &local.value)
    }

    fn debug_value_to_named_variable(&mut self, name: String, dv: &RenderedValue) -> Variable {
        let (value, type_field) = dv.dap_parts_for_client(Some(&name));
        let (value, var_ref) = if dv.has_children() {
            (value, self.store_debug_value(dv.clone()))
        } else {
            (value, 0)
        };
        Variable {
            name,
            value,
            type_field,
            variables_reference: var_ref,
            ..Default::default()
        }
    }

    fn expand_debug_value(&mut self, dv: &RenderedValue) -> Vec<Variable> {
        match dv {
            RenderedValue::Struct { fields, .. }
            | RenderedValue::MapKV { fields, .. }
            | RenderedValue::Address { fields, .. }
            | RenderedValue::CellLike { fields, .. }
            | RenderedValue::EnumValue { fields, .. }
            | RenderedValue::UnionCase { fields, .. }
            | RenderedValue::CellOf { fields, .. } => fields
                .iter()
                .map(|(name, val)| self.debug_value_to_named_variable(name.clone(), val))
                .collect(),
            RenderedValue::Tensor { items, .. } | RenderedValue::ArrayOf { items, .. } => items
                .iter()
                .enumerate()
                .map(|(i, val)| self.debug_value_to_named_variable(i.to_string(), val))
                .collect(),
            RenderedValue::LastSeen { inner } => self.expand_debug_value(inner),
            _ => Vec::new(),
        }
    }

    fn evaluate_response_from_value(&mut self, value: RenderedValue) -> EvaluateResponse {
        let (result, type_field) = value.dap_parts_for_client(None);
        let variables_reference = if value.has_children() {
            self.store_debug_value(value)
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

    fn handle_request(&mut self, req: Request, terminate_at_end: bool) -> anyhow::Result<bool> {
        let command = req.command.clone();
        match command {
            Command::Initialize(_) => {
                self.send_response(req.success(ResponseBody::Initialize(make_capabilities())))?;
                self.send_event(Event::Initialized)?;
            }
            Command::Launch(_) => {
                self.reapply_pending_debug_state();
                self.send_response(req.success(ResponseBody::Launch))?;
            }
            Command::Attach(AttachRequestArguments { .. }) => {
                self.reapply_pending_debug_state();
                self.send_response(req.success(ResponseBody::Attach))?;
            }
            Command::ConfigurationDone => {
                return self.handle_configuration_done(req, terminate_at_end);
            }
            Command::Threads => {
                self.send_response(req.success(ResponseBody::Threads(ThreadsResponse {
                    threads: vec![Thread {
                        id: THREAD_ID,
                        name: "main".to_string(),
                    }],
                })))?;
            }
            Command::SetBreakpoints(args) => {
                let body = self.set_breakpoints(&args);
                self.send_response(req.success(body))?;
            }
            Command::SetExceptionBreakpoints(args) => {
                let body = self.set_exception_breakpoints(&args);
                self.send_response(req.success(body))?;
            }
            Command::ExceptionInfo(_) => {
                self.send_response(self.handle_exception_info(req)?)?;
            }
            Command::StackTrace(_) => {
                let body = self.stack_trace();
                self.send_response(req.success(body))?;
            }
            Command::Scopes(args) => {
                let body = self.scopes(&args);
                self.send_response(req.success(body))?;
            }
            Command::Variables(args) => {
                let body = self.handle_variables(&args);
                self.send_response(req.success(body))?;
            }
            Command::Continue(_) => {
                self.send_response(req.success(ResponseBody::Continue(ContinueResponse {
                    all_threads_continued: Some(true),
                })))?;

                let is_end = self.step(StepMode::RunUntilBreakpoint);
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }
            }
            Command::StepIn(args) => {
                self.send_response(req.success(ResponseBody::StepIn))?;

                let mode = resolve_step_mode(args.granularity.as_ref(), StepMode::StepInto);
                let is_end = if mode == StepMode::StepInto {
                    self.step_into_until_user_visible_stop()
                } else {
                    self.step(mode)
                };
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }

                self.send_stop_reason(self.stop_reason_for_active_context())?;
            }
            Command::Next(args) => {
                self.send_response(req.success(ResponseBody::Next))?;

                let mode = resolve_step_mode(args.granularity.as_ref(), StepMode::StepOver);
                let is_end = self.step(mode);
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }

                self.send_stop_reason(self.stop_reason_for_active_context())?;
            }
            Command::StepOut(_) => {
                self.send_response(req.success(ResponseBody::StepOut))?;

                let is_end = self.step(StepMode::StepOut);
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }

                self.send_stop_reason(self.stop_reason_for_active_context())?;
            }
            Command::Disconnect(_) => {
                self.send_response(req.success(ResponseBody::Disconnect))?;
                self.stop_requested = true;
                return Ok(true);
            }
            Command::Terminate(_) => {
                self.send_response(req.success(ResponseBody::Terminate))?;
                self.stop_requested = true;
                return Ok(true);
            }
            Command::Evaluate(args) => {
                let response = match self.evaluate_on_frame(args.frame_id, &args.expression) {
                    Ok(value) => req.success(ResponseBody::Evaluate(
                        self.evaluate_response_from_value(value),
                    )),
                    Err(err) => req.error(&err.to_string()),
                };
                self.send_response(response)?;
            }
            _ => {
                return Err(anyhow!("Unhandled command: {:?}", req.command));
            }
        }

        Ok(false)
    }

    fn handle_exception_info(&self, req: Request) -> anyhow::Result<Response> {
        Ok(req.success(self.exception_info()?))
    }

    fn handle_configuration_done(
        &mut self,
        req: Request,
        terminate_at_end: bool,
    ) -> anyhow::Result<bool> {
        self.send_response(req.success(ResponseBody::ConfigurationDone))?;

        // Live executors do not start with a precomputed "entry" stop, so without
        // breakpoints we step once into the runtime and then report whatever stop
        // reason that initial movement produced.
        let step_mode = if self.has_breakpoints() {
            StepMode::RunUntilBreakpoint
        } else if self.active_context_uses_live_backend() {
            StepMode::StepInto
        } else {
            StepMode::StepOver
        };
        match self.advance_active_context(step_mode) {
            AdvanceOutcome::Terminated => {
                if terminate_at_end {
                    self.send_terminated()?;
                }
                return Ok(true);
            }
            AdvanceOutcome::Stopped(StopReason::Breakpoint(stop)) => {
                self.send_stopped(
                    StoppedEventReason::Breakpoint,
                    Some(stop.description),
                    Some(stop.ids),
                )?;
            }
            AdvanceOutcome::Stopped(StopReason::Exception(exc)) => {
                self.send_exception_stop(&exc)?;
            }
            AdvanceOutcome::Stopped(StopReason::Step) => {
                self.send_stopped(StoppedEventReason::Entry, None, None)?;
            }
        }

        Ok(false)
    }

    fn set_breakpoints(&mut self, args: &SetBreakpointsArguments) -> ResponseBody {
        let path = args
            .source
            .path
            .clone()
            .or_else(|| args.source.name.clone())
            .unwrap_or_default();
        let path_buf = PathBuf::from(&path);
        let requested_lines = args
            .breakpoints
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|bp| bp.line.max(1) as usize)
            .collect::<Vec<_>>();
        let resolved_lines = self.resolve_breakpoint_lines_for_path(&path_buf, &requested_lines);

        let mut breakpoints = Vec::new();
        let mut file_breakpoints = Vec::new();
        for (idx, bp) in args
            .breakpoints
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let id = self.next_breakpoint_id;
            self.next_breakpoint_id += 1;

            file_breakpoints.push(SourceBreakpointInfo {
                id,
                line: bp.line,
                condition: bp.condition.clone(),
            });
            breakpoints.push(Breakpoint {
                id: Some(id),
                verified: true,
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

        if file_breakpoints.is_empty() {
            self.breakpoints.remove(&path_buf);
        } else {
            self.breakpoints.insert(path_buf, file_breakpoints);
        }
        self.apply_breakpoints_to_all_contexts();

        ResponseBody::SetBreakpoints(SetBreakpointsResponse { breakpoints })
    }

    fn set_exception_breakpoints(
        &mut self,
        args: &SetExceptionBreakpointsArguments,
    ) -> ResponseBody {
        let mode = if args.filters.iter().any(|f| f == "all") {
            replayer::ExceptionBreakMode::All
        } else if args.filters.iter().any(|f| f == "uncaught") {
            replayer::ExceptionBreakMode::Uncaught
        } else {
            replayer::ExceptionBreakMode::Never
        };
        self.set_exception_mode(mode);

        ResponseBody::SetExceptionBreakpoints(SetExceptionBreakpointsResponse { breakpoints: None })
    }

    fn exception_info(&self) -> anyhow::Result<ResponseBody> {
        let exc = self
            .active_context()
            .and_then(|ctx| ctx.try_borrow().ok()?.replayer.last_exception().cloned())
            .ok_or_else(|| anyhow!("No exception"))?;

        let break_mode = if exc.is_uncaught {
            ExceptionBreakMode::Unhandled
        } else {
            ExceptionBreakMode::Always
        };
        let overview = exception_overview(&exc);
        let details = build_exception_details(&exc);

        Ok(ResponseBody::ExceptionInfo(ExceptionInfoResponse {
            exception_id: exc.errno,
            description: Some(overview.info_description),
            break_mode,
            details: Some(details),
        }))
    }

    fn stack_trace(&mut self) -> ResponseBody {
        self.frame_to_depth.clear();
        self.vars_debug_values.clear();
        self.runtime_register_scope_requests.clear();

        let collected = self.collect_visible_frames_snapshot();

        let mut stack_frames = Vec::new();
        for frame in collected {
            let id = self.alloc_frame_id(FrameLocator {
                context_idx: frame.context_idx,
                depth_from_top: frame.depth_from_top,
                snapshot_locals: frame.snapshot_locals,
            });

            stack_frames.push(StackFrame {
                id,
                name: frame.name,
                source: frame.source,
                line: frame.line,
                column: frame.column,
                end_line: (frame.end_line > 0).then_some(frame.end_line),
                end_column: (frame.end_column > 0).then_some(frame.end_column),
                presentation_hint: frame
                    .is_builtin
                    .then_some(StackFramePresentationhint::Subtle),
                ..Default::default()
            });
        }

        ResponseBody::StackTrace(StackTraceResponse {
            total_frames: Some(stack_frames.len() as i64),
            stack_frames,
        })
    }

    fn scopes(&mut self, args: &ScopesArguments) -> ResponseBody {
        let mut scopes = vec![Scope {
            name: "Locals".to_string(),
            variables_reference: args.frame_id,
            expensive: false,
            presentation_hint: Some(ScopePresentationhint::Locals),
            ..Default::default()
        }];

        let live_context_idx = self.frame_to_depth.get(&args.frame_id).and_then(|locator| {
            self.contexts
                .get(locator.context_idx)
                .and_then(|ctx| ctx.try_borrow().ok())
                .filter(|ctx| {
                    ctx.replayer.runtime_backend_kind() == replayer::RuntimeBackendKind::LiveVm
                })
                .map(|_| locator.context_idx)
        });

        if let Some(context_idx) = live_context_idx {
            // Register views exist only for live executors; VM-log replay cannot recover
            // stable c4/c5/c7 snapshots after the fact.
            let registers_ref = self.alloc_req_id();
            self.runtime_register_scope_requests
                .insert(registers_ref, context_idx);
            scopes.push(Scope {
                name: "Registers".to_string(),
                variables_reference: registers_ref,
                expensive: false,
                presentation_hint: Some(ScopePresentationhint::Registers),
                ..Default::default()
            });
        }

        ResponseBody::Scopes(ScopesResponse { scopes })
    }
}

impl ReplayerDebugSession {
    pub fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<bool> {
        for req in &self.transport.req_receiver.clone() {
            if self.handle_request(req.clone(), terminate_at_end)? {
                break;
            }
        }
        Ok(self.stop_requested)
    }

    pub const fn need_to_stop_child_thread_on_start(&self) -> bool {
        matches!(
            self.performing_step,
            Some(StepMode::StepInto | StepMode::EachAsmInstruction)
        )
    }

    pub fn begin_child_context(&mut self, spec: ChildDebugContextSpec) -> anyhow::Result<bool> {
        let Some(source_map) = spec.source_map else {
            return Ok(false);
        };
        let Ok(mut replayer) = TolkReplayer::new_live_vm(source_map.as_ref(), spec.executor) else {
            return Ok(false);
        };
        replayer.set_abi(spec.abi);
        replayer.set_exception_breakpoints(self.exception_mode);

        // Freeze the currently visible parent frames before switching active context.
        // They become the suffix of the child's stack trace until the child completes.
        let outer_frames = self.cached_visible_frames.borrow().clone();
        let label: Arc<str> = spec.name.into();
        self.contexts
            .push(Rc::new(RefCell::new(ReplayerContext::with_outer_frames(
                label,
                replayer,
                outer_frames,
            ))));
        let new_idx = self.contexts.len() - 1;
        self.apply_breakpoints_to_context(new_idx);

        if spec.stop_on_entry {
            let step_mode = self.child_stop_on_entry_step_mode();
            match self.advance_active_context(step_mode) {
                AdvanceOutcome::Terminated => {
                    self.send_terminated()?;
                    return Ok(true);
                }
                AdvanceOutcome::Stopped(StopReason::Breakpoint(stop)) => {
                    self.send_stopped(
                        StoppedEventReason::Breakpoint,
                        Some(stop.description),
                        Some(stop.ids),
                    )?;
                }
                AdvanceOutcome::Stopped(StopReason::Exception(exc)) => {
                    self.send_exception_stop(&exc)?;
                }
                AdvanceOutcome::Stopped(StopReason::Step) => {
                    self.send_stopped(
                        StoppedEventReason::Entry,
                        Some(self.contexts[new_idx].borrow().label.to_string()),
                        None,
                    )?;
                }
            }
        }

        Ok(true)
    }

    pub fn finish_child_context(&mut self, _thread_id: i64) -> anyhow::Result<()> {
        if self.contexts.len() <= 1 {
            return Ok(());
        }

        self.contexts.pop();
        if let Some(position) = self.active_step_start
            && position.context_idx == self.contexts.len().saturating_sub(1)
        {
            // The next parent step may first report the original call line.
            self.skip_active_step_start_once = true;
        }
        Ok(())
    }

    pub fn step(&mut self, mode: StepMode) -> bool {
        match self.advance_active_context(mode) {
            AdvanceOutcome::Terminated => true,
            AdvanceOutcome::Stopped(reason) => {
                if matches!(mode, StepMode::RunUntilBreakpoint)
                    && !matches!(reason, StopReason::Step)
                {
                    let _ = self.send_stop_reason(reason);
                }
                false
            }
        }
    }

    pub fn active_context_is_terminated(&self) -> bool {
        self.active_context()
            .and_then(|ctx| ctx.try_borrow().ok().map(|ctx| ctx.replayer.is_finished()))
            .unwrap_or(true)
    }

    pub const fn performing_step(&self) -> Option<StepMode> {
        self.performing_step
    }
}

#[derive(Debug, Clone)]
struct CollectedFrame {
    context_idx: usize,
    depth_from_top: usize,
    snapshot_locals: Option<Vec<LocalVarRendered>>,
    name: String,
    source: Option<Source>,
    line: i64,
    column: i64,
    end_line: i64,
    end_column: i64,
    is_builtin: bool,
}

#[derive(Debug, Clone)]
enum StopReason {
    Step,
    Breakpoint(BreakpointStopInfo),
    Exception(ExceptionInfo),
}

fn make_capabilities() -> dap::types::Capabilities {
    dap::types::Capabilities {
        supports_configuration_done_request: Some(true),
        supports_exception_info_request: Some(true),
        supports_evaluate_for_hovers: Some(true),
        supports_conditional_breakpoints: Some(true),
        exception_breakpoint_filters: Some(vec![
            ExceptionBreakpointsFilter {
                filter: "uncaught".to_string(),
                label: "Uncaught Exceptions".to_string(),
                description: Some("Break when an exception terminates execution".to_string()),
                default: Some(true),
                supports_condition: None,
                condition_description: None,
            },
            ExceptionBreakpointsFilter {
                filter: "all".to_string(),
                label: "All Exceptions".to_string(),
                description: Some("Break on any exception, including caught".to_string()),
                default: Some(false),
                supports_condition: None,
                condition_description: None,
            },
        ]),
        ..Default::default()
    }
}

fn evaluate_breakpoint_conditions(
    locals: &[LocalVarRendered],
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
