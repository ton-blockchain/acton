use crate::debugger::any_executor::AnyExecutor;
use crate::debugger::dap::{DapMessage, DapTransport};
use crate::formatter::FormatterContext;
use crate::vmtrace::SkipBlocksMode;
use anyhow::anyhow;
use dap::events::{Event, StoppedEventBody, ThreadEventBody};
use dap::prelude::{Command, Request, Response, ResponseBody};
use dap::responses::{
    ContinueResponse, EvaluateResponse, ScopesResponse, StackTraceResponse, ThreadsResponse,
    VariablesResponse,
};
use dap::types;
use dap::types::{
    Breakpoint, Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread,
    ThreadEventReason,
};
use log::debug;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use ton_source_map::{BytecodeLocation, DebugLocation, EntryContextDescription, SourceMap};
use tvmffi::stack::TupleItem;
use tycho_types::models::{OutAction, OwnedRelaxedMessage, RelaxedMsgInfo, StateInit};

pub static VARIABLE_REFERENCE_COUNTER: AtomicU64 = AtomicU64::new(1000);

#[derive(Debug, Clone)]
pub struct SourceBreakpoint {
    pub line: i64,
    pub column: Option<i64>,
    pub verified: bool,
}

#[derive(Debug, Clone)]
pub struct BreakpointInfo {
    pub id: i64,
    pub source_path: PathBuf,
    pub breakpoint: SourceBreakpoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    StepIn,
    StepOver,
    StepOut,
    Continue,
    ContinueWithoutBreakpoints, // used for disconnect and terminate
}

#[derive(Debug, Clone)]
pub enum StepKind {
    UnmappedAdvance,
    Mapped,
    SyntheticEnterFunction(String),
    SyntheticAfterFunctionCall(String),
    SyntheticEnterInlined(String),
    SyntheticLeaveInlined(String),
}

#[derive(Debug, Clone)]
pub struct DebugStep {
    pub kind: StepKind,
    pub loc: Option<DebugLocation>,
    pub pos: BytecodeLocation,
    pub thread_id: i64,
}

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub function_name: String,
    pub loc: DebugLocation,
    pub pos: BytecodeLocation,
}

pub struct Stepper {
    pub executors: Vec<AnyExecutor>,
    pub source_maps: Vec<SourceMap>,
    pub current_executor_id: usize,
    pub buffers: Vec<VecDeque<DebugStep>>,
    buffer: VecDeque<DebugStep>,
    last_breakpoint_line: i64,
    last_breakpoint_lines: Vec<i64>,
    current_step: Option<DebugStep>,
    terminated: bool,
    thread_id: i64,
    callstacks: Vec<Vec<CallFrame>>,
    callstack: Vec<CallFrame>,
    root_function_name: String,
    root_frame_added: bool,
}

impl Stepper {
    pub fn new(
        executor: AnyExecutor,
        source_map: SourceMap,
        thread_id: i64,
        root_function_name: String,
    ) -> Self {
        Stepper {
            executors: vec![executor],
            source_maps: vec![source_map],
            current_executor_id: 0,
            buffers: Vec::new(),
            buffer: VecDeque::new(),
            current_step: None,
            last_breakpoint_line: 0,
            last_breakpoint_lines: Vec::new(),
            terminated: false,
            thread_id,
            callstacks: Vec::new(),
            callstack: Vec::new(),
            root_function_name,
            root_frame_added: false,
        }
    }

    pub fn push_executor(&mut self, executor: AnyExecutor, source_map: SourceMap) {
        self.executors.push(executor);
        self.source_maps.push(source_map);
        self.buffers.push(self.buffer.clone());
        self.callstacks.push(self.callstack.clone());
        self.last_breakpoint_lines.push(self.last_breakpoint_line);
        self.current_executor_id += 1;
        self.buffer = VecDeque::new();
        self.callstack = Vec::new();
        self.root_frame_added = false;
    }

    pub fn pop_executor(&mut self) {
        self.executors.pop();
        self.source_maps.pop();
        self.buffer = self.buffers.pop().unwrap_or_default();
        self.callstack = self.callstacks.pop().unwrap_or_default();
        self.last_breakpoint_line = self.last_breakpoint_lines.pop().unwrap_or(0);
        if self.current_executor_id > 0 {
            self.current_executor_id -= 1;
        }
        self.terminated = false
    }

