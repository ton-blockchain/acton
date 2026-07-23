use anyhow::{Context, anyhow};
use faucet_config::ToncenterConfig;
use reqwest::header;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::time::sleep;
use tracing::warn;

pub struct ToncenterClient {
    client: reqwest::Client,
    base_url: String,
    max_retries: u32,
    retry_base_delay: Duration,
}

#[derive(Deserialize, Debug)]
pub struct GetMethodResult {
    pub stack: Vec<Value>,
}

impl ToncenterClient {
    pub fn new(config: &ToncenterConfig) -> anyhow::Result<Self> {
        let mut default_headers = header::HeaderMap::new();

        if let Some(api_key) = config.api_key.as_deref() {
            let api_key = header::HeaderValue::from_str(api_key)
                .context("Invalid Toncenter API key header value")?;

            default_headers.insert(header::HeaderName::from_static("x-api-key"), api_key);
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .connect_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .default_headers(default_headers)
            .user_agent(user_agent())
            .build()
            .context("Failed to build Toncenter HTTP client")?;

        Ok(Self {
            client,
            base_url: config.url.clone(),
            max_retries: config.max_retries,
            retry_base_delay: Duration::from_millis(config.retry_base_delay_ms),
        })
    }

    pub async fn get_wallet_seqno(&self, address: &str) -> anyhow::Result<u32> {
        let result = self.run_get_method(address, "seqno").await?;

        for first in result.stack {
            if let Some(value_str) = Self::stack_num_value(&first) {
                return Ok(Self::parse_seqno(value_str));
            }
        }

        Ok(0)
    }

    fn stack_num_value(value: &Value) -> Option<&str> {
        value
            .as_array()
            .filter(|items| items.len() == 2 && items[0].as_str() == Some("num"))
            .and_then(|items| items[1].as_str())
            .or_else(|| {
                (value.get("type").and_then(Value::as_str) == Some("num"))
                    .then(|| value.get("value").and_then(Value::as_str))
                    .flatten()
            })
    }

    fn parse_seqno(value: &str) -> u32 {
        u32::from_str_radix(value.trim_start_matches("0x"), 16).unwrap_or(0)
    }

    pub async fn run_get_method(
        &self,
        address: &str,
        method: &str,
    ) -> anyhow::Result<GetMethodResult> {
        let json = json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "runGetMethod",
            "params": {
                "address": address,
                "method": method,
                "stack": []
            }
        });

        let response = self.post_jsonrpc_with_retry(&json, "runGetMethod").await?;
        let result = response.get("result").cloned().unwrap_or(response);

