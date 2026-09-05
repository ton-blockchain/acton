//! Authenticated client shared by CLI commands and future application integrations.

mod network;
mod shutdown;

use crate::{Error, ServiceDescriptor, storage};
use reqwest::{Method, Url};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

/// A verified connection to a local service. Discovery tokens stay out of debug
/// output and request errors; transport failures never trigger mutation retries.
#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    descriptor: ServiceDescriptor,
    root: PathBuf,
}

impl Client {
    /// Identifies the service process verified through discovery. Launching clients
    /// use this to distinguish their child from a concurrently started service.
    #[must_use]
    pub const fn service_pid(&self) -> u32 {
        self.descriptor.pid
    }

    /// Reads private discovery data and verifies service identity before use.
    /// Restricting discovery to loopback prevents a stale descriptor from sending
    /// the local authorization token to an external endpoint.
    pub async fn connect(root: &Path) -> Result<Self, Error> {
        let descriptor: ServiceDescriptor =
            storage::read_json(&storage::service_descriptor_path(root)).await?;
        let url = Url::parse(&descriptor.url).map_err(|e| Error::invalid(e.to_string()))?;
        let loopback = url
            .host_str()
            .and_then(|host| {
                host.trim_matches(['[', ']'])
                    .parse::<std::net::IpAddr>()
                    .ok()
            })
            .is_some_and(|ip| ip.is_loopback());

        if descriptor.protocol_version != 1 || url.scheme() != "http" || !loopback {
            return Err(Error::invalid("Invalid localnet service descriptor"));
        }

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(2))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| Error::invalid(e.to_string()))?;
        let client = Self {
            http,
            descriptor,
            root: root.to_owned(),
        };
        let health: Value = client.request(Method::GET, "/v1/health", None).await?;

        if health["service"] != "acton-localnet" || health["protocolVersion"] != 1 {
            return Err(Error::invalid(
                "The discovered endpoint is not a compatible localnet service",
            ));
        }

        Ok(client)
    }

    /// Sends one API request without retrying side effects. The returned operation
    /// ID lets callers continue polling after their terminal disconnects.
    pub async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<T, Error> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.descriptor.url))
            .bearer_auth(&self.descriptor.token);

        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request.send().await.map_err(|e| Error::Internal {
            code: "service_unavailable",
            message: format!("Localnet service request failed: {e}"),
        })?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::invalid(e.to_string()))?;

        if !status.is_success() {
            let error: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            return Err(Error::Api {
                status: status.as_u16(),
                code: error["code"]
                    .as_str()
                    .unwrap_or("request_failed")
                    .to_owned(),
                message: error["message"]
                    .as_str()
                    .unwrap_or("Invalid error response")
                    .to_owned(),
            });
        }

        let bytes = if bytes.is_empty() {
            b"null".as_slice()
        } else {
            &bytes
        };
        serde_json::from_slice(bytes).map_err(|e| Error::Internal {
            code: "invalid_response",
            message: format!("Localnet API returned invalid JSON: {e}"),
        })
    }
}
