#[cfg(test)]
mod tests;

use ::toncenter::{v2, v3};
use anyhow::{Context, anyhow};
use num_bigint::BigInt;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
pub use ton_networks::{CustomNetworkUrls, Network};
use toncenter_client::V2Transport;
use toncenter_keys::api_key as toncenter_api_key;
use tvm_ffi::stack::TupleItem;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, HashBytes};

mod blocking;
mod deployment;
mod offchain;
pub mod toncenter;

pub use deployment::{DeploymentCandidate, extract_deployment_candidates};
pub use offchain::OffchainJsonResolver;

const USE_PROXY_ENV: &str = "ACTON_USE_PROXY";
const TEST_TONCENTER_RETRY_BACKOFF_MS_ENV: &str = "ACTON_TEST_TONCENTER_RETRY_BACKOFF_MS";
const TEST_TONCENTER_MIN_REQUEST_INTERVAL_MS_ENV: &str =
    "ACTON_TEST_TONCENTER_MIN_REQUEST_INTERVAL_MS";
pub const MASTERCHAIN_SNAPSHOT_CACHE_SUBDIR: &str = "masterchain-snapshots";
const MASTERCHAIN_SNAPSHOT_CACHE_SCHEMA_VERSION: u32 = 2;
const MASTERCHAIN_SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

const fn user_agent() -> &'static str {
    concat!("acton/", env!("CARGO_PKG_VERSION"))
}

fn async_http_client_builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder()
        .use_rustls_tls()
        .user_agent(user_agent());
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

