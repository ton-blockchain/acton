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
}

#[derive(Debug, Clone, Copy)]
struct FrameLocator {
    context_idx: usize,
    depth_from_top: usize,
}

struct ReplayerContext {
    label: Arc<str>,
    replayer: TolkReplayer,
    resolved_breakpoints: HashMap<(usize, usize), Vec<i64>>,
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
    contexts: Vec<Rc<RefCell<ReplayerContext>>>,
    breakpoints: HashMap<PathBuf, Vec<SourceBreakpointInfo>>,
    next_breakpoint_id: i64,
    exception_mode: replayer::ExceptionBreakMode,
    performing_step: Option<StepMode>,
    cached_visible_frames: RefCell<Vec<CollectedFrame>>,
    frame_to_depth: HashMap<i64, FrameLocator>,
    vars_debug_values: HashMap<i64, RenderedValue>,
    next_req_id: i64,
}

impl ReplayerDebugSession {
    fn is_transparent_step_into_function(path: &str, function_name: &str) -> bool {
        let normalized = path.replace('\\', "/");
        if !normalized.ends_with("/emulation/network.tolk") {
            return false;
        }

        matches!(
            function_name,
            "send"
                | "net.send"
                | "sendSingle"
                | "net.sendSingle"
                | "sendIter"
                | "net.sendIter"
                | "sendExternal"
                | "net.sendExternal"
                | "net.isDeployed"
                | "net.getDeployedCode"
        ) || function_name.contains("runGetMethod")
    }

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
            frame_to_depth: HashMap::new(),
            vars_debug_values: HashMap::new(),
            next_req_id: 1_000_000,
        }
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
                    .push(bp.id);
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

    fn current_breakpoint_ids(&self) -> Option<Vec<i64>> {
        let ctx = self.active_context()?;
        let ctx = ctx.try_borrow().ok()?;
        let file_id = ctx.replayer.current_file_id();
        let line = ctx.replayer.current_line();
        ctx.resolved_breakpoints.get(&(file_id, line)).cloned()
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
            );
        });
        ctx.replayer.is_finished()
    }

    fn step_active_context_without_breakpoints(&self, mode: StepMode) -> bool {
        let Some(ctx) = self.active_context() else {
            return true;
        };
        let visible_frames_cache = &self.cached_visible_frames;
        let mut ctx = ctx.borrow_mut();
        ctx.replayer.clear_all_breakpoints();
        ctx.replayer
            .set_exception_breakpoints(replayer::ExceptionBreakMode::Never);
        let context_idx = self.contexts.len().saturating_sub(1);
        let label = Arc::clone(&ctx.label);
        let outer_frames = ctx.outer_frames.clone();
        ctx.replayer.step_with_callback(mode, |_tick, replayer| {
            *visible_frames_cache.borrow_mut() = Self::build_visible_frames_for(
                context_idx,
                label.as_ref(),
                &outer_frames,
                replayer,
            );
        });
        ctx.replayer.is_finished()
    }

    fn stop_reason_for_active_context(&self) -> StopReason {
        if let Some(exc) = self
            .active_context()
            .and_then(|ctx| ctx.try_borrow().ok()?.replayer.last_exception().cloned())
        {
            return StopReason::Exception(exc);
        }

        if let Some(ids) = self.current_breakpoint_ids() {
            return StopReason::Breakpoint(ids);
        }

        StopReason::Step
    }

    fn send_stop_reason(&self, reason: StopReason) -> anyhow::Result<()> {
        match reason {
            StopReason::Step => self.send_stopped(StoppedEventReason::Step, None, None),
            StopReason::Breakpoint(ids) => self.send_stopped(
                StoppedEventReason::Breakpoint,
                Some("Breakpoint hit".to_string()),
                Some(ids),
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
        let file_id = top_frame
            .definition_loc
            .as_ref()
            .map(|loc| loc.file_id())
            .unwrap_or_else(|| ctx.replayer.current_file_id());
        let Some(path) = ctx.replayer.file_full_path(file_id) else {
            return true;
        };

        Self::is_transparent_step_into_function(path, top_frame.f_name.as_str())
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
    ) -> Vec<CollectedFrame> {
        let call_stack = replayer.call_stack();
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
        let top_is_builtin = top_frame.map(|f| f.is_builtin).unwrap_or(false);
        let stopped_on_exception = replayer.last_exception().is_some();

        let mut frames = Vec::new();
        frames.push(CollectedFrame {
            context_idx: ctx_index,
            depth_from_top: 0,
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

        if let Some(locator) = self.frame_to_depth.get(&req_id).copied() {
            let locals = self
                .contexts
                .get(locator.context_idx)
                .and_then(|ctx| {
                    ctx.try_borrow()
                        .ok()
                        .map(|ctx| ctx.replayer.locals_for_frame(locator.depth_from_top))
                })
                .unwrap_or_default();
            let variables = locals
                .into_iter()
                .map(|lv| self.debug_value_to_variable(lv))
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
        let (value, type_field) = dv.dap_parts_for_client();
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
            RenderedValue::Struct { fields, .. } | RenderedValue::Address { fields, .. } => fields
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
                self.send_response(req.success(self.scopes(&args)))?;
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
                self.performing_step = Some(StepMode::RunUntilBreakpoint);
                self.step_active_context_without_breakpoints(StepMode::RunUntilBreakpoint);
                return Ok(true);
            }
            Command::Terminate(_) => {
                self.send_response(req.success(ResponseBody::Terminate))?;
                self.performing_step = Some(StepMode::RunUntilBreakpoint);
                self.step_active_context_without_breakpoints(StepMode::RunUntilBreakpoint);
                return Ok(true);
            }
            Command::Evaluate(args) => {
                self.send_response(req.success(ResponseBody::Evaluate(EvaluateResponse {
                    result: args.expression,
                    type_field: None,
                    presentation_hint: None,
                    variables_reference: 0,
                    named_variables: None,
                    indexed_variables: None,
                    memory_reference: None,
                })))?;
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

        let step_mode = if self.has_breakpoints() {
            StepMode::RunUntilBreakpoint
        } else if self.active_context_uses_live_backend() {
            StepMode::StepInto
        } else {
            StepMode::StepOver
        };
        self.performing_step = Some(step_mode);
        let is_end = self.step_active_context(step_mode);
        if is_end {
            if terminate_at_end {
                self.send_terminated()?;
            }
            return Ok(true);
        }

        match self.stop_reason_for_active_context() {
            StopReason::Breakpoint(ids) => {
                self.send_stopped(
                    StoppedEventReason::Breakpoint,
                    Some("Breakpoint hit".to_string()),
                    Some(ids),
                )?;
            }
            StopReason::Exception(exc) => {
                self.send_exception_stop(&exc)?;
            }
            StopReason::Step => {
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

            file_breakpoints.push(SourceBreakpointInfo { id, line: bp.line });
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

        let collected = self.collect_visible_frames_snapshot();

        let mut stack_frames = Vec::new();
        for frame in collected {
            let id = self.alloc_frame_id(FrameLocator {
                context_idx: frame.context_idx,
                depth_from_top: frame.depth_from_top,
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

    fn scopes(&self, args: &ScopesArguments) -> ResponseBody {
        ResponseBody::Scopes(ScopesResponse {
            scopes: vec![Scope {
                name: "Locals".to_string(),
                variables_reference: args.frame_id,
                expensive: false,
                presentation_hint: Some(ScopePresentationhint::Locals),
                ..Default::default()
            }],
        })
    }
}

impl ReplayerDebugSession {
    pub fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<()> {
        for req in &self.transport.req_receiver.clone() {
            if self.handle_request(req.clone(), terminate_at_end)? {
                break;
            }
        }
        Ok(())
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
        replayer.set_exception_breakpoints(self.exception_mode);

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
            self.performing_step = Some(step_mode);
            let is_end = self.step_active_context(step_mode);

            if is_end {
                self.send_terminated()?;
                return Ok(true);
            }

            if let Some(ids) = self.current_breakpoint_ids() {
                self.send_stopped(
                    StoppedEventReason::Breakpoint,
                    Some("Breakpoint hit".to_string()),
                    Some(ids),
                )?;
            } else if let StopReason::Exception(exc) = self.stop_reason_for_active_context() {
                self.send_exception_stop(&exc)?;
            } else {
                self.send_stopped(
                    StoppedEventReason::Entry,
                    Some(self.contexts[new_idx].borrow().label.to_string()),
                    None,
                )?;
            }
        }

        Ok(true)
    }

    pub fn finish_child_context(&mut self, _thread_id: i64) -> anyhow::Result<()> {
        if self.contexts.len() > 1 {
            self.contexts.pop();
        }
        Ok(())
    }

    pub fn step(&mut self, mode: StepMode) -> bool {
        self.performing_step = Some(mode);
        let is_end = self.step_active_context(mode);

        if !is_end && mode == StepMode::RunUntilBreakpoint {
            let reason = self.stop_reason_for_active_context();
            if !matches!(reason, StopReason::Step) {
                let _ = self.send_stop_reason(reason);
            }
        }

        is_end
    }

    pub fn active_context_is_terminated(&self) -> bool {
        self.active_context()
            .and_then(|ctx| ctx.try_borrow().ok().map(|ctx| ctx.replayer.is_finished()))
            .unwrap_or(true)
    }

    pub const fn performing_step(&self) -> Option<StepMode> {
        self.performing_step
    }

    pub const fn advance_parent_after_child_return(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct CollectedFrame {
    context_idx: usize,
    depth_from_top: usize,
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
    Breakpoint(Vec<i64>),
    Exception(ExceptionInfo),
}

fn make_capabilities() -> dap::types::Capabilities {
    dap::types::Capabilities {
        supports_configuration_done_request: Some(true),
        supports_exception_info_request: Some(true),
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
