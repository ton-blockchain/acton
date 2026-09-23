use crate::storage::{AccountMeta, AccountStatus, CellStore};
use crate::types::{Addr, BocBytes, ExtraCurrency, Hash256};
use acton_config::config;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use ton_api::{MasterchainSnapshot, TonApiClient};
use ton_networks::Network;
use toncenter::v2;
use toncenter::v3;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{AccountState, ShardAccount, ShardIdent};
use tycho_types::prelude::HashBytes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RemoteShardBoundary {
    workchain: i32,
    shard: u64,
    seqno: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteProvider {
    pub network: Network,
    pub fork_block_number: Option<u64>,
    #[serde(skip)]
    pub(crate) fork_snapshot: Option<MasterchainSnapshot>,
    #[serde(skip)]
    pub(crate) client: Arc<OnceLock<toncenter_client::Client>>,
}

impl RemoteProvider {
    fn api(&self) -> anyhow::Result<&toncenter_client::Client> {
        if self.client.get().is_none() {
            let config = config::ActonConfig::load().unwrap_or_default();
            let networks = config.custom_networks();
            let proxy = std::env::var("ACTON_USE_PROXY")
                .is_ok_and(|value| matches!(value.trim(), "1" | "true"));
            let mut builder = toncenter_client::Client::builder()
                .v2_url(self.network.toncenter_v2_url(&networks)?)
                .api_key(toncenter_keys::api_key(&self.network))
                .user_agent(concat!("acton/", env!("CARGO_PKG_VERSION")))
                .system_proxy(proxy);
            if let Ok(url) = self.network.toncenter_v3_url(&networks) {
                builder = builder.v3_url(url);
            }
            let _ = self.client.set(builder.build()?);
        }
        Ok(self.client.get().expect("remote client initialized"))
    }

    pub async fn pinned(network: Network, fork_block_number: Option<u64>) -> anyhow::Result<Self> {
        let request_network = network.clone();
        let fork_snapshot = tokio::task::spawn_blocking(move || {
            create_api_client(request_network)?.get_masterchain_snapshot_cached(
                fork_block_number,
                &masterchain_snapshot_cache_dir(),
            )
        })
        .await
        .context("Failed to resolve fork snapshot")??;

        Ok(Self {
            network,
            fork_block_number: Some(fork_snapshot.seqno),
            fork_snapshot: Some(fork_snapshot),
            client: Arc::default(),
        })
    }

    pub(crate) fn snapshot_at(
        &mut self,
        seqno: u64,
    ) -> anyhow::Result<Option<MasterchainSnapshot>> {
        let Some(snapshot) = &self.fork_snapshot else {
            return Ok(None);
        };
        if snapshot.seqno == seqno {
            return Ok(Some(snapshot.clone()));
        }

        let snapshot = with_api_client(self, move |api_client| {
            api_client
                .get_masterchain_snapshot_cached(Some(seqno), &masterchain_snapshot_cache_dir())
        })?;
        self.fork_block_number = Some(snapshot.seqno);
        self.fork_snapshot = Some(snapshot.clone());
        Ok(Some(snapshot))
    }

    pub async fn contains_historical_block(
        &self,
        workchain: i32,
        shard: i64,
        seqno: u64,
    ) -> anyhow::Result<bool> {
        if workchain == ShardIdent::MASTERCHAIN.workchain() {
            return Ok(shard as u64 == ShardIdent::MASTERCHAIN.prefix()
                && self
                    .fork_block_number
                    .is_some_and(|fork_seqno| seqno <= fork_seqno));
        }

        let Some(requested_shard) = ShardIdent::new(workchain, shard as u64) else {
            return Ok(false);
        };
        Ok(contains_historical_shard_block(
            &self.fork_shards().await?,
            requested_shard,
            seqno,
        ))
    }

    pub async fn contains_historical_block_id(
        &self,
        block: &v2::responses::TonBlockIdExt,
    ) -> anyhow::Result<bool> {
        let shard = parse_remote_shard(&block.shard)?;
        self.contains_historical_block(
            block
                .workchain
                .try_into()
                .context("Remote workchain does not fit i32")?,
            shard as i64,
            block
                .seqno
                .try_into()
                .context("Remote block seqno is negative")?,
        )
        .await
    }

    async fn fork_shards(&self) -> anyhow::Result<Arc<Vec<RemoteShardBoundary>>> {
        let fork_seqno = self
            .fork_block_number
            .context("Historical provider must be pinned")?;
        let key = (self.network.as_str(), fork_seqno);
        let cached = {
            let cache = fork_shard_cache()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            cache.get(&key).cloned()
        };
        if let Some(shards) = cached {
            return Ok(shards);
        }

        let fork_seqno = u32::try_from(fork_seqno)
            .context("Fork block seqno does not fit TON Center v2 request")?;
        let shards = Arc::new(
            fetch_remote_shards_v2(self, fork_seqno)
                .await?
                .shards
                .into_iter()
                .map(RemoteShardBoundary::try_from)
                .collect::<anyhow::Result<Vec<_>>>()?,
        );
        fork_shard_cache()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key, Arc::clone(&shards));
        Ok(shards)
    }
}

