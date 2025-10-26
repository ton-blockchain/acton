use anyhow::anyhow;
use crossbeam_channel::{Sender, unbounded};
use dap::events::StoppedEventBody;
use dap::prelude::*;
use dap::responses::{
    ContinueResponse, ScopesResponse, StackTraceResponse, ThreadsResponse, VariablesResponse,
};
use dap::types::{
    Scope, ScopePresentationhint, Source, StackFrame, StoppedEventReason, Thread, Variable,
};
use std::io::{BufReader, BufWriter};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn main() -> anyhow::Result<()> {
    let (req_sender, req_receiver) = unbounded::<Request>();
    let (response_sender, response_receiver) = unbounded::<Response>();
    let (event_sender, event_receiver) = unbounded::<Event>();

    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
        println!("Server listening on 127.0.0.1:12345");

        for stream in listener.incoming() {
            let stream = stream.unwrap();
            println!("New connection established");

            let input_stream = stream.try_clone().unwrap();
            let output_stream = stream;
            let input = BufReader::new(input_stream);
            let output = BufWriter::new(output_stream);
            let mut server = Server::new(input, output);

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

                    default(std::time::Duration::from_millis(2)) => {
                        let req = server.poll_request();
                        match req {
                            Ok(Some(req)) => {
                                req_sender.send(req.clone()).unwrap();
                                // let need_break = on_request(&mut server, req, &mut index);
                                // match need_break {
                                //     Ok(true) => break,
                                //     Ok(false) => {}
                                //     Err(_) => {}
                                // }
                            }
                            Ok(None) => {
                                // No more requests, connection might be closed
                                break;
                            }
                            Err(e) => {
                                eprintln!("Error handling request: {}", e);
                                break;
                            }
                        }
                    }
                }
            }

            println!("Connection closed");
        }
    });

    for req in req_receiver.iter() {
        on_request(&response_sender, &event_sender, req)?;
    }

    Ok(())
}

fn on_request(
    response_sender: &Sender<Response>,
    event_sender: &Sender<Event>,
    req: Request,
) -> anyhow::Result<()> {
    match &req.command {
        Command::Initialize(args) => {
            let rsp = req.success(ResponseBody::Initialize(types::Capabilities {
                ..Default::default()
            }));

            response_sender.send(rsp)?;
            event_sender.send(Event::Initialized)?;
        }
        Command::Launch(args) => {
            println!("Launching {:?}", args);

            event_sender
                .send(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Step,
                    thread_id: Some(1),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))
                .unwrap();
        }
        Command::Threads => {
            let rsp = req.success(ResponseBody::Threads(ThreadsResponse {
                threads: vec![Thread {
                    id: 1,
                    name: "main thread".to_string(),
                }],
            }));
            response_sender.send(rsp).unwrap();
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
            response_sender.send(rsp).unwrap();
        }
        Command::Variables(_args) => {
            let rsp = req.success(ResponseBody::Variables(VariablesResponse {
                variables: vec![Variable {
                    name: "a".to_string(),
                    value: "1".to_string(),
                    ..Default::default()
                }],
            }));
            response_sender.send(rsp).unwrap();
        }
        Command::StackTrace(_args) => {
            let rsp = req.success(ResponseBody::StackTrace(StackTraceResponse {
                stack_frames: vec![StackFrame {
                    name: "3.tolk".to_string(),
                    line: 4,
                    column: 5,
                    source: Some(Source {
                        name: Some("3.tolk".to_string()),
                        path: Some(
                            "/Users/petrmakhnev/tolk-bench/contracts_Tolk/01_jetton/3.tolk"
                                .to_string(),
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                total_frames: None,
            }));
            response_sender.send(rsp).unwrap();
        }
        Command::Continue(_args) => {
            let rsp = req.success(ResponseBody::Continue(ContinueResponse {
                all_threads_continued: Some(true),
            }));
            response_sender.send(rsp).unwrap();
        }
        Command::Next(_args) => {
            let rsp = req.success(ResponseBody::Next);
            response_sender.send(rsp).unwrap();

            event_sender
                .send(Event::Stopped(StoppedEventBody {
                    reason: StoppedEventReason::Step,
                    thread_id: Some(1),
                    description: None,
                    preserve_focus_hint: None,
                    text: None,
                    all_threads_stopped: None,
                    hit_breakpoint_ids: None,
                }))
                .unwrap();
        }
        Command::SetExceptionBreakpoints(_) => {}
        Command::Disconnect(_) => {
            // drop(req_sender);
            // return
        }
        _ => {
            eprintln!("Unhandled command: {:?}", req.command);
            // return Err(anyhow!("Unhandled command: {:?}", req.command));
        }
    }
    Ok(())
}
