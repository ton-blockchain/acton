use crate::storage::{AccountMeta, AccountStatus, CellStore};
use crate::types::{Addr, BocBytes, Hash256};
use acton_config::config;
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

pub fn fetch_remote_library(hash: &Hash256, provider: &RemoteProvider) -> anyhow::Result<Cell> {
    let config = config::ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let api_client = TonApiClient::new(provider.network.clone(), custom_networks)?;

    let lib = api_client.get_library_by_hash(&HashBytes(hash.0))?;
    let actual_hash = Hash256(*lib.repr_hash().as_array());
    if actual_hash != *hash {
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

    let config = config::ActonConfig::load().unwrap_or_default();
    let custom_networks = config.custom_networks();
    let api_client = TonApiClient::new(provider.network.clone(), custom_networks)?;

    let cell = api_client.get_shard_account_cell(provider.fork_block_number, &addr.to_string())?;
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
            last_trans_lt: Some(shard_account.last_trans_lt),
            last_trans_hash: Some(Hash256(*shard_account.last_trans_hash.as_array())),
            code_hash: None,
            data_hash: None,
            frozen_hash: None,
        });
    };

    let balance = account.balance.tokens.into();
    let mut code_hash = None;
    let mut data_hash = None;
    let mut frozen_hash = None;
    let status = match account.state {
        AccountState::Active(state) => {
            code_hash = state.code.map(|cell| put_cell(cas, cell));
            data_hash = state.data.map(|cell| put_cell(cas, cell));
            AccountStatus::Active
        }
        AccountState::Uninit => AccountStatus::Uninit,
        AccountState::Frozen(hash) => {
            frozen_hash = Some(Hash256(*hash.as_array()));
            AccountStatus::Frozen
        }
    };

    Ok(AccountMeta {
        account_hash,
        status,
        balance,
        last_trans_lt: Some(shard_account.last_trans_lt),
        last_trans_hash: Some(Hash256(*shard_account.last_trans_hash.as_array())),
        code_hash,
        data_hash,
        frozen_hash,
    })
}

fn put_cell(cas: &mut CellStore, cell: Cell) -> Hash256 {
    let hash = Hash256(*cell.repr_hash().as_array());
    cas.put(Boc::encode(cell).into(), hash);
    hash
}
