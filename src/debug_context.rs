use crate::context::AnyExecutor;
use crate::dap::DapMessage;
use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, unbounded};
use dap::events::{Event, StoppedEventBody, ThreadEventBody};
use dap::prelude::{Command, Request, Response, ResponseBody};
use dap::responses::{
    ContinueResponse, ScopesResponse, StackTraceResponse, ThreadsResponse, VariablesResponse,
};
use dap::types;
use dap::types::{
    Breakpoint, Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread,
    ThreadEventReason,
};
use emulator::tuple::stack::TupleItem;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use tolkc::source_map::{DebugLocation, SourceMap};
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
    pub thread_id: i64,
}

pub struct Stepper {
    pub executors: Vec<AnyExecutor>,
    pub source_maps: Vec<SourceMap>,
    pub current_executor_id: usize,
    pub buffers: Vec<VecDeque<DebugStep>>,
    buffer: VecDeque<DebugStep>,
    terminated: bool,
    thread_id: i64,
}

impl Stepper {
    pub fn new(executor: AnyExecutor, source_map: SourceMap, thread_id: i64) -> Self {
        Stepper {
            executors: vec![executor],
            source_maps: vec![source_map],
            current_executor_id: 0,
            buffers: Vec::new(),
            buffer: VecDeque::new(),
            terminated: false,
            thread_id,
        }
    }

    pub fn push_executor(&mut self, executor: AnyExecutor, source_map: SourceMap) {
        self.executors.push(executor);
        self.source_maps.push(source_map);
        self.buffers.push(self.buffer.clone());
        self.current_executor_id += 1;
        self.buffer = VecDeque::new();
    }

    pub fn pop_executor(&mut self) {
        self.executors.pop();
        self.source_maps.pop();
        self.buffer = self.buffers.pop().unwrap_or(VecDeque::new());
        if self.current_executor_id > 0 {
            self.current_executor_id -= 1;
        }
        self.terminated = false
    }

    pub fn next(&mut self) -> Option<DebugStep> {
        if let Some(step) = self.buffer.pop_front() {
            return Some(step);
        }
        self.refill_from_vm()
    }

    fn refill_from_vm(&mut self) -> Option<DebugStep> {
        if self.terminated {
            return None;
        }

        loop {
            let executor = self.executors[self.current_executor_id].clone();
            let is_end = executor.step();
            if is_end {
                self.terminated = true;
                return None;
            }

            let source_map = &self.source_maps[self.current_executor_id];
            if let Some(locs) = get_locations(&executor, source_map) {
                for loc in locs {
                    let function_name = loc
                        .clone()
                        .context
                        .event_function
                        .unwrap_or(loc.context.containing_function.to_string())
                        .clone();

                    if let Some(event) = &loc.context.event
                        && let Some(name) = loc.clone().context.event_function
                    {
                        println!("{}: {}", event, name);
                    }

                    let step = match loc.context.event.as_deref() {
                        Some("EnterFunction") => DebugStep {
                            kind: StepKind::SyntheticEnterFunction(function_name),
                            loc: Some(loc),
                            thread_id: self.thread_id,
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
                        },
                        Some("EnterInlinedFunction") => DebugStep {
                            kind: StepKind::SyntheticEnterInlined(function_name),
                            loc: Some(loc),
                            thread_id: self.thread_id,
                        },
                        Some("LeaveInlinedFunction") => DebugStep {
                            kind: StepKind::SyntheticLeaveInlined(function_name),
                            loc: Some(loc),
                            thread_id: self.thread_id,
                        },
                        _ => DebugStep {
                            kind: StepKind::Mapped,
                            loc: Some(loc),
                            thread_id: self.thread_id,
                        },
                    };
                    self.buffer.push_back(step);
                }
                return self.buffer.pop_front();
            } else {
                return Some(DebugStep {
                    kind: StepKind::UnmappedAdvance,
                    loc: None,
                    thread_id: self.thread_id,
                });
            }
        }
    }

    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    pub fn get_current_step(&self) -> Option<&DebugStep> {
        self.buffer.front()
    }
}

