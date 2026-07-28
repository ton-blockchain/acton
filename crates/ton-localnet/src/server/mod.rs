pub mod handlers;
pub mod models;
pub mod router;

use crate::liteapi;
use crate::localnet::Localnet;
use crate::node::StateSource;
use acton_config::color::OwoColorize;
use axum::extract::FromRef;
use serde::Serialize;
use serde_json::Value;
use std::collections::VecDeque;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const MAX_EXTERNAL_API_CALLS: usize = 1_000;
const MAX_STUDIO_UI_API_CALLS: usize = 200;
const MAX_API_CALLS: usize = MAX_EXTERNAL_API_CALLS + MAX_STUDIO_UI_API_CALLS;

#[derive(Clone, Debug, Serialize)]
pub struct StartupAccount {
    pub name: String,
    pub version: String,
    pub network: String,
    pub address: String,
}

#[derive(Clone)]
pub struct ServerState {
    pub node: Arc<Localnet>,
    pub startup_accounts: Arc<Vec<StartupAccount>>,
    pub shutdown: ShutdownSignal,
    pub network_conditions: NetworkConditions,
    pub rate_limit_rps: Option<u32>,
    pub api_calls: ApiCallLog,
    pub auth_token: Option<Arc<str>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StateSourceInfo {
    pub state_source: &'static str,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
}

impl From<&StateSource> for StateSourceInfo {
    fn from(state_source: &StateSource) -> Self {
        match state_source {
            StateSource::Local => Self {
                state_source: "local",
                fork_network: None,
                fork_block_number: None,
            },
            StateSource::Remote(provider) => Self {
                state_source: "remote",
                fork_network: Some(provider.network.to_string()),
                fork_block_number: provider.fork_block_number,
            },
        }
    }
}

#[derive(Clone)]
pub struct NetworkConditions {
    response_delay_ms: Arc<AtomicU64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct NetworkConditionsInfo {
    pub response_delay_ms: u64,
}

impl NetworkConditions {
    fn new(response_delay_ms: Option<u64>) -> Self {
        Self {
            response_delay_ms: Arc::new(AtomicU64::new(response_delay_ms.unwrap_or_default())),
        }
    }

    #[must_use]
    pub fn response_delay_ms(&self) -> u64 {
        self.response_delay_ms.load(Ordering::Relaxed)
    }

    pub fn set_response_delay_ms(&self, response_delay_ms: u64) {
        self.response_delay_ms
            .store(response_delay_ms, Ordering::Relaxed);
    }

