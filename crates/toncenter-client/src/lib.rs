#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod api;
mod config;
mod decode;
mod error;
mod rate_limit;

pub use api::{V2, V3};
pub use config::ClientBuilder;
pub use error::{ApiError, Error, ErrorKind};
pub use toncenter;

use std::sync::Arc;
use std::time::{Duration, SystemTime};

use rand::Rng;
use reqwest::{Method, Request};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Semaphore;
use tokio::time::Instant;
use toncenter::{v2, v3};

use config::Endpoint;

/// Transport used for a v2 operation; successful calls return the same typed result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V2Transport {
    /// Parameters in the URL query. Use only methods that support GET.
    Get,
    /// Parameters as a JSON body on the method's REST route.
    Post,
    /// Parameters inside a JSON-RPC request sent to `/jsonRPC`.
    JsonRpc,
}

/// Reusable asynchronous client for independently configured v2 and v3 endpoints.
///
/// Dropping a call's future cancels its quota wait, active request, and further retries.
/// Clones share connections and limits; they do not create additional quota.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    v2: Option<Endpoint>,
    v3: Option<Endpoint>,
    config: ClientBuilder,
    permits: Semaphore,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.inner.config)
            .finish()
    }
}

impl Client {
    /// Starts configuration without selecting an API endpoint.
    #[must_use]
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Whether calls include `X-API-Key`. This describes configuration, not whether
    /// the server accepts the key or its subscription has remaining quota.
    #[must_use]
    pub fn has_api_key(&self) -> bool {
        self.inner.config.api_key.is_some()
    }

