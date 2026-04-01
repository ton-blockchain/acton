use crate::transport::{DapConnection, IncomingRequest};
use anyhow::Context;
use crossbeam_channel::{Receiver, Sender, unbounded};
use dap::events::Event;
use dap::prelude::{Request, Response};
use log::{debug, error, info, warn};
use std::io::{BufReader, Cursor};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub enum DapMessage {
    Response(Response),
    Event(Event),
}

#[derive(Debug, Clone)]
pub struct DapTransport {
    pub req_receiver: Receiver<Request>,
    pub dap_sender: Sender<DapMessage>,
}

impl DapTransport {
    #[must_use]
    pub fn dummy() -> DapTransport {
        let (_, req_receiver) = unbounded::<Request>();
        let (dap_sender, _) = unbounded::<DapMessage>();
        DapTransport {
            req_receiver,
            dap_sender,
        }
    }
}

fn port_bind_failure(service: &str, address: &str, flag: &str) -> String {
    format!(
        "Failed to start {service} on {address}\nChoose another port with {flag}\nOr stop the process currently listening on that port"
    )
}

pub fn reserve_dap_listener(port: u16) -> anyhow::Result<TcpListener> {
    let address = format!("127.0.0.1:{port}");
    TcpListener::bind(&address)
        .with_context(|| port_bind_failure("debug server", &address, "--debug-port"))
}

pub fn start_dap_server_with_listener(listener: TcpListener) -> anyhow::Result<DapTransport> {
    let address = listener
        .local_addr()
        .context("Failed to inspect reserved debug server address")?
        .to_string();
    let (req_sender, req_receiver) = unbounded::<Request>();
    let (dap_sender, dap_receiver) = unbounded::<DapMessage>();

    thread::spawn(move || {
        let server_result = || -> anyhow::Result<()> {
            println!("Debugger server listening on {address}");

            let stream = listener
                .incoming()
                .next()
                .expect("listener.incoming().next() cannot fail by design")?;
            println!("New connection established");

            let input_stream = stream.try_clone()?;
            let mut reader = DapConnection::new(BufReader::new(input_stream), std::io::sink());

            let req_sender_for_reader = req_sender.clone();

            // Since `poll_request` is blocking, run it in the separate thread
            let reader_thread = thread::spawn(move || -> anyhow::Result<()> {
                loop {
                    let req = reader.poll_request();
                    match req {
                        Ok(Some(IncomingRequest::Known(req))) => {
                            debug!("Processing DAP request: {:?}", req.command);
                            req_sender_for_reader.send(req.clone())?;
                        }
                        Ok(Some(IncomingRequest::Unsupported { command, .. })) => {
                            info!("Ignoring custom DAP request {command}");
                        }
                        Ok(None) => {
                            // No more requests, connection might be closed
                            info!("DAP connection closed - no more requests");
                            break;
                        }
                        Err(e) => {
                            warn!("Error handling DAP request: {e}");
                        }
                    }
                }

                Ok(())
            });

            // Server require an input, pass dummy one, that's safe since we never call `poll_request`
            // on server, since we use thread above.
            let dummy_input = BufReader::new(Cursor::new(b""));
            let output_stream = stream;
            let mut connection = DapConnection::new(dummy_input, output_stream);

            loop {
                crossbeam_channel::select! {
                    recv(dap_receiver) -> msg => {
                        let Ok(dap_msg) = msg else { break };
                        match dap_msg {
                            DapMessage::Response(rsp) => {
                                connection.respond(rsp)?;
                            }
                            DapMessage::Event(event) => {
                                connection.send_event(event)?;
                            }
                        }
                    }

                    default(Duration::from_millis(10)) => {
                        // ... waiting
                    }
                }
            }

            let error = reader_thread
                .join()
                .expect("[INTERNAL ERROR] DAP thread panicked");

            match error {
                Ok(()) => {}
                Err(err) => {
                    error!("[INTERNAL ERROR] DAP thread error: {err}");
                }
            }

            println!("Connection closed");
            Ok(())
        };

        if let Err(err) = server_result() {
            error!("[INTERNAL ERROR] DAP server error: {err}");
        }
    });
    Ok(DapTransport {
        req_receiver,
        dap_sender,
    })
}

pub fn start_dap_server(port: u16) -> anyhow::Result<DapTransport> {
    let listener = reserve_dap_listener(port)?;
    start_dap_server_with_listener(listener)
}
