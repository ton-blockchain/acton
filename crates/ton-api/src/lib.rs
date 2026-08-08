use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use reqwest::blocking::Response;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
pub use ton_networks::{CustomNetworkUrls, Network};
use toncenter::{v2, v3};
use toncenter_keys::api_key as toncenter_api_key;
use tvm_ffi::stack::TupleItem;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};

mod deployment;
mod offchain;
pub mod toncenter;

pub use deployment::{DeploymentCandidate, extract_deployment_candidates};
pub use offchain::OffchainJsonResolver;

const HTTP_RETRY_ATTEMPTS: usize = 3;
const HTTP_RETRY_BACKOFF_MS: [u64; 3] = [1000, 2000, 3000];
const HTTP_CONNECT_TIMEOUT_SECS: u64 = 10;
const HTTP_REQUEST_TIMEOUT_SECS: u64 = 30;
const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";
const TEST_TONCENTER_RETRY_BACKOFF_MS_ENV: &str = "ACTON_TEST_TONCENTER_RETRY_BACKOFF_MS";
const TEST_TONCENTER_MIN_REQUEST_INTERVAL_MS_ENV: &str =
    "ACTON_TEST_TONCENTER_MIN_REQUEST_INTERVAL_MS";
const TONCENTER_MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(1100);
pub const MASTERCHAIN_SNAPSHOT_CACHE_SUBDIR: &str = "masterchain-snapshots";
const MASTERCHAIN_SNAPSHOT_CACHE_SCHEMA_VERSION: u32 = 2;
const MASTERCHAIN_SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
static TONCENTER_REQUEST_GATE: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn http_client_builder() -> reqwest::blocking::ClientBuilder {
    let builder = reqwest::blocking::Client::builder().user_agent(user_agent());
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn async_http_client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder().user_agent(user_agent());
    if proxy_enabled() {
        builder
    } else {
        builder.no_proxy()
    }
}

fn proxy_enabled() -> bool {
    proxy_enabled_from_value(env::var_os(USE_PROXY_ENV).as_deref())
}

fn proxy_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        value == "1" || value == "true"
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendBocErrorKind {
    MissingAccountState,
    RejectedBeforeExecution,
    TransportFailure,
    Other,
}

#[derive(Debug, Clone)]
pub struct SendBocError {
    kind: SendBocErrorKind,
    raw: String,
}

impl SendBocError {
    fn new(kind: SendBocErrorKind, raw: impl Into<String>) -> Self {
        Self {
            kind,
            raw: raw.into(),
        }
    }

    #[must_use]
    pub const fn kind(&self) -> SendBocErrorKind {
        self.kind
    }

    #[must_use]
    pub fn raw(&self) -> &str {
        &self.raw
    }
}

impl fmt::Display for SendBocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl std::error::Error for SendBocError {}

pub struct TonApiClient {
    client: reqwest::blocking::Client,
    network: Network,
    api_key: Option<String>,
    custom_networks: HashMap<String, CustomNetworkUrls>,
}

#[derive(Debug, Clone)]
pub struct MasterchainSnapshot {
    pub seqno: u64,
    pub gen_utime: u32,
    pub config: Cell,
}

#[derive(Debug, Serialize, Deserialize)]
struct MasterchainSnapshotCacheEntry {
    schema_version: u32,
    network: Network,
    v2_url: String,
    seqno: u64,
    fetched_at: u64,
    gen_utime: Option<u32>,
    config_boc64: String,
}

struct CachedMasterchainSnapshot {
    fetched_at: u64,
    gen_utime: Option<u32>,
    config: Cell,
}

impl MasterchainSnapshotCacheEntry {
    fn new(
        network: &Network,
        v2_url: String,
        snapshot: &MasterchainSnapshot,
        gen_utime: Option<u32>,
        fetched_at: u64,
    ) -> Self {
        Self {
            schema_version: MASTERCHAIN_SNAPSHOT_CACHE_SCHEMA_VERSION,
            network: network.clone(),
            v2_url,
            seqno: snapshot.seqno,
            fetched_at,
            gen_utime,
            config_boc64: Boc::encode_base64(&snapshot.config),
        }
    }

    fn into_cached_snapshot(self) -> anyhow::Result<CachedMasterchainSnapshot> {
        Ok(CachedMasterchainSnapshot {
            fetched_at: self.fetched_at,
            gen_utime: self.gen_utime,
            config: Boc::decode_base64(&self.config_boc64)
                .context("Failed to decode cached blockchain config BOC")?,
        })
    }
}