pub struct DebugContext {
    pub stepper: Option<Stepper>,
    pub last_step: Option<DebugStep>,
    pub dap_sender: Sender<DapMessage>,
    pub req_receiver: Receiver<Request>,
    pub tuple_variables: HashMap<i64, TupleItem>,
    pub out_actions_variables: HashMap<i64, Vec<OutAction>>,
    pub out_action_variables: HashMap<i64, OutAction>,
    pub message_variables: HashMap<i64, OwnedRelaxedMessage>,
    pub msg_info_variables: HashMap<i64, RelaxedMsgInfo>,
    pub state_init_variables: HashMap<i64, StateInit>,
    pub performing_step: Option<StepMode>,
    pub breakpoints: HashMap<PathBuf, Vec<BreakpointInfo>>,
    pub next_breakpoint_id: i64,
}

impl DebugContext {
    pub fn empty() -> DebugContext {
        let (_, req_receiver) = unbounded::<Request>();
        let (dap_sender, _) = unbounded::<DapMessage>();

        DebugContext {
            stepper: None,
            last_step: None,
            dap_sender,
            req_receiver,
            tuple_variables: HashMap::new(),
            out_actions_variables: HashMap::new(),
            out_action_variables: HashMap::new(),
            message_variables: HashMap::new(),
            msg_info_variables: HashMap::new(),
            state_init_variables: HashMap::new(),
            performing_step: None,
            breakpoints: HashMap::new(),
            next_breakpoint_id: 1,
        }
    }

    pub fn new(
        executor: AnyExecutor,
        source_map: &SourceMap,
        req_receiver: &Receiver<Request>,
        dap_sender: Sender<DapMessage>,
    ) -> DebugContext {
        let stepper = Stepper::new(executor, source_map.clone(), 1);
        DebugContext {
            stepper: Some(stepper),
            last_step: None,
            dap_sender,
            req_receiver: req_receiver.clone(),
            tuple_variables: HashMap::new(),
            out_actions_variables: HashMap::new(),
            out_action_variables: HashMap::new(),
            message_variables: HashMap::new(),
            msg_info_variables: HashMap::new(),
            state_init_variables: HashMap::new(),
            performing_step: None,
            breakpoints: HashMap::new(),
            next_breakpoint_id: 1,
        }
    }

    pub fn send_response(&self, response: Response) -> anyhow::Result<()> {
        self.dap_sender.send(DapMessage::Response(response))?;
        Ok(())
    }

    pub fn send_event(&self, event: Event) -> anyhow::Result<()> {
        self.dap_sender.send(DapMessage::Event(event))?;
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
        let sm = source_map.unwrap_or(SourceMap::default());

        if let Some(stepper) = &mut self.stepper {
            stepper.push_executor(executor, sm);
        } else {
            self.stepper = Some(Stepper::new(executor, sm, id));
        }

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
        for req in self.req_receiver.clone().iter() {
            if let Command::Disconnect(req) = &req.command {
                println!("Disconnecting: {:?}", req);
                break;
            }
            println!("Processing request: {:?}", req);
            let is_end = self.on_request(req.clone())?;
            if is_end {
                if terminate_at_end {
                    self.send_event(Event::Terminated(None))?;
                }
                println!("Processing request: {:?}", req);
                println!("break");
                break;
            }
        }

        Ok(())
    }

    pub fn finish_thread(&mut self, id: i64) -> anyhow::Result<()> {
        if let Some(stepper) = &mut self.stepper {
            stepper.pop_executor();
        }

        self.last_step = None;
        self.tuple_variables.clear();
        self.out_actions_variables.clear();
        self.out_action_variables.clear();
        self.message_variables.clear();
        self.msg_info_variables.clear();
        self.state_init_variables.clear();
        self.send_event(Event::Thread(ThreadEventBody {
            reason: ThreadEventReason::Exited,
            thread_id: id,
        }))?;
        Ok(())
    }

