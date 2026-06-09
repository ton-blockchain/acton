use crate::common::assertion;
use crate::support::project::ProjectBuilder;
use crate::support::snapshots::normalize_output_preserve_escapes;
use base64::Engine;
use reqwest::blocking::{Client, Response};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

const STREAMING_TEST_ACCOUNT_A: &str =
    "0:84545d4d2cada0ce811705d534c298ca42d29315d03a16eee794cefd191dfa79";

#[test]
fn localnet_supports_streaming_v2_sse() {
    let project = ProjectBuilder::new("localnet-streaming-v2-sse").build();
    let node = project.localnet().start();
    let url = format!("{}/api/streaming/v2/sse", node.base_url());
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    let reader = thread::spawn(move || {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .expect("Failed to create SSE client");
        let mut response = client
            .post(url)
            .json(&json!({
                "id": "sse-1",
                "addresses": [STREAMING_TEST_ACCOUNT_A],
                "types": ["transactions", "account_state_change"],
                "min_finality": "confirmed",
                "include_address_book": true
            }))
            .send()
            .expect("Failed to open streaming SSE subscription");
        assert!(
            response.status().is_success(),
            "SSE subscription failed with status {}",
            response.status()
        );
        ready_tx.send(()).expect("Failed to report SSE readiness");
        read_sse_json_events(&mut response, 5, Duration::from_secs(12))
    });

    ready_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("SSE subscription did not become ready");

    let faucet = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": STREAMING_TEST_ACCOUNT_A,
            "amount": 250_000_000u128
        }),
    );
    assert_eq!(
        faucet["ok"].as_bool(),
        Some(true),
        "faucet failed: {}",
        serde_json::to_string_pretty(&faucet).unwrap_or_default()
    );

    let events = reader.join().expect("SSE reader thread panicked");
    let summary = summarize_streaming_sse_events(&events);

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_streaming_v2_sse.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_streaming_v2_sse_validates_subscription_shape() {
    let project = ProjectBuilder::new("localnet-streaming-v2-sse-validation").build();
    let node = project.localnet().start();

    let missing_types = node.post_json_with_status(
        "/api/streaming/v2/sse",
        &json!({
            "id": "missing-types",
            "addresses": [STREAMING_TEST_ACCOUNT_A]
        }),
    );
    let missing_trace_hash = node.post_json_with_status(
        "/api/streaming/v2/sse",
        &json!({
            "id": "missing-trace",
            "types": ["trace"],
            "addresses": [STREAMING_TEST_ACCOUNT_A]
        }),
    );
    let invalid_event_type = node.post_json_with_status(
        "/api/streaming/v2/sse",
        &json!({
            "id": "invalid-type",
            "addresses": [STREAMING_TEST_ACCOUNT_A],
            "types": ["trace_invalidated"]
        }),
    );

    let summary = json!({
        "missing_types": summarize_error_response(missing_types),
        "missing_trace_hash": summarize_error_response(missing_trace_hash),
        "invalid_event_type": summarize_error_response(invalid_event_type),
    });

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_streaming_v2_sse_validation.summary.json"),
    );

    node.stop();
}

#[test]
fn localnet_supports_streaming_v2_websocket() {
    let project = ProjectBuilder::new("localnet-streaming-v2-websocket").build();
    let node = project.localnet().start();
    let mut ws = connect_localnet_websocket(node.port());

    write_ws_text(
        &mut ws,
        &json!({
            "id": "ping-1",
            "operation": "ping"
        })
        .to_string(),
    );
    let pong = read_ws_json(&mut ws);

    write_ws_text(
        &mut ws,
        &json!({
            "id": "subscribe-1",
            "operation": "subscribe",
            "addresses": [STREAMING_TEST_ACCOUNT_A],
            "types": ["transactions"],
            "min_finality": "finalized"
        })
        .to_string(),
    );
    let subscribed = read_ws_json(&mut ws);

    let faucet = node.post_json(
        "/acton_fundAccount",
        &json!({
            "address": STREAMING_TEST_ACCOUNT_A,
            "amount": 123_000_000u128
        }),
    );
    assert_eq!(
        faucet["ok"].as_bool(),
        Some(true),
        "faucet failed: {}",
        serde_json::to_string_pretty(&faucet).unwrap_or_default()
    );
    let event = read_ws_json(&mut ws);

    write_ws_text(
        &mut ws,
        &json!({
            "id": "unsubscribe-1",
            "operation": "unsubscribe",
            "addresses": [STREAMING_TEST_ACCOUNT_A]
        })
        .to_string(),
    );
    let unsubscribed = read_ws_json(&mut ws);

    let summary = summarize_streaming_ws_flow(&pong, &subscribed, &event, &unsubscribed);

    assertion().eq(
        pretty_json_for_snapshot(&summary, project.path()),
        snapbox::file!("snapshots/localnet/test_localnet_streaming_v2_websocket.summary.json"),
    );

    node.stop();
}