    #[must_use]
    pub fn info(&self) -> NetworkConditionsInfo {
        NetworkConditionsInfo {
            response_delay_ms: self.response_delay_ms(),
        }
    }
}

#[derive(Clone)]
pub struct ApiCallLog {
    entries: Arc<Mutex<ApiCallEntries>>,
    next_sequence: Arc<AtomicU64>,
}

struct ApiCallEntries {
    external: VecDeque<ApiCallRecord>,
    studio_ui: VecDeque<ApiCallRecord>,
}

#[derive(Clone, Debug, Serialize)]
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
    pub request_id: Value,
    pub query_params: Option<Value>,
    pub request_body: Option<Value>,
    pub request_body_truncated: bool,
    pub response_body: Option<Value>,
    pub response_body_truncated: bool,
    pub timestamp_ms: u128,
    pub duration_ns: u128,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallStatus {
    Success,
    Failed,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallType {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallSource {
    External,
    StudioUi,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiCallFamily {
    Control,
    Emulate,
    JsonRpc,
    Streaming,
    V2,
    V3,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiCallLogSnapshot {
    pub calls: Vec<ApiCallRecord>,
    pub total_retained: usize,
    pub max_retained: usize,
}

#[derive(Clone, Debug)]
pub struct ApiCallStart {
    pub started_at: SystemTime,
    pub duration_start: Instant,
}

#[derive(Clone, Debug)]
pub struct ApiCallInput {
    pub source: ApiCallSource,
    pub call_type: ApiCallType,
    pub api_family: ApiCallFamily,
    pub http_method: String,
    pub path: String,
    pub method: String,
    pub request_id: Value,
    pub query_params: Option<Value>,
    pub request_body: Option<Value>,
    pub request_body_truncated: bool,
    pub status_code: u16,
}

impl ApiCallLog {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(ApiCallEntries {
                external: VecDeque::with_capacity(MAX_EXTERNAL_API_CALLS),
                studio_ui: VecDeque::with_capacity(MAX_STUDIO_UI_API_CALLS),
            })),
            next_sequence: Arc::new(AtomicU64::new(1)),
        }
    }

    #[must_use]
    pub fn start() -> ApiCallStart {
        ApiCallStart {
            started_at: SystemTime::now(),
            duration_start: Instant::now(),
        }
    }

    #[must_use]
    pub fn record(&self, input: ApiCallInput, start: ApiCallStart) -> u64 {
        let sequence = self.next_sequence.fetch_add(1, Ordering::Relaxed);
        let status = if input.status_code < 400 {
            ApiCallStatus::Success
        } else {
            ApiCallStatus::Failed
        };
        let timestamp_ms = start
            .started_at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis());

        let record = ApiCallRecord {
            sequence,
            status,
            status_code: input.status_code,
            source: input.source,
            call_type: input.call_type,
            api_family: input.api_family,
            http_method: input.http_method,
            path: input.path,
            method: input.method,
            request_id: input.request_id,
            query_params: input.query_params,
            request_body: input.request_body,
            request_body_truncated: input.request_body_truncated,
            response_body: None,
            response_body_truncated: false,
            timestamp_ms,
            duration_ns: start.duration_start.elapsed().as_nanos(),
        };

        let mut entries = self
            .entries
            .lock()
            .expect("API call log lock must not be poisoned");
        let (source_entries, max_entries) = match input.source {
            ApiCallSource::External => (&mut entries.external, MAX_EXTERNAL_API_CALLS),
            ApiCallSource::StudioUi => (&mut entries.studio_ui, MAX_STUDIO_UI_API_CALLS),
        };
        if source_entries.len() == max_entries {
            source_entries.pop_front();
        }
        source_entries.push_back(record);
        drop(entries);

        sequence
    }

    pub fn record_response(
        &self,
        sequence: u64,
        response_body: Option<Value>,
        response_body_truncated: bool,
    ) {
        let mut entries = self
            .entries
            .lock()
            .expect("API call log lock must not be poisoned");
        let ApiCallEntries {
            external,
            studio_ui,
        } = &mut *entries;
        let call = external
            .iter_mut()
            .chain(studio_ui.iter_mut())
            .find(|call| call.sequence == sequence);
        if let Some(call) = call {
            call.response_body = response_body;
            call.response_body_truncated = response_body_truncated;
        }
        drop(entries);
    }

    #[must_use]
    pub fn snapshot(&self, limit: Option<usize>) -> ApiCallLogSnapshot {
        let entries = self
            .entries
            .lock()
            .expect("API call log lock must not be poisoned");
        let total_retained = entries.external.len() + entries.studio_ui.len();
        let limit = limit.unwrap_or(MAX_API_CALLS).min(MAX_API_CALLS);
        let skip = total_retained.saturating_sub(limit);
        let mut calls = entries
            .external
            .iter()
            .chain(&entries.studio_ui)
            .cloned()
            .collect::<Vec<_>>();
        drop(entries);
        calls.sort_unstable_by_key(|call| call.sequence);
        let calls = calls.into_iter().skip(skip).collect();

        ApiCallLogSnapshot {
            calls,
            total_retained,
            max_retained: MAX_API_CALLS,
        }
    }
}

#[derive(Clone)]
pub struct ShutdownSignal {
    tx: broadcast::Sender<()>,
}

impl ShutdownSignal {
    fn new() -> Self {
        // Streaming handlers are long-lived; graceful shutdown waits until they exit.
        let (tx, _) = broadcast::channel(1);
        Self { tx }
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }

    fn notify(&self) {
        let _ = self.tx.send(());
    }
}

impl FromRef<ServerState> for Arc<Localnet> {
    fn from_ref(state: &ServerState) -> Self {
        state.node.clone()
    }
}

impl FromRef<ServerState> for Arc<Vec<StartupAccount>> {
    fn from_ref(state: &ServerState) -> Self {
        state.startup_accounts.clone()
    }
}

impl FromRef<ServerState> for ShutdownSignal {
    fn from_ref(state: &ServerState) -> Self {
        state.shutdown.clone()
    }
}

impl FromRef<ServerState> for NetworkConditions {
    fn from_ref(state: &ServerState) -> Self {
        state.network_conditions.clone()
    }
}

impl FromRef<ServerState> for ApiCallLog {
    fn from_ref(state: &ServerState) -> Self {
        state.api_calls.clone()
    }
}

pub struct ServerArgs {
    pub port: u16,
    pub db_path: Option<String>,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
    pub rate_limit_rps: Option<u32>,
    pub response_delay_ms: Option<u64>,
    pub startup_accounts: Vec<StartupAccount>,
    pub auth_token: Option<String>,
    pub liteapi: bool,
    pub liteapi_port: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("failed to bind localnet server to {address}")]
    Bind { address: String, source: io::Error },
    #[error("localnet LiteAPI port overflows u16 for HTTP port {http_port}")]
    LiteApiPortOverflow { http_port: u16 },
    #[error("failed to bind localnet LiteAPI to {address}")]
    LiteApiBind { address: String, source: io::Error },
    #[error("localnet server stopped with an error")]
    Serve { source: io::Error },
}

pub async fn run_server(node: Arc<Localnet>, args: ServerArgs) -> Result<(), ServerError> {
    let ServerArgs {
        port,
        db_path: _,
        fork_network,
        fork_block_number,
        rate_limit_rps,
        response_delay_ms,
        startup_accounts,
        auth_token,
        liteapi,
        liteapi_port,
    } = args;
    let auth_token = auth_token.map(Arc::<str>::from);

    let network_conditions = NetworkConditions::new(response_delay_ms);
    let api_calls = ApiCallLog::new();

    let shutdown = ShutdownSignal::new();
    let app = router::create_router(ServerState {
        node: Arc::clone(&node),
        startup_accounts: Arc::new(startup_accounts),
        shutdown: shutdown.clone(),
        network_conditions: network_conditions.clone(),
        rate_limit_rps,
        api_calls,
        auth_token: auth_token.clone(),
    });

    let address = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .map_err(|source| ServerError::Bind {
            address: address.clone(),
            source,
        })?;
    let liteapi_endpoint = if liteapi {
        let liteapi_port = match liteapi_port {
            Some(liteapi_port) => liteapi_port,
            None => port
                .checked_add(1)
                .ok_or(ServerError::LiteApiPortOverflow { http_port: port })?,
        };
        Some(
            liteapi::spawn_liteapi_server(Arc::clone(&node), liteapi_port)
                .await
                .map_err(|source| ServerError::LiteApiBind {
                    address: format!("127.0.0.1:{liteapi_port}"),
                    source,
                })?,
        )
    } else {
        None
    };
    println!(
        "    {} Localnet server and UI on http://{address}",
        "Starting".green().bold(),
    );
    if let Some(liteapi_endpoint) = liteapi_endpoint {
        println!(
            "    {} Localnet LiteAPI on tcp://{}",
            "Starting".green().bold(),
            liteapi_endpoint.address,
        );
        println!(
            "         {} LiteAPI public key: {}",
            "Key".yellow().bold(),
            liteapi_endpoint.public_key_base64
        );
    }
    if let Some(token) = auth_token.as_deref() {
        println!(
            "        {} Localnet API token: {}",
            "Auth".yellow().bold(),
            token
        );
    }
    if let Some(fork_network) = fork_network {
        let fork_source = fork_block_number
            .map(|seqno| format!("{fork_network} at seqno {seqno}"))
            .unwrap_or(fork_network);
        println!("     {} from {}", "Forking".green().bold(), fork_source);
    }
    if let Some(limit) = rate_limit_rps {
        println!(
            "    {} API requests to {} req/s",
            "Limiting".yellow().bold(),
            limit
        );
    }
    let delay_ms = network_conditions.response_delay_ms();
    if delay_ms > 0 {
        println!(
            "    {} API v2/v3/emulate responses by {}ms",
            "Delaying".yellow().bold(),
            delay_ms
        );
    }
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                println!("  {} Localnet server", "Stopping".yellow().bold());
                shutdown.notify();
            }
        })
        .await
        .map_err(|source| ServerError::Serve { source })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_call_sources_have_independent_retention_limits() {
        let log = ApiCallLog::new();

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
                let _ = log.record(
                    ApiCallInput {
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
                        status_code: 200,
                    },
                    ApiCallLog::start(),
                );
            }
        }

        let snapshot = log.snapshot(None);
        let external_count = snapshot
            .calls
            .iter()
            .filter(|call| matches!(call.source, ApiCallSource::External))
            .count();
        let studio_ui_count = snapshot.calls.len() - external_count;

        assert_eq!(external_count, MAX_EXTERNAL_API_CALLS);
        assert_eq!(studio_ui_count, MAX_STUDIO_UI_API_CALLS);
        assert_eq!(snapshot.total_retained, MAX_API_CALLS);
        assert_eq!(snapshot.max_retained, MAX_API_CALLS);
    }
}
