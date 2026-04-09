use anyhow::{Result, anyhow};
use crossbeam_channel::{Receiver, Sender};
use dap::events::Event;
use dap::prelude::{Command, Request, Response, ResponseBody};
use dap::requests::{
    ContinueArguments, ExceptionInfoArguments, InitializeArguments, LaunchRequestArguments,
    NextArguments, ScopesArguments, SetBreakpointsArguments, StackTraceArguments, StepInArguments,
    StepOutArguments, TerminateArguments, VariablesArguments,
};
use dap::responses::{
    ContinueResponse, ExceptionInfoResponse, ScopesResponse, SetBreakpointsResponse,
    StackTraceResponse, ThreadsResponse, VariablesResponse,
};
use dap::types::{Capabilities, Source, SourceBreakpoint};
use log::{debug, info};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

static REQUEST_SEQ: AtomicU64 = AtomicU64::new(1);

pub struct DapClient {
    writer: BufWriter<TcpStream>,
    reader: BufReader<TcpStream>,
    response_receiver: Receiver<(u64, Response)>,
    event_receiver: Receiver<Event>,
    response_sender: Sender<(u64, Response)>,
    event_sender: Sender<Event>,
}

impl DapClient {
    pub fn connect(address: &str) -> Result<Self> {
        let stream = TcpStream::connect(address)?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        stream.set_write_timeout(Some(Duration::from_secs(30)))?;

        let writer_stream = stream.try_clone()?;
        let reader_stream = stream.try_clone()?;

        let writer = BufWriter::new(writer_stream);
        let reader = BufReader::new(reader_stream);

        let (response_sender, response_receiver) = crossbeam_channel::unbounded();
        let (event_sender, event_receiver) = crossbeam_channel::unbounded();

        let client = Self {
            writer,
            reader,
            response_receiver,
            event_receiver,
            response_sender,
            event_sender,
        };

        Ok(client)
    }

    pub fn start(&mut self) -> Result<()> {
        let reader = self.reader.get_mut().try_clone()?;
        let mut reader = BufReader::new(reader);
        let response_sender = self.response_sender.clone();
        let event_sender = self.event_sender.clone();

        thread::spawn(move || {
            loop {
                match Self::read_message(&mut reader) {
                    Ok(Some(msg)) => {
                        debug!("Received message: {msg:?}");
                        match msg {
                            DapMessage::Response(rsp) => {
                                let seq = rsp.request_seq as u64;
                                response_sender.send((seq, rsp)).unwrap_or(());
                            }
                            DapMessage::Event(evt) => {
                                event_sender.send(evt).unwrap_or(());
                            }
                        }
                    }
                    Ok(None) => {
                        info!("Connection closed");
                        break;
                    }
                    Err(e) => {
                        debug!("Error reading message: {e}");
                        break;
                    }
                }
            }
        });

        Ok(())
    }

    fn read_message(reader: &mut BufReader<TcpStream>) -> Result<Option<DapMessage>> {
        let mut content_length: Option<usize> = None;
        let mut buffer = String::new();

        loop {
            buffer.clear();
            let read_size = reader.read_line(&mut buffer)?;
            if read_size == 0 {
                return Ok(None);
            }

            let line = buffer.trim_end();
            if line.is_empty() {
                if let Some(len) = content_length {
                    let mut content = vec![0u8; len];
                    reader.read_exact(&mut content)?;
                    let content_str = std::str::from_utf8(&content)?;
                    debug!("Received JSON: {content_str}");

                    let json_value: serde_json::Value = serde_json::from_str(content_str)?;
                    let msg_type = json_value.get("type").and_then(|v| v.as_str());

                    match msg_type {
                        Some("response") => {
                            let response: Response = serde_json::from_value(json_value)?;
                            return Ok(Some(DapMessage::Response(response)));
                        }
                        Some("event") => {
                            let event: Event = serde_json::from_value(json_value)?;
                            return Ok(Some(DapMessage::Event(event)));
                        }
                        Some("request") => {
                            debug!("Ignoring server->client request: {content_str}");
                            content_length = None;
                            continue;
                        }
                        other => {
                            debug!(
                                "Unknown DAP message type {other:?}, ignoring message: {content_str}"
                            );
                            content_length = None;
                            continue;
                        }
                    }
                }
                continue;
            }

            if let Some((key, value)) = line.split_once(':') {
                #[allow(clippy::single_match)]
                match key.trim() {
                    "Content-Length" => {
                        content_length = Some(value.trim().parse()?);
                    }
                    _ => {}
                }
            }
        }
    }

