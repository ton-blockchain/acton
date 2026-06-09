use crate::storage::{AccountMeta, AccountStatus, CellStore};
use crate::types::{Addr, BocBytes, Hash256};
use acton_config::config;
use base64::Engine;
use serde::{Deserialize, Serialize};
use ton_api::TonApiClient;
use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Lazy, Store};
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, ExtraCurrencyCollection, IntAddr, OptionalAccount,
    ShardAccount, StateInit, StdAddr, StorageInfo,
};
use tycho_types::num::Tokens;
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

    if let Ok(cell) =
        api_client.get_shard_account_cell(provider.fork_block_number, &addr.to_string())
    {
        let shard_account = cell.parse::<ShardAccount>()?;
        let boc = Boc::encode(cell);
        let meta = account_meta_from_shard_account(&shard_account, &boc, cas)?;
        return Ok((boc.into(), meta));
    }

    let info = api_client.get_account_info(provider.fork_block_number, &addr.to_string())?;

    let code = if info.code.is_empty() {
        None
    } else {
        let b = base64::engine::general_purpose::STANDARD.decode(&info.code)?;
        let h = compute_boc_hash(&b)?;
        cas.put(b.into(), h);
        Some(h)
    };

    let data_boc = if info.data.is_empty() {
        None
    } else {
        let b = base64::engine::general_purpose::STANDARD.decode(&info.data)?;
        let h = compute_boc_hash(&b)?;
        cas.put(b.into(), h);
        Some(h)
    };

    let last_trans_lt = info.last_transaction_id.lt.parse::<u64>().unwrap_or(0);
    let storage_last_trans_lt = last_trans_lt.saturating_add(1);
    let last_trans_hash = Hash256::from_base64(&info.last_transaction_id.hash).ok();

    let status = match info.state.as_str() {
        "active" => AccountStatus::Active,
        "frozen" => AccountStatus::Frozen,
        "uninitialized" => AccountStatus::Uninit,
        _ => AccountStatus::Nonexist,
    };

    let balance = u128::try_from(info.balance.to_bigint().unwrap_or_default())?;

    let account_state = if status == AccountStatus::Active {
        let code_cell = code
            .and_then(|h| cas.get(&h))
            .and_then(|b| Boc::decode(b).ok());
        let data_cell = data_boc
            .and_then(|h| cas.get(&h))
            .and_then(|b| Boc::decode(b).ok());

        AccountState::Active(StateInit {
            split_depth: None,
            special: None,
            code: code_cell,
            data: data_cell,
            libraries: tycho_types::dict::Dict::new(),
        })
    } else {
        AccountState::Uninit
    };

    let acc = Account {
        address: IntAddr::Std(StdAddr {
            anycast: None,
            workchain: addr.workchain as i8,
            address: HashBytes(addr.addr),
        }),
        balance: CurrencyCollection {
            tokens: Tokens::new(balance),
            other: ExtraCurrencyCollection::new(),
        },
        state: account_state,
        last_trans_lt: storage_last_trans_lt,
        storage_stat: StorageInfo::default(),
    };

    let sa = ShardAccount {
        account: Lazy::new(&OptionalAccount(Some(acc)))?,
        last_trans_hash: HashBytes(last_trans_hash.map_or([0; 32], |h| h.0)),
        last_trans_lt,
    };

    let mut builder = CellBuilder::new();
    sa.store_into(&mut builder, Cell::empty_context())?;
    let cell = builder.build()?;
    let boc = Boc::encode(cell);
    let account_hash = compute_boc_hash(&boc)?;
    cas.put(boc.clone().into(), account_hash);

    let meta = AccountMeta {
        account_hash,
        status,
        cached_balance: Some(balance),
        last_trans_lt: Some(last_trans_lt),
        last_trans_hash,
        code_hash: code,
        data_hash: data_boc,
        frozen_hash: None,
    };

    Ok((boc.into(), meta))
}

fn compute_boc_hash(boc: &[u8]) -> anyhow::Result<Hash256> {
    let cell = Boc::decode(boc)?;
    let hash = cell.repr_hash();
    Ok(Hash256(*hash.as_array()))
}

pub(crate) fn account_meta_from_shard_account(
    shard_account: &ShardAccount,
    shard_account_boc: &[u8],
    cas: &mut CellStore,
) -> anyhow::Result<AccountMeta> {
    let account_hash = compute_boc_hash(shard_account_boc)?;
    cas.put(shard_account_boc.to_vec().into(), account_hash);

    let optional_account = shard_account.account.load()?;
    let Some(account) = optional_account.0 else {
        return Ok(AccountMeta {
            account_hash,
            status: AccountStatus::Nonexist,
            cached_balance: Some(0),
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
        cached_balance: Some(balance),
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