#[derive(Clone)]
pub struct TonApiClient {
    client: blocking::BlockingClient,
    network: Network,
    has_api_key: bool,
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
        let options = toncenter_client::Client::builder()
            .user_agent(user_agent())
            .api_key(toncenter_api_key(&network));
        Self::with_options(network, custom_networks, options)
    }

    /// Creates an Acton client with application headers and transport options.
    /// Network URLs and Acton's proxy preference are applied here; the caller supplies credentials.
    pub fn with_options(
        network: Network,
        custom_networks: HashMap<String, CustomNetworkUrls>,
        options: toncenter_client::ClientBuilder,
    ) -> anyhow::Result<Self> {
        let mut builder = options
            .v2_url(network.toncenter_v2_url(&custom_networks)?)
            .system_proxy(proxy_enabled());
        if let Ok(url) = network.toncenter_v3_url(&custom_networks) {
            builder = builder.v3_url(url);
        }
        if let Some(delay) = test_retry_backoff_override() {
            builder = builder.retry_delays(delay, delay);
        }
        if let Ok(interval) = env::var(TEST_TONCENTER_MIN_REQUEST_INTERVAL_MS_ENV)
            && let Ok(interval) = interval.trim().parse::<u64>()
        {
            builder = builder.request_interval(Duration::from_millis(interval));
        }
        let client = blocking::BlockingClient::new(builder)?;
        let has_api_key = client.call(|client| async move { Ok(client.has_api_key()) })?;
        Ok(Self {
            client,
            has_api_key,
            network,
            custom_networks,
        })
    }

    #[must_use]
    pub const fn has_api_key(&self) -> bool {
        self.has_api_key
    }

    fn get_v2_result<T: DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        query: &(impl Serialize + Clone + Send + Sync + 'static),
    ) -> anyhow::Result<T> {
        let method = path.trim_start_matches('/').to_owned();
        let query = query.clone();
        self.client
            .call(move |client| async move {
                client.v2_request(V2Transport::Get, &method, &query).await
            })
            .map_err(Self::handle_fail)
    }

    fn get_v3<T: DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        query: &(impl Serialize + Clone + Send + Sync + 'static),
    ) -> anyhow::Result<T> {
        let path = path.to_owned();
        let query = query.clone();
        self.client
            .call(move |client| async move { client.v3_get(&path, &query).await })
    }

    fn get_v3_raw<T: DeserializeOwned + Send + 'static>(
        &self,
        path: &str,
        raw_query: &str,
    ) -> anyhow::Result<T> {
        let mut url = reqwest::Url::parse("http://localhost")?;
        url.set_query(Some(raw_query));
        let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
        self.get_v3(path, &query)
    }

    #[must_use]
    pub fn network(&self) -> Network {
        self.network.clone()
    }

    /// Get account state from `TON Center`
    pub fn get_account_state(&self, address: &str) -> anyhow::Result<v3::AccountStateFull> {
        let accounts = self.get_account_states(&[address])?;
        accounts
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Account not found"))
    }

    /// Get multiple account states from `TON Center`
    pub fn get_account_states(
        &self,
        addresses: &[&str],
    ) -> anyhow::Result<Vec<v3::AccountStateFull>> {
        if addresses.is_empty() {
            return Ok(vec![]);
        }

        let query: Vec<_> = addresses
            .iter()
            .map(|address| ("address", (*address).to_owned()))
            .collect();
        let data: v3::AccountStatesResponse = self.get_v3("accountStates", &query)?;

        Ok(data.accounts)
    }

    /// Get contract BOC from `TON Center` (tries mainnet first, then testnet)
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
    ) -> anyhow::Result<v2::responses::RunGetMethodResult<serde_json::Value>> {
        self.run_get_method_at_block(address, method, stack, None)
    }

    /// Run get method on contract at a specific masterchain block, when provided.
    pub fn run_get_method_at_block(
        &self,
        address: &str,
        method: &str,
        stack: &[serde_json::Value],
        seqno: Option<u64>,
    ) -> anyhow::Result<v2::responses::RunGetMethodResult<serde_json::Value>> {
        let seqno = seqno
            .map(i32::try_from)
            .transpose()
            .context("Masterchain seqno does not fit TON Center v2 request")?;
        let request = v2::requests::RunGetMethodRequest {
            address: address.to_owned(),
            method: method.into(),
            stack: stack.to_vec(),
            seqno,
        };
        self.client.call(move |client| async move {
            client
                .v2_request(V2Transport::JsonRpc, "runGetMethod", &request)
                .await
        })
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

        let stack = tvm_ffi::json_stack::json_to_legacy_stack(result.stack)
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
        let request = v2::requests::SendBocRequest {
            boc: boc.to_owned(),
        };
        self.client.call(move |client| async move {
            client.call_v2::<v2::endpoints::SendBoc>(&request).await
        }).map(|_| ()).map_err(Self::handle_send_boc_fail)
    }

    pub fn get_masterchain_info(
        &self,
    ) -> anyhow::Result<v2::TonlibResponse<v2::responses::MasterchainInfo>> {
        self.client
            .call(|client| async move {
                client
                    .v2_response(
                        V2Transport::Get,
                        "getMasterchainInfo",
                        &v2::requests::EmptyRequest {},
                    )
                    .await
            })
            .map_err(Self::handle_fail)
    }

    pub fn get_last_block_seqno(&self) -> anyhow::Result<u64> {
        u64::try_from(self.get_masterchain_info()?.result.last.seqno)
            .context("Invalid masterchain seqno")
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
            i32::try_from(seqno).context("Masterchain seqno does not fit TON Center v2 request")?;
        let header = self.get_block_header_v2(&v2::requests::BlockHeaderRequest {
            workchain: (-1).into(),
            shard: i64::MIN.into(),
            seqno: request_seqno.into(),
            root_hash: None,
            file_hash: None,
        })?;
        u32::try_from(header.gen_utime).context("Invalid block timestamp")
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
        self.get_v3_raw("blocks", raw_query)
    }

    pub fn get_transactions_v3(&self, raw_query: &str) -> anyhow::Result<v3::TransactionsResponse> {
        self.get_v3_raw("transactions", raw_query)
    }

    pub fn get_shards(&self, seqno: u32) -> anyhow::Result<v2::responses::Shards> {
        self.get_v2_result("getShards", &[("seqno", seqno)])
    }

    pub fn get_block_header_v2(
        &self,
        request: &v2::requests::BlockHeaderRequest,
    ) -> anyhow::Result<v2::responses::BlockHeader> {
        self.get_v2_result("/getBlockHeader", request)
    }

    /// Fetches the exact serialized block selected by a `TON Center` v2 block request.
    pub fn get_block_v2(
        &self,
        request: &v2::requests::BlockDataRequest,
    ) -> anyhow::Result<v2::responses::BlockData> {
        self.get_v2_result("/getBlock", request)
    }

    pub fn get_block_transactions_v2(
        &self,
        request: &v2::requests::BlockTransactionsRequest,
    ) -> anyhow::Result<v2::responses::BlockTransactions> {
        self.get_v2_result("/getBlockTransactions", request)
    }

    pub fn get_block_transactions_ext_v2(
        &self,
        request: &v2::requests::BlockTransactionsRequest,
    ) -> anyhow::Result<v2::responses::BlockTransactionsExt> {
        self.get_v2_result("/getBlockTransactionsExt", request)
    }

    pub fn lookup_block_v2(
        &self,
        request: &v2::requests::LookupBlockRequest,
    ) -> anyhow::Result<v2::responses::TonBlockIdExt> {
        self.get_v2_result("/lookupBlock", request)
    }

    pub fn get_account_info(
        &self,
        seqno: Option<u64>,
        address: &str,
    ) -> anyhow::Result<v2::responses::AddressInformation> {
        self.get_v2_result(
            "getAddressInformation",
            &v2::requests::AddressInformationRequest {
                address: address.to_owned(),
                seqno: seqno
                    .map(i32::try_from)
                    .transpose()
                    .context("Masterchain seqno does not fit i32")?
                    .map(Into::into),
            },
        )
    }

    pub fn get_shard_account_cell(
        &self,
        seqno: Option<u64>,
        address: &str,
    ) -> anyhow::Result<Cell> {
        let data: v2::stack::TvmCell = self.get_v2_result(
            "getShardAccountCell",
            &v2::requests::AddressInformationRequest {
                address: address.to_owned(),
                seqno: seqno
                    .map(i32::try_from)
                    .transpose()
                    .context("Masterchain seqno does not fit i32")?
                    .map(Into::into),
            },
        )?;
        Boc::decode_base64(&data.bytes).context("Failed to decode shard account cell BOC data")
    }

    pub fn get_library_by_hash(&self, hash: &HashBytes) -> anyhow::Result<Cell> {
        let hash_hex = hash.to_string();
        let data: v2::responses::LibraryResult =
            self.get_v2_result("getLibraries", &[("libraries", hash_hex.clone())])?;
        let boc_data = data
            .result
            .first()
            .map(|entry| entry.data.as_str())
            .ok_or_else(|| anyhow!("Library with hash {hash_hex} not found"))?;
        Boc::decode_base64(boc_data).context("Failed to decode library BOC data")
    }

    pub fn get_config_all(&self, seqno: Option<u64>) -> anyhow::Result<Cell> {
        let seqno = seqno
            .map(i32::try_from)
            .transpose()
            .context("Masterchain seqno does not fit TON Center v2 request")?;
        let data: v2::responses::ConfigInfo = self.get_v2_result(
            "/getConfigAll",
            &v2::requests::ConfigAllRequest {
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
    ) -> anyhow::Result<Vec<v2::responses::Transaction>> {
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

        self.get_v2_result("getTransactions", &params)
    }

    pub fn get_address_balance(&self, address: &str) -> anyhow::Result<BigInt> {
        let balance: String = self.get_v2_result(
            "getAddressBalance",
            &v2::requests::AddressBalanceRequest {
                address: address.to_owned(),
                seqno: None,
            },
        )?;
        balance.parse().context("Invalid account balance")
    }

    fn handle_fail(error: anyhow::Error) -> anyhow::Error {
        let Some(api) = error
            .downcast_ref::<toncenter_client::Error>()
            .and_then(toncenter_client::Error::api_error)
        else {
            return error;
        };
        let message = api.message.trim_start_matches("LITE_SERVER_UNKNOWN: ");
        anyhow!(
            normalize_toncenter_error_message(message)
                .unwrap_or(message)
                .to_owned()
        )
    }

    fn handle_send_boc_fail(error: anyhow::Error) -> SendBocError {
        if let Some(error) = error.downcast_ref::<toncenter_client::Error>() {
            if let Some(api) = error.api_error() {
                let message = api.message.trim_start_matches("LITE_SERVER_UNKNOWN: ");
                return SendBocError::new(classify_toncenter_send_boc_error(message), message);
            }
            if matches!(
                error.kind(),
                toncenter_client::ErrorKind::Transport | toncenter_client::ErrorKind::Timeout
            ) {
                return SendBocError::new(SendBocErrorKind::TransportFailure, error.to_string());
            }
        }
        SendBocError::new(SendBocErrorKind::Other, error.to_string())
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
    /// `msg_hash` is accepted in hex, base64, or base64url form. Repeated external
    /// messages can match multiple executions; `start_utime` excludes traces that
    /// started before that Unix second, before the server applies `limit`. Pass
    /// the TEP-467 `hash_norm` from `sendBocReturnHash` to avoid indexer false-misses on
    /// cell-layout variations.
    pub fn get_traces_by_msg_hash(
        &self,
        msg_hash: &str,
        limit: u32,
        start_utime: Option<i64>,
    ) -> anyhow::Result<Vec<v3::Trace>> {
        self.get_traces_by_hash_param("msg_hash", msg_hash, limit, start_utime)
    }

    /// Fetch a trace by its root transaction hash using toncenter v3.
    pub fn get_traces_by_tx_hash(
        &self,
        tx_hash: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<v3::Trace>> {
        self.get_traces_by_hash_param("tx_hash", tx_hash, limit, None)
    }

    fn get_traces_by_hash_param(
        &self,
        hash_param: &'static str,
        hash: &str,
        limit: u32,
        start_utime: Option<i64>,
    ) -> anyhow::Result<Vec<v3::Trace>> {
        let mut params: Vec<(&str, String)> =
            vec![(hash_param, hash.to_owned()), ("limit", limit.to_string())];
        if let Some(start_utime) = start_utime {
            params.push(("start_utime", start_utime.to_string()));
            params.push(("sort", "desc".to_owned()));
        }

        let data: v3::TracesResponse = self.get_v3("traces", &params)?;
        Ok(data.traces)
    }
}
