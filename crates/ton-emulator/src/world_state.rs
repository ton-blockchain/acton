//! This module provides the emulated world state management.
//!
//! It includes logic for handling account states, logical time (LT), current unix time,
//! and global libraries. The state can be managed purely locally or forked from a remote
//! TON network (mainnet or testnet).

use acton_config::config::{ActonConfig, project_root as configured_project_root};
use anyhow::{Context, anyhow};
use base64::Engine;
use num_traits::cast::ToPrimitive;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use ton_api::TonApiClient;
use ton_executor::{DEFAULT_CONFIG, DEFAULT_CONFIG_CELL, DEFAULT_CONFIG_DICT};
use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, HashBytes, Lazy, Store};
use tycho_types::dict;
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount, StateInit,
    StdAddr, StdAddrFormat, StorageInfo,
};

const WORLD_STATE_SNAPSHOT_VERSION: u32 = 1;
const FORK_ACCOUNT_CACHE_SCHEMA_VERSION: u32 = 1;
const EXOTIC_LIBRARY_TAG: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldStateSnapshot {
    pub version: u32,
    pub current_lt: u64,
    pub current_now: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub random_seed: Option<String>,
    pub config_boc64: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries_boc64: Vec<String>,
    pub accounts: Vec<WorldStateAccountSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldStateAccountSnapshot {
    pub address: String,
    pub shard_account_boc64: String,
}

/// Represents the source of the world state.
///
/// It can either be purely local or partially remote (forked from a network).
#[allow(clippy::large_enum_variant)]
pub enum AccountsState {
    /// Purely local state, stored in memory.
    Local(LocalAccountsState),
    /// State forked from a remote network, with local overrides.
    Remote(RemoteAccountState),
}

impl AccountsState {
    /// Retrieves an account by its address.
    ///
    /// If the account is not found in the local cache, it might be fetched from the remote network
    /// if the state is `Remote`.
    ///
    /// # Arguments
    ///
    /// * `address` - The raw address of the account.
    /// * `current_lt` - The current logical time of the world state.
    pub fn retrieve(&mut self, address: &StdAddr, current_lt: u64) -> ShardAccount {
        match self {
            Self::Local(r) => r.retrieve(address, current_lt),
            Self::Remote(r) => r.retrieve(address, current_lt),
        }
    }

    /// Updates or inserts an account in the local state.
    ///
    /// # Arguments
    ///
    /// * `address` - The raw address of the account.
    /// * `account` - The new shard account data.
    pub fn update(&mut self, address: &StdAddr, account: ShardAccount) {
        match self {
            Self::Local(r) => r.update(address, account),
            Self::Remote(r) => r.update(address, account),
        }
    }

    /// Invalidates cached remote accounts. Local state is left untouched.
    pub fn invalidate_remote_cache(&mut self) {
        if let Self::Remote(r) = self {
            r.invalidate_cache();
        }
    }

    /// Returns a reference to the underlying map of accounts.
    #[must_use]
    pub const fn accounts(&self) -> &FxHashMap<StdAddr, ShardAccount> {
        match self {
            Self::Local(r) => &r.accounts,
            Self::Remote(r) => &r.accounts,
        }
    }

    #[must_use]
    pub fn take_accounts(self) -> FxHashMap<StdAddr, ShardAccount> {
        match self {
            Self::Local(r) => r.accounts,
            Self::Remote(r) => r.accounts,
        }
    }
}

/// A purely local implementation of the world state.
pub struct LocalAccountsState {
    pub accounts: FxHashMap<StdAddr, ShardAccount>,
}

impl Default for LocalAccountsState {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalAccountsState {
    /// Creates a new empty local state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            accounts: FxHashMap::default(),
        }
    }

    fn retrieve(&mut self, address: &StdAddr, current_lt: u64) -> ShardAccount {
        if let Some(acc) = self.accounts.get(address) {
            return acc.clone();
        }

        let acc = ShardAccount {
            account: Lazy::new(&OptionalAccount(None)).expect("Failed to create empty account"),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: current_lt,
        };
        self.accounts.insert(address.clone(), acc.clone());
        acc
    }

    fn update(&mut self, address: &StdAddr, account: ShardAccount) {
        self.accounts.insert(address.clone(), account);
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RemoteCacheKey {
    fork_block_number: Option<u64>,
    fork_net: Network,
    address: StdAddr,
}

#[derive(Clone, Debug)]
pub struct RemoteSnapshotCache {
    inner: Rc<RefCell<HashMap<RemoteCacheKey, ShardAccount>>>,
}

impl Default for RemoteSnapshotCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteSnapshotCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn get(&self, key: &RemoteCacheKey) -> Option<ShardAccount> {
        self.inner.borrow().get(key).cloned()
    }

    pub fn insert(&self, key: RemoteCacheKey, val: ShardAccount) {
        self.inner.borrow_mut().insert(key, val);
    }

    pub fn clear(&self) {
        self.inner.borrow_mut().clear();
    }
}

#[derive(Clone, Debug)]
pub struct RemoteLibraryCache {
    inner: Rc<RefCell<HashMap<HashBytes, Cell>>>,
}

impl Default for RemoteLibraryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteLibraryCache {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    #[must_use]
    pub fn get(&self, hash: &HashBytes) -> Option<Cell> {
        self.inner.borrow().get(hash).cloned()
    }

    pub fn insert(&self, hash: HashBytes, lib: Cell) {
        self.inner.borrow_mut().insert(hash, lib);
    }

    pub fn clear(&self) {
        self.inner.borrow_mut().clear();
    }
}

#[derive(Clone, Debug)]
struct RemoteDiskCache {
    dir: PathBuf,
    fork_net: String,
    fork_block_number: u64,
}

#[derive(Clone, Debug)]
struct RemoteLibraryDiskCache {
    dir: PathBuf,
}

#[derive(Debug, Serialize, Deserialize)]
struct RemoteDiskCacheEntry {
    schema_version: u32,
    fork_net: String,
    fork_block_number: u64,
    address: String,
    shard_account_boc64: String,
    timestamp: u64,
}

impl RemoteDiskCache {
    fn new(dir: PathBuf, fork_net: &Network, fork_block_number: u64) -> Self {
        Self {
            dir,
            fork_net: fork_net.to_string(),
            fork_block_number,
        }
    }

    fn cache_file_path(&self, address: &StdAddr) -> PathBuf {
        self.dir
            .join(format!("{}.json", address_cache_key(address)))
    }

    fn read(&self, address: &StdAddr) -> Option<ShardAccount> {
        let path = self.cache_file_path(address);
        let entry =
            serde_json::from_reader::<_, RemoteDiskCacheEntry>(fs::File::open(path).ok()?).ok()?;

        if entry.schema_version != FORK_ACCOUNT_CACHE_SCHEMA_VERSION
            || entry.fork_net != self.fork_net
            || entry.fork_block_number != self.fork_block_number
            || entry.address != address.to_string()
        {
            return None;
        }

        decode_shard_account_boc64(&entry.shard_account_boc64).ok()
    }

    fn write(&self, address: &StdAddr, account: &ShardAccount) -> anyhow::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let entry = RemoteDiskCacheEntry {
            schema_version: FORK_ACCOUNT_CACHE_SCHEMA_VERSION,
            fork_net: self.fork_net.clone(),
            fork_block_number: self.fork_block_number,
            address: address.to_string(),
            shard_account_boc64: encode_shard_account_boc64(account)?,
            timestamp: now.as_secs(),
        };

        let path = self.cache_file_path(address);
        let temp_suffix = now.as_nanos();
        let temp_path =
            path.with_extension(format!("json.{}.{}.tmp", std::process::id(), temp_suffix));
        fs::write(&temp_path, serde_json::to_vec_pretty(&entry)?)?;
        fs::rename(&temp_path, &path).or_else(|_| {
            let contents = fs::read(&temp_path)?;
            fs::write(&path, contents)?;
            fs::remove_file(&temp_path).ok();
            Ok::<_, std::io::Error>(())
        })?;
        Ok(())
    }
}

