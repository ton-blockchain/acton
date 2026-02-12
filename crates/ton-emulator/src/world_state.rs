//! This module provides the emulated world state management.
//!
//! It includes logic for handling account states, logical time (LT), current unix time,
//! and global libraries. The state can be managed purely locally or forked from a remote
//! TON network (mainnet or testnet).

use crate::remote;
use acton_config::config::ActonConfig;
use anyhow::anyhow;
use num_traits::cast::ToPrimitive;
use rustc_hash::FxHashMap;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use ton_executor::{DEFAULT_CONFIG, DEFAULT_CONFIG_DICT};
use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellFamily, HashBytes, Lazy};
use tycho_types::dict;
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount, StateInit,
    StorageInfo,
};

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
    pub fn retrieve(&mut self, address: &str, current_lt: u64) -> ShardAccount {
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
    pub fn update(&mut self, address: &str, account: ShardAccount) {
        match self {
            Self::Local(r) => r.update(address, account),
            Self::Remote(r) => r.update(address, account),
        }
    }

    /// Returns a reference to the underlying map of accounts.
    #[must_use]
    pub const fn accounts(&self) -> &FxHashMap<String, ShardAccount> {
        match self {
            Self::Local(r) => &r.accounts,
            Self::Remote(r) => &r.accounts,
        }
    }
}

/// A purely local implementation of the world state.
pub struct LocalAccountsState {
    pub accounts: FxHashMap<String, ShardAccount>,
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

    fn retrieve(&mut self, address: &str, current_lt: u64) -> ShardAccount {
        if let Some(acc) = self.accounts.get(address) {
            return acc.clone();
        }

        let acc = ShardAccount {
            account: Lazy::new(&OptionalAccount(None)).expect("Failed to create empty account"),
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: current_lt,
        };
        self.accounts.insert(address.to_string(), acc.clone());
        acc
    }

    fn update(&mut self, address: &str, account: ShardAccount) {
        self.accounts.insert(address.to_owned(), account);
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct RemoteCacheKey {
    fork_block_number: Option<u64>,
    fork_net: String,
    address: String,
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
}

/// A state implementation that fetches missing accounts from a remote network.
pub struct RemoteAccountState {
    /// Local cache and overrides for accounts.
    pub accounts: FxHashMap<String, ShardAccount>,
    /// The network to fork from (e.g., "mainnet", "testnet").
    pub fork_net: Network,
    /// Optional block number to pin the state to.
    pub fork_block_number: Option<u64>,
    /// Optional API key for `TonCenter`.
    pub api_key: Option<String>,
    pub acton_config: Option<ActonConfig>,
    /// Cache for less network queries in subsequent tests.
    cache: RemoteSnapshotCache,
}

impl RemoteAccountState {
    /// Creates a new remote state for the given network.
    #[must_use]
    pub fn new(
        fork_net: Network,
        fork_block_number: Option<u64>,
        api_key: Option<String>,
        cache: RemoteSnapshotCache,
    ) -> Self {
        Self {
            accounts: FxHashMap::default(),
            fork_net,
            fork_block_number,
            api_key,
            acton_config: ActonConfig::load().ok(),
            cache,
        }
    }

    fn retrieve(&mut self, address: &str, current_lt: u64) -> ShardAccount {
        if let Some(acc) = self.accounts.get(address) {
            return acc.clone();
        }

        match self.resolve_remote_account(address, current_lt) {
            Ok(acc) => {
                self.accounts.insert(address.to_string(), acc.clone());
                acc
            }
            Err(err) => {
                eprintln!("Failed to resolve address {address} for account {current_lt}: {err}");

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

    fn update(&mut self, address: &str, account: ShardAccount) {
        self.accounts.insert(address.to_owned(), account);
    }

    fn resolve_remote_account(
        &self,
        address: &str,
        current_lt: u64,
    ) -> anyhow::Result<ShardAccount> {
        // return cached version if it already resolved earlier in current suite
        let cache_key = RemoteCacheKey {
            fork_block_number: self.fork_block_number,
            fork_net: self.fork_net.to_string(),
            address: address.to_owned(),
        };
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let network = &self.fork_net;
        let api_key = self
            .api_key
            .clone()
            .or_else(|| env::var("TONCENTER_API_KEY").ok());

        let mut custom_networks = HashMap::new();
        if let Some(config) = &self.acton_config {
            custom_networks = config.custom_networks()
        }

        let info = remote::get_account_info(
            self.fork_block_number,
            address,
            network,
            api_key,
            custom_networks,
        )?;

        let balance = info
            .balance
            .to_bigint()?
            .to_u128()
            .ok_or_else(|| anyhow!("Failed to convert balance to u128"))?;

        let account_state = match info.state.as_str() {
            "active" => AccountState::Active(StateInit {
                code: remote::decode_optional_cell(&info.code)?,
                data: remote::decode_optional_cell(&info.data)?,
                ..Default::default()
            }),
            "uninitialized" => AccountState::Uninit,
            "frozen" => AccountState::Frozen(HashBytes::from_str(info.frozen_hash.as_str())?),
            _ => {
                anyhow::bail!("Unknown account state: {}", info.state);
            }
        };

        let acc = ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                balance: CurrencyCollection::new(balance),
                address: IntAddr::from_str(address)?,
                last_trans_lt: info.last_transaction_id.lt.parse()?,
                state: account_state,
                storage_stat: StorageInfo::default(),
            })))?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: current_lt.to_u64().unwrap_or(0),
        };
        self.cache.insert(cache_key, acc.clone());
        Ok(acc)
    }
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
    pub const fn get_accounts(&self) -> &FxHashMap<String, ShardAccount> {
        self.accounts_state.accounts()
    }

    /// Returns a reference to the blockchain configuration.
    #[must_use]
    pub fn get_config(&self) -> Arc<dict::Dict<u32, Cell>> {
        self.config.clone()
    }

    /// Returns a  blockchain configuration as base64 encoded string.
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

    /// Sets the blockchain configuration.
    pub fn set_config(&mut self, config: dict::Dict<u32, Cell>) {
        self.config = Arc::new(config);
    }

    /// Checks if an account is deployed.
    ///
    /// If the state is `Remote` and the account is not in the local cache, it will
    /// attempt to fetch it from the network to determine its status.
    pub fn check_deployed(&mut self, raw_addr: &str) -> bool {
        let deployed = self.accounts_state.accounts().contains_key(raw_addr);
        if !deployed && matches!(self.accounts_state, AccountsState::Remote(_)) {
            // we need to populate address for the first time
            let account = self.get_account(raw_addr);
            return account
                .account
                .load()
                .map(|acc| acc.0.is_some())
                .unwrap_or(false);
        }
        deployed
    }

    /// Retrieves an account by its address, fetching it from the source if necessary.
    pub fn get_account(&mut self, raw_addr: &str) -> ShardAccount {
        self.accounts_state.retrieve(raw_addr, self.current_lt)
    }

    /// Updates an account's data in the world state.
    pub fn update_account(&mut self, addr: &str, account: &ShardAccount) {
        self.accounts_state.update(addr, account.clone());
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
}
