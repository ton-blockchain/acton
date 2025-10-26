use crate::context::AnyExecutor;
use anyhow::anyhow;
use crossbeam_channel::{Receiver, Sender, unbounded};
use dap::events::{Event, StoppedEventBody, ThreadEventBody};
use dap::prelude::{Command, Request, Response, ResponseBody};
use dap::responses::{
    ContinueResponse, ScopesResponse, StackTraceResponse, ThreadsResponse, VariablesResponse,
};
use dap::types;
use dap::types::{
    Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread,
    ThreadEventReason, Variable,
};
use emulator::tuple::stack::{TupleItem, parse_tuple};
use std::collections::HashMap;
use tolkc::source_map::{DebugLocation, HighLevelSourceMap, SourceMap};
use tonlib_core::cell::ArcCell;
use tonlib_core::tlb_types::tlb::TLB;

pub struct DebugContext {
    pub executors: Vec<AnyExecutor>,
    pub current_executor_id: usize,
    pub source_maps: Vec<SourceMap>,
    pub locations: Vec<DebugLocation>,
    pub pseudo_step: i64,
    pub response_sender: Sender<Response>,
    pub event_sender: Sender<Event>,
    pub req_receiver: Receiver<Request>,
}

impl DebugContext {
    pub fn empty() -> DebugContext {
        let (_, req_receiver) = unbounded::<Request>();
        let (response_sender, _) = unbounded::<Response>();
        let (event_sender, _) = unbounded::<Event>();

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
            response_sender,
            event_sender,
            req_receiver,
        }
    }

    pub fn new(
        executor: AnyExecutor,
        source_map: &SourceMap,
        req_receiver: &Receiver<Request>,
        response_sender: Sender<Response>,
        event_sender: Sender<Event>,
    ) -> DebugContext {
        DebugContext {
            executors: vec![executor],
            current_executor_id: 0,
            source_maps: vec![(*source_map).clone()],
            locations: vec![],
            pseudo_step: 0,
            response_sender,
            event_sender,
            req_receiver: req_receiver.clone(),
        }
    }

    pub fn begin_thread(
        &mut self,
        id: i64,
        executor: AnyExecutor,
        source_map: Option<SourceMap>,
        name: String,
    ) {
        self.executors.push(executor);

        self.source_maps
            .push(source_map.unwrap_or(SourceMap::default()));

        self.current_executor_id += 1;
        self.event_sender
            .send(Event::Thread(ThreadEventBody {
                reason: ThreadEventReason::Started,
                thread_id: id,
            }))
            .unwrap();
        self.event_sender
            .send(Event::Stopped(StoppedEventBody {
                reason: StoppedEventReason::Entry,
                description: Some(name),
                thread_id: Some(id),
                preserve_focus_hint: None,
                text: None,
                all_threads_stopped: None,
                hit_breakpoint_ids: None,
            }))
            .unwrap();
    }

    pub fn process_incoming_requests(&mut self) -> anyhow::Result<()> {
        for req in self.req_receiver.clone().iter() {
            if let Command::Disconnect(req) = &req.command {
                println!("Disconnecting: {:?}", req);
                break;
            }
            let is_end = self.on_request(req)?;
            if is_end {
                self.event_sender.send(Event::Terminated(None))?;
                break;
            }
        }

        Ok(())
    }

    pub fn finish_thread(&mut self, id: i64) {
        self.executors.pop().unwrap();
        self.locations = vec![];
        self.pseudo_step = 0;
        self.current_executor_id -= 1;
        self.event_sender
            .send(Event::Thread(ThreadEventBody {
                reason: ThreadEventReason::Exited,
                thread_id: id,
            }))
            .unwrap();
    }

    pub(crate) fn on_request(&mut self, req: Request) -> anyhow::Result<bool> {
        match &req.command {
            Command::Initialize(args) => {
                let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
                    ..Default::default()
                }));
                self.response_sender.send(rsp)?;
                self.event_sender.send(Event::Initialized)?;
            }
            Command::Launch(args) => {
                println!("Launching {:?}", args);

                self.event_sender.send(Event::Stopped(StoppedEventBody {
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
                self.response_sender.send(rsp)?;
            }
            Command::Scopes(_args) => {
                let rsp = req.success(ResponseBody::Scopes(ScopesResponse {
                    scopes: vec![Scope {
                        name: "Variables".to_string(),
                        variables_reference: 1,
                        expensive: false,
                        presentation_hint: Some(ScopePresentationhint::Locals),
                        ..Default::default()
                    }],
                }));
                self.response_sender.send(rsp)?;
            }
            Command::Variables(_args) => {
                let current_loc = &self.locations[self.pseudo_step as usize];

                let executor = &self.executors[self.current_executor_id];

                let stack = executor.get_stack();
                let stack = parse_tuple(&ArcCell::from_boc_b64(&stack)?)?;

                let variables = current_loc
                    .variables
                    .iter()
                    .enumerate()
                    .map(|(index, variable)| Variable {
                        name: variable.name.clone(),
                        value: format!("{}", stack.get(index).unwrap_or(&TupleItem::Null)),
                        ..Default::default()
                    })
                    .collect::<Vec<_>>();

                let rsp = req.success(ResponseBody::Variables(VariablesResponse { variables }));
                self.response_sender.send(rsp)?;
            }
            Command::StackTrace(_args) => {
                if self.locations.is_empty() {
                    let rsp = req.success(ResponseBody::StackTrace(StackTraceResponse {
                        stack_frames: vec![StackFrame {
                            ..Default::default()
                        }],
                        total_frames: None,
                    }));
                    self.response_sender.send(rsp)?;
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
                                current_loc.loc.file.to_string().replace("_script.tolk", ""),
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    total_frames: None,
                }));
                self.response_sender.send(rsp)?;
            }
            Command::Continue(_args) => {
                let rsp = req.success(ResponseBody::Continue(ContinueResponse {
                    all_threads_continued: Some(true),
                }));
                self.response_sender.send(rsp)?;

                self.continue_execution_while(|_| true)?;
            }
            Command::StepIn(_args) => {
                let rsp = req.success(ResponseBody::StepIn);
                self.response_sender.send(rsp)?;

                let is_end = self.next(true);
                if is_end {
                    return Ok(true);
                }

                self.event_sender.send(Event::Stopped(StoppedEventBody {
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
                self.response_sender.send(rsp)?;

                let is_end = self.next(false);
                if is_end {
                    return Ok(true);
                }

                self.event_sender.send(Event::Stopped(StoppedEventBody {
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

    pub(crate) fn next(&mut self, step_in: bool) -> bool {
        let executor = &self.executors[self.current_executor_id].clone();

        if self.pseudo_step + 1 >= self.locations.len() as i64 {
            loop {
                let is_end = executor.step();
                if is_end {
                    return true;
                }

                let source_map = &self.source_maps[self.current_executor_id];
                let locations = get_locations(executor, &source_map);
                if let Some(locations) = locations {
                    // Locations are like pseudo steps
                    self.locations = locations;
                    self.pseudo_step = -1;
                    // Step until reach some Tolk code
                    break;
                }
            }
        }

        if self.pseudo_step + 1 < self.locations.len() as i64 {
            let step = self.locations[(self.pseudo_step + 1) as usize].clone();

            if step.context.event == Some("EnterInlinedFunction".to_string()) {
                let is_end = self
                    .continue_execution_while(|loc| {
                        step_in || loc.context.event == Some("LeaveInlinedFunction".to_string())
                    })
                    .unwrap();
                if is_end {
                    return true;
                }
            }

            if step.context.containing_function != "foo"
                && step.context.containing_function != "processMessage"
                && step.context.event == Some("EnterFunction".to_string())
            {
                let is_end = self
                    .continue_execution_while(|loc| {
                        step_in
                            || (loc.context.event == Some("LeaveFunction".to_string())
                                && step.context.containing_function
                                    == loc.context.containing_function)
                    })
                    .unwrap();
                if is_end {
                    return true;
                }
            }

            // If there are more pseudo steps, select the next one
            self.pseudo_step += 1;
        }

        if self.pseudo_step >= self.locations.len() as i64 {
            loop {
                let is_end = executor.step();
                if is_end {
                    return true;
                }

                let source_map = &self.source_maps[self.current_executor_id];
                let locations = get_locations(executor, &source_map);
                if let Some(locations) = locations {
                    // Locations are like pseudo steps
                    self.locations = locations;
                    self.pseudo_step = 0;
                    // Step until reach some Tolk code
                    break;
                }
            }
        }

        false
    }

    fn continue_execution_while<Cond: Fn(&DebugLocation) -> bool>(
        &mut self,
        condition: Cond,
    ) -> anyhow::Result<bool> {
        loop {
            if self.pseudo_step + 1 < self.locations.len() as i64 {
                let step = &self.locations[(self.pseudo_step + 1) as usize];

                let condition_is_met = condition(step);

                // If there are more pseudo steps, select the next one
                self.pseudo_step += 1;

                if condition_is_met {
                    return Ok(false);
                }
            } else {
                // Otherwise perform a real step
                loop {
                    let executor = self.executors[self.current_executor_id].clone();
                    let is_end = executor.step();
                    if is_end {
                        return Ok(true);
                    }

                    let source_map = &self.source_maps[self.current_executor_id];
                    let locations = get_locations(&executor, &source_map);
                    if let Some(locations) = locations {
                        // Locations are like pseudo steps
                        self.locations = locations.clone();
                        self.pseudo_step = 0;
                        // Step until reach some Tolk code
                        break;
                    }
                }
            }
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