    fn send_request(&mut self, command: Command) -> Result<u64> {
        let seq = REQUEST_SEQ.fetch_add(1, Ordering::SeqCst);
        let request = Request {
            seq: seq as i64,
            command,
        };

        let json = serde_json::to_string(&request)?;
        let content_length = json.len();

        writeln!(self.writer, "Content-Length: {content_length}")?;
        writeln!(self.writer)?;
        self.writer.write_all(json.as_bytes())?;
        self.writer.flush()?;

        debug!("Sent request seq={}: {:?}", seq, request.command);
        Ok(seq)
    }

    pub fn wait_for_response(&self, seq: u64, timeout: Duration) -> Result<Response> {
        let start = std::time::Instant::now();
        let mut pending_events = Vec::new();
        loop {
            if start.elapsed() > timeout {
                for event in pending_events {
                    self.event_sender.send(event).unwrap_or(());
                }
                return Err(anyhow!("Timeout waiting for response seq={seq}"));
            }

            if let Ok((response_seq, response)) = self
                .response_receiver
                .recv_timeout(Duration::from_millis(100))
                && response_seq == seq
            {
                for event in pending_events {
                    self.event_sender.send(event).unwrap_or(());
                }
                return Ok(response);
            }

            if let Ok(event) = self.event_receiver.recv_timeout(Duration::from_millis(100)) {
                if matches!(event, Event::Terminated(_)) {
                    for event in pending_events {
                        self.event_sender.send(event).unwrap_or(());
                    }
                    anyhow::bail!(
                        "The debugger terminated, probably because you stepped too many times, check stacktrace"
                    );
                }

                pending_events.push(event);
            }
        }
    }