fn masterchain_snapshot_cache_path(cache_dir: &Path, network: &Network, seqno: u64) -> PathBuf {
    let network = match network {
        Network::Mainnet => "mainnet".to_owned(),
        Network::Testnet => "testnet".to_owned(),
        Network::Localnet => "localnet".to_owned(),
        Network::Custom(name) => format!("custom-{}", urlencoding::encode(name)),
    };
    cache_dir.join(network).join(format!("{seqno}.json"))
}

fn read_masterchain_snapshot_cache(
    path: &Path,
    network: &Network,
    v2_url: &str,
    seqno: u64,
) -> Option<CachedMasterchainSnapshot> {
    let entry =
        serde_json::from_slice::<MasterchainSnapshotCacheEntry>(&fs::read(path).ok()?).ok()?;
    if entry.schema_version != MASTERCHAIN_SNAPSHOT_CACHE_SCHEMA_VERSION
        || entry.network != *network
        || entry.v2_url != v2_url
        || entry.seqno != seqno
    {
        return None;
    }

    entry.into_cached_snapshot().ok()
}

fn write_masterchain_snapshot_cache(
    path: &Path,
    entry: &MasterchainSnapshotCacheEntry,
) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Masterchain snapshot cache path has no parent"))?;
    fs::create_dir_all(parent)?;
    fs::write(path, serde_json::to_vec_pretty(entry)?)?;
    Ok(())
}

