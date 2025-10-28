use crossbeam_channel::{Receiver, SendError, Sender, unbounded};
use dap::errors::{DeserializationError, ServerError};
use dap::events::Event;
use dap::prelude::{Request, Response, Server};
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
                        return Ok(Some(request));
                    }
                }
            }
            Err(e) => return Err(ServerError::IoError(e)),
        }
    }
}

pub fn start_dap_server() -> (Receiver<Request>, Sender<DapMessage>) {
    let (req_sender, req_receiver) = unbounded::<Request>();
    let (dap_message_sender, dap_message_receiver) = unbounded::<DapMessage>();

    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:12345").unwrap();
        println!("Debugger server listening on 127.0.0.1:12345");

        let stream = listener.incoming().next().unwrap().unwrap();
        println!("New connection established");

        let input_stream = stream.try_clone().unwrap();
        let mut input = BufReader::new(input_stream);

        let req_sender_for_reader = req_sender.clone();

        // Since `poll_request` is blocking, run it in the separate thread
        let reader_thread = thread::spawn(move || {
            loop {
                let req = poll_request(&mut input);
                println!("{:?}", req);
                match req {
                    Ok(Some(req)) => {
                        req_sender_for_reader.send(req.clone()).unwrap();
                    }
                    Ok(None) => {
                        // No more requests, connection might be closed
                        println!("Request is closed");
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error handling request: {}", e);
                    }
                }
            }
        });

        // Server require an input, pass dummy one, that's safe since we never call `pull_request`
        // on server, since we use thread above.
        let dummy_input = BufReader::new(Cursor::new("".as_bytes()));
        let output_stream = stream;
        let output = BufWriter::new(output_stream);
        let mut server = Server::new(dummy_input, output);

        loop {
            crossbeam_channel::select! {
                recv(dap_message_receiver) -> msg => {
                    let Ok(dap_msg) = msg else { break };
                    match dap_msg {
                        DapMessage::Response(rsp) => {
                            server.respond(rsp).unwrap();
                        }
                        DapMessage::Event(event) => {
                            server.send_event(event).unwrap();
                        }
                    }
                }

                default(Duration::from_millis(10)) => {
                    continue
                }
            }
        }

        reader_thread.join().unwrap();

        println!("Connection closed");
    });
    (req_receiver, dap_message_sender)
}