fn masterchain_snapshot_cache_dir() -> PathBuf {
    config::project_root()
        .join("build")
        .join("cache")
        .join(ton_api::MASTERCHAIN_SNAPSHOT_CACHE_SUBDIR)
}

type RemoteForkKey = (String, u64);
type RemoteForkShardCache = HashMap<RemoteForkKey, Arc<Vec<RemoteShardBoundary>>>;

fn fork_shard_cache() -> &'static Mutex<RemoteForkShardCache> {
    static CACHE: OnceLock<Mutex<RemoteForkShardCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn contains_historical_shard_block(
    fork_shards: &[RemoteShardBoundary],
    requested_shard: ShardIdent,
    requested_seqno: u64,
) -> bool {
    let mut matching_boundaries = fork_shards
        .iter()
        .filter_map(RemoteShardBoundary::shard_ident)
        .filter(|(fork_shard, _)| fork_shard.intersects(&requested_shard));
    let Some((_, fork_seqno)) = matching_boundaries.next() else {
        return false;
    };
    // An ancestor can intersect several active shards after a split. Without the
    // full shard history there is no single safe cutoff, so keep the request local.
    matching_boundaries.next().is_none() && requested_seqno <= fork_seqno
}

impl RemoteShardBoundary {
    fn shard_ident(&self) -> Option<(ShardIdent, u64)> {
        ShardIdent::new(self.workchain, self.shard).map(|shard| (shard, self.seqno))
    }
}

impl TryFrom<v2::responses::TonBlockIdExt> for RemoteShardBoundary {
    type Error = anyhow::Error;

    fn try_from(block: v2::responses::TonBlockIdExt) -> Result<Self, Self::Error> {
        let shard = parse_remote_shard(&block.shard)?;
        let workchain = block
            .workchain
            .try_into()
            .context("Remote workchain does not fit i32")?;
        anyhow::ensure!(
            ShardIdent::new(workchain, shard).is_some(),
            "Remote fork returned invalid shard {}:{}",
            block.workchain,
            block.shard
        );
        Ok(Self {
            workchain,
            shard,
            seqno: block
                .seqno
                .try_into()
                .context("Remote block seqno is negative")?,
        })
    }
}

fn parse_remote_shard(shard: &str) -> anyhow::Result<u64> {
    let shard = shard.trim();
    if shard.starts_with('-') {
        return shard
            .parse::<i64>()
            .map(|value| value as u64)
            .with_context(|| format!("Invalid remote shard `{shard}`"));
    }

    let hex = shard
        .strip_prefix("0x")
        .or_else(|| shard.strip_prefix("0X"))
        .unwrap_or(shard);
    u64::from_str_radix(hex, 16).with_context(|| format!("Invalid remote shard `{shard}`"))
}

fn create_api_client(network: Network) -> anyhow::Result<TonApiClient> {
    let config = config::ActonConfig::load().unwrap_or_default();
    TonApiClient::new(network, config.custom_networks())
}