impl RemoteLibraryDiskCache {
    const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn cache_file_path(&self, hash: &HashBytes) -> PathBuf {
        self.dir.join(format!("{hash}.boc"))
    }

    fn read(&self, hash: &HashBytes) -> Option<Cell> {
        let path = self.cache_file_path(hash);
        let bytes = fs::read(path).ok()?;
        let cell = Boc::decode(&bytes).ok()?;
        (*cell.repr_hash() == *hash).then_some(cell)
    }

    fn write(&self, hash: &HashBytes, lib: &Cell) -> anyhow::Result<()> {
        validate_library_hash(hash, lib, "Library cache entry")?;

        fs::create_dir_all(&self.dir)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let path = self.cache_file_path(hash);
        let temp_path =
            path.with_extension(format!("boc.{}.{}.tmp", std::process::id(), now.as_nanos()));
        fs::write(&temp_path, Boc::encode(lib))?;
        fs::rename(&temp_path, &path).or_else(|_| {
            let contents = fs::read(&temp_path)?;
            fs::write(&path, contents)?;
            fs::remove_file(&temp_path).ok();
            Ok::<_, std::io::Error>(())
        })?;
        Ok(())
    }
}

#[must_use]
fn fork_account_cache_dir(
    project_root: &Path,
    fork_net: &Network,
    fork_block_number: u64,
) -> PathBuf {
    project_root
        .join("build")
        .join("cache")
        .join(sanitize_cache_path_component(&fork_net.to_string()))
        .join(fork_block_number.to_string())
}

