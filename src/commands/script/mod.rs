use crate::context::{AnyExecutor, BuildCache, Context, DebugContext, KnownAddresses};
use crate::{asserts_exts, exts, io_exts};
use abi::{ContractAbi, contract_abi};
use anyhow::anyhow;
use crossbeam_channel::unbounded;
use dap::errors::{DeserializationError, ServerError};
use dap::events::{Event, StoppedEventBody};
use dap::prelude::{Command, Request, Response, ResponseBody, Server};
use dap::responses::{
    ContinueResponse, ScopesResponse, StackTraceResponse, ThreadsResponse, VariablesResponse,
};
use dap::types;
use dap::types::{
    Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread, Variable,
};
use emulator::blockchain::Blockchain;
use emulator::emulator::Emulator;
use emulator::get_executor::{GetMethodParams, GetMethodResult};
use emulator::step_get_executor::StepGetExecutor;
use emulator::tuple::stack::{TupleItem, parse_tuple};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Cursor, Read};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;
use std::{fs, thread};
use tolkc::source_map::{DebugLocation, SourceMap};
use tonlib_core::TonAddress;
use tonlib_core::cell::{ArcCell, CellBuilder};
use tonlib_core::tlb_types::tlb::TLB;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellFamily, CellSlice, Load};
use tycho_types::dict::{Dict, RawDict};

pub fn script_cmd(path: &String) -> Result<(), anyhow::Error> {
    let metadata = fs::metadata(path)?;
    if !metadata.is_file() {
        return Err(anyhow!("Path '{}' is not a file", path));
    }

    if !path.ends_with(".tolk") {
        return Err(anyhow!("File must end with .tolk"));
    }

    let content = fs::read_to_string(path)?;
    run_script_file(path, &content)
}

fn run_script_file(file_path: &str, content: &str) -> Result<(), anyhow::Error> {
    let abi = contract_abi(content, file_path);

    let executable_code = content.to_string();
    let tmp_script_filename = format!("{}_script.tolk", file_path);

    fs::write(&tmp_script_filename, executable_code)?;

    let compilation_result = tolkc::compile_debug(Path::new(&tmp_script_filename));
    let result = match compilation_result {
        tolkc::CompilerResult::Success(result) => {
            let code_cell = ArcCell::from_boc_b64(&*result.code_boc64).unwrap();
            let data_cell = ArcCell::default();

            let script_result = execute_script(
                &code_cell,
                &data_cell,
                &abi,
                &result.debug_marks,
                &result.source_map.unwrap(),
            );
            print_script_result(script_result?);
            Ok(())
        }
        tolkc::CompilerResult::Error(error) => {
            Err(anyhow!("Cannot compile script file {}", error.message))
        }
    };

    let _ = fs::remove_file(&tmp_script_filename);

    result
}

struct ScriptResult {
    get_result: GetMethodResult,
}

