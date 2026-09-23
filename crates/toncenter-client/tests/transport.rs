use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use expect_test::expect;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toncenter_client::{
    Client, ClientBuilder, ErrorKind, V2Transport,
    toncenter::{v2, v3},
};

#[derive(Debug)]
struct Request {
    route: String,
    headers: BTreeMap<String, String>,
    body: String,
}

struct Server {
    url: String,
    requests: Arc<Mutex<Vec<Request>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    async fn new(responses: Vec<(u16, String, String)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind HTTP fixture");
        let url = format!("http://{}", listener.local_addr().expect("fixture address"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&requests);
        let task = tokio::spawn(async move {
            let mut responses = VecDeque::from(responses);
            loop {
                let (mut stream, _) = listener.accept().await.expect("accept client connection");
                let mut bytes = Vec::new();
                let mut chunk = [0; 4096];
                let (header_end, headers) = loop {
                    let count = stream.read(&mut chunk).await.expect("read request headers");
                    if count == 0 {
                        return;
                    }
                    bytes.extend_from_slice(&chunk[..count]);
                    if let Some(end) = bytes.windows(4).position(|value| value == b"\r\n\r\n") {
                        break (
                            end + 4,
                            String::from_utf8(bytes[..end].to_vec())
                                .expect("UTF-8 request headers"),
                        );
                    }
                };
                let mut lines = headers.lines();
                let route = lines.next().expect("HTTP request line").to_owned();
                let headers: BTreeMap<_, _> = lines
                    .map(|line| {
                        let (name, value) = line.split_once(':').expect("HTTP header separator");
                        (name.to_ascii_lowercase(), value.trim().to_owned())
                    })
                    .collect();
                let length: usize = headers
                    .get("content-length")
                    .map_or(0, |value| value.parse().expect("numeric Content-Length"));
                while bytes.len() < header_end + length {
                    let count = stream.read(&mut chunk).await.expect("read request body");
                    assert_ne!(count, 0);
                    bytes.extend_from_slice(&chunk[..count]);
                }
                captured.lock().expect("request lock").push(Request {
                    route,
                    headers,
                    body: String::from_utf8(bytes[header_end..].to_vec()).expect("UTF-8 JSON body"),
                });
                let (status, body, headers) =
                    responses
                        .pop_front()
                        .unwrap_or((500, String::new(), String::new()));
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        Self {
            url,
            requests,
            task,
        }
    }

    fn builder(&self) -> ClientBuilder {
        Client::builder()
            .v2_url(format!("{}/rpc/v2", self.url))
            .v3_url(format!("{}/index/v3", self.url))
            .system_proxy(false)
            .operation_timeout(Duration::from_secs(5))
            .retry_delays(Duration::ZERO, Duration::ZERO)
    }

    fn count(&self) -> usize {
        self.requests.lock().expect("request lock").len()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn reply(status: u16, mut value: Value) -> (u16, String, String) {
    if value["ok"] == true {
        value["@extra"] = json!("");
    }
    (status, value.to_string(), String::new())
}

#[tokio::test]
async fn named_and_generic_calls_preserve_routes_parameters_and_headers() {
    let server = Server::new(vec![
        reply(
            200,
            json!({"ok":true,"result":"18446744073709551615","@extra":"metadata"}),
        ),
        reply(200, json!({"ok":true,"result":"42"})),
        reply(200, json!({"ok":true,"result":"43"})),
        reply(200, json!({"accounts":[],"address_book":{}})),
        reply(
            200,
            json!({"message_hash":"hash","message_hash_norm":"normalized"}),
        ),
    ])
    .await;
    let client = server
        .builder()
        .api_key(Some("test-key".to_owned()))
        .bearer_auth("gateway")
        .user_agent("test-app/1.0")
        .origin("https://example.test")
        .build()
        .unwrap();
    let request = v2::requests::AddressBalanceRequest {
        address: "0:abc+/=".to_owned(),
        seqno: None,
    };
    let balance = client.v2().get_address_balance(&request).await.unwrap();
    let posted = client
        .v2()
        .transport(V2Transport::Post)
        .get_address_balance(&request)
        .await
        .unwrap();
    let rpc = client
        .v2()
        .transport(V2Transport::JsonRpc)
        .get_address_balance(&request)
        .await
        .unwrap();
    client
        .v3()
        .get_account_states(&v3::requests::AccountStatesQuery {
            address: vec!["0:a".to_owned(), "0:b".to_owned()],
            include_boc: None,
        })
        .await
        .unwrap();
    let _: Value = client
        .v3_post("message", &json!({"boc":"same-message"}))
        .await
        .unwrap();
    let requests = server.requests.lock().unwrap();
    let summary: Vec<_> = requests.iter().map(|request| json!({
        "route": request.route,
        "headers": [request.headers["x-api-key"].as_str(), request.headers["authorization"].as_str(), request.headers["user-agent"].as_str(), request.headers["origin"].as_str()],
        "body": if request.body.is_empty() { Value::Null } else { serde_json::from_str::<Value>(&request.body).unwrap() },
    })).collect();
    drop(requests);
    expect![[r#"["18446744073709551615","42","43"]"#]]
        .assert_eq(&json!([balance, posted, rpc]).to_string());
    expect![[r#"[{"body":null,"headers":["test-key","Bearer gateway","test-app/1.0","https://example.test"],"route":"GET /rpc/v2/getAddressBalance?address=0%3Aabc%2B%2F%3D HTTP/1.1"},{"body":{"address":"0:abc+/="},"headers":["test-key","Bearer gateway","test-app/1.0","https://example.test"],"route":"POST /rpc/v2/getAddressBalance HTTP/1.1"},{"body":{"id":"1","jsonrpc":"2.0","method":"getAddressBalance","params":{"address":"0:abc+/="}},"headers":["test-key","Bearer gateway","test-app/1.0","https://example.test"],"route":"POST /rpc/v2/jsonRPC HTTP/1.1"},{"body":null,"headers":["test-key","Bearer gateway","test-app/1.0","https://example.test"],"route":"GET /index/v3/accountStates?address=0%3Aa&address=0%3Ab HTTP/1.1"},{"body":{"boc":"same-message"},"headers":["test-key","Bearer gateway","test-app/1.0","https://example.test"],"route":"POST /index/v3/message HTTP/1.1"}]"#]].assert_eq(&serde_json::to_string(&summary).unwrap());
}

#[tokio::test]
async fn retries_preserve_credentials_and_honor_the_attempt_budget() {
    let server = Server::new(vec![
        (503, "gateway unavailable".to_owned(), String::new());
        4
    ])
    .await;
    let client = server
        .builder()
        .max_attempts(4)
        .api_key(Some("test-key".to_owned()))
        .origin("https://example.test")
        .user_agent("app/2")
        .build()
        .unwrap();
    let error = client
        .v3_get::<Value>("transactions", &())
        .await
        .unwrap_err();
    let requests = server.requests.lock().unwrap();
    expect![[r"
        (
            Http,
            Some(
                503,
            ),
            4,
            4,
            true,
        )
    "]]
    .assert_debug_eq(&(
        error.kind(),
        error.status(),
        error.attempts(),
        requests.len(),
        requests.iter().all(|request| {
            request.headers["x-api-key"] == "test-key"
                && request.headers["origin"] == "https://example.test"
                && request.headers["user-agent"] == "app/2"
        }),
    ));
}

#[tokio::test]
async fn permanent_errors_and_broadcasts_do_not_retry() {
    let mut summary = Vec::new();
    for (method, status, body) in [
        (
            "getAddressBalance",
            200,
            json!({"ok":false,"code":500,"error":"contract failure"}),
        ),
        (
            "getAddressBalance",
            401,
            json!({"error":"invalid key","code":401}),
        ),
        ("sendBoc", 503, json!("unavailable")),
        ("unknownMutation", 503, json!("unavailable")),
    ] {
        let server = Server::new(vec![reply(status, body)]).await;
        let client = server.builder().build().unwrap();
        let error = client
            .v2_request::<Value>(V2Transport::Post, method, &json!({}))
            .await
            .unwrap_err();
        summary.push((method, error.kind(), error.attempts(), server.count()));
    }
    expect![[r#"
        [
            (
                "getAddressBalance",
                Api,
                1,
                1,
            ),
            (
                "getAddressBalance",
                Api,
                1,
                1,
            ),
            (
                "sendBoc",
                Http,
                1,
                1,
            ),
            (
                "unknownMutation",
                Http,
                1,
                1,
            ),
        ]
    "#]]
    .assert_debug_eq(&summary);
}

#[tokio::test]
async fn opted_in_broadcast_replays_the_same_body() {
    let server = Server::new(vec![
        (503, String::new(), String::new()),
        reply(200, json!({"ok":true,"result":{"@type":"ok"}})),
    ])
    .await;
    let client = server.builder().retry_broadcasts(true).build().unwrap();
    client
        .v2()
        .send_boc(&v2::requests::SendBocRequest {
            boc: "already-signed".to_owned(),
        })
        .await
        .unwrap();
    let requests = server.requests.lock().unwrap();
    expect![[r#"
        (
            2,
            true,
            "{\"boc\":\"already-signed\"}",
        )
    "#]]
    .assert_debug_eq(&(
        requests.len(),
        requests[0].body == requests[1].body,
        &requests[0].body,
    ));
}

#[tokio::test]
async fn shared_cooldown_and_operation_deadline_include_quota_waits() {
    let server = Server::new(vec![(
        200,
        json!({"ok":false,"code":429,"result":"Ratelimit exceed"}).to_string(),
        "Retry-After: 2\r\n".to_owned(),
    )])
    .await;
    let client = server.builder().max_attempts(1).build().unwrap();
    let error = client
        .v2_request::<Value>(V2Transport::Get, "getMasterchainInfo", &())
        .await
        .unwrap_err();
    let other = server
        .builder()
        .operation_timeout(Duration::from_millis(30))
        .build()
        .unwrap();
    let waiting = other
        .v3_get::<Value>("masterchainInfo", &())
        .await
        .unwrap_err();
    expect![[r#"
        (
            Some(
                429,
            ),
            "Ratelimit exceed",
            Timeout,
            0,
            1,
        )
    "#]]
    .assert_debug_eq(&(
        error.api_error().unwrap().code,
        &error.api_error().unwrap().message,
        waiting.kind(),
        waiting.attempts(),
        server.count(),
    ));
}

#[tokio::test]
async fn retry_after_http_date_is_included_in_the_deadline() {
    let date = httpdate::fmt_http_date(std::time::SystemTime::now() + Duration::from_secs(60));
    let server = Server::new(vec![(
        503,
        String::new(),
        format!("Retry-After: {date}\r\n"),
    )])
    .await;
    let client = server
        .builder()
        .operation_timeout(Duration::from_millis(30))
        .build()
        .unwrap();
    let error = client
        .v3_get::<Value>("transactions", &())
        .await
        .unwrap_err();
    expect![[r"
        (
            Timeout,
            1,
            1,
        )
    "]]
    .assert_debug_eq(&(error.kind(), error.attempts(), server.count()));
}

#[tokio::test]
async fn deeply_nested_responses_and_typed_decode_errors() {
    let mut nested = "0".to_owned();
    for _ in 0..300 {
        nested = format!("[{nested}]");
    }
    let server = Server::new(vec![
        (200, format!("{{\"decoded\":{nested}}}"), String::new()),
        reply(200, json!({"ok":true,"result":42})),
        reply(200, json!({"padding":"x".repeat(100)})),
    ])
    .await;
    let client = server.builder().build().unwrap();
    let decoded: Value = client.v3_get("messages", &()).await.unwrap();
    let error = client
        .v2_request::<String>(V2Transport::Post, "getAddressBalance", &json!({}))
        .await
        .unwrap_err();
    let limited = server.builder().max_response_bytes(32).build().unwrap();
    let oversized = limited.v3_get::<Value>("messages", &()).await.unwrap_err();
    expect![[r#"
        (
            true,
            Decode,
            Some(
                "response field result",
            ),
            ResponseTooLarge,
        )
    "#]]
    .assert_debug_eq(&(
        decoded["decoded"].is_array(),
        error.kind(),
        error.detail(),
        oversized.kind(),
    ));
}

#[tokio::test]
async fn redirects_are_not_followed_and_debug_omits_credentials() {
    let target = Server::new(vec![]).await;
    let server = Server::new(vec![(
        302,
        String::new(),
        format!("Location: {}/leak\r\n", target.url),
    )])
    .await;
    let builder = server
        .builder()
        .api_key(Some("secret-api".to_owned()))
        .bearer_auth("secret-bearer");
    assert!(!format!("{builder:?}").contains("secret"));
    let client = builder.build().unwrap();
    let error = client
        .v3_get::<Value>("transactions", &())
        .await
        .unwrap_err();
    assert!(!format!("{client:?} {error:?} {error}").contains("secret"));
    expect![[r"
        (
            Http,
            Some(
                302,
            ),
            1,
            0,
        )
    "]]
    .assert_debug_eq(&(error.kind(), error.status(), server.count(), target.count()));
}

#[tokio::test]
async fn cancelled_waiter_does_not_send_a_request() {
    let server = Server::new(vec![reply(200, json!({})); 2]).await;
    let client = server
        .builder()
        .request_interval(Duration::from_millis(60))
        .build()
        .unwrap();
    let _: Value = client.v3_get("transactions", &()).await.unwrap();
    let cancelled = tokio::time::timeout(
        Duration::from_millis(5),
        client.v3_get::<Value>("transactions", &()),
    )
    .await;
    let _: Value = client.v3_get("transactions", &()).await.unwrap();
    expect![[r"
        (
            true,
            2,
        )
    "]]
    .assert_debug_eq(&(cancelled.is_err(), server.count()));
}

#[tokio::test]
async fn invalid_configuration_is_reported_before_network_access() {
    let errors = [
        Client::builder().build().unwrap_err(),
        Client::builder()
            .mainnet()
            .api_key(Some("bad\nheader".to_owned()))
            .build()
            .unwrap_err(),
        Client::builder()
            .mainnet()
            .max_attempts(0)
            .build()
            .unwrap_err(),
    ];
    assert!(
        errors
            .iter()
            .all(|error| error.kind() == ErrorKind::Configuration)
    );
}