    pub(crate) fn on_request(&mut self, req: Request) -> anyhow::Result<bool> {
        match &req.command {
            Command::Initialize(_args) => {
                let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
                    supports_configuration_done_request: Some(true),
                    supports_breakpoint_locations_request: Some(false),
                    ..Default::default()
                }));
                self.send_response(rsp)?;
                self.send_event(Event::Initialized)?;
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
                            name: "get method".to_string(),
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
            Command::StackTrace(_args) => {
                let stack_frame = if let Some(step) = &self.last_step {
                    if let Some(loc) = &step.loc {
                        StackFrame {
                            name: "script.tolk".to_string(),
                            line: loc.loc.line + 1,
                            column: loc.loc.column + 2,
                            source: Some(Source {
                                name: Some("script.tolk".to_string()),
                                path: Some(
                                    loc.loc
                                        .file
                                        .to_string()
                                        .replace("_script.tolk", "")
                                        .replace("_test.tolk_test.tolk", "_test.tolk"),
                                ),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }
                    } else {
                        StackFrame::default()
                    }
                } else {
                    StackFrame::default()
                };

                let rsp = req.success(ResponseBody::StackTrace(StackTraceResponse {
                    stack_frames: vec![stack_frame],
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
                    thread_id: Some(1),
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
                    thread_id: Some(1),
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
                    thread_id: Some(1),
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
            Command::Evaluate(_) => {}
            _ => {
                eprintln!("Unhandled command: {:?}", req.command);
                return Err(anyhow!("Unhandled command: {:?}", req.command));
            }
        }

        Ok(false)
    }

    pub(crate) fn need_to_stop_child_thread_on_start(&self) -> bool {
        self.performing_step == Some(StepMode::StepIn)
    }

    fn check_breakpoint(&self, step: &DebugStep) -> Option<i64> {
        let loc = step.loc.as_ref()?;
        let file_path = PathBuf::from(&loc.loc.file);

        let normalized_path = if file_path.to_string_lossy().ends_with("_script.tolk") {
            PathBuf::from(file_path.to_string_lossy().replace("_script.tolk", ""))
        } else if file_path
            .to_string_lossy()
            .ends_with("_test.tolk_test.tolk")
        {
            PathBuf::from(
                file_path
                    .to_string_lossy()
                    .replace("_test.tolk_test.tolk", "_test.tolk"),
            )
        } else {
            file_path
        };

        let file_breakpoints = self.breakpoints.get(&normalized_path)?;

        for bp_info in file_breakpoints {
            if bp_info.breakpoint.line == loc.loc.line + 1 {
                return Some(bp_info.id);
            }
        }

        None
    }

    pub(crate) fn step(&mut self, mode: StepMode) -> bool {
        match mode {
            StepMode::StepIn => self.step_in_impl(),
            StepMode::StepOver => self.step_over_impl(),
            StepMode::StepOut => self.step_out_impl(),
            StepMode::Continue => self.continue_impl(),
        }
    }

    fn step_in_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::StepIn);

        let stepper = match &mut self.stepper {
            Some(s) => s,
            None => return true,
        };

        loop {
            let step = match stepper.next() {
                Some(s) => s,
                None => return true,
            };

            match step.kind {
                StepKind::UnmappedAdvance => continue,
                _ => {
                    self.last_step = Some(step);
                    return false;
                }
            }
        }
    }

    fn step_over_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::StepOver);

        let current_line = self
            .last_step
            .as_ref()
            .and_then(|s| s.loc.as_ref())
            .map(|loc| loc.loc.line);

        let stepper = match &mut self.stepper {
            Some(s) => s,
            None => return true,
        };

        loop {
            let step = match stepper.next() {
                Some(s) => s,
                None => return true,
            };

            match &step.kind {
                StepKind::UnmappedAdvance => continue,
                StepKind::SyntheticEnterInlined(func) => {
                    match skip_inlined_function_new(stepper, func.clone(), current_line) {
                        Some(next_step) => {
                            self.last_step = Some(next_step);
                            return false;
                        }
                        None => return true,
                    }
                }
                StepKind::SyntheticEnterFunction(func) => {
                    match skip_function_new(stepper, func.clone(), current_line) {
                        Some(next_step) => {
                            self.last_step = Some(next_step);
                            return false;
                        }
                        None => return true,
                    }
                }
                StepKind::Mapped => {
                    if let Some(curr_line) = current_line
                        && let Some(loc) = &step.loc
                    {
                        if loc.loc.line != curr_line {
                            self.last_step = Some(step);
                            return false;
                        }
                    } else {
                        self.last_step = Some(step);
                        return false;
                    }
                }
                _ => {}
            }
        }
    }