fn execute_script(
    code_cell: &ArcCell,
    data_cell: &ArcCell,
    abi: &ContractAbi,
    marks: &HashMap<String, Vec<(i32, i32)>>,
    source_map: &SourceMap,
) -> anyhow::Result<ScriptResult> {
    let dest_address = contract_address(code_cell);

    let params = GetMethodParams {
        code: code_cell.to_boc_b64(false).unwrap().to_string(),
        data: data_cell.to_boc_b64(false).unwrap().to_string(),
        verbosity: 5,
        libs: "".to_string(),
        address: dest_address.to_string(),
        unixtime: 0,
        balance: "10".to_string(),
        rand_seed: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        gas_limit: "0".to_string(),
        method_id: 0,
        debug_enabled: true,
        extra_currencies: HashMap::new(),
        prev_blocks_info: None,
    };

    // let mut get_executor = GetExecutor::new(params.clone());
    let mut get_executor = StepGetExecutor::prepare_get_method(Default::default(), params.clone());
    let mut emulator = Emulator::new();
    let mut blockchain = Blockchain::new();
    let mut build_cache = BuildCache::new();
    let mut known_addresses = KnownAddresses::new();

    let (req_sender, req_receiver) = unbounded::<Request>();
    let (response_sender, response_receiver) = unbounded::<Response>();
    let (event_sender, event_receiver) = unbounded::<Event>();

    let debug_get_executor = get_executor.clone();
    let mut dbg_ctx = DebugContext {
        executors: vec![AnyExecutor::Get(debug_get_executor)],
        current_executor_id: 0,
        marks: vec![marks.clone()],
        source_maps: vec![(*source_map).clone()],
        locations: vec![],
        pseudo_step: 0,
        response_sender,
        event_sender,
        req_receiver: req_receiver.clone(),
    };

    let mut ctx = Context {
        stdout_buffer: "".to_string(),
        stderr_buffer: "".to_string(),
        capture_test_output: false,
        assert_failure: &mut None,
        blockchain: &mut blockchain,
        emulator: &mut emulator,
        build_cache: &mut build_cache,
        known_addresses: &mut known_addresses,
        abi: (*abi).clone(),
        expected_exit_code: &mut None,
        dbg_ctx: &mut dbg_ctx,
    };

    exts::register_step_get_extensions(&mut get_executor, &mut ctx);
    io_exts::register_step_get_extensions(&mut get_executor, &mut ctx);
    asserts_exts::register_step_get_extensions(&mut get_executor, &mut ctx);

    get_executor.run_get_method(0, Default::default());

    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
        println!("Server listening on 127.0.0.1:12345");

        for stream in listener.incoming() {
            let stream = stream.unwrap();
            println!("New connection established");

            let input_stream = stream.try_clone().unwrap();
            let mut input = BufReader::new(input_stream);

            let req_sender_1 = req_sender.clone();

            let reader_thread = thread::spawn(move || {
                loop {
                    let req = poll_request(&mut input);
                    println!("{:?}", req);
                    match req {
                        Ok(Some(req)) => {
                            req_sender_1.send(req.clone()).unwrap();
                        }
                        Ok(None) => {
                            println!("Request is closed");
                            // No more requests, connection might be closed
                            break;
                        }
                        Err(e) => {
                            eprintln!("Error handling request: {}", e);
                        }
                    }
                }
            });

            let cursor = Cursor::new("".as_bytes());
            let dummy_input = BufReader::new(cursor);
            let output_stream = stream;
            let output = BufWriter::new(output_stream);
            let mut server = Server::new(dummy_input, output);

            loop {
                crossbeam_channel::select! {
                    recv(response_receiver) -> msg => {
                        let Ok(rsp) = msg else { break };
                        server.respond(rsp).unwrap();
                    }

                    recv(event_receiver) -> msg => {
                        let Ok(event) = msg else { break };
                        server.send_event(event).unwrap();
                    }

                    default(Duration::from_millis(10)) => {
                        continue
                    }
                }
            }

            reader_thread.join().unwrap();

            println!("Connection closed");
        }
    });

    for req in req_receiver.iter() {
        if let Command::Disconnect(req) = &req.command {
            println!("Disconnecting: {:?}", req);
            break;
        }
        let is_end = ctx.dbg_ctx.on_request(req)?;
        if is_end {
            ctx.dbg_ctx.event_sender.send(Event::Terminated(None))?;
            break;
        }
    }

    let result = get_executor.finish_get_method();
    // let result = get_executor.run_get_method(Default::default(), params);

    Ok(ScriptResult { get_result: result })
}

#[derive(Debug)]
enum ServerState {
    /// Expecting a header
    Header,
    /// Expecting content
    Content,
}