#[must_use]
fn fork_library_cache_dir(project_root: &Path) -> PathBuf {
    project_root.join("build").join("cache").join("libraries")
}

fn sanitize_cache_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn address_cache_key(address: &StdAddr) -> String {
    format!(
        "{}_{}",
        address.workchain,
        hex::encode(address.address.as_array())
    )
}

/// A state implementation that fetches missing accounts from a remote network.
pub struct RemoteAccountState {
    /// Local cache and overrides for accounts.
    pub accounts: FxHashMap<StdAddr, ShardAccount>,
    /// The network to fork from (e.g., "mainnet", "testnet").
    pub fork_net: Network,
    /// Optional block number to pin the state to.
    pub fork_block_number: Option<u64>,

    /// Shared API client for network fetches.
    api_client: OnceCell<TonApiClient>,
    /// Cache for less network queries in subsequent tests.
    cache: RemoteSnapshotCache,
    /// Cache for remote global libraries in subsequent tests.
    library_cache: RemoteLibraryCache,
    /// Persistent cache used across CLI process restarts for pinned forks.
    disk_cache: Option<RemoteDiskCache>,
    /// Persistent content-addressed global library cache.
    library_disk_cache: Option<RemoteLibraryDiskCache>,
}

impl RemoteAccountState {
    /// Creates a new remote state for the given network.
    #[must_use]
    pub fn new(
        fork_net: Network,
        fork_block_number: Option<u64>,
        cache: RemoteSnapshotCache,
        library_cache: RemoteLibraryCache,
        fork_cache_enabled: bool,
    ) -> Self {
        let disk_cache = match (fork_cache_enabled, fork_block_number) {
            (true, Some(fork_block_number)) => {
                let dir =
                    fork_account_cache_dir(configured_project_root(), &fork_net, fork_block_number);
                Some(RemoteDiskCache::new(dir, &fork_net, fork_block_number))
            }
            _ => None,
        };
        let library_disk_cache = fork_cache_enabled.then(|| {
            RemoteLibraryDiskCache::new(fork_library_cache_dir(configured_project_root()))
        });

        Self {
            accounts: FxHashMap::default(),
            fork_net,
            fork_block_number,
            api_client: OnceCell::new(),
            cache,
            library_cache,
            disk_cache,
            library_disk_cache,
        }
    }

    fn retrieve(&mut self, address: &StdAddr, current_lt: u64) -> ShardAccount {
        if let Some(acc) = self.accounts.get(address) {
            return acc.clone();
        }

        match self.resolve_remote_account(address) {
            Ok(acc) => {
                self.accounts.insert(address.clone(), acc.clone());
                acc
            }
            Err(err) => {
                eprintln!("Failed to resolve address {address}: {err}");

                // don't cache account on error
                ShardAccount {
                    account: Lazy::new(&OptionalAccount(None))
                        .expect("Failed to create empty account"),
                    last_trans_hash: HashBytes::ZERO,
                    last_trans_lt: current_lt,
                }
            }
        }
    }

    fn update(&mut self, address: &StdAddr, account: ShardAccount) {
        self.accounts.insert(address.clone(), account);
    }

    fn invalidate_cache(&mut self) {
        self.accounts.clear();
        self.cache.clear();
        self.library_cache.clear();
    }

    fn resolve_remote_account(&self, address: &StdAddr) -> anyhow::Result<ShardAccount> {
        // return cached version if it already resolved earlier in current suite
        let cache_key = RemoteCacheKey {
            fork_block_number: self.fork_block_number,
            fork_net: self.fork_net.clone(),
            address: address.clone(),
        };
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }
        if let Some(cached) = self
            .disk_cache
            .as_ref()
            .and_then(|disk_cache| disk_cache.read(address))
        {
            self.cache.insert(cache_key, cached.clone());
            return Ok(cached);
        }

        let api_client = self.api_client()?;
        if let Ok(cell) =
            api_client.get_shard_account_cell(self.fork_block_number, &address.to_string())
        {
            let acc = cell
                .parse::<ShardAccount>()
                .context("Failed to parse getShardAccountCell response as ShardAccount")?;
            self.store_resolved_account(cache_key, address, &acc);
            return Ok(acc);
        }

