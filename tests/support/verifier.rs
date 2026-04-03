#![allow(dead_code)]

use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct VerifierMockResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
    pub(crate) headers: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub(crate) struct CapturedVerifierRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

pub(crate) fn spawn_verifier_mock(
    responses: Vec<VerifierMockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedVerifierRequest>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind verifier mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set verifier mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get verifier mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedVerifierRequest>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for response in responses {
            let wait_until = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for verifier request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("verifier mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set verifier mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone verifier mock stream"),
            );
            let mut request_line = String::new();
            let read_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                request_line.clear();
                match reader.read_line(&mut request_line) {
                    Ok(0) => {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for verifier request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => break,
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for verifier request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("failed to read verifier request line: {err}"),
                }
            }

            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or_default().to_string();
            let path = parts.next().unwrap_or_default().to_string();

            let mut headers = Vec::new();
            let mut content_length = 0_usize;
            loop {
                let mut header_line = String::new();
                let read = reader
                    .read_line(&mut header_line)
                    .expect("failed to read verifier header line");
                if read == 0 || header_line == "\r\n" {
                    break;
                }

                if let Some((name, value)) = header_line.split_once(':') {
                    let name = name.trim().to_string();
                    let value = value.trim().to_string();
                    if name.eq_ignore_ascii_case("content-length") {
                        content_length = value.parse().unwrap_or(0);
                    }
                    headers.push((name, value));
                }
            }

            let mut body = Vec::new();
            if content_length > 0 {
                body.resize(content_length, 0);
                reader
                    .read_exact(&mut body)
                    .expect("failed to read verifier request body");
            }

            captured_requests_thread
                .lock()
                .expect("captured verifier requests mutex poisoned")
                .push(CapturedVerifierRequest {
                    method,
                    path,
                    headers,
                    body,
                });

            let mut response_headers = response.headers;
            if !response_headers
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
            {
                response_headers.push(("Content-Type".to_string(), "application/json".to_string()));
            }
            response_headers.push((
                "Content-Length".to_string(),
                response.body.len().to_string(),
            ));
            response_headers.push(("Connection".to_string(), "close".to_string()));

            let raw_response = format!(
                "HTTP/1.1 {} {}\r\n{}\r\n\r\n{}",
                response.status,
                status_text(response.status),
                response_headers
                    .iter()
                    .map(|(name, value)| format!("{name}: {value}"))
                    .collect::<Vec<_>>()
                    .join("\r\n"),
                response.body
            );

            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write verifier response");
            stream.flush().expect("failed to flush verifier response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        _ => "Unknown",
    }
}
