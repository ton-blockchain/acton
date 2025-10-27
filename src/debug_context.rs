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
    Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread, ThreadEventReason,
};
use emulator::tuple::stack::TupleItem;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use tolkc::source_map::{DebugLocation, HighLevelSourceMap, SourceMap};
use tycho_types::models::{OutAction, OwnedRelaxedMessage, RelaxedMsgInfo, StateInit};

pub static VARIABLE_REFERENCE_COUNTER: AtomicU64 = AtomicU64::new(1000);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepMode {
    StepIn,
    StepOver,
    StepOut,
    Continue,
}

pub struct DebugContext {
    pub executors: Vec<AnyExecutor>,
    pub current_executor_id: usize,
    pub source_maps: Vec<SourceMap>,
    pub locations: Vec<DebugLocation>,
    pub pseudo_step: i64,
    pub dap_sender: Sender<DapMessage>,
    pub req_receiver: Receiver<Request>,
    pub tuple_variables: HashMap<i64, TupleItem>,
    pub out_actions_variables: HashMap<i64, Vec<OutAction>>,
    pub out_action_variables: HashMap<i64, OutAction>,
    pub message_variables: HashMap<i64, OwnedRelaxedMessage>,
    pub msg_info_variables: HashMap<i64, RelaxedMsgInfo>,
    pub state_init_variables: HashMap<i64, StateInit>,
}

impl DebugContext {
    pub fn empty() -> DebugContext {
        let (_, req_receiver) = unbounded::<Request>();
        let (dap_sender, _) = unbounded::<DapMessage>();

        DebugContext {
            executors: vec![],
            current_executor_id: 0,
            source_maps: vec![SourceMap {
                debug_marks: HashMap::new(),
                high_level: HighLevelSourceMap {
                    version: "".to_string(),
                    language: None,
                    compiler_version: None,
                    files: vec![],
                    globals: vec![],
                    locations: vec![],
                },
            }],
            locations: vec![],
            pseudo_step: 0,
            dap_sender,
            req_receiver,
            tuple_variables: HashMap::new(),
            out_actions_variables: HashMap::new(),
            out_action_variables: HashMap::new(),
            message_variables: HashMap::new(),
            msg_info_variables: HashMap::new(),
            state_init_variables: HashMap::new(),
        }
    }