fn request_with_api_client<T>(
    network: Network,
    request: impl FnOnce(&TonApiClient) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    request(&create_api_client(network)?)
}

fn with_api_client<T: Send>(
    provider: &RemoteProvider,
    request: impl FnOnce(&TonApiClient) -> anyhow::Result<T> + Send,
) -> anyhow::Result<T> {
    request_with_api_client(provider.network.clone(), request)
}

pub(crate) async fn fetch_remote_blocks_v3(
    provider: &RemoteProvider,
    raw_query: String,
) -> anyhow::Result<v3::BlocksResponse> {
    let mut url = reqwest::Url::parse("http://localhost")?;
    url.set_query(Some(&raw_query));
    let query: Vec<_> = url.query_pairs().into_owned().collect();
    Ok(provider.api()?.v3_get("blocks", &query).await?)
}

pub(crate) async fn fetch_remote_transactions_v3(
    provider: &RemoteProvider,
    raw_query: String,
) -> anyhow::Result<v3::TransactionsResponse> {
    let mut url = reqwest::Url::parse("http://localhost")?;
    url.set_query(Some(&raw_query));
    let query: Vec<_> = url.query_pairs().into_owned().collect();
    Ok(provider.api()?.v3_get("transactions", &query).await?)
}

pub(crate) async fn fetch_remote_shards_v2(
    provider: &RemoteProvider,
    seqno: u32,
) -> anyhow::Result<v2::responses::Shards> {
    Ok(provider
        .api()?
        .v2()
        .get_shards(&v2::requests::ShardsRequest {
            seqno: i32::try_from(seqno)?.into(),
        })
        .await?)
}

pub(crate) async fn fetch_remote_block_header_v2(
    provider: &RemoteProvider,
    request: v2::requests::BlockHeaderRequest,
) -> anyhow::Result<v2::responses::BlockHeader> {
    Ok(provider.api()?.v2().get_block_header(&request).await?)
}

pub(crate) async fn fetch_remote_block_v2(
    provider: &RemoteProvider,
    request: v2::requests::BlockDataRequest,
) -> anyhow::Result<v2::responses::BlockData> {
    Ok(provider.api()?.v2().get_block(&request).await?)
}

pub(crate) async fn fetch_remote_block_transactions_v2(
    provider: &RemoteProvider,
    request: v2::requests::BlockTransactionsRequest,
) -> anyhow::Result<v2::responses::BlockTransactions> {
    Ok(provider
        .api()?
        .v2()
        .get_block_transactions(&request)
        .await?)
}

pub(crate) async fn fetch_remote_block_transactions_ext_v2(
    provider: &RemoteProvider,
    request: v2::requests::BlockTransactionsRequest,
) -> anyhow::Result<v2::responses::BlockTransactionsExt> {
    Ok(provider
        .api()?
        .v2()
        .get_block_transactions_ext(&request)
        .await?)
}

pub(crate) async fn fetch_remote_lookup_block_v2(
    provider: &RemoteProvider,
    request: v2::requests::LookupBlockRequest,
) -> anyhow::Result<v2::responses::TonBlockIdExt> {
    Ok(provider.api()?.v2().lookup_block(&request).await?)
}

pub fn fetch_remote_library(hash: &Hash256, provider: &RemoteProvider) -> anyhow::Result<Cell> {
    let hash = *hash;
    let lib = with_api_client(provider, move |api_client| {
        api_client.get_library_by_hash(&HashBytes(hash.0))
    })?;
    let actual_hash = Hash256::from(lib.repr_hash());
    if actual_hash != hash {
        anyhow::bail!(
            "Remote library hash mismatch: requested {}, got {}",
            hash.to_hex(),
            actual_hash.to_hex()
        );
    }
    Ok(lib)
}

