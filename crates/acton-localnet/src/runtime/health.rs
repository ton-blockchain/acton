//! Fresh API and Compose health sampling for the owned network.

use super::Runtime;
use crate::{
    ApiHealth, ApiHealthStatus, NetworkHealth, NetworkHealthSample, NetworkHealthStatus,
    ServiceHealthStatus, Status, docker::DockerNetwork,
};
use reqwest::Client;
use serde_json::Value;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const PROBE_TIMEOUT: Duration = Duration::from_millis(1_500);
const MAX_HISTORY_POINTS: usize = 300;
const MIN_SAMPLE_INTERVAL_MS: u64 = 1_000;

impl Runtime {
    /// Samples API readiness and Compose state and returns a bounded recent history.
    ///
    /// The owner process performs these probes so CLI and application clients share
    /// one definition of health. History is intentionally in memory: it describes
    /// this service lifetime and cannot become stale after a machine restart.
    pub async fn health(&self) -> Result<NetworkHealth, crate::Error> {
        let entry = self.entry().await?;
        let network = entry.record.read().await.clone();
        let observed_at_ms = unix_time_millis();
        let stopped = matches!(
            network.status,
            Status::Stopped | Status::Stopping | Status::Deleted
        );
        let client = Client::builder()
            .timeout(PROBE_TIMEOUT)
            .connect_timeout(Duration::from_millis(500))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| crate::Error::Internal {
                code: "health_client_failed",
                message: format!("Failed to create localnet health client: {error}"),
            })?;

        let docker = DockerNetwork::load(&entry.data_dir, &network).await;
        let services = async {
            match docker {
                Ok(Some(driver)) => driver
                    .service_health(&network.nodes)
                    .await
                    .map(|services| (services, None)),
                Ok(None) => Ok((Vec::new(), None)),
                Err(error) => Err(error),
            }
        };
        let probes = async {
            if stopped {
                return (
                    ApiHealth::stopped(network.endpoints.api_v2.clone()),
                    ApiHealth::stopped(network.endpoints.api_v3.clone()),
                );
            }

            tokio::join!(
                probe_v2(&client, &network.endpoints.api_v2, observed_at_ms),
                probe_v3(&client, &network.endpoints.api_v3),
            )
        };
        let (services, (api_v2, mut api_v3)) = tokio::join!(services, probes);
        let (services, infrastructure_error) = match services {
            Ok(result) => result,
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        let indexer_lag_blocks = match (api_v2.masterchain_seqno, api_v3.masterchain_seqno) {
            (Some(node), Some(indexer)) => Some(node.saturating_sub(indexer)),
            _ => None,
        };

        if api_v3.status == ApiHealthStatus::Ready && indexer_lag_blocks.is_some_and(|lag| lag > 1)
        {
            api_v3.status = ApiHealthStatus::Syncing;
        }

        let estimated_indexer_lag_ms = indexer_lag_blocks
            .zip(network.config.block_time_ms)
            .map(|(lag, block_time)| u64::from(lag) * u64::from(block_time));
        let has_failed_service = services.iter().any(|service| {
            matches!(
                service.status,
                ServiceHealthStatus::Failed | ServiceHealthStatus::Unknown
            )
        });
        let status = if stopped {
            NetworkHealthStatus::Stopped
        } else if api_v2.status == ApiHealthStatus::Ready
            && api_v3.status == ApiHealthStatus::Ready
            && !has_failed_service
            && infrastructure_error.is_none()
        {
            NetworkHealthStatus::Healthy
        } else if api_v2.status == ApiHealthStatus::Ready
            && api_v3.status == ApiHealthStatus::Syncing
            && !has_failed_service
            && infrastructure_error.is_none()
        {
            NetworkHealthStatus::Syncing
        } else {
            NetworkHealthStatus::Degraded
        };
        let sample = NetworkHealthSample {
            observed_at_ms,
            api_v2_latency_ms: api_v2.latency_ms,
            api_v3_latency_ms: api_v3.latency_ms,
            api_v2_seqno: api_v2.masterchain_seqno,
            api_v3_seqno: api_v3.masterchain_seqno,
            indexer_lag_blocks,
            block_age_ms: api_v2.block_age_ms,
        };
        let history = self.record_health_sample(sample).await;

        Ok(NetworkHealth {
            observed_at_ms,
            status,
            api_v2,
            api_v3,
            indexer_lag_blocks,
            estimated_indexer_lag_ms,
            services,
            history,
            infrastructure_error,
        })
    }

