use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;

use reqwest::Url;
use reqwest::header::{HeaderMap, HeaderValue, ORIGIN, USER_AGENT};
use tokio::sync::Semaphore;

use crate::rate_limit::Gate;
use crate::{Client, Error, Inner};

/// Configuration for an asynchronous client. Call `mainnet`, `testnet`, or set
/// version-specific URLs before building. A client may support only one API version.
///
/// Cloning a builder copies configuration; cloning a client also shares connections.
#[derive(Clone)]
pub struct ClientBuilder {
    v2_url: Option<String>,
    v3_url: Option<String>,
    pub(crate) api_key: Option<String>,
    bearer_token: Option<String>,
    origin: Option<String>,
    user_agent: String,
    system_proxy: bool,
    pub(crate) connect_timeout: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) operation_timeout: Duration,
    pub(crate) max_attempts: u32,
    pub(crate) retry_delay: Duration,
    pub(crate) max_retry_delay: Duration,
    pub(crate) retry_broadcasts: bool,
    pub(crate) max_response_bytes: usize,
    max_concurrent_requests: usize,
    interval: Option<Duration>,
    group: Option<String>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            v2_url: None,
            v3_url: None,
            api_key: None,
            bearer_token: None,
            origin: None,
            user_agent: concat!("toncenter-client/", env!("CARGO_PKG_VERSION")).to_owned(),
            system_proxy: true,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            operation_timeout: Duration::from_secs(90),
            max_attempts: 3,
            retry_delay: Duration::from_millis(500),
            max_retry_delay: Duration::from_secs(5),
            retry_broadcasts: false,
            max_response_bytes: 16 * 1024 * 1024,
            max_concurrent_requests: 16,
            interval: None,
            group: None,
        }
    }
}

impl std::fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("v2_configured", &self.v2_url.is_some())
            .field("v3_configured", &self.v3_url.is_some())
            .field("authenticated", &self.api_key.is_some())
            .finish_non_exhaustive()
    }
}

impl ClientBuilder {
    /// Selects the public mainnet endpoints for both API versions.
    #[must_use]
    pub fn mainnet(self) -> Self {
        self.v2_url("https://toncenter.com/api/v2")
            .v3_url("https://toncenter.com/api/v3")
    }

    /// Selects the public testnet endpoints for both API versions.
    #[must_use]
    pub fn testnet(self) -> Self {
        self.v2_url("https://testnet.toncenter.com/api/v2")
            .v3_url("https://testnet.toncenter.com/api/v3")
    }

    /// Sets the complete v2 base URL, including its API path prefix.
    #[must_use]
    pub fn v2_url(mut self, url: impl Into<String>) -> Self {
        self.v2_url = Some(url.into());
        self
    }

    /// Sets the complete v3 base URL independently of the v2 host and path.
    #[must_use]
    pub fn v3_url(mut self, url: impl Into<String>) -> Self {
        self.v3_url = Some(url.into());
        self
    }