pub fn fetch_remote_shard_account(
    addr: &Addr,
    provider: &RemoteProvider,
    cas: &mut CellStore,
) -> anyhow::Result<(BocBytes, AccountMeta)> {
    tracing::info!("Fetching remote account state for {}", addr);

    let fork_block_number = provider.fork_block_number;
    let address = addr.to_string();
    let cell = with_api_client(provider, move |api_client| {
        api_client.get_shard_account_cell(fork_block_number, &address)
    })?;
    let shard_account = cell.parse::<ShardAccount>()?;
    let boc = BocBytes::from(Boc::encode(cell));
    let meta = account_meta_from_shard_account(&shard_account, &boc, cas)?;
    Ok((boc, meta))
}

pub(crate) fn account_meta_from_shard_account(
    shard_account: &ShardAccount,
    shard_account_boc: &BocBytes,
    cas: &mut CellStore,
) -> anyhow::Result<AccountMeta> {
    let account_hash = shard_account_boc.hash()?;
    cas.put(shard_account_boc.clone(), account_hash);

    let optional_account = shard_account.account.load()?;
    let Some(account) = optional_account.0 else {
        return Ok(AccountMeta {
            account_hash,
            status: AccountStatus::Nonexist,
            balance: 0,
            extra_currencies: Vec::new(),
            last_trans_lt: Some(shard_account.last_trans_lt),
            last_trans_hash: Some(Hash256::from(&shard_account.last_trans_hash)),
            code_hash: None,
            data_hash: None,
            frozen_hash: None,
        });
    };

    let balance = account.balance.tokens.into();
    let extra_currencies = ExtraCurrency::from_collection(&account.balance.other)?;
    let mut code_hash = None;
    let mut data_hash = None;
    let mut frozen_hash = None;
    let status = match account.state {
        AccountState::Active(state) => {
            code_hash = state.code.map(|cell| cas.put_cell(cell));
            data_hash = state.data.map(|cell| cas.put_cell(cell));
            AccountStatus::Active
        }
        AccountState::Uninit => AccountStatus::Uninit,
        AccountState::Frozen(hash) => {
            frozen_hash = Some(Hash256::from(hash));
            AccountStatus::Frozen
        }
    };

    Ok(AccountMeta {
        account_hash,
        status,
        balance,
        extra_currencies,
        last_trans_lt: Some(shard_account.last_trans_lt),
        last_trans_hash: Some(Hash256::from(&shard_account.last_trans_hash)),
        code_hash,
        data_hash,
        frozen_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "current_thread")]
    async fn synchronous_api_client_request_is_safe_inside_runtime() {
        let provider = RemoteProvider {
            network: Network::Mainnet,
            fork_block_number: Some(81_000_000),
            fork_snapshot: None,
            client: Default::default(),
        };

        with_api_client(&provider, |_| Ok(())).unwrap();
    }

    #[test]
    fn shard_history_uses_the_matching_fork_frontier_instead_of_masterchain_seqno() {
        let boundaries = [
            RemoteShardBoundary {
                workchain: 0,
                shard: 0x4000_0000_0000_0000,
                seqno: 70,
            },
            RemoteShardBoundary {
                workchain: 0,
                shard: 0xc000_0000_0000_0000,
                seqno: 75,
            },
        ];
        let full = ShardIdent::BASECHAIN;
        let left = ShardIdent::new(0, 0x4000_0000_0000_0000).expect("left shard must be valid");
        let left_child =
            ShardIdent::new(0, 0x2000_0000_0000_0000).expect("left child must be valid");

        assert!(!contains_historical_shard_block(&boundaries, full, 75));
        assert!(!contains_historical_shard_block(&boundaries, full, 76));
        assert!(contains_historical_shard_block(&boundaries, left, 70));
        assert!(!contains_historical_shard_block(&boundaries, left, 71));
        assert!(!contains_historical_shard_block(
            &boundaries,
            left_child,
            71
        ));
    }

    #[test]
    fn remote_shard_parser_accepts_toncenter_hex_and_signed_formats() {
        assert_eq!(
            parse_remote_shard("8000000000000000").unwrap(),
            ShardIdent::PREFIX_FULL
        );
        assert_eq!(
            parse_remote_shard("-9223372036854775808").unwrap(),
            ShardIdent::PREFIX_FULL
        );
    }
}