    /// Accesses named v2 operations. Queries use GET when the endpoint supports it;
    /// operations with JSON bodies use POST. Use `V2::transport` to select JSON-RPC.
    #[must_use]
    pub const fn v2(&self) -> V2<'_> {
        V2 {
            client: self,
            transport: None,
        }
    }

    /// Accesses named v3 operations with their declared HTTP methods.
    #[must_use]
    pub const fn v3(&self) -> V3<'_> {
        V3 { client: self }
    }

    /// Calls a typed v2 endpoint using its default REST method and unwraps its successful envelope.
    pub async fn call_v2<E: v2::endpoints::Endpoint>(
        &self,
        params: &E::Request,
    ) -> Result<E::Response, Error>
    where
        E::Request: Sync,
    {
        self.v2().call::<E>(params).await
    }

    /// Calls a typed v3 endpoint with its declared HTTP method and direct result type.
    pub async fn call_v3<E: v3::endpoints::Endpoint>(
        &self,
        params: &E::Request,
    ) -> Result<E::Response, Error>
    where
        E::Request: Sync,
    {
        let path = E::PATH
            .strip_prefix("/api/v3/")
            .ok_or_else(|| Error::configuration("v3 endpoint must use an /api/v3/ path"))?;
        match E::METHOD {
            v3::endpoints::HttpMethod::Get => self.v3_get(path, params).await,
            v3::endpoints::HttpMethod::Post => self.v3_post(path, params).await,
        }
    }

    /// Calls v2 with an explicit transport and response type.
    /// Use this for JSON-RPC or explicitly selected simulator stack extensions.
    pub async fn v2_request<T: DeserializeOwned>(
        &self,
        transport: V2Transport,
        method: &str,
        params: &(impl Serialize + Sync + ?Sized),
    ) -> Result<T, Error> {
        Ok(self
            .v2_response::<T>(transport, method, params)
            .await?
            .result)
    }

    /// Calls v2 while retaining successful envelope metadata such as `@extra`.
    /// API errors, including errors returned with HTTP 200, become [`Error`].
    pub async fn v2_response<T: DeserializeOwned>(
        &self,
        transport: V2Transport,
        method: &str,
        params: &(impl Serialize + Sync + ?Sized),
    ) -> Result<v2::TonlibResponse<T>, Error> {
        validate_path(method, false)?;
        let endpoint = self
            .inner
            .v2
            .as_ref()
            .ok_or_else(|| Error::configuration("v2 URL is not configured"))?;
        let mut url = endpoint
            .url
            .join(method)
            .map_err(|_| Error::configuration("invalid v2 method"))?;
        let request = match transport {
            V2Transport::Get => {
                let query = serde_html_form::to_string(params).map_err(|_| {
                    Error::configuration(
                        "v2 query parameters must be scalars or repeated scalar values",
                    )
                })?;
                url.set_query((!query.is_empty()).then_some(&query));
                self.inner.http.get(url)
            }
            V2Transport::Post => self.inner.http.post(url).json(params),
            V2Transport::JsonRpc => {
                url.set_path(&format!("{}jsonRPC", endpoint.url.path()));
                self.inner.http.post(url).json(&serde_json::json!({
                    "jsonrpc": "2.0", "id": "1", "method": method, "params": params,
                }))
            }
        }
        .build()
        .map_err(|error| {
            Error::configuration("cannot serialize v2 request").with_source(error.without_url())
        })?;
        let read_only = v2::endpoints::METHODS.contains(&method)
            && !matches!(
                method,
                "sendBoc"
                    | "sendBocReturnHash"
                    | "sendBocReturnHashNoError"
                    | "sendQuery"
                    | "createQuery"
            );
        self.execute(
            endpoint,
            request,
            method,
            read_only
                || (self.inner.config.retry_broadcasts
                    && matches!(
                        method,
                        "sendBoc" | "sendBocReturnHash" | "sendBocReturnHashNoError" | "sendQuery"
                    )),
        )
        .await
    }

    /// Queries a v3 route, decoding the application's chosen response type.
    /// Arrays become repeated query parameters; absent optional values are omitted.
    pub async fn v3_get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &(impl Serialize + Sync + ?Sized),
    ) -> Result<T, Error> {
        self.v3_request(Method::GET, path, params).await
    }

    /// Sends a JSON body to a v3 route. Fee estimation and get methods may retry;
    /// message broadcasts require explicit opt-in to retry uncertain delivery.
    pub async fn v3_post<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &(impl Serialize + Sync + ?Sized),
    ) -> Result<T, Error> {
        self.v3_request(Method::POST, path, params).await
    }

    async fn v3_request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        params: &(impl Serialize + Sync + ?Sized),
    ) -> Result<T, Error> {
        validate_path(path, true)?;
        let endpoint = self
            .inner
            .v3
            .as_ref()
            .ok_or_else(|| Error::configuration("v3 URL is not configured"))?;
        let mut url = endpoint
            .url
            .join(path)
            .map_err(|_| Error::configuration("invalid v3 path"))?;
        let read_only = method == Method::GET || matches!(path, "runGetMethod" | "estimateFee");
        let request = if method == Method::GET {
            let query = serde_html_form::to_string(params).map_err(|_| {
                Error::configuration(
                    "v3 query parameters must be scalars or repeated scalar values",
                )
            })?;
            url.set_query((!query.is_empty()).then_some(&query));
            self.inner.http.get(url)
        } else {
            self.inner.http.post(url).json(params)
        }
        .build()
        .map_err(|error| {
            Error::configuration("cannot serialize v3 request").with_source(error.without_url())
        })?;
        self.execute(
            endpoint,
            request,
            path,
            read_only || (self.inner.config.retry_broadcasts && path == "message"),
        )
        .await
    }

    async fn execute<T: DeserializeOwned>(
        &self,
        endpoint: &Endpoint,
        request: Request,
        operation: &str,
        read_only: bool,
    ) -> Result<T, Error> {
        let mut progress = Progress {
            operation,
            target: endpoint.url.as_str(),
            started: std::time::Instant::now(),
            outcome: "cancelled",
            attempts: 0,
        };
        let config = &self.inner.config;
        let attempts = if read_only { config.max_attempts } else { 1 };
        let result = tokio::time::timeout(config.operation_timeout, async {
            let _permit = self.inner.permits.acquire().await
                .map_err(|_| Error::new(ErrorKind::Transport, operation))?;
            for attempt in 1..=attempts {
                endpoint.gate.acquire().await;
                progress.attempts = attempt;
                tracing::debug!(operation, target = progress.target, attempt, outcome = "start", "TON Center request");
                let current = request.try_clone()
                    .ok_or_else(|| Error::configuration("request body is not replayable"))?;
                let (result, retry_after) = self.attempt(current, operation).await;
                match result {
                    Ok(result) => return Ok(result),
                    Err(mut error) => {
                        error.attempts = attempt;
                        let rate_limited = error.status == Some(429)
                            || error.api.as_ref().is_some_and(|api| api.code == Some(429));
                        let backoff = config.retry_delay.saturating_mul(1 << (attempt - 1).min(20))
                            .min(config.max_retry_delay);
                        let backoff = backoff.mul_f64(rand::thread_rng().gen_range(0.5..=1.0));
                        let delay = retry_after.unwrap_or(backoff).max(
                            if rate_limited { Duration::from_millis(1100) } else { Duration::ZERO },
                        );
                        let until = Instant::now().checked_add(delay);
                        if rate_limited && let Some(until) = until {
                            endpoint.gate.postpone(until);
                        }
                        if attempt == attempts || !retryable(&error) || until.is_none() {
                            return Err(error);
                        }
                        tracing::debug!(operation, target = progress.target, attempt, delay_ms = delay.as_millis(), outcome = "retry", kind = ?error.kind, status = error.status, "TON Center retry");
                        if let Some(until) = until {
                            tokio::time::sleep_until(until).await;
                        }
                    }
                }
            }
            unreachable!("positive attempt budget always returns on its final attempt")
        }).await;
        let result = result.unwrap_or_else(|_| {
            let mut error = Error::new(ErrorKind::Timeout, operation);
            error.attempts = progress.attempts;
            Err(error)
        });
        progress.outcome = if result.is_ok() { "success" } else { "error" };
        result
    }

    async fn attempt<T: DeserializeOwned>(
        &self,
        request: Request,
        operation: &str,
    ) -> (Result<T, Error>, Option<Duration>) {
        let mut response = match self.inner.http.execute(request).await {
            Ok(response) => response,
            Err(error) => return (Err(transport_error(error, operation)), None),
        };
        let status = response.status();
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| {
                value
                    .parse::<u64>()
                    .map(Duration::from_secs)
                    .ok()
                    .or_else(|| {
                        httpdate::parse_http_date(value)
                            .ok()
                            .map(|date| date.duration_since(SystemTime::now()).unwrap_or_default())
                    })
            });
        let mut body = Vec::new();
        let limit = self.inner.config.max_response_bytes;
        if response
            .content_length()
            .is_some_and(|size| size > limit as u64)
        {
            let mut error = Error::new(ErrorKind::ResponseTooLarge, operation);
            error.status = Some(status.as_u16());
            return (Err(error), retry_after);
        }
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) if chunk.len() <= limit.saturating_sub(body.len()) => {
                    body.extend_from_slice(&chunk)
                }
                Ok(Some(_)) => {
                    let mut error = Error::new(ErrorKind::ResponseTooLarge, operation);
                    error.status = Some(status.as_u16());
                    return (Err(error), retry_after);
                }
                Ok(None) => break,
                Err(error) => {
                    let mut error = transport_error(error, operation);
                    error.status = Some(status.as_u16());
                    return (Err(error), retry_after);
                }
            }
        }
        let result = decode::response(&body, status, operation);
        (result, retry_after)
    }
}