    pub fn next_step(&mut self) -> Option<DebugStep> {
        let step = self.next_impl();
        self.current_step = step.clone();
        if let Some(step) = &step {
            self.update_callstack_for_step(step);
        }
        step
    }

    pub fn next_impl(&mut self) -> Option<DebugStep> {
        if let Some(step) = self.buffer.pop_front() {
            return Some(step);
        }
        self.refill_from_vm()
    }

    fn refill_from_vm(&mut self) -> Option<DebugStep> {
        if self.terminated {
            return None;
        }

        let executor = self.executors[self.current_executor_id].clone();
        let is_end = executor.step();
        if is_end {
            self.terminated = true;
        }

        let source_map = &self.source_maps[self.current_executor_id];
        let (hash, offset) = get_code_pos(&executor)?;
        let pos = BytecodeLocation { offset, hash };

        if let Some(locs) = get_locations(&executor, source_map) {
            for loc in locs {
                let function_name = loc
                    .clone()
                    .context
                    .event_function
                    .unwrap_or(loc.context.containing_function.to_string())
                    .clone();

                let step = match loc.context.event.as_deref() {
                    Some("EnterFunction") => DebugStep {
                        kind: StepKind::SyntheticEnterFunction(function_name),
                        loc: Some(loc),
                        thread_id: self.thread_id,
                        pos: pos.clone(),
                    },
                    Some("AfterFunctionCall") => DebugStep {
                        kind: StepKind::SyntheticAfterFunctionCall(
                            loc.clone()
                                .context
                                .event_function
                                .unwrap_or(loc.context.containing_function.to_string())
                                .clone(),
                        ),
                        loc: Some(loc),
                        thread_id: self.thread_id,
                        pos: pos.clone(),
                    },
                    Some("EnterInlinedFunction") => DebugStep {
                        kind: StepKind::SyntheticEnterInlined(function_name),
                        loc: Some(loc),
                        thread_id: self.thread_id,
                        pos: pos.clone(),
                    },
                    Some("LeaveInlinedFunction") => DebugStep {
                        kind: StepKind::SyntheticLeaveInlined(function_name),
                        loc: Some(loc),
                        thread_id: self.thread_id,
                        pos: pos.clone(),
                    },
                    _ => DebugStep {
                        kind: StepKind::Mapped,
                        loc: Some(loc),
                        thread_id: self.thread_id,
                        pos: pos.clone(),
                    },
                };
                self.buffer.push_back(step);
            }
            self.buffer.pop_front()
        } else {
            Some(DebugStep {
                kind: StepKind::UnmappedAdvance,
                loc: None,
                thread_id: self.thread_id,
                pos: pos.clone(),
            })
        }
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    pub fn get_current_step(&self) -> Option<&DebugStep> {
        self.current_step.as_ref()
    }

    pub fn get_current_step_line(&self) -> Option<i64> {
        self.get_current_step()
            .as_ref()
            .and_then(|s| s.loc.as_ref())
            .map(|loc| loc.loc.line)
    }

    pub fn get_callstack(&self) -> &Vec<CallFrame> {
        &self.callstack
    }

    fn update_callstack_for_step(&mut self, step: &DebugStep) {
        match &step.kind {
            StepKind::SyntheticEnterFunction(func_name) => {
                if let Some(loc) = &step.loc {
                    self.callstack.push(CallFrame {
                        function_name: func_name.clone(),
                        loc: loc.clone(),
                        pos: step.pos.clone(),
                    });
                }
            }
            StepKind::SyntheticAfterFunctionCall(func_name) => {
                if let Some(last) = self.callstack.last()
                    && last.function_name == *func_name
                {
                    self.callstack.pop();
                }
            }
            StepKind::SyntheticEnterInlined(func_name) => {
                if let Some(loc) = &step.loc {
                    self.callstack.push(CallFrame {
                        function_name: func_name.clone(),
                        loc: loc.clone(),
                        pos: step.pos.clone(),
                    });
                }
            }
            StepKind::SyntheticLeaveInlined(func_name) => {
                if let Some(last) = self.callstack.last()
                    && last.function_name == *func_name
                {
                    self.callstack.pop();
                }
            }
            _ => {}
        }
    }
}

#[derive(Default)]
pub struct DebugVariables {
    pub state_init: HashMap<i64, StateInit>,
    pub msg_info: HashMap<i64, RelaxedMsgInfo>,
    pub message: HashMap<i64, OwnedRelaxedMessage>,
    pub out_action: HashMap<i64, OutAction>,
    pub out_actions: HashMap<i64, Vec<OutAction>>,
    pub tuple: HashMap<i64, TupleItem>,
}

impl DebugVariables {
    pub fn clear(&mut self) {
        self.state_init.clear();
        self.msg_info.clear();
        self.message.clear();
        self.out_action.clear();
        self.out_actions.clear();
        self.tuple.clear();
    }
}

pub struct DebugContext {
    pub stepper: Stepper,
    pub transport: DapTransport,
    pub variables: DebugVariables,
    pub performing_step: Option<StepMode>,
    pub breakpoints: HashMap<PathBuf, Vec<BreakpointInfo>>,
    pub next_breakpoint_id: i64,
    pub formatter_context: FormatterContext,
    pub test_name: String,
}

impl DebugContext {
    pub fn new(
        transport: DapTransport,
        executor: AnyExecutor,
        source_map: &SourceMap,
        test_name: String,
    ) -> DebugContext {
        let stepper = Stepper::new(executor, source_map.clone(), 1, test_name.clone());
        DebugContext {
            stepper,
            transport,
            variables: DebugVariables::default(),
            performing_step: None,
            breakpoints: HashMap::new(),
            next_breakpoint_id: 1,
            formatter_context: FormatterContext::empty(),
            test_name,
        }
    }