        // Fallback to previous method
        let info = api_client.get_account_info(self.fork_block_number, &address.to_string())?;

        let balance = info
            .balance
            .to_bigint()?
            .to_u128()
            .ok_or_else(|| anyhow!("Failed to convert balance to u128"))?;

        let account_state = match info.state.as_str() {
            "active" => AccountState::Active(StateInit {
                code: TonApiClient::decode_optional_cell(&info.code)?,
                data: TonApiClient::decode_optional_cell(&info.data)?,
                ..Default::default()
            }),
            "uninitialized" => AccountState::Uninit,
            "frozen" => AccountState::Frozen(HashBytes::from_str(info.frozen_hash.as_str())?),
            _ => {
                anyhow::bail!("Unknown account state: {}", info.state);
            }
        };

        let last_trans_lt = info.last_transaction_id.lt.parse::<u64>()?;
        // TonCenter returns the transaction start LT. AccountStorage stores the
        // end LT, which is not available in v2, so use the minimal valid value.
        let storage_last_trans_lt = last_trans_lt.saturating_add(1);
        let last_trans_hash = decode_toncenter_hash(&info.last_transaction_id.hash)?;

        let acc = ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                balance: CurrencyCollection::new(balance),
                address: IntAddr::Std(address.clone()),
                last_trans_lt: storage_last_trans_lt,
                state: account_state,
                storage_stat: StorageInfo::default(),
            })))?,
            last_trans_hash,
            last_trans_lt,
        };
        self.store_resolved_account(cache_key, address, &acc);
        Ok(acc)
    }

    fn store_resolved_account(
        &self,
        cache_key: RemoteCacheKey,
        address: &StdAddr,
        account: &ShardAccount,
    ) {
        self.cache.insert(cache_key, account.clone());
        if let Some(disk_cache) = &self.disk_cache {
            let _ = disk_cache.write(address, account);
        }
    }

    fn api_client(&self) -> anyhow::Result<&TonApiClient> {
        if self.api_client.get().is_none() {
            let custom_networks = ActonConfig::load()
                .ok()
                .map(|config| config.custom_networks())
                .unwrap_or_default();
            let client = TonApiClient::new(self.fork_net.clone(), custom_networks)?;
            let _ = self.api_client.set(client);
        }

        self.api_client
            .get()
            .ok_or_else(|| anyhow!("Failed to initialize Ton API client"))
    }

    fn load_library(&self, hash: &HashBytes) -> anyhow::Result<Cell> {
        if let Some(lib) = self.library_cache.get(hash) {
            return Ok(lib);
        }

        if let Some(lib) = self
            .library_disk_cache
            .as_ref()
            .and_then(|disk_cache| disk_cache.read(hash))
        {
            self.library_cache.insert(*hash, lib.clone());
            return Ok(lib);
        }

        let lib = self.api_client()?.get_library_by_hash(hash)?;
        validate_library_hash(hash, &lib, "Fetched library")?;

        self.library_cache.insert(*hash, lib.clone());
        if let Some(disk_cache) = &self.library_disk_cache {
            let _ = disk_cache.write(hash, &lib);
        }
        Ok(lib)
    }
}

pub(crate) fn decode_toncenter_hash(hash: &str) -> anyhow::Result<HashBytes> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(hash)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(hash))
        .map_err(|err| anyhow!("Invalid TonCenter transaction hash '{hash}': {err}"))?;
    let len = decoded.len();
    let bytes: [u8; 32] = decoded
        .try_into()
        .map_err(|_| anyhow!("TonCenter transaction hash must be 32 bytes, got {len}"))?;

    Ok(HashBytes(bytes))
}

/// The main entry point for interacting with the emulated world state.
///
/// It manages logical time, current time, global libraries, and provides access
/// to the underlying account state.
///
/// # Examples
///
/// ```
/// use ton_emulator::world_state::{WorldState, AccountsState, LocalAccountsState};
///
/// let mut world_state = WorldState::new(AccountsState::Local(LocalAccountsState::new()), None).expect("Failed to create world state");
/// assert_eq!(world_state.get_now(), 0);
/// world_state.set_now(1000);
/// assert_eq!(world_state.get_now(), 1000);
/// ```
pub struct WorldState {
    /// The source of the state (local or remote). Contains the account cache.
    accounts_state: AccountsState,
    /// The current logical time of the world state.
    current_lt: u64,
    /// The current unix time of the world state.
    current_now: u32,
    /// Optional VM random seed used for future emulated transactions and get-methods.
    random_seed: Option<[u8; 32]>,
    /// Registered global library cells keyed by representation hash.
    libraries: FxHashMap<HashBytes, Cell>,
    /// Blockchain configuration
    config: Arc<dict::Dict<u32, Cell>>,
}