    /// Sends this key in `X-API-Key` on both API versions and every retry.
    ///
    /// Pass `None` to clear the key. The library does not read credentials from the environment.
    /// The key is marked sensitive and omitted from client debug output.
    #[must_use]
    pub fn api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key;
        self
    }

    /// Adds bearer authentication for a private gateway in front of the API.
    /// This header can be used together with `X-API-Key`; both are marked sensitive.
    #[must_use]
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Sets the `Origin` header on every request, including retries.
    /// Invalid HTTP header values are rejected by `build`.
    #[must_use]
    pub fn origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// Replaces the default `toncenter-client/<version>` User-Agent in full.
    #[must_use]
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Controls whether the HTTP transport reads system proxy settings. Defaults to true.
    #[must_use]
    pub const fn system_proxy(mut self, enabled: bool) -> Self {
        self.system_proxy = enabled;
        self
    }

    /// Limits connection establishment for each attempt. Defaults to 10 seconds.
    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Limits one HTTP attempt, including reading its response body. Defaults to 30 seconds.
    /// Quota waits and backoff are covered by `operation_timeout` instead.
    #[must_use]
    pub const fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Limits a complete call, including quota waits, retries, and response reads.
    /// Defaults to 90 seconds. Expiration returns `ErrorKind::Timeout` without another retry.
    #[must_use]
    pub const fn operation_timeout(mut self, timeout: Duration) -> Self {
        self.operation_timeout = timeout;
        self
    }

    /// Sets the total attempt budget, including the first attempt; must be positive.
    ///
    /// Defaults to 3. A value of 1 disables retries. Broadcasts still receive one
    /// attempt unless `retry_broadcasts(true)` is configured.
    #[must_use]
    pub const fn max_attempts(mut self, attempts: u32) -> Self {
        self.max_attempts = attempts;
        self
    }

    /// Sets exponential retry delays before jitter; `Retry-After` takes precedence.
    ///
    /// Defaults to 500 ms initially and 5 seconds maximum. Each failed attempt
    /// doubles the delay up to the maximum; jitter selects 50% to 100% of that value.
    /// Zero durations allow immediate retries except for shared 429 cooldowns.
    #[must_use]
    pub const fn retry_delays(mut self, initial: Duration, maximum: Duration) -> Self {
        self.retry_delay = initial;
        self.max_retry_delay = maximum;
        self
    }

    /// Allows retries of broadcasts after an uncertain delivery outcome.
    /// Retries send exactly the same body; they never create or sign a new message.
    /// The default is false, so broadcasts receive one attempt.
    #[must_use]
    pub const fn retry_broadcasts(mut self, enabled: bool) -> Self {
        self.retry_broadcasts = enabled;
        self
    }

    /// Limits response bytes before JSON decoding, including chunked responses.
    ///
    /// Defaults to 16 MiB. Exceeding the limit returns `ErrorKind::ResponseTooLarge`
    /// without retrying. Increase this for larger trace or block queries.
    #[must_use]
    pub const fn max_response_bytes(mut self, bytes: usize) -> Self {
        self.max_response_bytes = bytes;
        self
    }

    /// Limits concurrent calls across client clones; waiting counts toward the deadline.
    ///
    /// Defaults to 16. Must be positive and fit Tokio's semaphore capacity.
    /// A call holds its permit during quota waits and retries.
    #[must_use]
    pub const fn max_concurrent_requests(mut self, count: usize) -> Self {
        self.max_concurrent_requests = count;
        self
    }

    /// Overrides request spacing. Zero disables normal pacing, but preserves 429 cooldowns.
    ///
    /// Unauthenticated public endpoints use 1100 ms by default; other endpoints use zero.
    /// Shared groups retain the greatest configured interval for the lifetime of the group.
    #[must_use]
    pub const fn request_interval(mut self, interval: Duration) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Shares pacing and 429 cooldowns with clients using the same process-local group.
    /// Use a non-secret group name for endpoints or keys that share a server quota.
    #[must_use]
    pub fn quota_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Validates configuration and creates a reusable asynchronous HTTP client.
    /// Redirects are disabled so credentials and application headers stay on configured endpoints.
    pub fn build(self) -> Result<Client, Error> {
        if self.v2_url.is_none() && self.v3_url.is_none() {
            return Err(Error::configuration("configure at least one API base URL"));
        }
        if self.max_attempts == 0
            || self.max_concurrent_requests == 0
            || self.max_response_bytes == 0
        {
            return Err(Error::configuration(
                "attempt, concurrency, and response-size limits must be positive",
            ));
        }
        if self.max_concurrent_requests > Semaphore::MAX_PERMITS {
            return Err(Error::configuration(
                "concurrency limit exceeds Tokio's maximum",
            ));
        }

        let v2 = self
            .v2_url
            .as_deref()
            .map(|url| self.endpoint(url))
            .transpose()?;
        let v3 = self
            .v3_url
            .as_deref()
            .map(|url| self.endpoint(url))
            .transpose()?;
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, header(&self.user_agent, "User-Agent")?);
        if let Some(origin) = &self.origin {
            headers.insert(ORIGIN, header(origin, "Origin")?);
        }
        if let Some(key) = &self.api_key {
            let mut value = header(key, "X-API-Key")?;
            value.set_sensitive(true);
            headers.insert("x-api-key", value);
        }
        if let Some(token) = &self.bearer_token {
            let mut value = header(&format!("Bearer {token}"), "Authorization")?;
            value.set_sensitive(true);
            headers.insert(reqwest::header::AUTHORIZATION, value);
        }
        let mut http = reqwest::Client::builder()
            .use_rustls_tls()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(self.connect_timeout)
            .timeout(self.request_timeout)
            .default_headers(headers);
        if !self.system_proxy {
            http = http.no_proxy();
        }
        let http = http.build().map_err(|error| {
            Error::configuration("cannot create HTTP client").with_source(error.without_url())
        })?;
        let permits = Semaphore::new(self.max_concurrent_requests);
        Ok(Client {
            inner: Arc::new(Inner {
                http,
                v2,
                v3,
                config: self,
                permits,
            }),
        })
    }

    fn endpoint(&self, value: &str) -> Result<Endpoint, Error> {
        let mut url =
            Url::parse(value).map_err(|_| Error::configuration("invalid API base URL"))?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Error::configuration(
                "API base URL must use HTTP(S), without credentials, query, or fragment",
            ));
        }
        let path = format!("{}/", url.path().trim_end_matches('/'));
        url.set_path(&path);
        let public = matches!(
            url.host_str(),
            Some("toncenter.com" | "testnet.toncenter.com")
        );
        let interval = self.interval.unwrap_or_else(|| {
            if public && self.api_key.is_none() {
                Duration::from_millis(1100)
            } else {
                Duration::ZERO
            }
        });
        let group = self.group.as_ref().map_or_else(
            || {
                let mut hasher = DefaultHasher::new();
                self.api_key.hash(&mut hasher);
                format!(
                    "endpoint:{}:{}",
                    url.origin().ascii_serialization(),
                    hasher.finish()
                )
            },
            |group| format!("quota:{group}"),
        );
        let gate = Gate::shared(group, interval);
        Ok(Endpoint { url, gate })
    }
}

fn header(value: &str, name: &str) -> Result<HeaderValue, Error> {
    HeaderValue::from_str(value).map_err(|_| Error::configuration(format!("invalid {name} header")))
}

pub(crate) struct Endpoint {
    pub(crate) url: Url,
    pub(crate) gate: Arc<Gate>,
}