fn read_sse_json_events(response: &mut Response, expected: usize, timeout: Duration) -> Vec<Value> {
    let deadline = Instant::now() + timeout;
    let mut buffer = Vec::new();
    let mut events = Vec::new();
    let mut byte = [0_u8; 1];

    while events.len() < expected {
        assert!(
            Instant::now() < deadline,
            "Timed out reading SSE events; got {events:?}"
        );

        let read = response.read(&mut byte).expect("Failed to read SSE byte");
        assert_ne!(read, 0, "SSE stream closed after {events:?}");
        buffer.push(byte[0]);

        if buffer.ends_with(b"\n\n") {
            if let Some(event) = parse_sse_frame(&buffer) {
                events.push(event);
            }
            buffer.clear();
        }
    }

    events
}

fn parse_sse_frame(frame: &[u8]) -> Option<Value> {
    let text = std::str::from_utf8(frame).expect("SSE frame must be UTF-8");
    let mut event_name = None;
    let mut data = None;

    for line in text.lines() {
        if let Some(value) = line.strip_prefix("event: ") {
            event_name = Some(value.to_string());
        } else if let Some(value) = line.strip_prefix("data: ") {
            data = Some(value.to_string());
        }
    }

    let data = data?;
    let mut parsed: Value = serde_json::from_str(&data).expect("SSE data must be JSON");
    if let Some(event_name) = event_name
        && let Some(object) = parsed.as_object_mut()
    {
        object.insert("_event".to_string(), json!(event_name));
    }
    Some(parsed)
}

fn connect_localnet_websocket(port: u16) -> TcpStream {
    let mut stream =
        TcpStream::connect(("127.0.0.1", port)).expect("Failed to connect websocket TCP stream");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("Failed to set websocket read timeout");
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .expect("Failed to set websocket write timeout");

    let key = base64::engine::general_purpose::STANDARD.encode([7_u8; 16]);
    write!(
        stream,
        "GET /api/streaming/v2/ws HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {key}\r\n\
         Sec-WebSocket-Version: 13\r\n\
         \r\n"
    )
    .expect("Failed to write websocket handshake");
    stream.flush().expect("Failed to flush websocket handshake");

    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .expect("Failed to read websocket handshake");
        response.push(byte[0]);
    }
    let response = String::from_utf8(response).expect("Handshake response must be UTF-8");
    assert!(
        response.starts_with("HTTP/1.1 101"),
        "Expected websocket upgrade, got:\n{response}"
    );

    stream
}