impl WorldState {
    /// Creates a new `WorldState` instance with the given initial state.
    pub fn new(accounts_state: AccountsState, config_b64: Option<&str>) -> anyhow::Result<Self> {
        if config_b64.is_none() {
            // fast path
            return Ok(Self {
                accounts_state,
                current_lt: 0,
                current_now: 0,
                random_seed: None,
                libraries: FxHashMap::default(),
                config: DEFAULT_CONFIG_DICT.clone(),
            });
        }

        let config_str = config_b64.unwrap_or(DEFAULT_CONFIG);
        let config = Boc::decode_base64(config_str)
            .ok()
            .and_then(|cell| {
                let mut slice = cell.as_slice_allow_exotic();
                dict::Dict::load_from_root_ext(&mut slice, Cell::empty_context()).ok()
            })
            .ok_or_else(|| anyhow::anyhow!("Corrupted blockchain config for world state"))?;

        Ok(Self {
            accounts_state,
            current_lt: 0,
            current_now: 0,
            random_seed: None,
            libraries: FxHashMap::default(),
            config: Arc::new(config),
        })
    }

    /// Returns a reference to the map of accounts currently in the world state.
    #[must_use]
    pub const fn get_accounts(&self) -> &FxHashMap<StdAddr, ShardAccount> {
        self.accounts_state.accounts()
    }

    #[must_use]
    pub fn take_accounts(self) -> FxHashMap<StdAddr, ShardAccount> {
        self.accounts_state.take_accounts()
    }

    /// Returns a reference to the blockchain configuration.
    #[must_use]
    pub fn get_config(&self) -> Arc<dict::Dict<u32, Cell>> {
        self.config.clone()
    }