    pub fn new(
        executor: AnyExecutor,
        source_map: &SourceMap,
        req_receiver: &Receiver<Request>,
        dap_sender: Sender<DapMessage>,
    ) -> DebugContext {
        DebugContext {
            executors: vec![executor],
            current_executor_id: 0,
            source_maps: vec![(*source_map).clone()],
            locations: vec![],
            pseudo_step: 0,
            dap_sender,
            req_receiver: req_receiver.clone(),
            tuple_variables: HashMap::new(),
            out_actions_variables: HashMap::new(),
            out_action_variables: HashMap::new(),
            message_variables: HashMap::new(),
            msg_info_variables: HashMap::new(),
            state_init_variables: HashMap::new(),
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
    ) -> anyhow::Result<()> {
        self.executors.push(executor);

        self.source_maps
            .push(source_map.unwrap_or(SourceMap::default()));

        self.current_executor_id += 1;
        self.send_event(Event::Thread(ThreadEventBody {
            reason: ThreadEventReason::Started,
            thread_id: id,
        }))?;
        self.send_event(Event::Stopped(StoppedEventBody {
            reason: StoppedEventReason::Entry,
            description: Some(name),
            thread_id: Some(id),
            preserve_focus_hint: None,
            text: None,
            all_threads_stopped: None,
            hit_breakpoint_ids: None,
        }))?;

        Ok(())
    }

    pub fn process_incoming_requests(&mut self, terminate_at_end: bool) -> anyhow::Result<()> {
        for req in self.req_receiver.clone().iter() {
            if let Command::Disconnect(req) = &req.command {
                println!("Disconnecting: {:?}", req);
                break;
            }
            let is_end = self.on_request(req)?;
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
        self.executors.pop().unwrap();
        self.locations = vec![];
        self.pseudo_step = 0;
        self.current_executor_id -= 1;
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
            Command::Initialize(args) => {
                let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
                    ..Default::default()
                }));
                self.send_response(rsp)?;
                self.send_event(Event::Initialized)?;
            }
            Command::Launch(args) => {
                println!("Launching {:?}", args);

                self.step(StepMode::StepIn, true);

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
                if self.locations.is_empty() {
                    let rsp = req.success(ResponseBody::StackTrace(StackTraceResponse {
                        stack_frames: vec![StackFrame {
                            ..Default::default()
                        }],
                        total_frames: None,
                    }));
                    self.send_response(rsp)?;
                    return Ok(false);
                };

                let current_loc = &self.locations[self.pseudo_step as usize];

                let rsp = req.success(ResponseBody::StackTrace(StackTraceResponse {
                    stack_frames: vec![StackFrame {
                        name: "script.tolk".to_string(),
                        line: current_loc.loc.line + 1,
                        column: current_loc.loc.column + 2,
                        source: Some(Source {
                            name: Some("script.tolk".to_string()),
                            path: Some(
                                current_loc
                                    .loc
                                    .file
                                    .to_string()
                                    .replace("_script.tolk", "")
                                    .replace("_test.tolk_test.tolk", "_test.tolk"),
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    total_frames: None,
                }));
                self.send_response(rsp)?;
            }
            Command::Continue(_args) => {
                let rsp = req.success(ResponseBody::Continue(ContinueResponse {
                    all_threads_continued: Some(true),
                }));
                self.send_response(rsp)?;

                let is_end = self.step(StepMode::Continue, false);
                if is_end {
                    return Ok(true);
                }
            }
            Command::StepIn(_args) => {
                let rsp = req.success(ResponseBody::StepIn);
                self.send_response(rsp)?;

                let is_end = self.step(StepMode::StepIn, false);
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

                let is_end = self.step(StepMode::StepOver, false);
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

                let is_end = self.step(StepMode::StepOut, false);
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
            Command::SetExceptionBreakpoints(_) => {}
            Command::Disconnect(_) => {} // do nothing, should be handled in the request loop
            _ => {
                eprintln!("Unhandled command: {:?}", req.command);
                return Err(anyhow!("Unhandled command: {:?}", req.command));
            }
        }

        Ok(false)
    }

    pub(crate) fn step(&mut self, mode: StepMode, stop_on_first: bool) -> bool {
        match mode {
            StepMode::StepIn => self.step_in_impl(stop_on_first),
            StepMode::StepOver => self.step_over_impl(),
            StepMode::StepOut => self.step_out_impl(),
            StepMode::Continue => self.continue_impl(),
        }
    }

    fn step_in_impl(&mut self, stop_on_first: bool) -> bool {
        if stop_on_first && !self.locations.is_empty() {
            self.pseudo_step = 0;
            return false;
        }

        if self.has_next_synthetic_step() {
            self.pseudo_step += 1;
            return false;
        }

        self.perform_real_step_until_mapped()
    }

    fn step_over_impl(&mut self) -> bool {
        let current_loc = self.get_current_location();
        let current_line = current_loc.as_ref().map(|loc| loc.loc.line);

        if !self.has_next_synthetic_step() {
            let is_end = self.perform_real_step_until_mapped();
            if is_end {
                return true;
            }

            if let Some(line) = current_line {
                self.skip_same_line_steps(line);
            }
            return false;
        }

        let next_loc = &self.locations[(self.pseudo_step + 1) as usize];

        if next_loc.context.event == Some("EnterInlinedFunction".to_string()) {
            return self.skip_inlined_function();
        }

        if next_loc.context.event == Some("EnterFunction".to_string()) {
            return self.skip_function();
        }

        self.pseudo_step += 1;

        if let Some(current_line) = current_line {
            let new_loc = self.get_current_location();
            if let Some(new_loc) = new_loc {
                if new_loc.loc.line == current_line {
                    return self.step_over_impl();
                }
            }
        }

        false
    }

    fn step_out_impl(&mut self) -> bool {
        let current_loc = match self.get_current_location() {
            Some(loc) => loc.clone(),
            None => return self.continue_impl(),
        };

        let current_function = current_loc.context.containing_function.clone();
        let current_line = current_loc.loc.line;

        loop {
            if self.has_next_synthetic_step() {
                let next_loc = &self.locations[(self.pseudo_step + 1) as usize];

                if next_loc.context.event == Some("LeaveFunction".to_string())
                    && next_loc.context.containing_function == current_function
                {
                    self.pseudo_step += 1;
                    self.skip_same_line_steps(current_line);

                    if self.has_next_synthetic_step() {
                        self.pseudo_step += 1;
                        return false;
                    } else {
                        let is_end = self.perform_real_step_until_mapped();
                        if is_end {
                            return true;
                        }
                        self.skip_same_line_steps(current_line);
                        return false;
                    }
                }

                self.pseudo_step += 1;
            } else {
                let is_end = self.perform_real_step_until_mapped();
                if is_end {
                    return true;
                }

                if let Some(new_loc) = self.get_current_location() {
                    if new_loc.context.containing_function != current_function {
                        self.skip_same_line_steps(current_line);
                        return false;
                    }
                }
            }
        }
    }

    fn continue_impl(&mut self) -> bool {
        loop {
            let executor = self.executors[self.current_executor_id].clone();
            let is_end = executor.step();
            if is_end {
                return true;
            }
        }
    }

    fn has_next_synthetic_step(&self) -> bool {
        (self.pseudo_step + 1) < self.locations.len() as i64
    }

    fn get_current_location(&self) -> Option<&DebugLocation> {
        if self.pseudo_step >= 0 && (self.pseudo_step as usize) < self.locations.len() {
            Some(&self.locations[self.pseudo_step as usize])
        } else {
            None
        }
    }

    fn skip_same_line_steps(&mut self, original_line: i64) {
        while let Some(loc) = self.get_current_location() {
            if loc.loc.line != original_line {
                break;
            }

            if !self.has_next_synthetic_step() {
                break;
            }

            self.pseudo_step += 1;
        }
    }

    fn perform_real_step_until_mapped(&mut self) -> bool {
        loop {
            let executor = self.executors[self.current_executor_id].clone();
            let is_end = executor.step();
            if is_end {
                return true;
            }

            let source_map = &self.source_maps[self.current_executor_id];
            let locations = get_locations(&executor, source_map);

            if let Some(locations) = locations {
                self.locations = locations;
                self.pseudo_step = 0;
                return false;
            }
        }
    }

    fn skip_inlined_function(&mut self) -> bool {
        let current_line = self.get_current_location().map(|loc| loc.loc.line);
        let mut depth = 0;

        loop {
            if !self.has_next_synthetic_step() {
                let is_end = self.perform_real_step_until_mapped();
                if is_end {
                    return true;
                }
                if let Some(line) = current_line {
                    self.skip_same_line_steps(line);
                }
                continue;
            }

            let next_loc = &self.locations[(self.pseudo_step + 1) as usize];

            if next_loc.context.event == Some("EnterInlinedFunction".to_string()) {
                depth += 1;
            } else if next_loc.context.event == Some("LeaveInlinedFunction".to_string()) {
                if depth == 0 {
                    self.pseudo_step += 1;
                    if let Some(line) = current_line {
                        self.skip_same_line_steps(line);
                    }

                    if self.has_next_synthetic_step() {
                        self.pseudo_step += 1;
                    } else {
                        let is_end = self.perform_real_step_until_mapped();
                        if is_end {
                            return true;
                        }
                        if let Some(line) = current_line {
                            self.skip_same_line_steps(line);
                        }
                    }
                    return false;
                }
                depth -= 1;
            }

            self.pseudo_step += 1;
        }
    }

    fn skip_function(&mut self) -> bool {
        let current_line = self.get_current_location().map(|loc| loc.loc.line);
        let function_name = self.locations[(self.pseudo_step + 1) as usize]
            .context
            .containing_function
            .clone();
        let mut depth = 0;

        loop {
            if !self.has_next_synthetic_step() {
                let is_end = self.perform_real_step_until_mapped();
                if is_end {
                    return true;
                }
                if let Some(line) = current_line {
                    self.skip_same_line_steps(line);
                }
                continue;
            }

            let next_loc = &self.locations[(self.pseudo_step + 1) as usize];

            if next_loc.context.event == Some("EnterFunction".to_string())
                && next_loc.context.containing_function == function_name
            {
                depth += 1;
            } else if next_loc.context.event == Some("LeaveFunction".to_string())
                && next_loc.context.containing_function == function_name
            {
                if depth == 0 {
                    self.pseudo_step += 1;
                    if let Some(line) = current_line {
                        self.skip_same_line_steps(line);
                    }

                    if self.has_next_synthetic_step() {
                        self.pseudo_step += 1;
                    } else {
                        let is_end = self.perform_real_step_until_mapped();
                        if is_end {
                            return true;
                        }
                        if let Some(line) = current_line {
                            self.skip_same_line_steps(line);
                        }
                    }
                    return false;
                }
                depth -= 1;
            }

            self.pseudo_step += 1;
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
