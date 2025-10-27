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
    Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread,
    ThreadEventReason, Variable,
};
use emulator::executor::StoreExt;
use emulator::tuple::stack::{TupleItem, parse_tuple, parse_tuple_item};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tolkc::source_map::{DebugLocation, HighLevelSourceMap, SourceMap};
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;
use tycho_types::models::{
    CurrencyCollection, IntAddr, OutAction, OutActionsRevIter, OwnedRelaxedMessage, RelaxedMsgInfo,
    ReserveCurrencyFlags, SendMsgFlags, StateInit,
};
use tycho_types::num::Tokens;

static VARIABLE_REFERENCE_COUNTER: AtomicU64 = AtomicU64::new(1000);

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

                self.next(true, true);

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
                let current_loc = &self.locations[self.pseudo_step as usize];

                let executor = &self.executors[self.current_executor_id];

                let variables = if args.variables_reference == 1 {
                    let stack = executor.get_stack();
                    let stack = parse_tuple(&ArcCell::from_boc_b64(&stack)?)?;

                    current_loc
                        .variables
                        .iter()
                        .rev()
                        .enumerate()
                        .map(|(index, variable)| {
                            let value = stack
                                .get(stack.len() - 1 - index)
                                .unwrap_or(&TupleItem::Null);
                            let value2 = TupleItem::TypedTuple {
                                contract_abi: Default::default(),
                                abi: None,
                                items: vec![value.clone()],
                                type_name: variable.var_type.clone(),
                                accounts: HashMap::new(),
                                build_cache: Default::default(),
                                known_addresses: Default::default(),
                            };
                            Variable {
                                name: variable.name.clone(),
                                type_field: Some(variable.var_type.clone()),
                                value: format!("{}", value2),
                                ..Default::default()
                            }
                        })
                        .collect::<Vec<_>>()
                } else if args.variables_reference == 2 {
                    let mut variables = Vec::new();

                    // c4 register (storage)
                    if let Ok(out_actions) = self.get_storage(executor) {
                        variables.push(Variable {
                            name: "c4 (storage)".to_string(),
                            type_field: Some("storage".to_string()),
                            value: out_actions.to_boc_hex(false).unwrap(),
                            ..Default::default()
                        });
                    }

                    // c5 register (out actions)
                    if let Ok(out_actions) = self.get_out_actions(executor) {
                        let c5_ref =
                            VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                        self.out_actions_variables
                            .insert(c5_ref, out_actions.clone());

                        variables.push(Variable {
                            name: "c5 (output actions)".to_string(),
                            type_field: Some("out_actions".to_string()),
                            value: format!("{} out actions", out_actions.len()),
                            variables_reference: c5_ref,
                            ..Default::default()
                        });
                    }

                    // c7 register
                    let c7 = executor.get_c7();
                    let c7_cell = &ArcCell::from_boc_b64(&c7)?;
                    let mut c7_slice = c7_cell.parser();
                    let c7_tuple = parse_tuple_item(&mut c7_slice)?;
                    let c7_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                    self.tuple_variables.insert(c7_ref, c7_tuple.clone());

                    variables.push(Variable {
                        name: "c7 (temporary data)".to_string(),
                        type_field: Some("tuple".to_string()),
                        value: format!("{}", c7_tuple),
                        variables_reference: c7_ref,
                        ..Default::default()
                    });

                    variables
                } else if args.variables_reference == 3 {
                    let stack_boc = executor.get_stack();
                    let stack_cell = ArcCell::from_boc_b64(&stack_boc)?;
                    let stack_tuple = parse_tuple(&stack_cell)?;

                    stack_tuple
                        .iter()
                        .rev()
                        .enumerate()
                        .map(|(index, item)| Variable {
                            name: format!("s{}", index),
                            type_field: Some("stack_item".to_string()),
                            value: format!("{}", item),
                            ..Default::default()
                        })
                        .collect::<Vec<_>>()
                } else if args.variables_reference > 3 {
                    if let Some(tuple_item) = self.tuple_variables.get(&args.variables_reference) {
                        self.build_tuple_children(&tuple_item.clone())
                    } else if let Some(out_actions) =
                        self.out_actions_variables.get(&args.variables_reference)
                    {
                        self.build_out_actions_children(&out_actions.clone())
                    } else if let Some(out_action) =
                        self.out_action_variables.get(&args.variables_reference)
                    {
                        self.build_out_action_children(&out_action.clone())
                    } else if let Some(message) =
                        self.message_variables.get(&args.variables_reference)
                    {
                        self.build_message_children(&message.clone())
                    } else if let Some(msg_info) =
                        self.msg_info_variables.get(&args.variables_reference)
                    {
                        self.build_msg_info_children(msg_info)
                    } else if let Some(state_init) =
                        self.state_init_variables.get(&args.variables_reference)
                    {
                        self.build_state_init_children(state_init)
                    } else {
                        vec![]
                    }
                } else {
                    vec![]
                };

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

                self.continue_execution_while(|_| true)?;
            }
            Command::StepIn(_args) => {
                let rsp = req.success(ResponseBody::StepIn);
                self.send_response(rsp)?;

                let is_end = self.next(true, true);
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

                let is_end = self.next(false, false);
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

    pub(crate) fn next(&mut self, step_in: bool, stop_on_first: bool) -> bool {
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

                    if stop_on_first {
                        self.pseudo_step = 0;
                        return false;
                    }

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

    fn build_tuple_children(&mut self, tuple_item: &TupleItem) -> Vec<Variable> {
        match tuple_item {
            TupleItem::Tuple(items) => items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let item_ref = if Self::has_children(item) {
                        let ref_id =
                            VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                        self.tuple_variables.insert(ref_id, item.clone());
                        ref_id
                    } else {
                        0
                    };
                    Variable {
                        name: format!("[{}]", index),
                        type_field: Some(Self::get_item_type(item)),
                        value: format!("{}", item),
                        variables_reference: item_ref,
                        ..Default::default()
                    }
                })
                .collect(),
            TupleItem::TypedTuple {
                items, type_name, ..
            } => items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let item_ref = if Self::has_children(item) {
                        let ref_id =
                            VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                        self.tuple_variables.insert(ref_id, item.clone());
                        ref_id
                    } else {
                        0
                    };
                    Variable {
                        name: format!("[{}]", index),
                        type_field: Some(Self::get_item_type(item)),
                        value: format!("{}", item),
                        variables_reference: item_ref,
                        ..Default::default()
                    }
                })
                .collect(),
            _ => vec![],
        }
    }

    fn has_children(item: &TupleItem) -> bool {
        matches!(item, TupleItem::Tuple(_) | TupleItem::TypedTuple { .. })
    }

    fn get_item_type(item: &TupleItem) -> String {
        match item {
            TupleItem::Null => "null".to_string(),
            TupleItem::Int(_) => "int".to_string(),
            TupleItem::Nan => "nan".to_string(),
            TupleItem::Cell(_) => "cell".to_string(),
            TupleItem::Slice(_) => "slice".to_string(),
            TupleItem::Builder(_) => "builder".to_string(),
            TupleItem::Tuple(_) => "tuple".to_string(),
            TupleItem::TypedTuple { type_name, .. } => type_name.clone(),
        }
    }

    fn get_storage(&self, executor: &AnyExecutor) -> anyhow::Result<ArcCell> {
        let c4 = executor.get_control_register(4);
        let c4_cell = &ArcCell::from_boc_b64(&c4)?;
        let mut c4_slice = c4_cell.parser();

        if let TupleItem::Cell(c4_tuple) = parse_tuple_item(&mut c4_slice)? {
            Ok(c4_tuple)
        } else {
            Ok(ArcCell::default())
        }
    }

    fn get_out_actions(&self, executor: &AnyExecutor) -> anyhow::Result<Vec<OutAction>> {
        let c5 = executor.get_control_register(5);
        let c5_cell = &ArcCell::from_boc_b64(&c5)?;
        let mut c5_slice = c5_cell.parser();

        if let TupleItem::Cell(c5_tuple) = parse_tuple_item(&mut c5_slice)? {
            let c5_cell = &Boc::decode_base64(&c5_tuple.to_boc_b64(false).unwrap())?;
            let c5_slice = c5_cell.as_slice().unwrap();

            let out_actions = OutActionsRevIter::new(c5_slice)
                .filter_map(|action| action.ok())
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .cloned()
                .collect();

            Ok(out_actions)
        } else {
            Ok(vec![])
        }
    }

    fn build_out_actions_children(&mut self, out_actions: &[OutAction]) -> Vec<Variable> {
        out_actions
            .iter()
            .enumerate()
            .map(|(index, action)| {
                let action_type = match action {
                    OutAction::SendMsg { .. } => "SendMsg",
                    OutAction::SetCode { .. } => "SetCode",
                    OutAction::ReserveCurrency { .. } => "ReserveCurrency",
                    OutAction::ChangeLibrary { .. } => "ChangeLibrary",
                };

                let action_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                self.out_action_variables.insert(action_ref, action.clone());

                let value = match action {
                    OutAction::SendMsg { mode, out_msg } => {
                        if let Ok(message) = out_msg.load() {
                            format!(
                                "{} and {}",
                                Self::format_relaxed_msg_info(&message.info),
                                Self::format_send_msg_flags(*mode)
                            )
                        } else {
                            Self::format_send_msg_flags(*mode)
                        }
                    }
                    OutAction::ReserveCurrency { mode, value } => {
                        format!(
                            "{} with {}",
                            Self::format_currency_collection(value),
                            Self::format_reserve_currency_flags(*mode)
                        )
                    }
                    _ => format!("{:?}", action),
                };

                Variable {
                    name: format!("[{}] {}", index, action_type),
                    type_field: Some(action_type.to_string()),
                    value,
                    variables_reference: action_ref,
                    ..Default::default()
                }
            })
            .collect()
    }

    fn build_out_action_children(&mut self, out_action: &OutAction) -> Vec<Variable> {
        match out_action {
            OutAction::SendMsg { mode, out_msg } => {
                let mut variables = vec![Variable {
                    name: "mode".to_string(),
                    type_field: Some("SendMsgFlags".to_string()),
                    value: Self::format_send_msg_flags(*mode),
                    ..Default::default()
                }];

                let message_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
                if let Ok(message) = out_msg.load() {
                    self.message_variables.insert(message_ref, message);
                    variables.push(Variable {
                        name: "out_msg".to_string(),
                        type_field: Some("OwnedRelaxedMessage".to_string()),
                        value: "RelaxedMessage".to_string(),
                        variables_reference: message_ref,
                        ..Default::default()
                    });
                } else {
                    variables.push(Variable {
                        name: "out_msg".to_string(),
                        type_field: Some("Lazy<OwnedRelaxedMessage>".to_string()),
                        value: format!("{:?}", out_msg),
                        ..Default::default()
                    });
                }

                variables.push(Variable {
                    name: "out_msg_raw".to_string(),
                    type_field: Some("cell".to_string()),
                    value: Boc::encode_hex(out_msg.inner()),
                    ..Default::default()
                });

                variables
            }
            OutAction::SetCode { new_code } => vec![Variable {
                name: "new_code".to_string(),
                type_field: Some("Cell".to_string()),
                value: format!("{:?}", new_code),
                ..Default::default()
            }],
            OutAction::ReserveCurrency { mode, value } => vec![
                Variable {
                    name: "mode".to_string(),
                    type_field: Some("ReserveCurrencyFlags".to_string()),
                    value: Self::format_reserve_currency_flags(*mode),
                    ..Default::default()
                },
                Variable {
                    name: "value".to_string(),
                    type_field: Some("CurrencyCollection".to_string()),
                    value: Self::format_currency_collection(value),
                    ..Default::default()
                },
            ],
            OutAction::ChangeLibrary { mode, lib } => vec![
                Variable {
                    name: "mode".to_string(),
                    type_field: Some("ChangeLibraryMode".to_string()),
                    value: format!("{:?}", mode),
                    ..Default::default()
                },
                Variable {
                    name: "lib".to_string(),
                    type_field: Some("LibRef".to_string()),
                    value: format!("{:?}", lib),
                    ..Default::default()
                },
            ],
        }
    }

    fn format_send_msg_flags(flags: SendMsgFlags) -> String {
        let mut flag_names = Vec::new();

        if flags.contains(SendMsgFlags::PAY_FEE_SEPARATELY) {
            flag_names.push("PAY_FEES_SEPARATELY");
        }
        if flags.contains(SendMsgFlags::IGNORE_ERROR) {
            flag_names.push("IGNORE_ERRORS");
        }
        if flags.contains(SendMsgFlags::BOUNCE_ON_ERROR) {
            flag_names.push("BOUNCE_ON_ACTION_FAIL");
        }
        if flags.contains(SendMsgFlags::DELETE_IF_EMPTY) {
            flag_names.push("DESTROY");
        }
        if flags.contains(SendMsgFlags::WITH_REMAINING_BALANCE) {
            flag_names.push("CARRY_ALL_REMAINING_MESSAGE_VALUE");
        }
        if flags.contains(SendMsgFlags::ALL_BALANCE) {
            flag_names.push("CARRY_ALL_BALANCE");
        }

        if flag_names.is_empty() {
            "REGULAR".to_string()
        } else {
            flag_names.join(" | ")
        }
    }

    fn format_reserve_currency_flags(flags: ReserveCurrencyFlags) -> String {
        let mut flag_names = Vec::new();

        if flags.contains(ReserveCurrencyFlags::ALL_BUT) {
            flag_names.push("ALL_BUT_AMOUNT");
        }
        if flags.contains(ReserveCurrencyFlags::IGNORE_ERROR) {
            flag_names.push("AT_MOST");
        }
        if flags.contains(ReserveCurrencyFlags::WITH_ORIGINAL_BALANCE) {
            flag_names.push("INCREASE_BY_ORIGINAL_BALANCE");
        }
        if flags.contains(ReserveCurrencyFlags::REVERSE) {
            flag_names.push("NEGATE_AMOUNT");
        }
        if flags.contains(ReserveCurrencyFlags::BOUNCE_ON_ERROR) {
            flag_names.push("BOUNCE_ON_ACTION_FAIL");
        }

        if flag_names.is_empty() {
            "EXACT_AMOUNT".to_string()
        } else {
            flag_names.join(" | ")
        }
    }

    fn format_tokens(tokens: &Tokens) -> String {
        format!("{:.9} TON", tokens.into_inner() as f64 / 1_000_000_000.0)
    }

    fn format_int_addr(addr: &IntAddr) -> String {
        match addr {
            IntAddr::Std(std_addr) => std_addr.display_base64(true).to_string(),
            IntAddr::Var(var_addr) => format!("{:?}", var_addr), // fallback for VarAddr
        }
    }

    fn format_relaxed_msg_info(info: &RelaxedMsgInfo) -> String {
        match info {
            RelaxedMsgInfo::Int(int_info) => {
                format!(
                    "to {} with {}",
                    Self::format_int_addr(&int_info.dst),
                    Self::format_currency_collection(&int_info.value)
                )
            }
            RelaxedMsgInfo::ExtOut(ext_info) => {
                format!(
                    "to {}",
                    ext_info
                        .dst
                        .as_ref()
                        .map_or("None".to_string(), |addr| addr.to_string())
                )
            }
        }
    }

    fn format_currency_collection(currency: &CurrencyCollection) -> String {
        let ton_amount = currency.tokens.into_inner() as f64 / 1_000_000_000.0;

        let mut result = format!("{:.9} TON ", ton_amount);

        if !currency.other.is_empty() {
            let mut other_currencies = Vec::new();
            let dict = currency.other.as_dict();
            for entry in dict.iter() {
                if let Ok((currency_id, amount)) = entry {
                    other_currencies.push(format!("{}: {}", currency_id, amount));
                }
            }
            if !other_currencies.is_empty() {
                result.push_str(&format!(" + [{}]", other_currencies.join(", ")));
            }
        }

        result
    }

    fn build_message_children(&mut self, message: &OwnedRelaxedMessage) -> Vec<Variable> {
        let mut variables = Vec::new();

        let info_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
        self.msg_info_variables
            .insert(info_ref, message.info.clone());
        variables.push(Variable {
            name: "info".to_string(),
            type_field: Some("RelaxedMsgInfo".to_string()),
            value: "RelaxedMsgInfo".to_string(),
            variables_reference: info_ref,
            ..Default::default()
        });

        if let Some(init) = &message.init {
            let init_ref = VARIABLE_REFERENCE_COUNTER.fetch_add(1, Ordering::SeqCst) as i64;
            self.state_init_variables.insert(init_ref, init.clone());
            variables.push(Variable {
                name: "init".to_string(),
                type_field: Some("StateInit".to_string()),
                value: "StateInit".to_string(),
                variables_reference: init_ref,
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "init".to_string(),
                type_field: Some("Option<StateInit>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        let msg_cell = message.body.1.clone();
        let msg_offset = message.body.0.offset();
        let mut msg_slice = msg_cell.as_slice().unwrap();
        msg_slice
            .skip_first(msg_offset.bits, msg_offset.refs)
            .unwrap();
        let msg_cell = msg_slice.to_cell();

        variables.push(Variable {
            name: "body".to_string(),
            type_field: Some("CellSliceParts".to_string()),
            value: Boc::encode_hex(msg_cell),
            ..Default::default()
        });

        variables
    }

    fn build_msg_info_children(&self, msg_info: &RelaxedMsgInfo) -> Vec<Variable> {
        match msg_info {
            RelaxedMsgInfo::Int(int_info) => vec![
                Variable {
                    name: "ihr_disabled".to_string(),
                    type_field: Some("bool".to_string()),
                    value: int_info.ihr_disabled.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "bounce".to_string(),
                    type_field: Some("bool".to_string()),
                    value: int_info.bounce.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "bounced".to_string(),
                    type_field: Some("bool".to_string()),
                    value: int_info.bounced.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "src".to_string(),
                    type_field: Some("Option<IntAddr>".to_string()),
                    value: match &int_info.src {
                        Some(addr) => Self::format_int_addr(addr),
                        None => "None".to_string(),
                    },
                    ..Default::default()
                },
                Variable {
                    name: "dst".to_string(),
                    type_field: Some("IntAddr".to_string()),
                    value: Self::format_int_addr(&int_info.dst),
                    ..Default::default()
                },
                Variable {
                    name: "value".to_string(),
                    type_field: Some("CurrencyCollection".to_string()),
                    value: Self::format_currency_collection(&int_info.value),
                    ..Default::default()
                },
                Variable {
                    name: "ihr_fee".to_string(),
                    type_field: Some("Tokens".to_string()),
                    value: Self::format_tokens(&int_info.ihr_fee),
                    ..Default::default()
                },
                Variable {
                    name: "fwd_fee".to_string(),
                    type_field: Some("Tokens".to_string()),
                    value: Self::format_tokens(&int_info.fwd_fee),
                    ..Default::default()
                },
                Variable {
                    name: "created_lt".to_string(),
                    type_field: Some("u64".to_string()),
                    value: int_info.created_lt.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "created_at".to_string(),
                    type_field: Some("u32".to_string()),
                    value: int_info.created_at.to_string(),
                    ..Default::default()
                },
            ],
            RelaxedMsgInfo::ExtOut(ext_info) => vec![
                Variable {
                    name: "src".to_string(),
                    type_field: Some("Option<IntAddr>".to_string()),
                    value: match &ext_info.src {
                        Some(addr) => Self::format_int_addr(addr),
                        None => "None".to_string(),
                    },
                    ..Default::default()
                },
                Variable {
                    name: "dst".to_string(),
                    type_field: Some("Option<ExtAddr>".to_string()),
                    value: match &ext_info.dst {
                        Some(addr) => addr.to_string(),
                        None => "None".to_string(),
                    },
                    ..Default::default()
                },
                Variable {
                    name: "created_lt".to_string(),
                    type_field: Some("u64".to_string()),
                    value: ext_info.created_lt.to_string(),
                    ..Default::default()
                },
                Variable {
                    name: "created_at".to_string(),
                    type_field: Some("u32".to_string()),
                    value: ext_info.created_at.to_string(),
                    ..Default::default()
                },
            ],
        }
    }

    fn build_state_init_children(&self, state_init: &StateInit) -> Vec<Variable> {
        let mut variables = Vec::new();

        if let Some(split_depth) = &state_init.split_depth {
            variables.push(Variable {
                name: "split_depth".to_string(),
                type_field: Some("SplitDepth".to_string()),
                value: format!("{:?}", split_depth),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "split_depth".to_string(),
                type_field: Some("Option<SplitDepth>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        if let Some(special) = &state_init.special {
            variables.push(Variable {
                name: "special".to_string(),
                type_field: Some("SpecialFlags".to_string()),
                value: format!("tick: {}, tock: {}", special.tick, special.tock),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "special".to_string(),
                type_field: Some("Option<SpecialFlags>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        if let Some(code) = &state_init.code {
            variables.push(Variable {
                name: "code".to_string(),
                type_field: Some("Cell".to_string()),
                value: Boc::encode_hex(code),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "code".to_string(),
                type_field: Some("Option<Cell>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        if let Some(data) = &state_init.data {
            variables.push(Variable {
                name: "data".to_string(),
                type_field: Some("Cell".to_string()),
                value: Boc::encode_hex(data),
                ..Default::default()
            });
        } else {
            variables.push(Variable {
                name: "data".to_string(),
                type_field: Some("Option<Cell>".to_string()),
                value: "None".to_string(),
                ..Default::default()
            });
        }

        variables.push(Variable {
            name: "libraries".to_string(),
            type_field: Some("Dict<HashBytes, SimpleLib>".to_string()),
            value: if state_init.libraries.is_empty() {
                "empty".to_string()
            } else {
                format!(
                    "{} libraries",
                    state_init.libraries.iter().collect::<Vec<_>>().len()
                )
            },
            ..Default::default()
        });

        variables
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