    pub fn send_response(&self, response: Response) -> anyhow::Result<()> {
        debug!(
            "Sending DAP response: request_seq={}, success={}",
            response.request_seq, response.success
        );
        self.transport
            .dap_sender
            .send(DapMessage::Response(response))?;
        Ok(())
    }

    pub fn send_event(&self, event: Event) -> anyhow::Result<()> {
        debug!("Sending DAP event: {event:?}");
        self.transport.dap_sender.send(DapMessage::Event(event))?;
        Ok(())
    }

    pub fn begin_thread(
        &mut self,
        id: i64,
        executor: AnyExecutor,
        source_map: Option<SourceMap>,
        name: String,
        stop_on_entry: bool,
    ) -> anyhow::Result<()> {
        let sm = source_map.unwrap_or_default();
        let root_name = self.get_root_function_name(id);

        self.stepper.push_executor(executor, sm);
        self.stepper.thread_id = id;
        self.stepper.root_function_name = root_name;

        self.send_event(Event::Thread(ThreadEventBody {
            reason: ThreadEventReason::Started,
            thread_id: id,
        }))?;

        if stop_on_entry {
            self.send_event(Event::Stopped(StoppedEventBody {
                reason: StoppedEventReason::Entry,
                description: Some(name),
                thread_id: Some(id),
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: None,
                hit_breakpoint_ids: None,
            }))?;
        }

        Ok(())
    }

    pub fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<()> {
        for req in self.transport.req_receiver.clone().iter() {
            if let Command::Disconnect(args) = &req.command {
                println!("Disconnecting");
                debug!("Disconnecting: {args:?}");
                let rsp = req.success(ResponseBody::Disconnect);
                self.send_response(rsp)?;
                self.step(StepMode::ContinueWithoutBreakpoints);
                break;
            }
            if let Command::Terminate(args) = &req.command {
                println!("Terminating");
                debug!("Terminating: {args:?}");
                let rsp = req.success(ResponseBody::Terminate);
                self.send_response(rsp)?;
                self.step(StepMode::ContinueWithoutBreakpoints);
                break;
            }
            let is_end = self.on_request(req.clone())?;
            if is_end {
                if terminate_at_end {
                    self.send_event(Event::Terminated(None))?;
                }
                break;
            }
        }

        Ok(())
    }

