//! This module provides the emulated world state management.
//!
//! It includes logic for handling account states, logical time (LT), current unix time,
//! and global libraries. The state can be managed purely locally or forked from a remote
//! TON network (mainnet or testnet).

use acton_config::config::ActonConfig;
use anyhow::{Context, anyhow};
use base64::Engine;
use num_traits::cast::ToPrimitive;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::{OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorldStateSnapshot {
    pub version: u32,
    pub current_lt: u64,
    pub current_now: u32,
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
}

impl RemoteAccountState {
    /// Creates a new remote state for the given network.
    #[must_use]
    pub fn new(
        fork_net: Network,
        fork_block_number: Option<u64>,
        cache: RemoteSnapshotCache,
    ) -> Self {
        Self {
            accounts: FxHashMap::default(),
            fork_net,
            fork_block_number,
            api_client: OnceCell::new(),
            cache,
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

        let api_client = self.api_client()?;
        if let Ok(cell) =
            api_client.get_shard_account_cell(self.fork_block_number, &address.to_string())
        {
            let acc = cell
                .parse::<ShardAccount>()
                .context("Failed to parse getShardAccountCell response as ShardAccount")?;
            self.cache.insert(cache_key, acc.clone());
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
        self.cache.insert(cache_key, acc.clone());
        Ok(acc)
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
    /// List of registered global library cells.
    libraries: Vec<Cell>,
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
                libraries: vec![],
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
            libraries: vec![],
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

    /// Returns a list of all registered global libraries.
    #[must_use]
    pub fn libs(&self) -> Vec<Cell> {
        self.libraries.clone()
    }

    /// Finds a registered global library by its representation hash.
    #[must_use]
    pub fn find_lib_by_hash(&self, hash: &HashBytes) -> Option<Cell> {
        self.libraries
            .iter()
            .find(|lib| *lib.repr_hash() == *hash)
            .cloned()
    }

    /// Registers a new global library cell.
    pub fn register_lib(&mut self, lib: Cell) {
        self.libraries.push(lib);
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

        let libraries_boc64 = self
            .libraries
            .iter()
            .map(Boc::encode_base64)
            .collect::<Vec<_>>();

        Ok(WorldStateSnapshot {
            version: WORLD_STATE_SNAPSHOT_VERSION,
            current_lt: self.current_lt,
            current_now: self.current_now,
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
        .map(|loaded| loaded.0.is_some())
        .unwrap_or(false)
}

fn shard_account_is_active(account: &ShardAccount) -> bool {
    account
        .account
        .load()
        .map(|loaded| {
            loaded
                .0
                .is_some_and(|account| matches!(account.state, AccountState::Active(_)))
        })
        .unwrap_or(false)
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

        let mut remote = RemoteAccountState::new(Network::Testnet, None, cache.clone());
        remote.accounts.insert(address, account.clone());
        cache.insert(cache_key.clone(), account);

        remote.invalidate_cache();

        assert!(remote.accounts.is_empty());
        assert!(cache.get(&cache_key).is_none());
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
}
