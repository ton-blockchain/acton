use std::collections::{HashMap, VecDeque};
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use axum::body::{Body, BodyDataStream, Bytes};
use axum::extract::Request;
use axum::response::Response;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

pub(crate) const REQUEST_SOURCE_HEADER: &str = "x-acton-request-source";

const STUDIO_UI_REQUEST_SOURCE: &str = "studio-ui";
const MAX_EXTERNAL_API_CALLS: usize = 1_000;
const MAX_STUDIO_UI_API_CALLS: usize = 200;
const MAX_API_CALLS: usize = MAX_EXTERNAL_API_CALLS + MAX_STUDIO_UI_API_CALLS;
const MAX_STORED_BODY_BYTES: usize = 64 * 1024;

#[derive(Clone, Default)]
pub(crate) struct ApiCallLog {
    entries: Arc<Mutex<HashMap<String, ApiCallEntries>>>,
    next_sequence: Arc<AtomicU64>,
}

#[derive(Default)]
struct ApiCallEntries {
    external: VecDeque<ApiCallRecord>,
    studio_ui: VecDeque<ApiCallRecord>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ApiCallRecord {
    pub sequence: u64,
    pub status: ApiCallStatus,
    pub status_code: u16,
    pub source: ApiCallSource,
    pub call_type: ApiCallType,
    pub api_family: ApiCallFamily,
    pub http_method: String,
    pub path: String,
    pub method: String,
    #[schema(value_type = Object, nullable = true)]
    pub request_id: Value,
    #[schema(value_type = Object, nullable = true)]
    pub query_params: Option<Value>,
    #[schema(value_type = Object, nullable = true)]
    pub request_body: Option<Value>,
    pub request_body_truncated: bool,
    #[schema(value_type = Object, nullable = true)]
    pub response_body: Option<Value>,
    pub response_body_truncated: bool,
    pub timestamp_ms: u128,
    pub duration_ns: u128,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallStatus {
    Success,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallType {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallSource {
    External,
    StudioUi,
}

#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallFamily {
    Control,
    Emulate,
    JsonRpc,
    Streaming,
    V2,
    V3,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ApiCallLogSnapshot {
    pub calls: Vec<ApiCallRecord>,
    pub total_retained: usize,
    pub max_retained: usize,
}

#[derive(Default, Deserialize)]
pub(crate) struct GetApiCallsQuery {
    pub(crate) limit: Option<usize>,
}

pub(crate) struct ApiCallCapture {
    log: ApiCallLog,
    environment_id: String,
    started_at: SystemTime,
    duration_start: Instant,
    source: ApiCallSource,
    http_method: String,
    path: String,
    query_params: Option<Value>,
    request_body: Arc<Mutex<CapturedBody>>,
}

#[derive(Default)]
struct CapturedBody {
    bytes: Vec<u8>,
    total_bytes: usize,
    truncated: bool,
}

impl CapturedBody {
    fn push(&mut self, bytes: &[u8]) {
        self.total_bytes = self.total_bytes.saturating_add(bytes.len());
        let remaining = MAX_STORED_BODY_BYTES.saturating_sub(self.bytes.len());
        let visible_len = bytes.len().min(remaining);
        self.bytes.extend_from_slice(&bytes[..visible_len]);
        self.truncated |= visible_len < bytes.len();
    }

    fn value(&self) -> (Option<Value>, bool) {
        (
            stored_body_value(&self.bytes, self.truncated),
            self.truncated,
        )
    }

    fn is_complete(&self, expected_bytes: Option<usize>) -> bool {
        if self.truncated {
            return false;
        }
        expected_bytes.map_or_else(
            || serde_json::from_slice::<Value>(&self.bytes).is_ok(),
            |expected_bytes| self.total_bytes == expected_bytes,
        )
    }
}

impl ApiCallLog {
    pub(crate) fn capture(
        &self,
        environment_id: String,
        path: String,
        request: Request,
    ) -> (Request, ApiCallCapture) {
        let source = request
            .headers()
            .get(REQUEST_SOURCE_HEADER)
            .and_then(|value| value.to_str().ok())
            .filter(|value| value.eq_ignore_ascii_case(STUDIO_UI_REQUEST_SOURCE))
            .map_or(ApiCallSource::External, |_| ApiCallSource::StudioUi);
        let http_method = request.method().as_str().to_owned();
        let query_params = query_params_value(request.uri().query());
        let request_body = Arc::new(Mutex::new(CapturedBody::default()));
        let stream_capture = Arc::clone(&request_body);
        let (parts, body) = request.into_parts();
        let stream = body.into_data_stream().map(move |chunk| {
            if let Ok(bytes) = &chunk {
                stream_capture
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push(bytes);
            }
            chunk
        });
        let request = Request::from_parts(parts, Body::from_stream(stream));
        let capture = ApiCallCapture {
            log: self.clone(),
            environment_id,
            started_at: SystemTime::now(),
            duration_start: Instant::now(),
            source,
            http_method,
            path: if path.is_empty() {
                "/".to_owned()
            } else {
                format!("/{}", path.trim_start_matches('/'))
            },
            query_params,
            request_body,
        };
        (request, capture)
    }

    #[must_use]
    pub(crate) fn snapshot(
        &self,
        environment_id: &str,
        limit: Option<usize>,
    ) -> ApiCallLogSnapshot {
        let environments = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entries) = environments.get(environment_id) else {
            return ApiCallLogSnapshot {
                calls: Vec::new(),
                total_retained: 0,
                max_retained: MAX_API_CALLS,
            };
        };
        let total_retained = entries.external.len() + entries.studio_ui.len();
        let limit = limit.unwrap_or(MAX_API_CALLS).min(MAX_API_CALLS);
        let mut calls = entries
            .external
            .iter()
            .chain(&entries.studio_ui)
            .cloned()
            .collect::<Vec<_>>();
        drop(environments);
        calls.sort_unstable_by_key(|call| call.sequence);
        let calls = calls
            .into_iter()
            .skip(total_retained.saturating_sub(limit))
            .collect();

        ApiCallLogSnapshot {
            calls,
            total_retained,
            max_retained: MAX_API_CALLS,
        }
    }

    pub(crate) fn remove(&self, environment_id: &str) {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(environment_id);
    }

    fn record(&self, environment_id: &str, record: ApiCallRecord) {
        let mut environments = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let entries = environments.entry(environment_id.to_owned()).or_default();
        let (calls, max_calls) = match record.source {
            ApiCallSource::External => (&mut entries.external, MAX_EXTERNAL_API_CALLS),
            ApiCallSource::StudioUi => (&mut entries.studio_ui, MAX_STUDIO_UI_API_CALLS),
        };
        if calls.len() == max_calls {
            calls.pop_front();
        }
        calls.push_back(record);
        drop(environments);
    }

    fn record_response(
        &self,
        environment_id: &str,
        sequence: u64,
        response_body: Option<Value>,
        response_body_truncated: bool,
    ) {
        let mut environments = self
            .entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(entries) = environments.get_mut(environment_id) else {
            return;
        };
        if let Some(call) = entries
            .external
            .iter_mut()
            .chain(&mut entries.studio_ui)
            .find(|call| call.sequence == sequence)
        {
            call.response_body = response_body;
            call.response_body_truncated = response_body_truncated;
        }
        drop(environments);
    }
}

impl ApiCallCapture {
    pub(crate) fn finish(self, response: Response) -> Response {
        let (request_body, request_body_truncated) = self
            .request_body
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .value();
        let (api_family, method, request_id) = classify_api_call(&self.path, request_body.as_ref());
        let response_status = response.status();
        let status_code = response_status.as_u16();
        let response_has_no_body = self.http_method == "HEAD"
            || response_status.is_informational()
            || matches!(
                response_status,
                axum::http::StatusCode::NO_CONTENT | axum::http::StatusCode::NOT_MODIFIED
            );
        let sequence = self.log.next_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let timestamp_ms = self
            .started_at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());
        let record = ApiCallRecord {
            sequence,
            status: if status_code < 400 {
                ApiCallStatus::Success
            } else {
                ApiCallStatus::Failed
            },
            status_code,
            source: self.source,
            call_type: classify_call_type(&self.http_method, api_family, &method),
            api_family,
            http_method: self.http_method,
            path: self.path,
            method,
            request_id,
            query_params: self.query_params,
            request_body,
            request_body_truncated,
            response_body: None,
            response_body_truncated: false,
            timestamp_ms,
            duration_ns: self.duration_start.elapsed().as_nanos(),
        };
        let environment_id = self.environment_id;
        self.log.record(&environment_id, record);
        capture_response_body(
            response,
            self.log,
            environment_id,
            sequence,
            response_has_no_body,
        )
    }
}

fn classify_api_call(path: &str, request_body: Option<&Value>) -> (ApiCallFamily, String, Value) {
    let normalized = path.trim_matches('/');
    let (api_family, mut method) = if matches!(
        normalized,
        "api/v2" | "api/v2/jsonRPC" | "api/v2/v2/jsonRPC"
    ) {
        (ApiCallFamily::JsonRpc, "jsonRPC".to_owned())
    } else if let Some(path) = normalized.strip_prefix("api/v2/") {
        (
            ApiCallFamily::V2,
            path.split('/').next().unwrap_or("v2").to_owned(),
        )
    } else if let Some(path) = normalized.strip_prefix("api/v3/") {
        (
            ApiCallFamily::V3,
            path.split('/').next().unwrap_or("v3").to_owned(),
        )
    } else if let Some(path) = normalized.strip_prefix("api/emulate/") {
        (
            ApiCallFamily::Emulate,
            path.split('/').next_back().unwrap_or("emulate").to_owned(),
        )
    } else if let Some(path) = normalized.strip_prefix("api/streaming/") {
        (ApiCallFamily::Streaming, format!("streaming/{path}"))
    } else {
        (
            ApiCallFamily::Control,
            normalized
                .rsplit('/')
                .next()
                .filter(|method| !method.is_empty())
                .unwrap_or("control")
                .to_owned(),
        )
    };
    let request_id = if matches!(api_family, ApiCallFamily::JsonRpc)
        && let Some(body) = request_body.and_then(Value::as_object)
    {
        if let Some(json_rpc_method) = body.get("method").and_then(Value::as_str) {
            json_rpc_method.clone_into(&mut method);
        }
        body.get("id").cloned().unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    (api_family, method, request_id)
}

fn classify_call_type(http_method: &str, api_family: ApiCallFamily, method: &str) -> ApiCallType {
    if matches!(http_method, "GET" | "HEAD" | "OPTIONS") {
        return ApiCallType::Read;
    }
    match api_family {
        ApiCallFamily::JsonRpc if matches!(method, "sendBoc" | "sendBocReturnHash") => {
            ApiCallType::Write
        }
        ApiCallFamily::JsonRpc | ApiCallFamily::Emulate | ApiCallFamily::Streaming => {
            ApiCallType::Read
        }
        ApiCallFamily::V2 if matches!(method, "runGetMethod" | "runGetMethodStd") => {
            ApiCallType::Read
        }
        ApiCallFamily::V3 if matches!(method, "estimateFee" | "runGetMethod") => ApiCallType::Read,
        ApiCallFamily::Control if method == "acton_buildSourceTrace" => ApiCallType::Read,
        ApiCallFamily::Control | ApiCallFamily::V2 | ApiCallFamily::V3 => ApiCallType::Write,
    }
}

fn query_params_value(query: Option<&str>) -> Option<Value> {
    let query = query?;
    let mut params = serde_json::Map::new();
    for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
        let value = Value::String(value.into_owned());
        match params.entry(key.into_owned()) {
            serde_json::map::Entry::Vacant(entry) => {
                entry.insert(value);
            }
            serde_json::map::Entry::Occupied(mut entry) => match entry.get_mut() {
                Value::Array(values) => values.push(value),
                previous => {
                    *previous = Value::Array(vec![std::mem::take(previous), value]);
                }
            },
        }
    }
    Some(Value::Object(params))
}

fn stored_body_value(bytes: &[u8], truncated: bool) -> Option<Value> {
    if bytes.is_empty() {
        return None;
    }
    Some(if truncated {
        Value::String(String::from_utf8_lossy(bytes).into_owned())
    } else {
        serde_json::from_slice(bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(bytes).into_owned()))
    })
}

fn capture_response_body(
    response: Response,
    log: ApiCallLog,
    environment_id: String,
    sequence: u64,
    response_has_no_body: bool,
) -> Response {
    let expected_body_bytes = if response_has_no_body {
        Some(0)
    } else {
        response
            .headers()
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse().ok())
    };
    let (parts, body) = response.into_parts();
    let stream = ApiCallResponseStream {
        inner: body.into_data_stream(),
        capture: Some(ApiCallResponseCapture {
            log,
            environment_id,
            sequence,
            expected_body_bytes,
            body: CapturedBody::default(),
        }),
    };
    Response::from_parts(parts, Body::from_stream(stream))
}