    pub fn finish_thread(&mut self, id: i64) -> anyhow::Result<()> {
        self.stepper.pop_executor();
        self.stepper.thread_id = 1;

        self.variables.clear();
        self.send_event(Event::Thread(ThreadEventBody {
            reason: ThreadEventReason::Exited,
            thread_id: id,
        }))?;
        Ok(())
    }

    pub fn on_request(&mut self, req: Request) -> anyhow::Result<bool> {
        debug!("DAP on_request: {:?}", req.command);
        match &req.command {
            Command::Initialize(args) => {
                let client_name = args.client_name.clone();
                let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
                    supports_configuration_done_request: Some(true),
                    supports_breakpoint_locations_request: Some(false),
                    ..Default::default()
                }));
                self.send_response(rsp)?;
                self.send_event(Event::Initialized)?;

                println!("Client: {}", client_name.unwrap_or("Unknown".to_string()));
            }
            Command::Launch(_args) => {
                let rsp = req.success(ResponseBody::Launch);
                self.send_response(rsp)?;
            }
            Command::Threads => {
                let rsp = req.success(ResponseBody::Threads(ThreadsResponse {
                    threads: vec![
                        Thread {
                            id: 1,
                            name: "main thread".to_string(),
                        },
                        Thread {
                            id: 2,
                            name: "send/get thread".to_string(),
                        },
                    ],
                }));
                self.send_response(rsp)?;
            }
            Command::Scopes(_args) => {
                let rsp = req.success(ResponseBody::Scopes(ScopesResponse {
                    scopes: vec![
                        Scope {
                            name: "Variables".to_string(),
                            variables_reference: 1,
                            expensive: false,
                            presentation_hint: Some(ScopePresentationhint::Locals),
                            ..Default::default()
                        },
                        Scope {
                            name: "Registers".to_string(),
                            variables_reference: 2,
                            expensive: false,
                            presentation_hint: Some(ScopePresentationhint::Registers),
                            ..Default::default()
                        },
                        Scope {
                            name: "Stack".to_string(),
                            variables_reference: 3,
                            expensive: false,
                            presentation_hint: Some(ScopePresentationhint::Locals),
                            ..Default::default()
                        },
                    ],
                }));
                self.send_response(rsp)?;
            }
            Command::Variables(args) => {
                let variables = self.process_variables(&args)?;
                let rsp = req.success(ResponseBody::Variables(VariablesResponse { variables }));
                self.send_response(rsp)?;
            }
            Command::StackTrace(args) => {
                let stack_frames = self.build_stack_frames(args.thread_id);

                let rsp = req.success(ResponseBody::StackTrace(StackTraceResponse {
                    stack_frames,
                    total_frames: None,
                }));
                self.send_response(rsp)?;
            }
            Command::Continue(_args) => {
                let rsp = req.success(ResponseBody::Continue(ContinueResponse {
                    all_threads_continued: Some(true),
                }));
                self.send_response(rsp)?;

                let is_end = self.step(StepMode::Continue);
                if is_end {
                    return Ok(true);
                }
            }
            Command::StepIn(_args) => {
                let rsp = req.success(ResponseBody::StepIn);
                self.send_response(rsp)?;

                let is_end = self.step(StepMode::StepIn);
                if is_end {
                    return Ok(true);
                }

                self.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Step,
                    thread_id: Some(self.current_thread_id()),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))?;
            }
            Command::Next(_args) => {
                let rsp = req.success(ResponseBody::Next);
                self.send_response(rsp)?;

                let is_end = self.step(StepMode::StepOver);
                if is_end {
                    return Ok(true);
                }

                self.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Step,
                    thread_id: Some(self.current_thread_id()),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))?;
            }
            Command::StepOut(_args) => {
                let rsp = req.success(ResponseBody::StepOut);
                self.send_response(rsp)?;

                let is_end = self.step(StepMode::StepOut);
                if is_end {
                    return Ok(true);
                }

                self.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Step,
                    thread_id: Some(self.current_thread_id()),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))?;
            }
            Command::SetBreakpoints(args) => {
                let source_path = args
                    .source
                    .path
                    .as_ref()
                    .map(PathBuf::from)
                    .unwrap_or_default();

                let mut breakpoints = Vec::new();

                if let Some(source_breakpoints) = &args.breakpoints {
                    self.breakpoints.remove(&source_path);
                    let mut file_breakpoints = Vec::new();

                    for bp in source_breakpoints {
                        let bp_id = self.next_breakpoint_id;
                        self.next_breakpoint_id += 1;

                        let breakpoint_info = BreakpointInfo {
                            id: bp_id,
                            source_path: source_path.clone(),
                            breakpoint: SourceBreakpoint {
                                line: bp.line,
                                column: bp.column,
                                verified: true,
                            },
                        };

                        file_breakpoints.push(breakpoint_info.clone());

                        breakpoints.push(Breakpoint {
                            id: Some(bp_id),
                            verified: true,
                            line: Some(bp.line),
                            column: bp.column,
                            ..Default::default()
                        });
                    }

                    if !file_breakpoints.is_empty() {
                        self.breakpoints.insert(source_path, file_breakpoints);
                    }
                }

                let rsp = req.success(ResponseBody::SetBreakpoints(
                    dap::responses::SetBreakpointsResponse { breakpoints },
                ));
                self.send_response(rsp)?;
            }
            Command::ConfigurationDone => {
                let rsp = req.success(ResponseBody::ConfigurationDone);
                self.send_response(rsp)?;

                self.step(StepMode::StepIn);

                self.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Step,
                    thread_id: Some(1),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))?;
            }
            Command::SetExceptionBreakpoints(_) => {}
            Command::Disconnect(_) => {} // do nothing, should be handled in the request loop
            Command::Evaluate(args) => {
                let expression = args.expression.clone();
                let rsp = req.success(ResponseBody::Evaluate(EvaluateResponse {
                    result: expression,
                    type_field: None,
                    presentation_hint: None,
                    variables_reference: 0,
                    named_variables: None,
                    indexed_variables: None,
                    memory_reference: None,
                }));
                self.send_response(rsp)?;
            }
            Command::Attach(_) => {
                let rsp = req.success(ResponseBody::Attach);
                self.send_response(rsp)?;
            }
            _ => {
                eprintln!("Unhandled command: {:?}", req.command);
                return Err(anyhow!("Unhandled command: {:?}", req.command));
            }
        }

        Ok(false)
    }

    fn current_thread_id(&self) -> i64 {
        self.stepper.thread_id
    }

    fn normalize_path(file: &String) -> String {
        file.to_string()
            .replace(".test.tolk.test.tolk", ".test.tolk")
    }

    fn get_root_function_name(&self, thread_id: i64) -> String {
        if thread_id == 1 {
            self.test_name.clone()
        } else {
            "onInternalMessage".to_string()
        }
    }

    fn create_stack_frame(
        &self,
        loc: &DebugLocation,
        function_name: String,
        pos: &BytecodeLocation,
    ) -> StackFrame {
        let file_path = Self::normalize_path(&loc.loc.file.to_string());
        let file_name = std::path::Path::new(&file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.tolk")
            .to_string();

        let (line, column) = if let EntryContextDescription::Basic { ast_kind } =
            &loc.context.description
            && ast_kind == "ast_block_statement"
        {
            // For blocks set position to closing bracket
            // TODO: maybe we want to setup it this way for any position with line != end_line
            (loc.loc.end_line + 1, loc.loc.end_column + 2)
        } else {
            (loc.loc.line + 1, loc.loc.column + 2)
        };

        let end_line = if loc.loc.end_line == 0 {
            None
        } else {
            Some(loc.loc.end_line + 1)
        };

        let end_column = if loc.loc.end_column == 0 {
            None
        } else {
            Some(loc.loc.end_column + 2)
        };

        StackFrame {
            name: function_name,
            line,
            column,
            end_line,
            end_column,
            source: Some(Source {
                name: Some(file_name),
                path: Some(file_path),
                ..Default::default()
            }),
            instruction_pointer_reference: Some(format!("{}:{}", pos.hash, pos.offset)),
            ..Default::default()
        }
    }

    fn build_stack_frames(&self, thread_id: i64) -> Vec<StackFrame> {
        let stepper = &self.stepper;

        let callstack = if thread_id == 1 {
            stepper
                .callstacks
                .first()
                .unwrap_or(stepper.get_callstack())
        } else if stepper.thread_id > 1 {
            stepper.get_callstack()
        } else {
            &Vec::new()
        };

        let step = stepper.get_current_step();

        let Some(step) = step else {
            return Vec::new();
        };
        let Some(loc) = &step.loc else {
            return Vec::new();
        };

        let top_frame_name = if let Some(last) = callstack.last() {
            last.function_name.clone()
        } else {
            self.get_root_function_name(thread_id)
        };
        let top_frame = self.create_stack_frame(loc, top_frame_name, &step.pos);

        let final_callstack = if thread_id == stepper.thread_id {
            vec![top_frame]
        } else {
            vec![]
        };

        let remaining_callstack = callstack
            .iter()
            .enumerate()
            .rev()
            .map(|(idx, frame)| {
                let function_name = if idx > 0
                    && let Some(prev) = callstack.get(idx - 1)
                {
                    prev.function_name.clone()
                } else {
                    self.get_root_function_name(thread_id)
                };

                self.create_stack_frame(&frame.loc, function_name, &frame.pos)
            })
            .collect::<Vec<_>>();

        final_callstack
            .into_iter()
            .chain(remaining_callstack)
            .collect()
    }

    pub fn need_to_stop_child_thread_on_start(&self) -> bool {
        self.performing_step == Some(StepMode::StepIn)
    }

    fn check_breakpoint(&mut self, step: &DebugStep) -> Option<i64> {
        let loc = step.loc.as_ref()?;
        let normalized_path = PathBuf::from(Self::normalize_path(&loc.loc.file));

        let file_breakpoints = self.breakpoints.get(&normalized_path)?;

        for bp_info in file_breakpoints {
            if bp_info.breakpoint.line == loc.loc.line + 1
                && bp_info.breakpoint.line != self.stepper.last_breakpoint_line
            {
                self.stepper.last_breakpoint_line = bp_info.breakpoint.line;
                return Some(bp_info.id);
            }
        }

        None
    }

    pub fn step(&mut self, mode: StepMode) -> bool {
        match mode {
            StepMode::StepIn => self.step_in_impl(),
            StepMode::StepOver => self.step_over_impl(),
            StepMode::StepOut => self.step_out_impl(),
            StepMode::Continue => self.continue_impl(),
            StepMode::ContinueWithoutBreakpoints => self.stop_impl(),
        }
    }

    fn step_in_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::StepIn);

        loop {
            let step = match self.stepper.next_step() {
                Some(s) => s,
                None => return true,
            };

            match step.kind {
                StepKind::UnmappedAdvance => continue,
                _ => {
                    return false;
                }
            }
        }
    }

    fn step_over_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::StepOver);

        let stepper = &mut self.stepper;

        // Step over performs a step as follows:
        // 1. If the current position describes a (inlined) function call, then this call
        //    is skipped entirely, including nested calls, and execution stops at the instruction
        //    immediately after the function finishes execution.
        // 2. If the current position describes a regular instruction, execution continues
        //    until a new position with a line number different from the line number before
        //    the step is reached.

        let Some(current_step) = stepper.current_step.clone() else {
            debug!("cannot execute step over since current step is None");
            return false;
        };
        let current_line = current_step.loc.as_ref().map(|loc| loc.loc.line);

        // First step, we skip function if current step describes a function call
        let (skipped, is_end) = match &current_step.kind {
            StepKind::SyntheticEnterInlined(func) => (true, skip_inlined_function(stepper, func)),
            StepKind::SyntheticEnterFunction(func) => (true, skip_function(stepper, func)),
            _ => (false, false),
        };

        if is_end {
            // For now, if execution is completed, return true immediately
            return true;
        }

        // After skipping function (for example `__null()`) current line can be the same before stepping
        // since step over should change the line, execute another step
        if current_line != stepper.get_current_step_line() {
            // fast path, line changed, end step over
            return false;
        }

        if skipped {
            // If we skipped some call, don't execute any other steps for now
            return false;
        }

        let mut current_line = stepper.get_current_step_line();

        loop {
            let step = match stepper.next_step() {
                Some(s) => s,
                None => return true,
            };

            match &step.kind {
                StepKind::UnmappedAdvance => continue,
                StepKind::SyntheticEnterInlined(_) | StepKind::SyntheticEnterFunction(_) => {
                    if skipped {
                        // Call is already skipped, but the next step is another call, we don't
                        // want to skip it, since this way we can actually step in, which can be unexpected
                        return false;
                    }

                    // But if call is not skipped, we don't want to skip it as well :)
                    return false;
                }
                StepKind::Mapped => {
                    let Some(loc) = &step.loc else {
                        // step.loc is None only for Unmapped
                        return false;
                    };

                    let Some(current_line) = current_line else {
                        // unexpected None, so return for now
                        return false;
                    };

                    if loc.loc.line != current_line {
                        // found step with different line
                        return false;
                    }
                }
                _ => {}
            }

            // new step still doesn't satisfy condition, so setup current line again
            current_line = stepper.get_current_step_line()
        }
    }

    fn step_out_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::StepIn);

        let stepper = &mut self.stepper;

        let current_function = match &stepper.get_current_step() {
            Some(step) => match &step.loc {
                Some(loc) => loc.context.containing_function.clone(),
                None => return self.continue_impl(),
            },
            None => return self.continue_impl(),
        };

        loop {
            let step = match stepper.next_step() {
                Some(s) => s,
                None => return true,
            };

            match &step.kind {
                StepKind::UnmappedAdvance => continue,
                StepKind::SyntheticAfterFunctionCall(func) if func == &current_function => {
                    stepper.buffer.push_front(step.clone());
                    return false;
                }
                StepKind::SyntheticLeaveInlined(func) if func == &current_function => {
                    stepper.buffer.push_front(step.clone());
                    return false;
                }
                _ => {}
            }
        }
    }

    fn continue_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::Continue);

        loop {
            let step = match self.stepper.next_step() {
                Some(s) => s,
                None => return true,
            };

            if let Some(bp_id) = self.check_breakpoint(&step) {
                if let Err(e) = self.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Breakpoint,
                    thread_id: Some(step.thread_id),
                    description: Some("Breakpoint hit".to_string()),
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: Some(true),
                    hit_breakpoint_ids: Some(vec![bp_id]),
                })) {
                    eprintln!("Failed to send breakpoint event: {e:?}");
                }

                return false;
            }
        }
    }

    fn stop_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::ContinueWithoutBreakpoints);

        loop {
            match self.stepper.next_step() {
                Some(_) => continue,
                None => return true,
            };
        }
    }
}

