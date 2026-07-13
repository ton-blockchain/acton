use crate::storage::{AccountMeta, AccountStatus, CellStore};
use crate::types::{Addr, BocBytes, ExtraCurrency, Hash256};
use acton_config::config;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use ton_api::TonApiClient;
use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::Cell;
use tycho_types::models::{AccountState, ShardAccount};
use tycho_types::prelude::HashBytes;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteProvider {
    pub network: Network,
    pub fork_block_number: Option<u64>,
}

// TODO: remove
fn with_api_client<T: Send + 'static>(
    provider: &RemoteProvider,
    request: impl FnOnce(&TonApiClient) -> anyhow::Result<T> + Send + 'static,
) -> anyhow::Result<T> {
    let network = provider.network.clone();
    std::thread::Builder::new()
        .name("acton-remote-provider".to_owned())
        .spawn(move || {
            let config = config::ActonConfig::load().unwrap_or_default();
            let api_client = TonApiClient::new(network, config.custom_networks())?;
            request(&api_client)
        })
        .context("Failed to start remote provider worker")?
        .join()
        .map_err(|_| anyhow::anyhow!("Remote provider worker panicked"))?
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
