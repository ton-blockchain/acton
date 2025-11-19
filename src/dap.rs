use crossbeam_channel::{Receiver, Sender, unbounded};
use dap::errors::{DeserializationError, ServerError};
use dap::events::Event;
use dap::prelude::{Request, Response, Server};
use log::{debug, info, warn};
use std::io::{BufRead, BufReader, BufWriter, Cursor, Read};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub enum DapMessage {
    Response(Response),
    Event(Event),
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
                        debug!("Received DAP request: {request:?}");
                        return Ok(Some(request));
                    }
                }
            }
            Err(e) => return Err(ServerError::IoError(e)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DapTransport {
    pub req_receiver: Receiver<Request>,
    pub dap_sender: Sender<DapMessage>,
}

impl DapTransport {
    pub fn dummy() -> DapTransport {
        let (_, req_receiver) = unbounded::<Request>();
        let (dap_sender, _) = unbounded::<DapMessage>();
        DapTransport {
            req_receiver,
            dap_sender,
        }
    }
}

pub fn start_dap_server(port: u16) -> DapTransport {
    let address = format!("127.0.0.1:{port}");
    let (req_sender, req_receiver) = unbounded::<Request>();
    let (dap_sender, dap_receiver) = unbounded::<DapMessage>();

    thread::spawn(move || -> anyhow::Result<()> {
        let listener = TcpListener::bind(&address)?;
        println!("Debugger server listening on {address}");

        let stream = listener
            .incoming()
            .next()
            .expect("listener.incoming().next() cannot fail by design")?;
        println!("New connection established");

        let input_stream = stream.try_clone()?;
        let mut input = BufReader::new(input_stream);

        let req_sender_for_reader = req_sender.clone();

        // Since `poll_request` is blocking, run it in the separate thread
        let reader_thread = thread::spawn(move || -> anyhow::Result<()> {
            loop {
                let req = poll_request(&mut input);
                match req {
                    Ok(Some(req)) => {
                        debug!("Processing DAP request: {:?}", req.command);
                        req_sender_for_reader.send(req.clone())?;
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
        let dummy_input = BufReader::new(Cursor::new("".as_bytes()));
        let output_stream = stream;
        let output = BufWriter::new(output_stream);
        let mut server = Server::new(dummy_input, output);

        loop {
            crossbeam_channel::select! {
                recv(dap_receiver) -> msg => {
                    let Ok(dap_msg) = msg else { break };
                    match dap_msg {
                        DapMessage::Response(rsp) => {
                            server.respond(rsp)?;
                        }
                        DapMessage::Event(event) => {
                            server.send_event(event)?;
                        }
                    }
                }

                default(Duration::from_millis(10)) => {
                    continue
                }
            }
        }

        reader_thread
            .join()
            .expect("[INTERNAL ERROR] DAP thread panicked")
            .expect("[INTERNAL ERROR] Cannot send DAP message");

        println!("Connection closed");
        Ok(())
    });
    DapTransport {
        req_receiver,
        dap_sender,
    }
}