fn skip_inlined_function(stepper: &mut Stepper, func_name: &String) -> bool {
    let mut depth = 1;

    loop {
        let step = match stepper.next_step() {
            Some(s) => s,
            None => return true,
        };

        match &step.kind {
            StepKind::SyntheticEnterInlined(f) if f == func_name => {
                depth += 1;
            }
            StepKind::SyntheticLeaveInlined(f) if f == func_name => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            StepKind::UnmappedAdvance => {}
            _ => {
                // let loc = step.loc.unwrap();
                // println!(
                //     "skipping {}, event: {:?} {:?}",
                //     loc.loc.format(),
                //     loc.context.event,
                //     loc.context.event_function
                // );
            }
        }
    }
}

fn skip_function(stepper: &mut Stepper, func_name: &String) -> bool {
    let mut depth = 1;

    loop {
        let step = match stepper.next_step() {
            Some(s) => s,
            None => return true,
        };

        match &step.kind {
            StepKind::SyntheticEnterFunction(f) if f == func_name => {
                depth += 1;
            }
            StepKind::SyntheticAfterFunctionCall(f) if f == func_name => {
                depth -= 1;
                if depth == 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
}

fn get_locations(executor: &AnyExecutor, source_map: &SourceMap) -> Option<Vec<DebugLocation>> {
    let (hash, offset) = get_code_pos(executor)?;
    crate::vmtrace::low_level_loc_to_debug_locations(
        source_map,
        hash.as_str(),
        offset,
        SkipBlocksMode::None,
        false,
    )
}

fn get_code_pos(executor: &AnyExecutor) -> Option<(String, u16)> {
    let pos = executor.get_code_pos();
    let (hash, offset) = pos.split_once(":")?;
    let offset = offset.parse::<u16>().ok()?;
    Some((hash.to_string(), offset))
}
