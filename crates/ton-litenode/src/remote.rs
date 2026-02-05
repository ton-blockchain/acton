use crate::storage::{AccountMeta, AccountStatus, CellStore};
use crate::types::{Addr, BocBytes, Hash256};
use anyhow::Context;
use base64::Engine;
use serde::Deserialize;
use tycho_types::boc::Boc;
use tycho_types::cell::{CellBuilder, CellFamily, Store};
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount, StateInit,
    StdAddr, StorageInfo,
};
use tycho_types::num::Tokens;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RemoteProvider {
    pub network: String, // "mainnet" or "testnet"
    pub api_key: Option<String>,
}

pub fn fetch_remote_shard_account(
    addr: &Addr,
    provider: &RemoteProvider,
    cas: &mut CellStore,
) -> anyhow::Result<(BocBytes, AccountMeta)> {
    tracing::info!("Fetching remote account state for {}", addr);
    let address_str = format!("{}:{}", addr.workchain, hex::encode(addr.addr));
    let base_url = match provider.network.as_str() {
        "mainnet" => "https://toncenter.com/api/v2",
        "testnet" => "https://testnet.toncenter.com/api/v2",
        _ => anyhow::bail!("Unsupported network: {}", provider.network),
    };

    let url = format!("{}/getAddressInformation?address={}", base_url, address_str);
    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url).header("User-Agent", "acton-litenode");

    if let Some(key) = &provider.api_key {
        request = request.header("X-API-Key", key);
    }

    let response = request
        .send()
        .context("Failed to send request to TonCenter")?;

    if !response.status().is_success() {
        anyhow::bail!("TonCenter API returned status: {}", response.status());
    }

    #[derive(Deserialize)]
    struct TonCenterResponse {
        result: TonCenterAccountInfo,
    }

    #[derive(Deserialize)]
    struct TonCenterAccountInfo {
        state: String,
        balance: String,
        code: String,
        data: String,
        last_transaction_id: TonCenterTxId,
    }

    #[derive(Deserialize)]
    struct TonCenterTxId {
        lt: String,
        hash: String,
    }

    let data: TonCenterResponse = response
        .json()
        .context("Failed to parse TonCenter response")?;

    let info = data.result;
    if info.state == "uninitialized" || info.state == "nonexist" {
        anyhow::bail!("Account does not exist on remote");
    }

    let code = if info.code.is_empty() {
        None
    } else {
        let b = base64::engine::general_purpose::STANDARD.decode(&info.code)?;
        let h = compute_boc_hash(&b)?;
        cas.put(b, h);
        Some(h)
    };

    let data_boc = if info.data.is_empty() {
        None
    } else {
        let b = base64::engine::general_purpose::STANDARD.decode(&info.data)?;
        let h = compute_boc_hash(&b)?;
        cas.put(b, h);
        Some(h)
    };

    let last_trans_lt = info.last_transaction_id.lt.parse::<u64>().unwrap_or(0);
    let last_trans_hash = Hash256::from_base64(&info.last_transaction_id.hash).ok();

    let status = match info.state.as_str() {
        "active" => AccountStatus::Active,
        "frozen" => AccountStatus::Frozen,
        "uninitialized" => AccountStatus::Uninit,
        _ => AccountStatus::Nonexist,
    };

    let balance = info.balance.parse::<u128>().unwrap_or(0);

    // Construct ShardAccount BOC
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
            address: tycho_types::prelude::HashBytes(addr.addr),
        }),
        balance: CurrencyCollection {
            tokens: Tokens::new(balance),
            other: tycho_types::models::ExtraCurrencyCollection::new(),
        },
        state: account_state,
        last_trans_lt,
        storage_stat: StorageInfo::default(),
    };

    let sa = ShardAccount {
        account: tycho_types::cell::Lazy::new(&OptionalAccount(Some(acc)))?,
        last_trans_hash: tycho_types::prelude::HashBytes(
            last_trans_hash.map(|h| h.0).unwrap_or([0; 32]),
        ),
        last_trans_lt,
    };

    let mut builder = CellBuilder::new();
    sa.store_into(&mut builder, tycho_types::cell::Cell::empty_context())?;
    let cell = builder.build()?;
    let boc = Boc::encode(cell);
    let account_hash = compute_boc_hash(&boc)?;
    cas.put(boc.clone(), account_hash);

    let meta = AccountMeta {
        account_hash,
        status,
        cached_balance: Some(balance),
        last_trans_lt: Some(last_trans_lt),
        last_trans_hash,
        code_hash: code,
        data_hash: data_boc,
    };

    Ok((boc, meta))
}

fn compute_boc_hash(boc: &[u8]) -> anyhow::Result<Hash256> {
    let cell = Boc::decode(boc)?;
    let hash = cell.repr_hash();
    Ok(Hash256(*hash.as_array()))
}