    /// Returns a blockchain configuration as base64 encoded string.
    #[must_use]
    pub fn get_config_b64(&self) -> Cow<'_, str> {
        if self.config == *DEFAULT_CONFIG_DICT {
            return Cow::Borrowed(DEFAULT_CONFIG);
        }
        Cow::Owned(
            self.config
                .root()
                .clone()
                .map(Boc::encode_base64)
                .expect("Config has no root"),
        )
    }

    /// Returns a blockchain configuration as a cell.
    #[must_use]
    pub fn get_config_cell(&self) -> Cell {
        if self.config == *DEFAULT_CONFIG_DICT {
            return DEFAULT_CONFIG_CELL.clone();
        }
        self.config.root().clone().expect("Config has no root")
    }

    /// Sets the blockchain configuration.
    pub fn set_config(&mut self, config: dict::Dict<u32, Cell>) {
        self.config = Arc::new(config);
    }

    /// Checks if an account is deployed.
    ///
    /// If the state is `Remote` and the account is not in the local cache, it will
    /// attempt to fetch it from the network to determine its status.
    pub fn check_deployed(&mut self, raw_addr: &StdAddr) -> bool {
        let deployed = self
            .accounts_state
            .accounts()
            .get(raw_addr)
            .is_some_and(shard_account_is_active);
        if !deployed && matches!(self.accounts_state, AccountsState::Remote(_)) {
            // we need to populate address for the first time
            let account = self.get_account(raw_addr);
            return shard_account_is_active(&account);
        }
        deployed
    }

    /// Retrieves an account by its address, fetching it from the source if necessary.
    pub fn get_account(&mut self, addr: &StdAddr) -> ShardAccount {
        let account = self.accounts_state.retrieve(addr, self.current_lt);
        self.current_lt = self.current_lt.max(account.last_trans_lt);

        if let AccountsState::Remote(remote) = &self.accounts_state {
            let loaded_libs = preload_remote_account_libraries(remote, &self.libraries, &account);
            for lib in loaded_libs {
                self.register_lib(lib);
            }
        }
        account
    }

    /// Updates an account's data in the world state.
    pub fn update_account(&mut self, addr: &StdAddr, account: &ShardAccount) {
        self.accounts_state.update(addr, account.clone());
    }

    /// Clears cached remote accounts so subsequent reads refetch live network state.
    pub fn invalidate_remote_cache(&mut self) {
        self.accounts_state.invalidate_remote_cache();
    }

    /// Increments and returns the current logical time.
    ///
    /// Each call increments the time by 1,000,000 to ensure enough gap for transactions.
    pub const fn get_lt(&mut self) -> u64 {
        self.current_lt += 1_000_000u64;
        self.current_lt
    }

    /// Returns registered global libraries keyed by representation hash.
    #[must_use]
    pub const fn libs(&self) -> &FxHashMap<HashBytes, Cell> {
        &self.libraries
    }

    /// Finds a registered global library by its representation hash.
    #[must_use]
    pub fn find_lib_by_hash(&self, hash: &HashBytes) -> Option<Cell> {
        self.libraries.get(hash).cloned()
    }

    /// Registers a new global library cell.
    pub fn register_lib(&mut self, lib: Cell) {
        self.libraries.insert(*lib.repr_hash(), lib);
    }

    /// Returns a reference to the current state source.
    #[must_use]
    pub const fn state(&self) -> &AccountsState {
        &self.accounts_state
    }

    /// Sets the current unix time of the world state.
    pub const fn set_now(&mut self, now: u32) {
        self.current_now = now;
    }

    /// Returns the current unix time of the world state.
    #[must_use]
    pub const fn get_now(&self) -> u32 {
        self.current_now
    }

    /// Sets the optional random seed used for future emulated VM runs.
    pub const fn set_random_seed(&mut self, seed: Option<[u8; 32]>) {
        self.random_seed = seed;
    }

    /// Returns the optional random seed used for future emulated VM runs.
    #[must_use]
    pub const fn get_random_seed(&self) -> Option<[u8; 32]> {
        self.random_seed
    }

    pub fn snapshot(&self) -> anyhow::Result<WorldStateSnapshot> {
        let mut accounts = self
            .accounts_state
            .accounts()
            .iter()
            .map(|(address, account)| {
                if !shard_account_exists(account) {
                    return Ok(None);
                }

                Ok(Some(WorldStateAccountSnapshot {
                    address: address.display_base64_url(false).to_string(),
                    shard_account_boc64: encode_shard_account_boc64(account)?,
                }))
            })
            .collect::<anyhow::Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        accounts.sort_by(|left, right| left.address.cmp(&right.address));

        let mut libraries_boc64 = self
            .libraries
            .values()
            .map(Boc::encode_base64)
            .collect::<Vec<_>>();
        libraries_boc64.sort();

        Ok(WorldStateSnapshot {
            version: WORLD_STATE_SNAPSHOT_VERSION,
            current_lt: self.current_lt,
            current_now: self.current_now,
            random_seed: self.random_seed.map(hex::encode),
            config_boc64: self.snapshot_config_b64()?.into_owned(),
            libraries_boc64,
            accounts,
        })
    }

    pub fn from_snapshot(snapshot: WorldStateSnapshot) -> anyhow::Result<Self> {
        if snapshot.version != WORLD_STATE_SNAPSHOT_VERSION {
            anyhow::bail!(
                "Unsupported world state snapshot version: {}",
                snapshot.version
            );
        }

        let mut accounts = FxHashMap::default();
        for entry in snapshot.accounts {
            let (address, _) = StdAddr::from_str_ext(&entry.address, StdAddrFormat::any())
                .map_err(|_| anyhow!("Invalid account address in snapshot: {}", entry.address))?;
            let shard_account = decode_shard_account_boc64(&entry.shard_account_boc64)?;
            if accounts.insert(address, shard_account).is_some() {
                anyhow::bail!("Duplicate account address in snapshot: {}", entry.address);
            }
        }

        let mut state = Self::new(
            AccountsState::Local(LocalAccountsState { accounts }),
            Some(&snapshot.config_boc64),
        )?;
        state.current_lt = snapshot.current_lt;
        state.current_now = snapshot.current_now;
        state.random_seed = snapshot
            .random_seed
            .map(|seed| {
                let bytes = hex::decode(&seed)
                    .map_err(|err| anyhow!("Invalid random seed in snapshot: {err}"))?;
                bytes.try_into().map_err(|bytes: Vec<u8>| {
                    anyhow!(
                        "Invalid random seed in snapshot: expected 32 bytes, got {}",
                        bytes.len()
                    )
                })
            })
            .transpose()?;

        for lib_boc64 in snapshot.libraries_boc64 {
            state.register_lib(Boc::decode_base64(&lib_boc64)?);
        }

        Ok(state)
    }

    pub fn load_snapshot(&mut self, snapshot: WorldStateSnapshot) -> anyhow::Result<()> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }
}