    pub fn try_receive_event(&self, timeout: Duration) -> Result<Option<Event>> {
        match self.event_receiver.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(_) => Ok(None),
        }
    }

    pub fn initialize(&mut self) -> Result<Capabilities> {
        let seq = self.send_request(Command::Initialize(InitializeArguments {
            client_id: Some("dap-client".to_string()),
            client_name: Some("DAP Test Client".to_string()),
            adapter_id: "tolk-debugger".to_string(),
            locale: None,
            lines_start_at1: Some(true),
            columns_start_at1: Some(true),
            path_format: None,
            supports_variable_type: Some(false),
            supports_variable_paging: Some(false),
            supports_run_in_terminal_request: Some(false),
            supports_memory_references: Some(false),
            supports_progress_reporting: Some(false),
            supports_invalidated_event: Some(false),
            supports_memory_event: Some(false),
            supports_args_can_be_interpreted_by_shell: Some(false),
            supports_start_debugging_request: Some(false),
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Initialize response: {response:?}");

        match response.body {
            Some(ResponseBody::Initialize(capabilities)) => Ok(capabilities),
            _ => Ok(Capabilities::default()),
        }
    }

    pub fn launch(&mut self) -> Result<()> {
        let seq = self.send_request(Command::Launch(LaunchRequestArguments {
            no_debug: Some(false),
            restart_data: None,
            additional_data: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Launch response: {response:?}");
        Ok(())
    }

    pub fn set_breakpoints(
        &mut self,
        source: Source,
        breakpoints: Vec<SourceBreakpoint>,
    ) -> Result<SetBreakpointsResponse> {
        let seq = self.send_request(Command::SetBreakpoints(SetBreakpointsArguments {
            source,
            breakpoints: Some(breakpoints),
            #[allow(deprecated)]
            lines: None,
            source_modified: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("SetBreakpoints response: {response:?}");

        match response.body {
            Some(ResponseBody::SetBreakpoints(result)) => Ok(result),
            _ => Ok(SetBreakpointsResponse {
                breakpoints: Vec::new(),
            }),
        }
    }

    pub fn configuration_done(&mut self) -> Result<()> {
        let seq = self.send_request(Command::ConfigurationDone)?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("ConfigurationDone response: {response:?}");
        Ok(())
    }

    pub fn continue_execution(&mut self, thread_id: i64) -> Result<ContinueResponse> {
        let seq = self.send_request(Command::Continue(ContinueArguments {
            thread_id,
            single_thread: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Continue response: {response:?}");

        match response.body {
            Some(ResponseBody::Continue(result)) => Ok(result),
            _ => Ok(ContinueResponse {
                all_threads_continued: None,
            }),
        }
    }

    pub fn threads(&mut self) -> Result<ThreadsResponse> {
        let seq = self.send_request(Command::Threads)?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Threads response: {response:?}");

        match response.body {
            Some(ResponseBody::Threads(threads)) => Ok(threads),
            _ => Ok(ThreadsResponse {
                threads: Vec::new(),
            }),
        }
    }

    pub fn stack_trace(&mut self, thread_id: i64) -> Result<StackTraceResponse> {
        let seq = self.send_request(Command::StackTrace(StackTraceArguments {
            thread_id,
            start_frame: None,
            levels: None,
            format: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("StackTrace response: {response:?}");

        match response.body {
            Some(ResponseBody::StackTrace(result)) => Ok(result),
            _ => Ok(StackTraceResponse {
                stack_frames: Vec::new(),
                total_frames: None,
            }),
        }
    }

    pub fn scopes(&mut self, frame_id: i64) -> Result<ScopesResponse> {
        let seq = self.send_request(Command::Scopes(ScopesArguments { frame_id }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Scopes response: {response:?}");

        match response.body {
            Some(ResponseBody::Scopes(result)) => Ok(result),
            _ => Ok(ScopesResponse { scopes: Vec::new() }),
        }
    }

    pub fn variables(&mut self, variables_reference: i64) -> Result<VariablesResponse> {
        let seq = self.send_request(Command::Variables(VariablesArguments {
            variables_reference,
            filter: None,
            start: None,
            count: None,
            format: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Variables response: {response:?}");

        match response.body {
            Some(ResponseBody::Variables(result)) => Ok(result),
            _ => Ok(VariablesResponse {
                variables: Vec::new(),
            }),
        }
    }

    pub fn exception_info(&mut self, thread_id: i64) -> Result<ExceptionInfoResponse> {
        let seq =
            self.send_request(Command::ExceptionInfo(ExceptionInfoArguments { thread_id }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("ExceptionInfo response: {response:?}");

        match response.body {
            Some(ResponseBody::ExceptionInfo(result)) => Ok(result),
            _ => Ok(ExceptionInfoResponse {
                exception_id: String::new(),
                break_mode: dap::types::ExceptionBreakMode::Never,
                description: None,
                details: None,
            }),
        }
    }

    pub fn step_in(&mut self, thread_id: i64) -> Result<()> {
        let seq = self.send_request(Command::StepIn(StepInArguments {
            thread_id,
            single_thread: None,
            target_id: None,
            granularity: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("StepIn response: {response:?}");
        Ok(())
    }

    pub fn step_over(&mut self, thread_id: i64) -> Result<()> {
        let seq = self.send_request(Command::Next(NextArguments {
            thread_id,
            single_thread: None,
            granularity: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("StepOver response: {response:?}");
        Ok(())
    }

    pub fn step_out(&mut self, thread_id: i64) -> Result<()> {
        let seq = self.send_request(Command::StepOut(StepOutArguments {
            thread_id,
            single_thread: None,
            granularity: None,
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("StepOut response: {response:?}");
        Ok(())
    }

    pub fn terminate(&mut self) -> Result<()> {
        let seq = self.send_request(Command::Terminate(TerminateArguments {
            restart: Some(false),
        }))?;

        let response = self.wait_for_response(seq, Duration::from_secs(10))?;
        debug!("Terminate response: {response:?}");
        Ok(())
    }
}

#[derive(Debug)]
enum DapMessage {
    Response(Response),
    Event(Event),
}