fn masterchain_snapshot_cache_entry_is_fresh(fetched_at: u64, now: u64) -> bool {
    fetched_at
        .checked_add(MASTERCHAIN_SNAPSHOT_CACHE_TTL.as_secs())
        .is_some_and(|expires_at| now < expires_at)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl TonApiClient {
    pub fn new(
        network: Network,
        custom_networks: HashMap<String, CustomNetworkUrls>,
    ) -> anyhow::Result<TonApiClient> {
        let client_builder = http_client_builder()
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .timeout(Duration::from_secs(HTTP_REQUEST_TIMEOUT_SECS));

        Ok(TonApiClient {
            client: client_builder
                .build()
                .context("Cannot create HTTP client, please check if network is available")?,
            api_key: toncenter_api_key(&network),
            network,
            custom_networks,
        })
    }

    #[must_use]
    pub fn with_network(mut self, network: Network) -> Self {
        self.network = network;
        self
    }

    #[must_use]
    pub const fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    fn build_request(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        let mut request = self.client.get(url);

        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }

        request
    }

    fn build_post_request(&self, url: &str) -> reqwest::blocking::RequestBuilder {
        let mut request = self.client.post(url);

        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }

        request
    }

    fn get_json<T: DeserializeOwned>(
        &self,
        url: &str,
        transport_error_context: &str,
        response_error_context: &str,
    ) -> anyhow::Result<T> {
        let response = self.send_with_retry(|| self.build_request(url), transport_error_context)?;
        if !response.status().is_success() {
            anyhow::bail!("TonCenter API returned status: {}", response.status());
        }
        response.json().context(response_error_context.to_owned())
    }

    fn get_v2_result<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &impl Serialize,
    ) -> anyhow::Result<T> {
        let url = format!(
            "{}{path}",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );
        let response = self.send_with_retry(
            || self.build_request(&url).query(query),
            "Failed to send TonCenter v2 request",
        )?;
        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }
        let response: v2::TonlibResponse<T> = response
            .json()
            .context("Failed to parse TonCenter v2 response")?;
        Ok(response.result)
    }

    fn send_with_retry<F>(
        &self,
        mut build_request: F,
        transport_error_context: &str,
    ) -> anyhow::Result<Response>
    where
        F: FnMut() -> reqwest::blocking::RequestBuilder,
    {
        for attempt in 0..HTTP_RETRY_ATTEMPTS {
            self.maybe_wait_for_rate_limit();
            let request = build_request();
            log::info!("Send {request:?}");
            return match request.send() {
                Ok(response) => {
                    if Self::should_retry_status(response.status())
                        && attempt + 1 < HTTP_RETRY_ATTEMPTS
                    {
                        std::thread::sleep(Self::http_retry_backoff(attempt));
                        continue;
                    }
                    Ok(response)
                }
                Err(err) => {
                    if Self::should_retry_transport_error(&err) && attempt + 1 < HTTP_RETRY_ATTEMPTS
                    {
                        std::thread::sleep(Self::http_retry_backoff(attempt));
                        continue;
                    }
                    Err(err).context(transport_error_context.to_owned())
                }
            };
        }

        unreachable!("retry loop must return on success or final failure");
    }

    fn maybe_wait_for_rate_limit(&self) {
        if self.api_key.is_some() {
            return;
        }

        if self.network == Network::Localnet {
            // we don't have rate limit on localnet by default
            return;
        }

        let mut last_request = TONCENTER_REQUEST_GATE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        if let Some(last) = *last_request {
            let elapsed = last.elapsed();
            let min_interval = toncenter_min_request_interval();
            if elapsed < min_interval {
                let wait_for = min_interval - elapsed;
                log::debug!("throttle for {wait_for:?}");
                std::thread::sleep(wait_for);
            }
        }

        *last_request = Some(Instant::now());
    }

    fn should_retry_status(status: reqwest::StatusCode) -> bool {
        status.is_server_error()
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::REQUEST_TIMEOUT
    }

    fn should_retry_transport_error(err: &reqwest::Error) -> bool {
        err.is_timeout() || err.is_connect() || err.is_request()
    }

    fn http_retry_backoff(attempt: usize) -> Duration {
        if let Some(duration) = test_retry_backoff_override() {
            return duration;
        }

        let index = attempt.min(HTTP_RETRY_BACKOFF_MS.len() - 1);
        Duration::from_millis(HTTP_RETRY_BACKOFF_MS[index])
    }

    #[must_use]
    pub fn network(&self) -> Network {
        self.network.clone()
    }

    /// Get account state from `TonCenter`
    pub fn get_account_state(&self, address: &str) -> anyhow::Result<v3::AccountStateFull> {
        let accounts = self.get_account_states(&[address])?;
        accounts
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Account not found"))
    }

    /// Get multiple account states from `TonCenter`
    pub fn get_account_states(
        &self,
        addresses: &[&str],
    ) -> anyhow::Result<Vec<v3::AccountStateFull>> {
        if addresses.is_empty() {
            return Ok(vec![]);
        }

        let mut url = format!(
            "{}/accountStates?",
            self.network.toncenter_v3_url(&self.custom_networks)?
        );
        for (i, address) in addresses.iter().enumerate() {
            if i > 0 {
                url.push('&');
            }
            url.push_str("address=");
            url.push_str(&urlencoding::encode(address));
        }

        let response = self.send_with_retry(
            || self.build_request(&url),
            "Failed to send request to TonCenter",
        )?;

        if !response.status().is_success() {
            anyhow::bail!("TonCenter API returned status: {}", response.status());
        }

        let data: v3::AccountStatesResponse = response
            .json()
            .context("Failed to parse TonCenter response")?;

        Ok(data.accounts)
    }

    /// Get contract BOC from `TonCenter` (tries mainnet first, then testnet)
    pub fn get_contract_boc(&self, address: &str) -> anyhow::Result<String> {
        let state = self.get_account_state(address)?;

        if state.status != "active" {
            anyhow::bail!("Contract is not active (status: {})", state.status);
        }

        state
            .code_boc
            .ok_or_else(|| anyhow!("Contract has no code"))
    }

    /// Run get method on contract
    pub fn run_get_method(
        &self,
        address: &str,
        method: &str,
        stack: &[serde_json::Value],
    ) -> anyhow::Result<v2::RunGetMethodResult> {
        self.run_get_method_at_block(address, method, stack, None)
    }

    /// Run get method on contract at a specific masterchain block, when provided.
    pub fn run_get_method_at_block(
        &self,
        address: &str,
        method: &str,
        stack: &[serde_json::Value],
        seqno: Option<u64>,
    ) -> anyhow::Result<v2::RunGetMethodResult> {
        let url = format!(
            "{}/jsonRPC",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let seqno = seqno
            .map(u32::try_from)
            .transpose()
            .context("Masterchain seqno does not fit TonCenter v2 request")?;
        let json = v2::JsonRpcRequest::new(
            "1",
            "runGetMethod",
            v2::RunGetMethodRequest {
                address: address.to_owned(),
                method: v2::StringOrNumber::String(method.to_owned()),
                stack: stack.to_vec(),
                seqno: seqno.map(Into::into),
            },
        );

        let response = self.send_with_retry(
            || self.build_post_request(&url).json(&json),
            "Failed to send runGetMethod request",
        )?;

        if !response.status().is_success() {
            let error_text = response
                .text()
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Run get method failed: {error_text}");
        }

        let result: v2::JsonRpcResponse<v2::RunGetMethodResult> = response
            .json()
            .context("Failed to parse runGetMethod response")?;

        Ok(result.into_result())
    }

    /// Get wallet seqno
    pub fn get_wallet_seqno(&self, address: &str) -> anyhow::Result<(u32, bool)> {
        let result = self.run_get_method(address, "seqno", &[]);

        let Ok(result) = result else {
            // likely uninit wallet
            return Ok((0, true));
        };

        if result.exit_code == -13 {
            // likely uninit wallet
            return Ok((0, true));
        }

        let stack = result
            .parse_stack_tuple()
            .context("Failed to parse runGetMethod stack for seqno")?;

        if let Some(TupleItem::Int(value)) = stack.first() {
            let seqno: u32 = value
                .to_str_radix(10)
                .parse()
                .context("Failed to parse wallet seqno from stack integer")?;
            if seqno == 85143 {
                return Ok((0, true));
            }
            return Ok((seqno, false));
        }

        Ok((0, false))
    }

    /// Send BOC to network
    pub fn send_boc(&self, boc: &str) -> Result<(), SendBocError> {
        let base_url = self
            .network
            .toncenter_v2_url(&self.custom_networks)
            .map_err(|err| SendBocError::new(SendBocErrorKind::Other, format!("{err:#}")))?;
        let url = format!("{base_url}/sendBoc");

        let json = v2::SendBocRequest {
            boc: boc.to_owned(),
        };

        let response = self
            .send_with_retry(
                || self.build_post_request(&url).json(&json),
                "Failed to send BOC",
            )
            .map_err(|err| {
                SendBocError::new(SendBocErrorKind::TransportFailure, format!("{err:#}"))
            })?;

        if !response.status().is_success() {
            return Err(Self::handle_send_boc_fail(response));
        }

        Ok(())
    }

    pub fn get_masterchain_info(&self) -> anyhow::Result<v2::TonlibResponse<v2::MasterchainInfo>> {
        let url = format!(
            "{}/getMasterchainInfo",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let response = self.send_with_retry(
            || self.build_request(&url),
            "Failed to send request to TonCenter",
        )?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        response
            .json()
            .context("Failed to parse TonCenter response")
    }

    pub fn get_last_block_seqno(&self) -> anyhow::Result<u64> {
        Ok(self.get_masterchain_info()?.result.last.seqno)
    }

    pub fn get_masterchain_snapshot(
        &self,
        seqno: Option<u64>,
    ) -> anyhow::Result<MasterchainSnapshot> {
        let use_current_time = seqno.is_none();
        let seqno = seqno.map_or_else(|| self.get_last_block_seqno(), Ok)?;
        let gen_utime = if use_current_time {
            u32::try_from(unix_timestamp()).context("Current Unix time does not fit u32")?
        } else {
            self.get_masterchain_block_time(seqno)?
        };
        let config = self.get_config_all(Some(seqno))?;

        Ok(MasterchainSnapshot {
            seqno,
            gen_utime,
            config,
        })
    }

    fn get_masterchain_block_time(&self, seqno: u64) -> anyhow::Result<u32> {
        let request_seqno =
            u32::try_from(seqno).context("Masterchain seqno does not fit TonCenter v2 request")?;
        let header = self.get_block_header_v2(&v2::BlockHeaderRequest {
            workchain: (-1).into(),
            shard: v2::StringOrNumber::String(i64::MIN.to_string()),
            seqno: request_seqno.into(),
            root_hash: None,
            file_hash: None,
        })?;
        Ok(header.gen_utime)
    }

    pub fn get_masterchain_snapshot_cached(
        &self,
        seqno: Option<u64>,
        cache_dir: &Path,
    ) -> anyhow::Result<MasterchainSnapshot> {
        let use_current_time = seqno.is_none();
        let seqno = seqno.map_or_else(|| self.get_last_block_seqno(), Ok)?;
        let now = unix_timestamp();
        let current_gen_utime = use_current_time
            .then(|| u32::try_from(now).context("Current Unix time does not fit u32"))
            .transpose()?;
        let v2_url = self.network.toncenter_v2_url(&self.custom_networks)?;
        let cache_path = masterchain_snapshot_cache_path(cache_dir, &self.network, seqno);
        let cached = read_masterchain_snapshot_cache(&cache_path, &self.network, &v2_url, seqno);
        if let Some(cached) = cached.as_ref()
            && masterchain_snapshot_cache_entry_is_fresh(cached.fetched_at, now)
        {
            let gen_utime = match current_gen_utime.or(cached.gen_utime) {
                Some(gen_utime) => gen_utime,
                None => self.get_masterchain_block_time(seqno)?,
            };
            let snapshot = MasterchainSnapshot {
                seqno,
                gen_utime,
                config: cached.config.clone(),
            };
            if cached.gen_utime.is_none() && !use_current_time {
                let entry = MasterchainSnapshotCacheEntry::new(
                    &self.network,
                    v2_url,
                    &snapshot,
                    Some(gen_utime),
                    cached.fetched_at,
                );
                if let Err(error) = write_masterchain_snapshot_cache(&cache_path, &entry) {
                    log::debug!(
                        "Failed to update masterchain snapshot cache {}: {error:#}",
                        cache_path.display()
                    );
                }
            }
            return Ok(snapshot);
        }

        let snapshot = if let Some(gen_utime) = current_gen_utime {
            self.get_config_all(Some(seqno))
                .map(|config| MasterchainSnapshot {
                    seqno,
                    gen_utime,
                    config,
                })
        } else {
            self.get_masterchain_snapshot(Some(seqno))
        };
        match snapshot {
            Ok(snapshot) => {
                let entry = MasterchainSnapshotCacheEntry::new(
                    &self.network,
                    v2_url,
                    &snapshot,
                    (!use_current_time).then_some(snapshot.gen_utime),
                    now,
                );
                if let Err(error) = write_masterchain_snapshot_cache(&cache_path, &entry) {
                    log::debug!(
                        "Failed to write masterchain snapshot cache {}: {error:#}",
                        cache_path.display()
                    );
                }
                Ok(snapshot)
            }
            Err(error) => {
                if let Some(cached) = cached {
                    let Some(gen_utime) = current_gen_utime.or(cached.gen_utime) else {
                        return Err(error);
                    };
                    log::debug!(
                        "Failed to refresh masterchain snapshot for {} at seqno {seqno}: {error:#}; using stale cache",
                        self.network
                    );
                    return Ok(MasterchainSnapshot {
                        seqno,
                        gen_utime,
                        config: cached.config,
                    });
                }
                Err(error)
            }
        }
    }

    pub fn get_blocks_v3(&self, raw_query: &str) -> anyhow::Result<v3::BlocksResponse> {
        let mut url = format!(
            "{}/blocks",
            self.network.toncenter_v3_url(&self.custom_networks)?
        );
        if !raw_query.is_empty() {
            url.push('?');
            url.push_str(raw_query);
        }
        self.get_json(
            &url,
            "Failed to send blocks request",
            "Failed to parse blocks response",
        )
    }

    pub fn get_transactions_v3(&self, raw_query: &str) -> anyhow::Result<v3::TransactionsResponse> {
        let mut url = format!(
            "{}/transactions",
            self.network.toncenter_v3_url(&self.custom_networks)?
        );
        if !raw_query.is_empty() {
            url.push('?');
            url.push_str(raw_query);
        }
        self.get_json(
            &url,
            "Failed to send transactions request",
            "Failed to parse transactions response",
        )
    }

    pub fn get_shards(&self, seqno: u32) -> anyhow::Result<v2::Shards> {
        let url = format!(
            "{}/getShards?seqno={seqno}",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );
        let response: v2::TonlibResponse<v2::Shards> = self.get_json(
            &url,
            "Failed to send getShards request",
            "Failed to parse getShards response",
        )?;
        Ok(response.result)
    }

    pub fn get_block_header_v2(
        &self,
        request: &v2::BlockHeaderRequest,
    ) -> anyhow::Result<v2::BlockHeader> {
        self.get_v2_result("/getBlockHeader", request)
    }

    pub fn get_block_transactions_v2(
        &self,
        request: &v2::BlockTransactionsRequest,
    ) -> anyhow::Result<v2::BlockTransactions> {
        self.get_v2_result("/getBlockTransactions", request)
    }

    pub fn get_block_transactions_ext_v2(
        &self,
        request: &v2::BlockTransactionsRequest,
    ) -> anyhow::Result<v2::BlockTransactionsExt> {
        self.get_v2_result("/getBlockTransactionsExt", request)
    }

    pub fn lookup_block_v2(
        &self,
        request: &v2::LookupBlockRequest,
    ) -> anyhow::Result<v2::TonBlockIdExt> {
        self.get_v2_result("/lookupBlock", request)
    }

    pub fn get_account_info(
        &self,
        seqno: Option<u64>,
        address: &str,
    ) -> anyhow::Result<v2::AddressInformation> {
        let url = format!(
            "{}/getAddressInformation?address={}{}",
            self.network.toncenter_v2_url(&self.custom_networks)?,
            urlencoding::encode(address),
            seqno
                .map(|seqno| format!("&seqno={seqno}"))
                .unwrap_or_default(),
        );

        let response = self.send_with_retry(
            || self.build_request(&url),
            "Failed to send request to TonCenter",
        )?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        let data: v2::TonlibResponse<v2::AddressInformation> = response
            .json()
            .context("Failed to parse TonCenter response")?;

        Ok(data.result)
    }

    pub fn get_shard_account_cell(
        &self,
        seqno: Option<u64>,
        address: &str,
    ) -> anyhow::Result<Cell> {
        let url = format!(
            "{}/getShardAccountCell?address={}{}",
            self.network.toncenter_v2_url(&self.custom_networks)?,
            urlencoding::encode(address),
            seqno
                .map(|seqno| format!("&seqno={seqno}"))
                .unwrap_or_default(),
        );

        let response = self.send_with_retry(
            || self.build_request(&url),
            "Failed to send getShardAccountCell request to TonCenter",
        )?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        let data: v2::TonlibResponse<v2::TvmCell> = response
            .json()
            .context("Failed to parse getShardAccountCell response")?;

        let cell_boc = data.result.bytes;

        Boc::decode_base64(&cell_boc).context("Failed to decode shard account cell BOC data")
    }

    pub fn get_library_by_hash(&self, hash: &HashBytes) -> anyhow::Result<Cell> {
        let url = format!(
            "{}/getLibraries",
            self.network.toncenter_v2_url(&self.custom_networks)?,
        );
        let hash_hex = hash.to_string();

        let response = self.send_with_retry(
            || {
                self.build_request(&url)
                    .query(&[("libraries", hash_hex.as_str())])
            },
            "Failed to send request to TonCenter for library",
        )?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        let data: v2::TonlibResponse<v2::LibraryResult> = response
            .json()
            .context("Failed to parse TonCenter libraries response")?;

        let boc_data = data
            .result
            .result
            .first()
            .map(|entry| entry.data.as_str())
            .ok_or_else(|| anyhow::anyhow!("Library with hash {hash_hex} not found"))?;

        Boc::decode_base64(boc_data).context("Failed to decode library BOC data")
    }

    pub fn get_config_all(&self, seqno: Option<u64>) -> anyhow::Result<Cell> {
        let seqno = seqno
            .map(u32::try_from)
            .transpose()
            .context("Masterchain seqno does not fit TonCenter v2 request")?;
        let data: v2::ConfigInfo = self.get_v2_result(
            "/getConfigAll",
            &v2::ConfigAllRequest {
                seqno: seqno.map(Into::into),
            },
        )?;

        Boc::decode_base64(&data.config.bytes)
            .context("Failed to decode blockchain config BOC data")
    }

    pub fn decode_optional_cell(cell_data: &String) -> anyhow::Result<Option<Cell>> {
        if cell_data.is_empty() {
            return Ok(None);
        }
        Ok(Some(Boc::decode_base64(cell_data)?))
    }

    pub fn get_transactions(
        &self,
        address: &str,
        limit: Option<u32>,
        lt: Option<String>,
        hash: Option<String>,
    ) -> anyhow::Result<Vec<v2::Transaction>> {
        let url = format!(
            "{}/getTransactions",
            self.network.toncenter_v2_url(&self.custom_networks)?
        );

        let mut params = vec![("address", address.to_string())];
        if let Some(limit) = limit {
            params.push(("limit", limit.to_string()));
        }
        if let Some(lt) = lt {
            params.push(("lt", lt));
        }
        if let Some(hash) = hash {
            params.push(("hash", hash));
        }

        let response = self.send_with_retry(
            || self.build_request(&url).query(&params),
            "Failed to send getTransactions request",
        )?;

        if !response.status().is_success() {
            anyhow::bail!("TonCenter API returned status: {}", response.status());
        }

        let data: v2::TonlibResponse<Vec<v2::Transaction>> = response
            .json()
            .context("Failed to parse getTransactions response")?;

        Ok(data.result)
    }

    pub fn get_address_balance(&self, address: &str) -> anyhow::Result<BigInt> {
        let url = format!(
            "{}/getAddressBalance?address={}",
            self.network.toncenter_v2_url(&self.custom_networks)?,
            urlencoding::encode(address)
        );

        let response = self.send_with_retry(
            || self.build_request(&url),
            "Failed to send getAddressBalance request",
        )?;

        if !response.status().is_success() {
            return Err(Self::handle_fail(response));
        }

        let data: v2::TonlibResponse<v2::StringOrNumber> = response
            .json()
            .context("Failed to parse getAddressBalance response")?;

        data.result.to_bigint()
    }

    fn handle_fail(response: Response) -> anyhow::Error {
        let status = response.status();
        let Ok(data) = response.json::<v2::TonlibErrorResponse>() else {
            return anyhow!("TonCenter API returned status: {status}");
        };

        let raw_msg = data
            .error
            .trim_start_matches("LITE_SERVER_UNKNOWN: ")
            .to_owned();

        if let Some(message) = normalize_toncenter_error_message(&raw_msg) {
            return anyhow!(message);
        }

        anyhow!(raw_msg)
    }

    fn handle_send_boc_fail(response: Response) -> SendBocError {
        let status = response.status();
        let Ok(data) = response.json::<v2::TonlibErrorResponse>() else {
            return SendBocError::new(
                SendBocErrorKind::Other,
                format!("TonCenter API returned status: {status}"),
            );
        };

        let raw_msg = data
            .error
            .trim_start_matches("LITE_SERVER_UNKNOWN: ")
            .to_owned();

        SendBocError::new(classify_toncenter_send_boc_error(&raw_msg), raw_msg)
    }
}

fn test_retry_backoff_override() -> Option<Duration> {
    let value = env::var(TEST_TONCENTER_RETRY_BACKOFF_MS_ENV).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    value.parse::<u64>().ok().map(Duration::from_millis)
}

fn toncenter_min_request_interval() -> Duration {
    env::var(TEST_TONCENTER_MIN_REQUEST_INTERVAL_MS_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map_or(TONCENTER_MIN_REQUEST_INTERVAL, Duration::from_millis)
}

fn classify_toncenter_send_boc_error(raw_msg: &str) -> SendBocErrorKind {
    if raw_msg == "cannot apply external message to current state : Failed to unpack account state"
    {
        return SendBocErrorKind::MissingAccountState;
    }

    if raw_msg.starts_with(
        "cannot apply external message to current state : External message was not accepted: cannot run message on account:",
    ) && raw_msg.contains("before smart-contract execution")
    {
        return SendBocErrorKind::RejectedBeforeExecution;
    }

    SendBocErrorKind::Other
}

fn normalize_toncenter_error_message(raw_msg: &str) -> Option<&'static str> {
    if raw_msg == "cannot apply external message to current state : Failed to unpack account state"
    {
        return Some(
            "external message not accepted because account has no state; check if wallet/contract is deployed",
        );
    }

    if raw_msg.starts_with(
        "cannot apply external message to current state : External message was not accepted: cannot run message on account:",
    ) && raw_msg.contains("before smart-contract execution")
    {
        return Some(
            "wallet/contract rejected the external message before contract execution; likely causes:
- not enough balance
- wallet/contract is not deployed
- seqno is stale
- message expired",
        );
    }

    None
}

impl TonApiClient {
    /// Fetch traces that include a message with the given hash using toncenter v3.
    ///
    /// `msg_hash` is accepted in hex, base64, or base64url form. A transaction may be part
    /// of at most one trace, so callers typically want the first (or only) result. Pass
    /// the TEP-467 `hash_norm` from `sendBocReturnHash` to avoid indexer false-misses on
    /// cell-layout variations.
    pub fn get_traces_by_msg_hash(
        &self,
        msg_hash: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<v3::Trace>> {
        self.get_traces_by_hash_param("msg_hash", msg_hash, limit)
    }

    /// Fetch a trace by its root transaction hash using toncenter v3.
    pub fn get_traces_by_tx_hash(
        &self,
        tx_hash: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<v3::Trace>> {
        self.get_traces_by_hash_param("tx_hash", tx_hash, limit)
    }

    fn get_traces_by_hash_param(
        &self,
        hash_param: &str,
        hash: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<v3::Trace>> {
        let url = format!(
            "{}/traces",
            self.network.toncenter_v3_url(&self.custom_networks)?
        );

        let params: Vec<(&str, String)> =
            vec![(hash_param, hash.to_owned()), ("limit", limit.to_string())];

        let response = self.send_with_retry(
            || self.build_request(&url).query(&params),
            "Failed to send traces request",
        )?;

        if !response.status().is_success() {
            anyhow::bail!("TonCenter v3 traces returned status: {}", response.status());
        }

        let data: v3::TracesResponse =
            response.json().context("Failed to parse traces response")?;
        Ok(data.traces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;
    use std::sync::Arc;

    #[test]
    fn acton_use_proxy_is_disabled_by_default() {
        assert!(!proxy_enabled_from_value(None));
    }

    #[test]
    fn acton_use_proxy_accepts_1_or_true() {
        for value in ["1", "true"] {
            assert!(proxy_enabled_from_value(Some(OsStr::new(value))));
        }
    }

    #[test]
    fn acton_use_proxy_rejects_other_values() {
        for value in ["", "0", "false", "TRUE", "yes"] {
            assert!(!proxy_enabled_from_value(Some(OsStr::new(value))));
        }
    }

    #[test]
    fn normalize_toncenter_error_message_maps_missing_account_state() {
        assert_eq!(
            normalize_toncenter_error_message(
                "cannot apply external message to current state : Failed to unpack account state",
            ),
            Some(
                "external message not accepted because account has no state; check if wallet/contract is deployed",
            ),
        );
    }

    #[test]
    fn normalize_toncenter_error_message_maps_pre_execution_wallet_rejection() {
        assert_eq!(
            normalize_toncenter_error_message(
                "cannot apply external message to current state : External message was not accepted: cannot run message on account: inbound external message rejected by account 3029B3EAEDA86A5381D86100F2A8B761C38DE45642EDB6E4BB1CCA2E6DD7FFED before smart-contract execution",
            ),
            Some(
                r"wallet/contract rejected the external message before contract execution; likely causes:
- not enough balance
- wallet/contract is not deployed
- seqno is stale
- message expired",
            ),
        );
    }

    #[test]
    fn normalize_toncenter_error_message_preserves_other_errors() {
        assert_eq!(
            normalize_toncenter_error_message("mock toncenter failure"),
            None,
        );
    }

    #[test]
    fn masterchain_snapshot_cache_round_trips() {
        let temp_dir = tempfile::tempdir().expect("temporary cache directory");
        let network = Network::Custom(Arc::from("snapshot-test"));
        let snapshot = MasterchainSnapshot {
            seqno: 123_456,
            gen_utime: 1_700_000_000,
            config: Cell::default(),
        };
        let entry = MasterchainSnapshotCacheEntry::new(
            &network,
            "http://127.0.0.1:8080/api/v2".to_owned(),
            &snapshot,
            Some(snapshot.gen_utime),
            100,
        );
        let path = masterchain_snapshot_cache_path(temp_dir.path(), &network, snapshot.seqno);

        write_masterchain_snapshot_cache(&path, &entry).expect("write snapshot cache");
        let cached = read_masterchain_snapshot_cache(
            &path,
            &network,
            "http://127.0.0.1:8080/api/v2",
            snapshot.seqno,
        )
        .expect("read snapshot cache");

        assert_eq!(cached.gen_utime, Some(snapshot.gen_utime));
        assert_eq!(cached.config.repr_hash(), snapshot.config.repr_hash());
        let latest_cached = MasterchainSnapshotCacheEntry::new(
            &network,
            "http://127.0.0.1:8080/api/v2".to_owned(),
            &snapshot,
            None,
            100,
        )
        .into_cached_snapshot()
        .expect("decode latest snapshot cache");
        assert_eq!(latest_cached.gen_utime, None);
        assert!(
            read_masterchain_snapshot_cache(
                &path,
                &network,
                "http://127.0.0.1:8081/api/v2",
                snapshot.seqno,
            )
            .is_none(),
            "cache from another endpoint must not be reused"
        );
    }

    #[test]
    fn masterchain_snapshot_cache_expires_after_ttl() {
        assert!(masterchain_snapshot_cache_entry_is_fresh(
            100,
            100 + MASTERCHAIN_SNAPSHOT_CACHE_TTL.as_secs() - 1
        ));
        assert!(!masterchain_snapshot_cache_entry_is_fresh(
            100,
            100 + MASTERCHAIN_SNAPSHOT_CACHE_TTL.as_secs()
        ));
    }

    #[test]
    fn masterchain_snapshot_cache_separates_builtin_and_custom_networks() {
        let root = Path::new("/tmp/acton-project/build/cache/masterchain-snapshots");
        let builtin = masterchain_snapshot_cache_path(root, &Network::Mainnet, 42);
        let custom =
            masterchain_snapshot_cache_path(root, &Network::Custom(Arc::from("mainnet")), 42);

        assert_ne!(builtin, custom);
        assert_eq!(builtin, root.join("mainnet").join("42.json"));
        assert_eq!(custom, root.join("custom-mainnet").join("42.json"));
    }
}