fn preload_remote_account_libraries(
    remote: &RemoteAccountState,
    registered_libs: &FxHashMap<HashBytes, Cell>,
    account: &ShardAccount,
) -> Vec<Cell> {
    let mut pending = match collect_account_library_refs(account) {
        Ok(hashes) => hashes,
        Err(err) => {
            eprintln!("Failed to inspect remote account libraries: {err}");
            return vec![];
        }
    };
    let mut processed = FxHashSet::<HashBytes>::default();
    let mut loaded_libs = Vec::new();

    while let Some(hash) = pending.pop() {
        if !processed.insert(hash) || registered_libs.contains_key(&hash) {
            continue;
        }

        let lib = match remote.load_library(&hash) {
            Ok(lib) => lib,
            Err(err) => {
                eprintln!("Failed to resolve library {hash}: {err}");
                continue;
            }
        };

        if let Ok(mut nested_refs) = collect_library_refs(&lib) {
            pending.append(&mut nested_refs);
        }
        loaded_libs.push(lib);
    }

    loaded_libs
}

fn collect_account_library_refs(account: &ShardAccount) -> anyhow::Result<Vec<HashBytes>> {
    let Some(account) = account.account.load()?.0 else {
        return Ok(vec![]);
    };
    let AccountState::Active(state) = account.state else {
        return Ok(vec![]);
    };
    let Some(code) = state.code else {
        return Ok(vec![]);
    };
    collect_library_refs(&code)
}

fn collect_library_refs(root: &Cell) -> anyhow::Result<Vec<HashBytes>> {
    let mut hashes = FxHashSet::<HashBytes>::default();
    let mut visited = FxHashSet::<HashBytes>::default();
    collect_library_refs_inner(root, &mut hashes, &mut visited)?;
    Ok(hashes.into_iter().collect())
}

fn collect_library_refs_inner(
    cell: &Cell,
    hashes: &mut FxHashSet<HashBytes>,
    visited: &mut FxHashSet<HashBytes>,
) -> anyhow::Result<()> {
    if !visited.insert(*cell.repr_hash()) {
        return Ok(());
    }

    if let Some(hash) = library_ref_hash(cell)? {
        hashes.insert(hash);
    }

    for index in 0..cell.reference_count() {
        if let Some(child) = cell.reference_cloned(index) {
            collect_library_refs_inner(&child, hashes, visited)?;
        }
    }
    Ok(())
}

fn library_ref_hash(cell: &Cell) -> anyhow::Result<Option<HashBytes>> {
    if !cell.is_exotic() {
        return Ok(None);
    }

    let slice = cell.as_slice_allow_exotic();
    if slice.size_bits() != 8 + 256 {
        return Ok(None);
    }

    let mut slice = cell.as_slice_allow_exotic();
    if slice.load_u8()? != EXOTIC_LIBRARY_TAG {
        return Ok(None);
    }
    Ok(Some(slice.load_u256()?))
}

fn validate_library_hash(hash: &HashBytes, lib: &Cell, source: &str) -> anyhow::Result<()> {
    if *lib.repr_hash() != *hash {
        anyhow::bail!(
            "{source} hash mismatch: requested {hash}, got {}",
            lib.repr_hash()
        );
    }
    Ok(())
}

fn encode_shard_account_boc64(account: &ShardAccount) -> anyhow::Result<String> {
    let mut builder = CellBuilder::new();
    account.store_into(&mut builder, Cell::empty_context())?;
    Ok(Boc::encode_base64(builder.build()?))
}

fn decode_shard_account_boc64(boc64: &str) -> anyhow::Result<ShardAccount> {
    Ok(Boc::decode_base64(boc64)?.parse::<ShardAccount>()?)
}

fn shard_account_exists(account: &ShardAccount) -> bool {
    account
        .account
        .load()
        .is_ok_and(|loaded| loaded.0.is_some())
}

fn shard_account_is_active(account: &ShardAccount) -> bool {
    account.account.load().is_ok_and(|loaded| {
        loaded
            .0
            .is_some_and(|account| matches!(account.state, AccountState::Active(_)))
    })
}

impl WorldState {
    fn snapshot_config_b64(&self) -> anyhow::Result<Cow<'_, str>> {
        if self.config == *DEFAULT_CONFIG_DICT {
            return Ok(Cow::Borrowed(DEFAULT_CONFIG));
        }

