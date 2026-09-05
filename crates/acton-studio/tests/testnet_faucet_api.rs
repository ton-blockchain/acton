use std::sync::{Arc, Mutex};

use acton_studio::{StudioServer, StudioServerConfig};
use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request as AxumRequest, State};
use axum::http::{Request, Response, StatusCode};
use axum::routing::any;
use expect_test::expect;
use tower::ServiceExt;

#[tokio::test]
async fn guest_testnet_faucet_proxy_forwards_only_supported_requests() {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let upstream = Router::new()
        .route("/{*path}", any(faucet_upstream))
        .with_state(Arc::clone(&captured));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("faucet listener must bind");
    let address = listener
        .local_addr()
        .expect("faucet listener address must be available");
    let serving = tokio::spawn(async move {
        axum::serve(listener, upstream)
            .await
            .expect("faucet fixture must serve");
    });
    let faucet_url = reqwest::Url::parse(&format!("http://{address}/"))
        .expect("faucet fixture URL must be valid");
    let app = StudioServer::new(
        StudioServerConfig::new("1.2.3-test").with_testnet_faucet_url(faucet_url),
    )
    .router();

    let status = app
        .clone()
        .oneshot(
            Request::get("/api/v1/testnet-faucet/auth/status")
                .header("x-device-uid", "0123456789abcdef0123456789abcdef")
                .body(Body::empty())
                .expect("status request must be valid"),
        )
        .await
        .expect("status request must succeed");
    let challenge = app
        .clone()
        .oneshot(
            Request::post("/api/v1/testnet-faucet/challenge")
                .header("content-type", "application/json")
                .header("authorization", "Bearer must-not-be-forwarded")
                .header("x-device-uid", "0123456789abcdef0123456789abcdef")
                .body(Body::from(r#"{"address":"0:test","type":1}"#))
                .expect("challenge request must be valid"),
        )
        .await
        .expect("challenge request must succeed");
    let claim = app
        .clone()
        .oneshot(
            Request::post("/api/v1/testnet-faucet/claim")
                .header("content-type", "application/json")
                .header("x-device-uid", "0123456789abcdef0123456789abcdef")
                .body(Body::from(r#"{"challenge":"test","nonce":0}"#))
                .expect("claim request must be valid"),
        )
        .await
        .expect("claim request must succeed");
    let missing_identity = app
        .oneshot(
            Request::get("/api/v1/testnet-faucet/auth/status")
                .body(Body::empty())
                .expect("invalid request must be valid HTTP"),
        )
        .await
        .expect("invalid request must receive a response");

    let actual = format!(
        "STATUS\n{}\n\nCHALLENGE\n{}\n\nCLAIM\n{}\n\nMISSING IDENTITY\n{}\n\nUPSTREAM\n{}",
        response_snapshot(status).await,
        response_snapshot(challenge).await,
        response_snapshot(claim).await,
        response_snapshot(missing_identity).await,
        captured
            .lock()
            .expect("captured request lock must not be poisoned")
            .join("\n\n"),
    );

    expect![[r#"
        STATUS
        status: 200 OK
        content-type: application/json
        retry-after: <missing>
        body: {"enabled":true,"guestMaxRequests":2,"verifiedMaxRequests":4,"establishedMaxRequests":8,"windowSeconds":3600}

        CHALLENGE
        status: 200 OK
        content-type: application/json
        retry-after: <missing>
        body: {"version":1,"challenge":"studio-test","difficulty":0,"max_solve_ttl_seconds":30,"max_nonce_attempts":100}

        CLAIM
        status: 429 Too Many Requests
        content-type: application/json
        retry-after: 60
        body: {"error":"Guest limit reached"}

        MISSING IDENTITY
        status: 400 Bad Request
        content-type: application/json
        retry-after: <missing>
        body: {"error":{"code":"testnet_faucet_invalid_request","message":"Missing Studio device identity"}}

        UPSTREAM
        GET /auth/status
        user-agent: acton/1.2.3-test
        x-device-uid: 0123456789abcdef0123456789abcdef
        authorization: <missing>
        body: <empty>

        POST /challenge
        user-agent: acton/1.2.3-test
        x-device-uid: 0123456789abcdef0123456789abcdef
        authorization: <missing>
        body: {"address":"0:test","type":1}

        POST /claim
        user-agent: acton/1.2.3-test
        x-device-uid: 0123456789abcdef0123456789abcdef
        authorization: <missing>
        body: {"challenge":"test","nonce":0}"#]]
    .assert_eq(&actual);

    serving.abort();
}

async fn faucet_upstream(
    State(captured): State<Arc<Mutex<Vec<String>>>>,
    request: AxumRequest,
) -> Response<Body> {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let headers = request.headers().clone();
    let body = to_bytes(request.into_body(), 16 * 1024)
        .await
        .expect("fixture request body must be readable");
    let body = if body.is_empty() {
        "<empty>".to_owned()
    } else {
        String::from_utf8_lossy(&body).into_owned()
    };
    captured
        .lock()
        .expect("captured request lock must not be poisoned")
        .push(format!(
            "{method} {path}\nuser-agent: {}\nx-device-uid: {}\nauthorization: {}\nbody: {}",
            header(&headers, "user-agent"),
            header(&headers, "x-device-uid"),
            header(&headers, "authorization"),
            body,
        ));

    match path.as_str() {
        "/auth/status" => json_response(
            StatusCode::OK,
            r#"{"enabled":true,"guestMaxRequests":2,"verifiedMaxRequests":4,"establishedMaxRequests":8,"windowSeconds":3600}"#,
        ),
        "/challenge" => json_response(
            StatusCode::OK,
            r#"{"version":1,"challenge":"studio-test","difficulty":0,"max_solve_ttl_seconds":30,"max_nonce_attempts":100}"#,
        ),
        "/claim" => Response::builder()
            .status(StatusCode::TOO_MANY_REQUESTS)
            .header("content-type", "application/json")
            .header("retry-after", "60")
            .body(Body::from(r#"{"error":"Guest limit reached"}"#))
            .expect("fixture claim response must be valid"),
        _ => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#),
    }
}

fn json_response(status: StatusCode, body: &'static str) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .expect("fixture response must be valid")
}

fn header<'a>(headers: &'a axum::http::HeaderMap, name: &'static str) -> &'a str {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<missing>")
}

async fn response_snapshot(response: Response<Body>) -> String {
    let status = response.status();
    let content_type = header(response.headers(), "content-type").to_owned();
    let retry_after = header(response.headers(), "retry-after").to_owned();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body must be readable");
    let body = if body.is_empty() {
        "<empty>".to_owned()
    } else {
        String::from_utf8_lossy(&body).into_owned()
    };

    format!(
        "status: {status}\ncontent-type: {content_type}\nretry-after: {retry_after}\nbody: {body}",
    )
}
