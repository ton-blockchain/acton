pub mod handlers;
pub mod models;
pub mod router;

use crate::localnet::Localnet;
use acton_config::color::OwoColorize;
use axum::extract::FromRef;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

const MAX_API_CALLS: usize = 500;

#[derive(Clone, Debug, Serialize)]
pub struct StartupWallet {
    pub name: String,
    pub mnemonic: Vec<String>,
    pub version: String,
    pub network: String,
    pub address: String,
    pub public_key: String,
    pub wallet_id: i32,
}

#[derive(Clone)]
pub struct ServerState {
    pub node: Arc<Localnet>,
    pub startup_wallets: Arc<Vec<StartupWallet>>,
    pub state_source: Arc<StateSourceInfo>,
    pub shutdown: ShutdownSignal,
    pub network_conditions: NetworkConditions,
    pub api_calls: ApiCallLog,
    pub auth_token: Option<Arc<str>>,
}

#[derive(Clone, Debug, Serialize)]
pub struct StateSourceInfo {
    pub state_source: &'static str,
    pub fork_network: Option<String>,
    pub fork_block_number: Option<u64>,
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
    entries: Arc<Mutex<VecDeque<ApiCallRecord>>>,
    next_sequence: Arc<AtomicU64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ApiCallRecord {
    pub sequence: u64,
    pub status: ApiCallStatus,
    pub status_code: u16,
    pub call_type: ApiCallType,
    pub api_family: ApiCallFamily,
    pub http_method: String,
    pub path: String,
    pub method: String,
    pub request_id: Value,
    pub timestamp_ms: u128,
    pub duration_ms: u128,
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
    pub call_type: ApiCallType,
    pub api_family: ApiCallFamily,
    pub http_method: String,
    pub path: String,
    pub method: String,
    pub request_id: Value,
    pub status_code: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ApiCallAlreadyRecorded;

impl ApiCallLog {
    fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_API_CALLS))),
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

    pub fn record(&self, input: ApiCallInput, start: ApiCallStart) {
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
            call_type: input.call_type,
            api_family: input.api_family,
            http_method: input.http_method,
            path: input.path,
            method: input.method,
            request_id: input.request_id,
            timestamp_ms,
            duration_ms: start.duration_start.elapsed().as_millis(),
        };

        let mut entries = self
            .entries
            .lock()
            .expect("API call log lock must not be poisoned");
        if entries.len() == MAX_API_CALLS {
            entries.pop_front();
        }
        entries.push_back(record);
    }

    #[must_use]
    pub fn snapshot(&self, limit: Option<usize>) -> ApiCallLogSnapshot {
        let entries = self
            .entries
            .lock()
            .expect("API call log lock must not be poisoned");
        let limit = limit.unwrap_or(MAX_API_CALLS).min(MAX_API_CALLS);
        let skip = entries.len().saturating_sub(limit);
        let calls = entries.iter().skip(skip).cloned().collect();

        ApiCallLogSnapshot {
            calls,
            total_retained: entries.len(),
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

impl FromRef<ServerState> for Arc<Vec<StartupWallet>> {
    fn from_ref(state: &ServerState) -> Self {
        state.startup_wallets.clone()
    }
}

impl FromRef<ServerState> for Arc<StateSourceInfo> {
    fn from_ref(state: &ServerState) -> Self {
        state.state_source.clone()
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
    pub startup_wallets: Vec<StartupWallet>,
    pub auth_token: Option<String>,
}

pub async fn run_server(node: Arc<Localnet>, args: ServerArgs) -> anyhow::Result<()> {
    let ServerArgs {
        port,
        db_path: _,
        fork_network,
        fork_block_number,
        rate_limit_rps,
        response_delay_ms,
        startup_wallets,
        auth_token,
    } = args;
    let auth_token = auth_token.map(Arc::<str>::from);

    seed_startup_wallet_names(&node, &startup_wallets).await?;
    let network_conditions = NetworkConditions::new(response_delay_ms);
    let api_calls = ApiCallLog::new();

    let state_source = StateSourceInfo {
        state_source: if fork_network.is_some() {
            "remote"
        } else {
            "local"
        },
        fork_network: fork_network.clone(),
        fork_block_number,
    };
    let shutdown = ShutdownSignal::new();
    let app = router::create_router(
        ServerState {
            node,
            startup_wallets: Arc::new(startup_wallets),
            state_source: Arc::new(state_source),
            shutdown: shutdown.clone(),
            network_conditions: network_conditions.clone(),
            api_calls,
            auth_token: auth_token.clone(),
        },
        rate_limit_rps,
    );

    let address = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!(
        "    {} Localnet server and UI on http://{address}",
        "Starting".green().bold(),
    );
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
        .await?;
    Ok(())
}

async fn seed_startup_wallet_names(
    node: &Localnet,
    startup_wallets: &[StartupWallet],
) -> anyhow::Result<()> {
    let mut seen_addresses = HashSet::new();
    let mut named_wallets = Vec::new();

    for wallet in startup_wallets {
        let address = wallet.address.trim();
        let name = wallet.name.trim();
        if address.is_empty() || name.is_empty() || !seen_addresses.insert(address.to_string()) {
            continue;
        }
        named_wallets.push((address.to_string(), name.to_string()));
    }

    if named_wallets.is_empty() {
        return Ok(());
    }

    let existing_names = node
        .get_address_names(
            named_wallets
                .iter()
                .map(|(address, _)| address.clone())
                .collect(),
        )
        .await?;

    for ((address, name), (_, existing_name)) in named_wallets.into_iter().zip(existing_names) {
        if existing_name.is_none() {
            node.set_address_name(address, name).await?;
        }
    }

    Ok(())
}