fn validate_path(path: &str, nested: bool) -> Result<(), Error> {
    if path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || (nested && byte == b'/'))
        || path.contains("//")
    {
        return Err(Error::configuration(
            "operation must be a relative API path without query or traversal segments",
        ));
    }
    Ok(())
}

fn transport_error(error: reqwest::Error, operation: &str) -> Error {
    let kind = if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    };
    Error::new(kind, operation).with_source(error.without_url())
}

fn retryable(error: &Error) -> bool {
    match error.kind {
        ErrorKind::Transport | ErrorKind::Timeout => true,
        ErrorKind::Http => error.status.is_some_and(transient_status),
        ErrorKind::Api => error.api.as_ref().is_some_and(|api| {
            api.code
                .is_some_and(|code| matches!(code, 408 | 429 | 502 | 503 | 504))
        }),
        _ => false,
    }
}

const fn transient_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504)
}

struct Progress<'a> {
    operation: &'a str,
    target: &'a str,
    started: std::time::Instant,
    outcome: &'static str,
    attempts: u32,
}

impl Drop for Progress<'_> {
    fn drop(&mut self) {
        tracing::debug!(
            operation = self.operation,
            target = self.target,
            duration_ms = self.started.elapsed().as_millis(),
            attempts = self.attempts,
            outcome = self.outcome,
            "TON Center call completed"
        );
    }
}
