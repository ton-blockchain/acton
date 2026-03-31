#![allow(dead_code)]

use std::fs;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(crate) struct ToncenterV2MockResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Clone)]
pub(crate) struct ToncenterV3MockResponse {
    pub(crate) status: u16,
    pub(crate) body: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CapturedToncenterRequest {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) headers: Vec<(String, String)>,
}

pub(crate) fn spawn_toncenter_v2_mock(
    responses: Vec<ToncenterV2MockResponse>,
) -> (String, thread::JoinHandle<()>) {
    let (url, handle, _) = spawn_toncenter_v2_mock_with_capture(responses);
    (url, handle)
}

pub(crate) fn spawn_toncenter_v2_mock_with_capture(
    responses: Vec<ToncenterV2MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    spawn_toncenter_mock_with_capture(
        responses
            .into_iter()
            .map(|response| (response.status, response.body))
            .collect(),
    )
}

pub(crate) fn spawn_toncenter_v3_mock(
    responses: Vec<ToncenterV3MockResponse>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    spawn_toncenter_mock_with_capture(
        responses
            .into_iter()
            .map(|response| (response.status, response.body))
            .collect(),
    )
}

pub(crate) fn spawn_toncenter_mock_with_capture(
    responses: Vec<(u16, String)>,
) -> (
    String,
    thread::JoinHandle<()>,
    Arc<Mutex<Vec<CapturedToncenterRequest>>>,
) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind toncenter v2 mock");
    listener
        .set_nonblocking(true)
        .expect("failed to set toncenter v2 mock non-blocking");
    let addr = listener
        .local_addr()
        .expect("failed to get toncenter v2 mock address");

    let captured_requests = Arc::new(Mutex::new(Vec::<CapturedToncenterRequest>::new()));
    let captured_requests_thread = Arc::clone(&captured_requests);

    let handle = thread::spawn(move || {
        for (status, body) in responses {
            let wait_until = Instant::now() + Duration::from_secs(30);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(err) if err.kind() == ErrorKind::WouldBlock => {
                        assert!(
                            Instant::now() <= wait_until,
                            "timed out waiting for toncenter v2 request"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("toncenter v2 mock accept failed: {err}"),
                }
            };

            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("failed to set toncenter v2 mock read timeout");

            let mut reader = BufReader::new(
                stream
                    .try_clone()
                    .expect("failed to clone toncenter v2 mock stream"),
            );
            let mut request_line = String::new();
            let read_deadline = Instant::now() + Duration::from_secs(2);
            loop {
                request_line.clear();
                match reader.read_line(&mut request_line) {
                    Ok(0) => {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter v2 request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Ok(_) => break,
                    Err(err)
                        if matches!(err.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                    {
                        assert!(
                            Instant::now() <= read_deadline,
                            "timed out waiting for toncenter v2 request line"
                        );
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(err) => panic!("failed to read toncenter v2 request line: {err}"),
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
                    .expect("failed to read toncenter v2 header line");
                if read == 0 || header_line == "\r\n" {
                    break;
                }

                if let Some((name, value)) = header_line.split_once(':')
                    && name.trim().eq_ignore_ascii_case("content-length")
                {
                    content_length = value.trim().parse().unwrap_or(0);
                }

                if let Some((name, value)) = header_line.split_once(':') {
                    headers.push((name.trim().to_string(), value.trim().to_string()));
                }
            }

            if content_length > 0 {
                let mut request_body = vec![0_u8; content_length];
                reader
                    .read_exact(&mut request_body)
                    .expect("failed to read toncenter v2 request body");
            }

            captured_requests_thread
                .lock()
                .expect("captured toncenter requests mutex poisoned")
                .push(CapturedToncenterRequest {
                    method,
                    path,
                    headers,
                });

            let raw_response = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                status_text(status),
                body.len(),
                body
            );
            stream
                .write_all(raw_response.as_bytes())
                .expect("failed to write toncenter v2 response");
            stream
                .flush()
                .expect("failed to flush toncenter v2 response");
        }
    });

    (format!("http://{addr}"), handle, captured_requests)
}

pub(crate) fn append_custom_network(project_path: &Path, network_name: &str, v2_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r#"

[networks.{network_name}]
api = {{ v2 = "{v2_url}" }}
"#
    ));
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with custom network");
}

pub(crate) fn append_custom_network_with_urls(
    project_path: &Path,
    network_name: &str,
    v2_url: &str,
    v3_url: &str,
) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r#"

[networks.{network_name}]
api = {{ v2 = "{v2_url}", v3 = "{v3_url}" }}
"#
    ));
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with custom network");
}

pub(crate) fn append_localnet_network(project_path: &Path, v2_url: &str) {
    let acton_toml_path = project_path.join("Acton.toml");
    let mut acton_toml =
        fs::read_to_string(&acton_toml_path).expect("failed to read generated Acton.toml");
    acton_toml.push_str(&format!(
        r#"

[networks.localnet]
api = {{ v2 = "{v2_url}" }}
"#
    ));
    fs::write(&acton_toml_path, acton_toml)
        .expect("failed to write Acton.toml with localnet network");
}

pub(crate) fn toncenter_v2_seqno_ok_response() -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "result": {
                "stack": [["num", "0x0"]],
                "exit_code": 0
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_error_response(status: u16, error: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status,
        body: serde_json::json!({
            "ok": false,
            "error": error
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v2_get_libraries_ok_response(data: &str) -> ToncenterV2MockResponse {
    ToncenterV2MockResponse {
        status: 200,
        body: serde_json::json!({
            "ok": true,
            "result": {
                "result": [{
                    "found": true,
                    "data": data
                }]
            }
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v3_account_states_ok_response(
    address: &str,
    code_boc: Option<&str>,
    status: &str,
) -> ToncenterV3MockResponse {
    ToncenterV3MockResponse {
        status: 200,
        body: serde_json::json!({
            "accounts": [{
                "address": address,
                "balance": "0",
                "code_boc": code_boc,
                "status": status
            }]
        })
        .to_string(),
    }
}

pub(crate) fn toncenter_v3_error_response(status: u16, error: &str) -> ToncenterV3MockResponse {
    ToncenterV3MockResponse {
        status,
        body: serde_json::json!({
            "error": error
        })
        .to_string(),
    }
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