fn write_ws_text(stream: &mut TcpStream, text: &str) {
    let payload = text.as_bytes();
    let mut frame = vec![0x81];

    if payload.len() <= 125 {
        frame.push(0x80 | payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(0x80 | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(0x80 | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }

    let mask = [1_u8, 2, 3, 4];
    frame.extend_from_slice(&mask);
    for (index, byte) in payload.iter().enumerate() {
        frame.push(byte ^ mask[index % mask.len()]);
    }

    stream
        .write_all(&frame)
        .expect("Failed to write websocket frame");
    stream.flush().expect("Failed to flush websocket frame");
}

fn read_ws_json(stream: &mut TcpStream) -> Value {
    let mut header = [0_u8; 2];
    stream
        .read_exact(&mut header)
        .expect("Failed to read websocket frame header");
    let opcode = header[0] & 0x0f;
    assert!(
        opcode == 0x1 || opcode == 0x2,
        "Expected text/binary websocket frame, got opcode {opcode}"
    );

    let masked = header[1] & 0x80 != 0;
    let mut len = u64::from(header[1] & 0x7f);
    if len == 126 {
        let mut extended = [0_u8; 2];
        stream
            .read_exact(&mut extended)
            .expect("Failed to read websocket extended length");
        len = u64::from(u16::from_be_bytes(extended));
    } else if len == 127 {
        let mut extended = [0_u8; 8];
        stream
            .read_exact(&mut extended)
            .expect("Failed to read websocket extended length");
        len = u64::from_be_bytes(extended);
    }

    let mask = if masked {
        let mut mask = [0_u8; 4];
        stream
            .read_exact(&mut mask)
            .expect("Failed to read websocket mask");
        Some(mask)
    } else {
        None
    };

    let mut payload = vec![0_u8; len as usize];
    stream
        .read_exact(&mut payload)
        .expect("Failed to read websocket payload");
    if let Some(mask) = mask {
        for (index, byte) in payload.iter_mut().enumerate() {
            *byte ^= mask[index % mask.len()];
        }
    }

    serde_json::from_slice(&payload).unwrap_or_else(|e| {
        panic!(
            "Websocket payload is not JSON: {e}\n{}",
            String::from_utf8_lossy(&payload)
        )
    })
}

fn summarize_streaming_sse_events(events: &[Value]) -> Value {
    json!(
        events
            .iter()
            .map(summarize_streaming_event)
            .collect::<Vec<_>>()
    )
}

fn summarize_streaming_ws_flow(
    pong: &Value,
    subscribed: &Value,
    event: &Value,
    unsubscribed: &Value,
) -> Value {
    json!({
        "pong": {
            "id": pong["id"],
            "status": pong["status"],
        },
        "subscribed": {
            "id": subscribed["id"],
            "status": subscribed["status"],
        },
        "event": summarize_streaming_event(event),
        "unsubscribed": {
            "id": unsubscribed["id"],
            "status": unsubscribed["status"],
        },
    })
}

fn summarize_streaming_event(event: &Value) -> Value {
    if event.get("status").is_some() {
        return json!({
            "event": event["_event"],
            "id": event["id"],
            "status": event["status"],
        });
    }

    match event["type"].as_str().unwrap_or_default() {
        "transactions" => {
            let transactions = event["transactions"]
                .as_array()
                .expect("transactions event must include transactions array");
            json!({
                "event": event.get("_event").cloned().unwrap_or(Value::Null),
                "type": event["type"],
                "finality": event["finality"],
                "trace_external_hash_norm_present": event["trace_external_hash_norm"].as_str().is_some_and(|hash| !hash.is_empty()),
                "transactions_len": transactions.len(),
                "first_account": transactions.first().and_then(|tx| tx["account"].as_str()).unwrap_or_default(),
                "address_book_len": event["address_book"].as_object().map_or(0, serde_json::Map::len),
            })
        }
        "account_state_change" => {
            json!({
                "event": event.get("_event").cloned().unwrap_or(Value::Null),
                "type": event["type"],
                "finality": event["finality"],
                "account": event["account"],
                "state_status": event["state"]["status"],
                "balance_nonzero": event["state"]["balance"].as_str().is_some_and(|balance| balance != "0"),
            })
        }
        other => json!({
            "event": event.get("_event").cloned().unwrap_or(Value::Null),
            "type": other,
        }),
    }
}

fn summarize_error_response((status, response): (u16, Value)) -> Value {
    json!({
        "status": status,
        "id": response.get("id").cloned().unwrap_or(Value::Null),
        "error": response.get("error").cloned().unwrap_or(Value::Null),
    })
}

fn pretty_json_for_snapshot(value: &Value, project_path: &Path) -> String {
    let response_json = format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("Failed to serialize JSON snapshot")
    );
    normalize_output_preserve_escapes(&response_json, project_path)
}