        serde_json::from_value(result).context("Failed to parse runGetMethod response payload")
    }

    pub async fn send_boc(&self, boc: &str) -> anyhow::Result<Value> {
        let json = json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "sendBoc",
            "params": {
                "boc": boc
            }
        });

        self.post_jsonrpc_with_retry(&json, "sendBoc").await
    }

    pub async fn get_address_balance(&self, address: &str) -> anyhow::Result<u64> {
        let json = json!({
            "id": "1",
            "jsonrpc": "2.0",
            "method": "getAddressBalance",
            "params": {
                "address": address
            }
        });

        let response = self
            .post_jsonrpc_with_retry(&json, "getAddressBalance")
            .await?;
        let result = response.get("result").unwrap_or(&response);

        parse_balance(result).context("Failed to parse getAddressBalance response payload")
    }

    fn jsonrpc_url(&self) -> String {
        format!("{}/api/v2/jsonRPC", self.base_url)
    }

    fn retry_delay(&self, attempt: u32) -> Duration {
        let multiplier = 1u64 << attempt.min(8);
        self.retry_base_delay.saturating_mul(multiplier as u32)
    }

    fn is_retryable_status(status: reqwest::StatusCode) -> bool {
        status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
    }

    fn is_retryable_error(error: &reqwest::Error) -> bool {
        error.is_timeout() || error.is_connect() || error.is_request()
    }

    fn is_retryable_rpc_error(error: &Value) -> bool {
        if let Some(code) = error.get("code").and_then(|v| v.as_i64())
            && (code == 429 || code >= 500)
        {
            return true;
        }

        if let Some(message) = error.get("message").and_then(|v| v.as_str()) {
            let msg = message.to_ascii_lowercase();
            return msg.contains("rate limit")
                || msg.contains("too many requests")
                || msg.contains("timeout")
                || msg.contains("temporary");
        }

        false
    }

    async fn post_jsonrpc_with_retry(
        &self,
        payload: &Value,
        operation: &str,
    ) -> anyhow::Result<Value> {
        let url = self.jsonrpc_url();

        for attempt in 0..=self.max_retries {
            let request = self.client.post(&url).json(payload);

            let response = match request.send().await {
                Ok(response) => response,
                Err(err) => {
                    if attempt < self.max_retries && Self::is_retryable_error(&err) {
                        warn!(
                            operation,
                            attempt = attempt + 1,
                            max_attempts = self.max_retries + 1,
                            error = %err,
                            "Toncenter request failed, retrying"
                        );
                        sleep(self.retry_delay(attempt)).await;
                        continue;
                    }

                    return Err(err).context(format!("Failed to send {} request", operation));
                }
            };

            let status = response.status();
            let body = response
                .text()
                .await
                .context(format!("Failed to read {} response body", operation))?;

            if !status.is_success() {
                if attempt < self.max_retries && Self::is_retryable_status(status) {
                    warn!(
                        operation,
                        attempt = attempt + 1,
                        max_attempts = self.max_retries + 1,
                        status = %status,
                        "Toncenter returned retryable HTTP status"
                    );
                    sleep(self.retry_delay(attempt)).await;
                    continue;
                }

                return Err(anyhow!(
                    "TonCenter API returned status: {} for {}. Error: {}",
                    status,
                    operation,
                    body
                ));
            }

            let response_json: Value = serde_json::from_str(&body)
                .context(format!("Failed to parse {} response as JSON", operation))?;

            if let Some(error) = response_json.get("error").filter(|e| !e.is_null()) {
                if attempt < self.max_retries && Self::is_retryable_rpc_error(error) {
                    warn!(
                        operation,
                        attempt = attempt + 1,
                        max_attempts = self.max_retries + 1,
                        error = %error,
                        "Toncenter returned retryable JSON-RPC error"
                    );
                    sleep(self.retry_delay(attempt)).await;
                    continue;
                }

                return Err(anyhow!(
                    "TonCenter JSON-RPC error for {}: {}",
                    operation,
                    error
                ));
            }

            return Ok(response_json);
        }

        Err(anyhow!(
            "Exceeded retry budget for Toncenter operation: {}",
            operation
        ))
    }
}

fn user_agent() -> String {
    let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
    format!("faucet/{} ({git_hash})", env!("CARGO_PKG_VERSION"))
}

fn parse_balance(value: &Value) -> Option<u64> {
    if let Some(balance) = value.as_u64() {
        return Some(balance);
    }

    if let Some(balance) = value.as_i64() {
        return u64::try_from(balance).ok();
    }

    if let Some(balance) = value.as_str() {
        return balance.parse().ok();
    }

    value.get("balance").and_then(parse_balance)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_balance;

    #[test]
    fn parses_balance_payloads() {
        assert_eq!(parse_balance(&json!("25000000000")), Some(25_000_000_000));
        assert_eq!(
            parse_balance(&json!(25_000_000_000u64)),
            Some(25_000_000_000)
        );
        assert_eq!(
            parse_balance(&json!({ "balance": "25000000000" })),
            Some(25_000_000_000)
        );
        assert_eq!(parse_balance(&json!(-1)), None);
        assert_eq!(parse_balance(&json!("not-number")), None);
    }
}