    fn step_out_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::StepIn);

        let (current_function, current_line) = match &self.last_step {
            Some(step) => match &step.loc {
                Some(loc) => (loc.context.containing_function.clone(), loc.loc.line),
                None => return self.continue_impl(),
            },
            None => return self.continue_impl(),
        };

        let stepper = match &mut self.stepper {
            Some(s) => s,
            None => return true,
        };

        loop {
            let step = match stepper.next() {
                Some(s) => s,
                None => return true,
            };

            match &step.kind {
                StepKind::UnmappedAdvance => continue,
                StepKind::SyntheticAfterFunctionCall(func) if func == &current_function => loop {
                    let next_step = match stepper.next() {
                        Some(s) => s,
                        None => return true,
                    };

                    match next_step.kind {
                        StepKind::UnmappedAdvance => continue,
                        StepKind::Mapped => {
                            if let Some(loc) = &next_step.loc {
                                if loc.loc.line != current_line {
                                    self.last_step = Some(next_step);
                                    return false;
                                }
                            }
                        }
                        _ => {
                            self.last_step = Some(next_step);
                            return false;
                        }
                    }
                },
                _ => {}
            }
        }
    }

    fn continue_impl(&mut self) -> bool {
        self.performing_step = Some(StepMode::Continue);

        loop {
            let step = {
                let stepper = match &mut self.stepper {
                    Some(s) => s,
                    None => return true,
                };

                match stepper.next() {
                    Some(s) => s,
                    None => return true,
                }
            };

            if let Some(bp_id) = self.check_breakpoint(&step) {
                self.last_step = Some(step.clone());

                if let Err(e) = self.send_event(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Breakpoint,
                    thread_id: Some(step.thread_id),
                    description: Some("Breakpoint hit".to_string()),
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: Some(true),
                    hit_breakpoint_ids: Some(vec![bp_id]),
                })) {
                    eprintln!("Failed to send breakpoint event: {:?}", e);
                }

                return false;
            }
        }
    }
}

fn skip_inlined_function_new(
    stepper: &mut Stepper,
    func_name: String,
    _current_line: Option<i64>,
) -> Option<DebugStep> {
    let mut depth = 1;

    loop {
        let step = match stepper.next() {
            Some(s) => s,
            None => return None,
        };

        match &step.kind {
            StepKind::SyntheticEnterInlined(f) if f == &func_name => {
                depth += 1;
            }
            StepKind::SyntheticLeaveInlined(f) if f == &func_name => {
                depth -= 1;
                if depth == 0 {
                    return Some(step);
                }
            }
            _ => {}
        }
    }
}

fn skip_function_new(
    stepper: &mut Stepper,
    func_name: String,
    _current_line: Option<i64>,
) -> Option<DebugStep> {
    let mut depth = 1;

    loop {
        let step = match stepper.next() {
            Some(s) => s,
            None => return None,
        };

        match &step.kind {
            StepKind::SyntheticEnterFunction(f) if f == &func_name => {
                depth += 1;
            }
            StepKind::SyntheticAfterFunctionCall(f) if f == &func_name => {
                depth -= 1;
                if depth == 0 {
                    return Some(step);
                    // loop {
                    //     let next_step = match stepper.next() {
                    //         Some(s) => s,
                    //         None => return None,
                    //     };
                    //
                    //     match next_step.kind {
                    //         StepKind::UnmappedAdvance => continue,
                    //         StepKind::Mapped => {
                    //             if let Some(curr_line) = current_line {
                    //                 if let Some(loc) = &next_step.loc {
                    //                     if loc.loc.line != curr_line {
                    //                         return Some(next_step);
                    //                     }
                    //                 }
                    //             } else {
                    //                 return Some(next_step);
                    //             }
                    //         }
                    //         _ => return Some(next_step),
                    //     }
                    // }
                }
            }
            _ => {}
        }
    }
}

fn get_locations(executor: &AnyExecutor, source_map: &SourceMap) -> Option<Vec<DebugLocation>> {
    let pos = executor.get_code_pos();
    let (hash, offset) = pos.split_once(":").unwrap();
    let offset = offset.parse::<i32>().unwrap();

    let Some(marks) = source_map.debug_marks.get(hash) else {
        return None;
    };

    let debug_pairs = marks
        .iter()
        .filter(|(mark_offset, _)| return *mark_offset == offset)
        .collect::<Vec<_>>();

    let locs = source_map
        .high_level
        .locations
        .iter()
        .filter(|loc| {
            debug_pairs
                .iter()
                .find(|(_, debug_id)| (*debug_id) as i64 == loc.idx)
                .is_some()
        })
        .filter(|loc| !loc.loc.file.is_empty() && !loc.loc.file.starts_with("@stdlib/"))
        .map(|loc| (*loc).clone())
        .collect::<Vec<_>>();

    if locs.is_empty() {
        return None;
    }

    Some(locs)
}
