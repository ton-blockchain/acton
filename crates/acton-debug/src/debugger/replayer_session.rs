use crate::debugger::dap::{DapMessage, DapTransport};
use crate::debugger::session::{ChildDebugContextSpec, DebugSession, StepMode};
use crate::replayer::{self, CallFrameInfo, ExceptionInfo, LocalVarRendered, TolkReplayer};
use crate::types_render::RenderedValue;
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
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

const THREAD_ID: i64 = 1;

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
}

impl ReplayerContext {
    fn new(label: Arc<str>, replayer: TolkReplayer) -> Self {
        Self {
            label,
            replayer,
            resolved_breakpoints: HashMap::new(),
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
    frame_to_depth: HashMap<i64, FrameLocator>,
    vars_debug_values: HashMap<i64, RenderedValue>,
    next_req_id: i64,
}

impl ReplayerDebugSession {
    pub fn new(transport: DapTransport, replayer: TolkReplayer, root_name: Arc<str>) -> Self {
        Self {
            transport,
            contexts: vec![Rc::new(RefCell::new(ReplayerContext::new(
                root_name, replayer,
            )))],
            breakpoints: HashMap::new(),
            next_breakpoint_id: 1,
            exception_mode: replayer::ExceptionBreakMode::Never,
            performing_step: None,
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
        self.send_event(Event::Stopped(StoppedEventBody {
            reason: StoppedEventReason::Exception,
            description: Some("Paused on exception".to_string()),
            thread_id: Some(THREAD_ID),
            preserve_focus_hint: None,
            text: Some(format!("Exit code {}", exc.errno)),
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

    const fn replayer_step_mode(mode: StepMode) -> replayer::StepMode {
        match mode {
            StepMode::StepIn => replayer::StepMode::StepInto,
            StepMode::StepOver => replayer::StepMode::StepOver,
            StepMode::StepOut => replayer::StepMode::StepOut,
            StepMode::Continue | StepMode::ContinueWithoutBreakpoints => {
                replayer::StepMode::RunUntilBreakpoint
            }
        }
    }

    fn step_active_context(&self, mode: StepMode) -> bool {
        let Some(ctx) = self.active_context() else {
            return true;
        };
        let mut ctx = ctx.borrow_mut();

        if mode == StepMode::ContinueWithoutBreakpoints {
            ctx.replayer.clear_all_breakpoints();
            ctx.replayer
                .set_exception_breakpoints(replayer::ExceptionBreakMode::Never);
        }

        ctx.replayer.step(Self::replayer_step_mode(mode));
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

    fn send_initial_stop(&self) -> anyhow::Result<()> {
        self.send_stopped(StoppedEventReason::Step, None, None)
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

    fn build_source(replayer: &TolkReplayer, file_id: usize) -> Source {
        Source {
            name: Some(replayer.file_display_name(file_id).to_string()),
            path: replayer.file_full_path(file_id).map(|s| s.to_string()),
            ..Default::default()
        }
    }

    fn build_replayer_frames(
        &self,
        ctx_index: usize,
        ctx: &ReplayerContext,
    ) -> Vec<CollectedFrame> {
        let call_stack = ctx.replayer.call_stack();
        let file_id = ctx.replayer.current_file_id();
        let line = ctx.replayer.current_line();
        let column = ctx.replayer.current_column();
        let top_source = Self::build_source(&ctx.replayer, file_id);
        let top_name = Self::format_frame_name(
            ctx.label.as_ref(),
            call_stack.last(),
            ctx.replayer.current_file_name(),
        );
        let top_is_builtin = call_stack.last().map(|f| f.is_builtin).unwrap_or(false);
        let stopped_on_exception = ctx.replayer.last_exception().is_some();

        let mut frames = Vec::new();
        frames.push(CollectedFrame {
            context_idx: ctx_index,
            depth_from_top: 0,
            name: top_name,
            source: Some(top_source),
            line: line as i64,
            column: column as i64,
            is_builtin: top_is_builtin && !stopped_on_exception,
        });

        let n = call_stack.len();
        for depth in 1..n {
            let frame_idx = n - 1 - depth;
            let frame = &call_stack[frame_idx];
            let child_frame = &call_stack[frame_idx + 1];
            let (source, line, col) = match &child_frame.call_site_loc {
                Some(loc) => (
                    Some(Self::build_source(&ctx.replayer, loc.file_id())),
                    loc.start_line() as i64,
                    loc.start_col() as i64,
                ),
                None => (None, 0, 0),
            };
            frames.push(CollectedFrame {
                context_idx: ctx_index,
                depth_from_top: depth,
                name: Self::format_frame_name(
                    ctx.label.as_ref(),
                    Some(frame),
                    frame.f_name.as_str(),
                ),
                source,
                line,
                column: col,
                is_builtin: frame.is_builtin,
            });
        }

        frames
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
        let (value, var_ref) = if dv.has_children() {
            (dv.dap_value(), self.store_debug_value(dv.clone()))
        } else {
            (dv.dap_value(), 0)
        };
        Variable {
            name,
            value,
            variables_reference: var_ref,
            ..Default::default()
        }
    }

    fn expand_debug_value(&mut self, dv: &RenderedValue) -> Vec<Variable> {
        match dv {
            RenderedValue::Struct { fields, .. } => fields
                .iter()
                .map(|(name, val)| self.debug_value_to_named_variable(name.clone(), val))
                .collect(),
            RenderedValue::Tensor { items } | RenderedValue::ArrayOf { items } => items
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
                self.send_response(req.success(ResponseBody::Launch))?;
            }
            Command::Attach(AttachRequestArguments { .. }) => {
                self.send_response(req.success(ResponseBody::Attach))?;
            }
            Command::ConfigurationDone => {
                self.send_response(req.success(ResponseBody::ConfigurationDone))?;

                let is_end = self.step(StepMode::StepIn);
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }

                self.send_initial_stop()?;
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
                self.send_response(req.success(self.exception_info()?))?;
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

                let is_end = self.step(StepMode::Continue);
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }
            }
            Command::StepIn(_) => {
                self.send_response(req.success(ResponseBody::StepIn))?;

                let is_end = self.step(StepMode::StepIn);
                if is_end {
                    if terminate_at_end {
                        self.send_terminated()?;
                    }
                    return Ok(true);
                }

                self.send_stop_reason(self.stop_reason_for_active_context())?;
            }
            Command::Next(_) => {
                self.send_response(req.success(ResponseBody::Next))?;

                let is_end = self.step(StepMode::StepOver);
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
                self.step(StepMode::ContinueWithoutBreakpoints);
                return Ok(true);
            }
            Command::Terminate(_) => {
                self.send_response(req.success(ResponseBody::Terminate))?;
                self.step(StepMode::ContinueWithoutBreakpoints);
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

    fn set_breakpoints(&mut self, args: &SetBreakpointsArguments) -> ResponseBody {
        let path = args
            .source
            .path
            .clone()
            .or_else(|| args.source.name.clone())
            .unwrap_or_default();
        let path_buf = PathBuf::from(&path);

        let mut breakpoints = Vec::new();
        let mut file_breakpoints = Vec::new();
        for bp in args.breakpoints.as_deref().unwrap_or_default() {
            let id = self.next_breakpoint_id;
            self.next_breakpoint_id += 1;

            file_breakpoints.push(SourceBreakpointInfo { id, line: bp.line });
            breakpoints.push(Breakpoint {
                id: Some(id),
                verified: true,
                line: Some(bp.line),
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

        Ok(ResponseBody::ExceptionInfo(ExceptionInfoResponse {
            exception_id: exc.errno.clone(),
            description: Some(format!("TVM exit code {}", exc.errno)),
            break_mode,
            details: None,
        }))
    }

    fn stack_trace(&mut self) -> ResponseBody {
        self.frame_to_depth.clear();
        self.vars_debug_values.clear();

        let mut collected = Vec::new();
        for (idx, ctx) in self.contexts.iter().enumerate().rev() {
            if let Ok(ctx) = ctx.try_borrow() {
                collected.extend(self.build_replayer_frames(idx, &ctx));
            }
        }

        let mut stack_frames = Vec::new();
        for frame in collected {
            let id = encode_frame_id(frame.context_idx, frame.depth_from_top);
            self.frame_to_depth.insert(
                id,
                FrameLocator {
                    context_idx: frame.context_idx,
                    depth_from_top: frame.depth_from_top,
                },
            );

            stack_frames.push(StackFrame {
                id,
                name: frame.name,
                source: frame.source,
                line: frame.line,
                column: frame.column,
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

impl DebugSession for ReplayerDebugSession {
    fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<()> {
        for req in &self.transport.req_receiver.clone() {
            if self.handle_request(req.clone(), terminate_at_end)? {
                break;
            }
        }
        Ok(())
    }

    fn need_to_stop_child_thread_on_start(&self) -> bool {
        self.performing_step == Some(StepMode::StepIn)
    }

    fn begin_child_context(&mut self, spec: ChildDebugContextSpec) -> anyhow::Result<bool> {
        let Some(tolk_source_map) = spec.tolk_source_map else {
            return Ok(false);
        };
        let Some(marks_dict) = tolk_source_map.marks_dict.as_ref() else {
            return Ok(false);
        };
        let mut replayer = TolkReplayer::new_live_vm(
            tolk_source_map.source_map.clone(),
            marks_dict,
            spec.executor,
        );
        replayer.set_exception_breakpoints(self.exception_mode);

        let label: Arc<str> = spec.name.into();
        self.contexts
            .push(Rc::new(RefCell::new(ReplayerContext::new(label, replayer))));
        let new_idx = self.contexts.len() - 1;
        self.apply_breakpoints_to_context(new_idx);

        if spec.stop_on_entry {
            self.send_stopped(
                StoppedEventReason::Entry,
                Some(self.contexts[new_idx].borrow().label.to_string()),
                None,
            )?;
        }

        Ok(true)
    }

    fn finish_child_context(&mut self, _thread_id: i64) -> anyhow::Result<()> {
        if self.contexts.len() > 1 {
            self.contexts.pop();
        }
        Ok(())
    }

    fn step(&mut self, mode: StepMode) -> bool {
        self.performing_step = Some(mode);
        let is_end = self.step_active_context(mode);

        if !is_end && mode == StepMode::Continue {
            let reason = self.stop_reason_for_active_context();
            if !matches!(reason, StopReason::Step) {
                let _ = self.send_stop_reason(reason);
            }
        }

        is_end
    }

    fn active_context_is_terminated(&self) -> bool {
        self.active_context()
            .and_then(|ctx| ctx.try_borrow().ok().map(|ctx| ctx.replayer.is_finished()))
            .unwrap_or(true)
    }

    fn performing_step(&self) -> Option<StepMode> {
        self.performing_step
    }

    fn advance_parent_after_child_return(&mut self) -> anyhow::Result<()> {
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
    is_builtin: bool,
}

#[derive(Debug, Clone)]
enum StopReason {
    Step,
    Breakpoint(Vec<i64>),
    Exception(ExceptionInfo),
}

const fn encode_frame_id(context_idx: usize, depth_from_top: usize) -> i64 {
    (((context_idx as i64) + 1) << 32) | ((depth_from_top as i64) + 1)
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