        let root = self
            .config
            .root()
            .clone()
            .ok_or_else(|| anyhow!("Config has no root"))?;
        Ok(Cow::Owned(Boc::encode_base64(root)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_address(seed: u8) -> StdAddr {
        StdAddr {
            anycast: None,
            workchain: 0,
            address: HashBytes([seed; 32]),
        }
    }

    fn empty_shard_account() -> ShardAccount {
        ShardAccount {
            account: Lazy::new(&OptionalAccount(None)).expect("empty account should serialize"),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: 0,
        }
    }

    #[test]
    fn get_account_advances_logical_time_past_loaded_account_lt() {
        let address = test_address(3);
        let mut account = empty_shard_account();
        account.last_trans_lt = 74118931000008;
        let mut state = WorldState::new(
            AccountsState::Local(LocalAccountsState {
                accounts: FxHashMap::from_iter([(address.clone(), account)]),
            }),
            None,
        )
        .unwrap();

        state.get_account(&address);

        assert_eq!(state.get_lt(), 74118932000008);
    }

    #[test]
    fn remote_cache_invalidation_clears_local_and_shared_caches() {
        let cache = RemoteSnapshotCache::new();
        let address = test_address(1);
        let account = empty_shard_account();
        let cache_key = RemoteCacheKey {
            fork_block_number: None,
            fork_net: Network::Testnet,
            address: address.clone(),
        };

        let library_cache = RemoteLibraryCache::new();
        let mut remote = RemoteAccountState::new(
            Network::Testnet,
            None,
            cache.clone(),
            library_cache.clone(),
            false,
        );
        remote.accounts.insert(address, account.clone());
        cache.insert(cache_key.clone(), account);
        library_cache.insert(HashBytes([0xbb; 32]), Cell::default());

        remote.invalidate_cache();

        assert!(remote.accounts.is_empty());
        assert!(cache.get(&cache_key).is_none());
        assert!(library_cache.get(&HashBytes([0xbb; 32])).is_none());
    }

    #[test]
    fn local_cache_invalidation_keeps_local_accounts() {
        let address = test_address(2);
        let account = empty_shard_account();
        let mut state = AccountsState::Local(LocalAccountsState {
            accounts: FxHashMap::from_iter([(address.clone(), account)]),
        });

        state.invalidate_remote_cache();

        assert!(state.accounts().contains_key(&address));
    }

    #[test]
    fn collect_library_refs_detects_root_library_reference() {
        let code = CellBuilder::build_library(&HashBytes([0x11; 32]));

        let refs = collect_library_refs(&code).unwrap();

        assert_eq!(refs, vec![HashBytes([0x11; 32])]);
    }

    #[test]
    fn collect_library_refs_detects_nested_library_reference_once() {
        let library_reference = CellBuilder::build_library(&HashBytes([0x22; 32]));
        let mut builder = CellBuilder::new();
        builder.store_u32(0xcafe_babe).unwrap();
        builder.store_reference(library_reference.clone()).unwrap();
        builder.store_reference(library_reference).unwrap();
        let code = builder.build().unwrap();

        let refs = collect_library_refs(&code).unwrap();

        assert_eq!(refs, vec![HashBytes([0x22; 32])]);
    }

    #[test]
    fn collect_library_refs_ignores_ordinary_cells() {
        let mut builder = CellBuilder::new();
        builder.store_u32(0xcafe_babe).unwrap();
        let code = builder.build().unwrap();

        let refs = collect_library_refs(&code).unwrap();

        assert!(refs.is_empty());
    }

    #[test]
    fn remote_library_cache_round_trip() {
        let cache = RemoteLibraryCache::new();
        let mut builder = CellBuilder::new();
        builder.store_u32(0xfeed_face).unwrap();
        let lib = builder.build().unwrap();
        let hash = *lib.repr_hash();

        cache.insert(hash, lib.clone());

        assert_eq!(cache.get(&hash), Some(lib));
    }

    #[test]
    fn validate_library_hash_rejects_mismatched_cell() {
        let mut builder = CellBuilder::new();
        builder.store_u32(0xfeed_face).unwrap();
        let lib = builder.build().unwrap();

        let err = validate_library_hash(&HashBytes([0xff; 32]), &lib, "test library")
            .expect_err("mismatched hash must be rejected");

        assert!(err.to_string().contains("test library hash mismatch"));
    }

    #[test]
    fn remote_library_disk_cache_round_trip_and_rejects_mismatch() {
        let dir = PathBuf::from(format!(
            "/tmp/acton-remote-library-cache-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let cache = RemoteLibraryDiskCache::new(dir.clone());
        let mut builder = CellBuilder::new();
        builder.store_u32(0x0bad_cafe).unwrap();
        let lib = builder.build().unwrap();
        let hash = *lib.repr_hash();

        cache.write(&hash, &lib).unwrap();

        assert_eq!(cache.read(&hash), Some(lib.clone()));
        assert!(
            cache.write(&HashBytes([0xee; 32]), &lib).is_err(),
            "disk cache must reject cells stored under the wrong hash"
        );

        fs::remove_dir_all(dir).ok();
    }
}