struct ApiCallResponseCapture {
    log: ApiCallLog,
    environment_id: String,
    sequence: u64,
    expected_body_bytes: Option<usize>,
    body: CapturedBody,
}

impl ApiCallResponseCapture {
    fn finish(mut self, incomplete: bool) {
        let complete = self.body.is_complete(self.expected_body_bytes);
        self.body.truncated |= incomplete && !complete;
        let (response_body, response_body_truncated) = self.body.value();
        self.log.record_response(
            &self.environment_id,
            self.sequence,
            response_body,
            response_body_truncated,
        );
    }
}

struct ApiCallResponseStream {
    inner: BodyDataStream,
    capture: Option<ApiCallResponseCapture>,
}

impl ApiCallResponseStream {
    fn finish(&mut self, incomplete: bool) {
        if let Some(capture) = self.capture.take() {
            capture.finish(incomplete);
        }
    }
}

impl Stream for ApiCallResponseStream {
    type Item = Result<Bytes, axum::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.inner).poll_next(cx);
        match &result {
            Poll::Ready(Some(Ok(bytes))) => {
                if let Some(capture) = this.capture.as_mut() {
                    capture.body.push(bytes);
                }
            }
            Poll::Ready(Some(Err(_))) => this.finish(true),
            Poll::Ready(None) => this.finish(false),
            Poll::Pending => {}
        }
        result
    }
}