pub fn poll_request(
    input_buffer: &mut BufReader<TcpStream>,
) -> Result<Option<Request>, ServerError> {
    let mut state = ServerState::Header;
    let mut buffer = String::new();
    let mut content_length: usize = 0;

    loop {
        match input_buffer.read_line(&mut buffer) {
            Ok(read_size) => {
                if read_size == 0 {
                    break Ok(None);
                }
                match state {
                    ServerState::Header => {
                        let parts: Vec<&str> = buffer.trim_end().split(':').collect();
                        if parts.len() == 2 {
                            match parts[0] {
                                "Content-Length" => {
                                    content_length = match parts[1].trim().parse() {
                                        Ok(val) => val,
                                        Err(_) => {
                                            return Err(ServerError::HeaderParseError {
                                                line: buffer,
                                            });
                                        }
                                    };
                                    buffer.clear();
                                    buffer.reserve(content_length);
                                    state = ServerState::Content;
                                }
                                other => {
                                    return Err(ServerError::UnknownHeader {
                                        header: other.to_string(),
                                    });
                                }
                            }
                        } else {
                            return Err(ServerError::HeaderParseError { line: buffer });
                        }
                    }
                    ServerState::Content => {
                        buffer.clear();
                        let mut content = vec![0; content_length];
                        input_buffer
                            .read_exact(content.as_mut_slice())
                            .map_err(ServerError::IoError)?;

                        let content = std::str::from_utf8(content.as_slice()).map_err(|e| {
                            ServerError::ParseError(DeserializationError::DecodingError(e))
                        })?;
                        let request: Request = serde_json::from_str(content).map_err(|e| {
                            ServerError::ParseError(DeserializationError::SerdeError(e))
                        })?;
                        return Ok(Some(request));
                    }
                }
            }
            Err(e) => return Err(ServerError::IoError(e)),
        }
    }
}

fn contract_address(code: &ArcCell) -> TonAddress {
    let state_init = CellBuilder::new()
        .store_bit(false)
        .unwrap()
        .store_bit(false)
        .unwrap()
        .store_ref_cell_optional(Some(code))
        .unwrap()
        .store_ref_cell_optional(Some(&ArcCell::default()))
        .unwrap()
        .store_bit(false)
        .unwrap()
        .build()
        .unwrap();

    let dest_address = TonAddress::new(0, state_init.cell_hash());
    dest_address
}

fn print_script_result(result: ScriptResult) {
    match &result.get_result {
        GetMethodResult::Success(success_result) => {
            let exit_code = success_result.vm_exit_code;
            std::process::exit(exit_code);
        }
        GetMethodResult::Error(error) => {
            println!("{} {}", "Execution error:".red(), error.error.red());
            std::process::exit(1);
        }
    }
}

impl DebugContext {
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
                // let stack = get_executor.get_stack();
                // let stack = parse_tuple(&ArcCell::from_boc_b64(&stack).unwrap()).unwrap();
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

                let marks = &self.marks[self.current_executor_id];
                let source_map = &self.source_maps[self.current_executor_id];
                let locations = get_locations(executor, marks, &source_map);
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

            // println!("Step {:?}", step);
            // println!("Event {:?}", step.context.event);

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

                let mark = &self.marks[self.current_executor_id];
                let source_map = &self.source_maps[self.current_executor_id];
                let locations = get_locations(executor, mark, &source_map);
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

                    let mark = &self.marks[self.current_executor_id];
                    let source_map = &self.source_maps[self.current_executor_id];
                    let locations = get_locations(&executor, mark, &source_map);
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

fn get_locations(
    executor: &AnyExecutor,
    marks: &HashMap<String, Vec<(i32, i32)>>,
    source_map: &SourceMap,
) -> Option<Vec<DebugLocation>> {
    let pos = executor.get_code_pos();
    let (hash, offset) = pos.split_once(":").unwrap();
    let offset = offset.parse::<i32>().unwrap();

    let Some(marks) = marks.get(hash) else {
        return None;
    };

    let debug_pair = marks
        .iter()
        .find(|(mark_offset, _)| return *mark_offset == offset);

    let debug_pairs = marks
        .iter()
        .filter(|(mark_offset, _)| return *mark_offset == offset)
        .collect::<Vec<_>>();

    let Some((mark_offset, debug_id)) = debug_pair else {
        return None;
    };

    let loc = source_map
        .locations
        .iter()
        .find(|loc| loc.idx == (*debug_id) as i64);

    let locs = source_map
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
