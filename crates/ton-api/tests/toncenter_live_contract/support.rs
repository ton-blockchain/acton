use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use reqwest::blocking::{Client, RequestBuilder};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::env;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use ton_api::toncenter::v3;

const DEFAULT_V2_URL: &str = "https://toncenter.com/api/v2";
const DEFAULT_V3_URL: &str = "https://toncenter.com/api/v3";
const DEFAULT_EMULATE_URL: &str = "https://toncenter.com/api/emulate/v1";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub(crate) struct Live {
    client: Client,
    pub api_key: Option<String>,
    pub v2_url: String,
    pub v3_url: String,
    pub emulate_url: String,
}

#[derive(Clone)]
pub(crate) struct Fixture {
    pub transaction: v3::Transaction,
    pub block: v3::Block,
}

pub(crate) enum TypedResponse<S, E> {
    Success(S),
    Error(E),
}

static LAST_REQUEST: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
static FIXTURE: OnceLock<Mutex<Option<&'static Fixture>>> = OnceLock::new();

impl Live {
    pub(crate) fn from_env() -> Result<Option<Self>> {
        if !env_flag("ACTON_TONCENTER_LIVE") {
            return Ok(None);
        }

        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(REQUEST_TIMEOUT)
            .build()
            .context("failed to construct live TonCenter HTTP client")?;

        Ok(Some(Self {
            client,
            api_key: env::var("ACTON_TONCENTER_LIVE_API_KEY")
                .or_else(|_| env::var("TONCENTER_API_KEY"))
                .ok(),
            v2_url: env_url("ACTON_TONCENTER_LIVE_V2_URL", DEFAULT_V2_URL),
            v3_url: env_url("ACTON_TONCENTER_LIVE_V3_URL", DEFAULT_V3_URL),
            emulate_url: env_url("ACTON_TONCENTER_LIVE_EMULATE_URL", DEFAULT_EMULATE_URL),
        }))
    }

    pub(crate) fn require_api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub(crate) fn get<T, Q>(&self, base: &str, path: &str, query: &Q) -> Result<T>
    where
        T: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let pairs = query_pairs(query)?;
        let request = self.client.get(endpoint(base, path)).query(&pairs);
        self.send_success(request, path)
    }

    pub(crate) fn post<T, B>(&self, base: &str, path: &str, body: &B) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let request = self.client.post(endpoint(base, path)).json(body);
        self.send_success(request, path)
    }

    pub(crate) fn post_error<T, B>(&self, base: &str, path: &str, body: &B) -> Result<T>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let (status, text) = self.send(self.client.post(endpoint(base, path)).json(body), path)?;
        if status.is_success() {
            bail!("{path} unexpectedly accepted the deliberately invalid request: {text}");
        }
        decode(&text, path, status)
    }

    pub(crate) fn post_either<S, E, B>(
        &self,
        base: &str,
        path: &str,
        body: &B,
    ) -> Result<TypedResponse<S, E>>
    where
        S: DeserializeOwned,
        E: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        self.send_either(self.client.post(endpoint(base, path)).json(body), path)
    }

    pub(crate) fn get_either<S, E, Q>(
        &self,
        base: &str,
        path: &str,
        query: &Q,
    ) -> Result<TypedResponse<S, E>>
    where
        S: DeserializeOwned,
        E: DeserializeOwned,
        Q: Serialize + ?Sized,
    {
        let pairs = query_pairs(query)?;
        self.send_either(self.client.get(endpoint(base, path)).query(&pairs), path)
    }

    fn send_either<S, E>(&self, request: RequestBuilder, path: &str) -> Result<TypedResponse<S, E>>
    where
        S: DeserializeOwned,
        E: DeserializeOwned,
    {
        let (status, text) = self.send(request, path)?;
        if let Ok(response) = serde_json::from_str(&text) {
            return Ok(TypedResponse::Success(response));
        }
        if let Ok(response) = serde_json::from_str(&text) {
            return Ok(TypedResponse::Error(response));
        }
        bail!(
            "{path}: response matches neither `{}` nor `{}` (HTTP {status}); body: {}",
            std::any::type_name::<S>(),
            std::any::type_name::<E>(),
            abbreviated(&text)
        )
    }

    pub(crate) fn authorized(&self, request: RequestBuilder) -> RequestBuilder {
        match &self.api_key {
            Some(api_key) => request.header("X-API-Key", api_key),
            None => request,
        }
    }

    pub(crate) fn post_request(&self, url: impl reqwest::IntoUrl) -> RequestBuilder {
        self.client.post(url)
    }

    pub(crate) fn wait_for_rate_limit(&self) -> Result<()> {
        let delay = if self.api_key.is_some() {
            Duration::from_millis(150)
        } else {
            Duration::from_millis(1100)
        };
        let lock = LAST_REQUEST.get_or_init(|| Mutex::new(None));
        let mut last = lock
            .lock()
            .map_err(|_| anyhow::anyhow!("live TonCenter rate limiter is poisoned"))?;
        if let Some(previous) = *last {
            thread::sleep(delay.saturating_sub(previous.elapsed()));
        }
        *last = Some(Instant::now());
        drop(last);
        Ok(())
    }

    pub(crate) fn send_raw(
        &self,
        request: RequestBuilder,
        operation: &str,
    ) -> Result<reqwest::blocking::Response> {
        self.wait_for_rate_limit()?;
        self.authorized(request)
            .send()
            .with_context(|| format!("{operation}: live TonCenter request failed"))
    }

    fn send_success<T>(&self, request: RequestBuilder, operation: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let (status, text) = self.send(request, operation)?;
        if !status.is_success() {
            bail!("{operation}: TonCenter returned HTTP {status}: {text}");
        }
        decode(&text, operation, status)
    }

    fn send(&self, request: RequestBuilder, operation: &str) -> Result<(StatusCode, String)> {
        const MAX_ATTEMPTS: usize = 3;
        for attempt in 1..=MAX_ATTEMPTS {
            let current = request
                .try_clone()
                .context("TonCenter request body cannot be retried")?;
            let response = self.send_raw(current, operation)?;
            let status = response.status();
            let text = response
                .text()
                .with_context(|| format!("{operation}: failed to read TonCenter response body"))?;
            if attempt == MAX_ATTEMPTS
                || (status != StatusCode::TOO_MANY_REQUESTS && !status.is_server_error())
            {
                return Ok((status, text));
            }
            thread::sleep(Duration::from_millis(250 * attempt as u64));
        }
        unreachable!("retry loop always returns on its last attempt")
    }
}