impl Drop for ApiCallResponseStream {
    fn drop(&mut self) {
        self.finish(true);
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use super::*;

    #[test]
    fn api_call_sources_have_independent_retention_limits_per_environment() {
        let log = ApiCallLog::default();
        let mut sequence = 0;

        for source in [
            ApiCallSource::External,
            ApiCallSource::StudioUi,
            ApiCallSource::External,
        ] {
            let entries = match source {
                ApiCallSource::External => MAX_EXTERNAL_API_CALLS,
                ApiCallSource::StudioUi => MAX_STUDIO_UI_API_CALLS,
            };
            for _ in 0..entries {
                sequence += 1;
                log.record(
                    "environment-1",
                    ApiCallRecord {
                        sequence,
                        status: ApiCallStatus::Success,
                        status_code: 200,
                        source,
                        call_type: ApiCallType::Read,
                        api_family: ApiCallFamily::V3,
                        http_method: "GET".to_owned(),
                        path: "/api/v3/blocks".to_owned(),
                        method: "blocks".to_owned(),
                        request_id: Value::Null,
                        query_params: None,
                        request_body: None,
                        request_body_truncated: false,
                        response_body: None,
                        response_body_truncated: false,
                        timestamp_ms: 0,
                        duration_ns: 0,
                    },
                );
            }
        }

        let snapshot = log.snapshot("environment-1", None);
        let external_count = snapshot
            .calls
            .iter()
            .filter(|call| matches!(call.source, ApiCallSource::External))
            .count();

        assert_eq!(external_count, MAX_EXTERNAL_API_CALLS);
        assert_eq!(
            snapshot.calls.len() - external_count,
            MAX_STUDIO_UI_API_CALLS
        );
        assert_eq!(snapshot.total_retained, MAX_API_CALLS);
        assert_eq!(snapshot.max_retained, MAX_API_CALLS);
        assert_eq!(log.snapshot("environment-2", None).total_retained, 0);
    }

    #[tokio::test]
    async fn complete_json_response_is_not_truncated_when_transport_ends_by_content_length() {
        const RESPONSE: &[u8] = br#"{"balance":"0","code":null,"data":null,"last_transaction_lt":"0","last_transaction_hash":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=","frozen_hash":null,"status":"uninit"}"#;

        let log = ApiCallLog::default();
        let (request, capture) = log.capture(
            "environment-1".to_owned(),
            "api/v3/addressInformation".to_owned(),
            Request::get("/")
                .body(Body::empty())
                .expect("request must be valid"),
        );
        drop(request);
        let body =
            futures::stream::once(async { Ok::<_, Infallible>(Bytes::from_static(RESPONSE)) })
                .chain(futures::stream::pending());
        let response = Response::builder()
            .header(axum::http::header::CONTENT_LENGTH, RESPONSE.len())
            .body(Body::from_stream(body))
            .expect("response must be valid");
        let response = capture.finish(response);

        let mut body = response.into_body().into_data_stream();
        assert_eq!(body.next().await.unwrap().unwrap(), RESPONSE);
        drop(body);

        let snapshot = log.snapshot("environment-1", None);
        let call = snapshot.calls.first().expect("API call must be recorded");
        assert_eq!(call.response_body.as_ref().unwrap()["status"], "uninit");
        assert!(!call.response_body_truncated);
    }

    #[test]
    fn bodyless_response_is_not_truncated_when_the_client_does_not_poll_it() {
        let log = ApiCallLog::default();
        let (request, capture) = log.capture(
            "environment-1".to_owned(),
            "acton_mine".to_owned(),
            Request::post("/")
                .body(Body::empty())
                .expect("request must be valid"),
        );
        drop(request);
        drop(
            capture.finish(
                Response::builder()
                    .status(axum::http::StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .expect("response must be valid"),
            ),
        );

        let snapshot = log.snapshot("environment-1", None);
        let call = snapshot.calls.first().expect("API call must be recorded");
        assert!(call.response_body.is_none());
        assert!(!call.response_body_truncated);
    }
}