    async fn record_health_sample(&self, sample: NetworkHealthSample) -> Vec<NetworkHealthSample> {
        let mut history = self.inner.health_history.lock().await;

        if history.back().is_some_and(|previous| {
            sample
                .observed_at_ms
                .saturating_sub(previous.observed_at_ms)
                < MIN_SAMPLE_INTERVAL_MS
        }) {
            history.pop_back();
        }
        history.push_back(sample);

        while history.len() > MAX_HISTORY_POINTS {
            history.pop_front();
        }

        history.iter().cloned().collect()
    }
}

impl ApiHealth {
    const fn stopped(endpoint: String) -> Self {
        Self {
            status: ApiHealthStatus::Stopped,
            endpoint,
            latency_ms: None,
            masterchain_seqno: None,
            block_time_unix: None,
            block_age_ms: None,
            error: None,
        }
    }
}

async fn probe_v2(client: &Client, endpoint: &str, observed_at_ms: u64) -> ApiHealth {
    let url = format!("{}/getMasterchainInfo", endpoint.trim_end_matches('/'));
    let started = Instant::now();
    let response = request_json(client, &url).await;
    let latency_ms = started.elapsed().as_millis() as u64;
    let value = match response {
        Ok(value) if value.pointer("/ok").and_then(Value::as_bool) == Some(true) => value,
        Ok(_) => {
            return ApiHealth::unavailable(
                endpoint,
                latency_ms,
                "API v2 returned an unsuccessful response",
            );
        }
        Err(error) => return ApiHealth::unavailable(endpoint, latency_ms, error),
    };
    let Some(seqno) = json_u32(&value, "/result/last/seqno") else {
        return ApiHealth::unavailable(
            endpoint,
            latency_ms,
            "API v2 response did not contain a masterchain seqno",
        );
    };
    let header_url = format!(
        "{}/getBlockHeader?workchain=-1&shard=-9223372036854775808&seqno={seqno}",
        endpoint.trim_end_matches('/')
    );
    let block_time_unix = request_json(client, &header_url)
        .await
        .ok()
        .and_then(|header| json_u64(&header, "/result/gen_utime"));
    let block_age_ms = block_time_unix.map(|time| observed_at_ms.saturating_sub(time * 1_000));

    ApiHealth {
        status: ApiHealthStatus::Ready,
        endpoint: endpoint.to_owned(),
        latency_ms: Some(latency_ms),
        masterchain_seqno: Some(seqno),
        block_time_unix,
        block_age_ms,
        error: None,
    }
}

async fn probe_v3(client: &Client, endpoint: &str) -> ApiHealth {
    let root = endpoint.trim_end_matches("/api/v3").trim_end_matches('/');
    let health_url = format!("{root}/healthcheck");
    let head_url = format!("{}/masterchainInfo", endpoint.trim_end_matches('/'));
    let started = Instant::now();
    let (health, head) = tokio::join!(
        request_ok(client, &health_url),
        request_json(client, &head_url)
    );
    let latency_ms = started.elapsed().as_millis() as u64;

    if let Err(error) = health {
        return ApiHealth::unavailable(endpoint, latency_ms, error);
    }

    let value = match head {
        Ok(value) => value,
        Err(error) => return ApiHealth::unavailable(endpoint, latency_ms, error),
    };
    let Some(seqno) = json_u32(&value, "/last/seqno") else {
        return ApiHealth::unavailable(
            endpoint,
            latency_ms,
            "API v3 response did not contain an indexed masterchain seqno",
        );
    };

    ApiHealth {
        status: ApiHealthStatus::Ready,
        endpoint: endpoint.to_owned(),
        latency_ms: Some(latency_ms),
        masterchain_seqno: Some(seqno),
        block_time_unix: None,
        block_age_ms: None,
        error: None,
    }
}

impl ApiHealth {
    fn unavailable(endpoint: &str, latency_ms: u64, error: impl Into<String>) -> Self {
        Self {
            status: ApiHealthStatus::Unavailable,
            endpoint: endpoint.to_owned(),
            latency_ms: Some(latency_ms),
            masterchain_seqno: None,
            block_time_unix: None,
            block_age_ms: None,
            error: Some(error.into()),
        }
    }
}

async fn request_ok(client: &Client, url: &str) -> Result<(), String> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("{url} returned HTTP {}", response.status()))
    }
}

async fn request_json(client: &Client, url: &str) -> Result<Value, String> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();

    if !status.is_success() {
        return Err(format!("{url} returned HTTP {status}"));
    }

    response
        .json()
        .await
        .map_err(|error| format!("{url} returned invalid JSON: {error}"))
}

fn json_u32(value: &Value, pointer: &str) -> Option<u32> {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}

fn json_u64(value: &Value, pointer: &str) -> Option<u64> {
    value.pointer(pointer).and_then(Value::as_u64)
}

fn unix_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