pub(crate) fn fixture(live: &Live) -> Result<&'static Fixture> {
    let fixture = FIXTURE.get_or_init(|| Mutex::new(None));
    let mut fixture = fixture
        .lock()
        .map_err(|_| anyhow::anyhow!("live TonCenter fixture cache is poisoned"))?;
    if let Some(fixture) = *fixture {
        return Ok(fixture);
    }

    let loaded = Box::leak(Box::new(load_fixture(live)?));
    *fixture = Some(loaded);
    drop(fixture);
    Ok(loaded)
}

fn load_fixture(live: &Live) -> Result<Fixture> {
    let transactions: v3::TransactionsResponse = live.get(
        &live.v3_url,
        "/transactions",
        &v3::TransactionsQuery {
            limit: Some(20),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
    )?;
    let transaction = transactions
        .transactions
        .into_iter()
        .next()
        .context("TonCenter v3 returned no recent transactions for live-test fixture")?;

    let blocks: v3::BlocksResponse = live.get(
        &live.v3_url,
        "/blocks",
        &v3::BlocksQuery {
            limit: Some(1),
            offset: Some(20),
            sort: Some("desc".to_owned()),
            ..Default::default()
        },
    )?;
    let block = blocks
        .blocks
        .into_iter()
        .next()
        .context("TonCenter v3 returned no recent blocks for live-test fixture")?;

    Ok(Fixture { transaction, block })
}

pub(crate) fn decode<T>(text: &str, operation: &str, status: StatusCode) -> Result<T>
where
    T: DeserializeOwned,
{
    serde_json::from_str(text).with_context(|| {
        format!(
            "{operation}: response does not match `{}` (HTTP {status}); body: {}",
            std::any::type_name::<T>(),
            abbreviated(text)
        )
    })
}

pub(crate) const fn invalid_boc() -> &'static str {
    "not-a-valid-boc"
}

fn query_pairs<T>(query: &T) -> Result<Vec<(String, String)>>
where
    T: Serialize + ?Sized,
{
    let Value::Object(fields) =
        serde_json::to_value(query).context("failed to serialize typed TonCenter query")?
    else {
        bail!("TonCenter query must serialize to a JSON object");
    };

    let mut pairs = Vec::new();
    for (name, value) in fields {
        match value {
            Value::Null => {}
            Value::Array(values) => {
                for value in values {
                    pairs.push((name.clone(), scalar_query_value(&name, value)?));
                }
            }
            value => pairs.push((name.clone(), scalar_query_value(&name, value)?)),
        }
    }
    Ok(pairs)
}

fn scalar_query_value(name: &str, value: Value) -> Result<String> {
    match value {
        Value::String(value) => Ok(value),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        value => bail!("query parameter `{name}` is not a scalar: {value}"),
    }
}

fn env_flag(name: &str) -> bool {
    env::var(name).is_ok_and(|value| matches!(value.as_str(), "1" | "true" | "yes"))
}

fn env_url(name: &str, default: &str) -> String {
    env::var(name)
        .unwrap_or_else(|_| default.to_owned())
        .trim_end_matches('/')
        .to_owned()
}

fn endpoint(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn abbreviated(value: &str) -> String {
    const MAX_CHARS: usize = 2_000;
    let mut chars = value.chars();
    let abbreviated: String = chars.by_ref().take(MAX_CHARS).collect();
    if chars.next().is_some() {
        format!("{abbreviated}...")
    } else {
        abbreviated
    }
}
