//! Validated account edits. Balances are decimal nanotons; cells are base64 BoCs.
use crate::{AccountWrite, AdminBatch, HardforkSources};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tycho_types::{
    boc::Boc,
    cell::{Cell, CellBuilder, CellFamily, Lazy, Store},
    models::{
        Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount,
        ShardStateUnsplit, StateInit, StdAddr, StorageExtra, StorageInfo, StorageUsed,
    },
    num::VarUint56,
    prelude::HashBytes,
};

#[derive(Clone, Debug, Serialize)]
pub struct AccountEdit {
    pub address: String,
    #[serde(flatten)]
    pub change: AccountChange,
}

impl<'de> Deserialize<'de> for AccountEdit {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut fields = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
        let address = fields
            .remove("address")
            .and_then(|v| v.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("Expected account address"))?;
        let allowed: &[&str] = match fields.get("type").and_then(serde_json::Value::as_str) {
            Some("balance" | "uninit") => &["type", "balance"],
            Some("code" | "data" | "replace") => &["type", "boc"],
            Some("freeze" | "delete") => &["type"],
            _ => return Err(D::Error::custom("Unknown account action")),
        };
        if fields.keys().any(|key| !allowed.contains(&key.as_str())) {
            return Err(D::Error::custom("Unknown account edit field"));
        }
        let change =
            serde_json::from_value(serde_json::Value::Object(fields)).map_err(D::Error::custom)?;
        Ok(Self { address, change })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum AccountChange {
    Balance { balance: String },
    Code { boc: String },
    Data { boc: String },
    Freeze,
    Uninit { balance: Option<String> },
    Delete,
    Replace { boc: String },
}

pub fn decode_cell(value: &str) -> Result<Cell> {
    ensure!(value.len() <= 16 * 1024 * 1024, "BoC exceeds 16 MiB");
    Boc::decode(
        STANDARD
            .decode(value.trim())
            .context("Invalid base64 BoC")?,
    )
    .context("Invalid BoC")
}

impl AccountEdit {
    pub fn validate(&self) -> Result<StdAddr> {
        let address: StdAddr = self
            .address
            .parse()
            .context("Expected a workchain:hex address")?;
        ensure!(
            matches!(address.workchain, -1 | 0),
            "Only masterchain and basechain accounts are supported"
        );
        match &self.change {
            AccountChange::Balance { balance }
            | AccountChange::Uninit {
                balance: Some(balance),
            } => {
                parse_balance(balance)?;
            }
            AccountChange::Code { boc } | AccountChange::Data { boc } => {
                decode_cell(boc)?;
            }
            AccountChange::Replace { boc } => {
                let account = decode_cell(boc)?
                    .parse::<ShardAccount>()
                    .context("Expected a ShardAccount BoC")?;
                ensure!(
                    account
                        .load_account()?
                        .context("Use delete for a nonexistent account")?
                        .address
                        == IntAddr::Std(address.clone()),
                    "Replacement account address does not match the target"
                );
            }
            _ => {}
        }
        Ok(address)
    }
}

pub fn account_batch(sources: &HardforkSources, edits: &[AccountEdit]) -> Result<AdminBatch> {
    ensure!(
        !edits.is_empty() && edits.len() <= 100,
        "An operation must contain 1–100 account edits"
    );
    let mc = sources.masterchain_state.parse::<ShardStateUnsplit>()?;
    let shard = sources
        .basechain
        .as_ref()
        .map(|s| s.state.parse::<ShardStateUnsplit>())
        .transpose()?;
    let mut seen = BTreeSet::new();
    let mut batch = AdminBatch::default();
    for edit in edits {
        let address = edit.validate()?;
        ensure!(
            seen.insert(address.to_string()),
            "Duplicate account edit: {address}"
        );
        let state = if address.workchain == -1 {
            &mc
        } else {
            shard.as_ref().context("Basechain state is missing")?
        };
        let existing = state.accounts.load()?.get(address.address)?.map(|(_, a)| a);
        let write = apply_edit(&address, existing, &edit.change, mc.gen_utime)?;
        if address.workchain == -1 {
            batch.masterchain.push(write);
        } else {
            batch.basechain.push(write);
        }
    }
    Ok(batch)
}

fn parse_balance(value: &str) -> Result<tycho_types::num::Tokens> {
    ensure!(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "Balance must be a nonnegative integer in nanotons"
    );
    let balance = tycho_types::num::Tokens::new(value.parse().context("Balance is too large")?);
    ensure!(balance.is_valid(), "Balance exceeds the TON currency limit");
    Ok(balance)
}

fn apply_edit(
    address: &StdAddr,
    existing: Option<ShardAccount>,
    change: &AccountChange,
    now: u32,
) -> Result<AccountWrite> {
    if matches!(change, AccountChange::Delete) {
        return Ok(AccountWrite::remove(address.address));
    }
    let mut record = existing.unwrap_or(ShardAccount {
        account: Lazy::new(&OptionalAccount(None))?,
        last_trans_hash: HashBytes::ZERO,
        last_trans_lt: 0,
    });
    if let AccountChange::Replace { boc } = change {
        record = decode_cell(boc)?.parse()?;
    }
    let mut account = record.load_account()?.unwrap_or(Account {
        address: IntAddr::Std(address.clone()),
        storage_stat: StorageInfo {
            used: StorageUsed::ZERO,
            storage_extra: StorageExtra::None,
            last_paid: now,
            due_payment: None,
        },
        last_trans_lt: 0,
        balance: CurrencyCollection::ZERO,
        state: AccountState::Uninit,
    });
    match change {
        AccountChange::Balance { balance } => account.balance.tokens = parse_balance(balance)?,
        AccountChange::Code { boc } => {
            if matches!(account.state, AccountState::Uninit) {
                account.state = AccountState::Active(StateInit::default());
            }
            let AccountState::Active(state) = &mut account.state else {
                bail!("Cannot replace code of a frozen account; replace its state first");
            };
            state.code = Some(decode_cell(boc)?);
        }
        AccountChange::Data { boc } => {
            let AccountState::Active(state) = &mut account.state else {
                bail!("Data can only be changed on an active account");
            };
            state.data = Some(decode_cell(boc)?);
        }
        AccountChange::Freeze => {
            let AccountState::Active(state) = &account.state else {
                bail!("Only an active account can be frozen");
            };
            account.state = AccountState::Frozen(*CellBuilder::build_from(state)?.repr_hash());
        }
        AccountChange::Uninit { balance } => {
            account.state = AccountState::Uninit;
            if let Some(balance) = balance {
                account.balance.tokens = parse_balance(balance)?;
            }
        }
        _ => {}
    }
    // Compute AccountStorage, excluding Account's address and StorageInfo. The
    // upstream 0.3.5 StorageUsed::compute has an inverted validity predicate.
    let mut storage = CellBuilder::new();
    storage.store_u64(account.last_trans_lt)?;
    account
        .balance
        .store_into(&mut storage, Cell::empty_context())?;
    account
        .state
        .store_into(&mut storage, Cell::empty_context())?;
    let stats = storage
        .build()?
        .compute_unique_stats(1_000_000)
        .context("Account storage exceeds the cell limit")?;
    account.storage_stat.used = StorageUsed {
        cells: VarUint56::new(stats.cell_count),
        bits: VarUint56::new(stats.bit_count),
    };
    account.storage_stat.storage_extra = StorageExtra::None;
    record.account = Lazy::new(&OptionalAccount(Some(account)))?;
    Ok(AccountWrite::set(address.address, record))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_account_edits_deserialize_and_validate() {
        let edits: Vec<AccountEdit> = serde_json::from_value(serde_json::json!([
            {"address": format!("0:{}", "11".repeat(32)), "type": "balance", "balance": "1000000000"},
            {"address": format!("-1:{}", "22".repeat(32)), "type": "uninit"}
        ])).unwrap();
        for edit in edits {
            edit.validate().unwrap();
        }
    }

    #[test]
    fn invalid_balances_and_unknown_fields_are_rejected() {
        for value in ["-1", "1.5", "", "340282366920938463463374607431768211455"] {
            assert!(parse_balance(value).is_err());
        }
        assert!(
            serde_json::from_value::<AccountEdit>(
                serde_json::json!({"address": "0:00", "type": "delete", "typo": true})
            )
            .is_err()
        );
    }

    #[test]
    fn changing_balance_preserves_history_and_updates_storage() {
        let address = StdAddr::new(0, HashBytes([1; 32]));
        let created = apply_edit(
            &address,
            None,
            &AccountChange::Balance {
                balance: "1000000000".into(),
            },
            50,
        )
        .unwrap();
        let mut record = *created.account.unwrap();
        record.last_trans_hash = HashBytes([2; 32]);
        record.last_trans_lt = 17;
        let updated = apply_edit(
            &address,
            Some(record),
            &AccountChange::Balance {
                balance: "2000000000".into(),
            },
            51,
        )
        .unwrap()
        .account
        .unwrap();
        assert_eq!(updated.last_trans_hash, HashBytes([2; 32]));
        assert_eq!(updated.last_trans_lt, 17);
        let account = updated.load_account().unwrap().unwrap();
        assert_eq!(account.balance.tokens.into_inner(), 2000000000);
        assert!(account.storage_stat.used.bits.into_inner() > 0);
        assert_eq!(account.storage_stat.last_paid, 50);
    }

    #[test]
    fn frozen_or_uninitialized_data_cannot_be_overwritten() {
        let address = StdAddr::new(0, HashBytes([1; 32]));
        let cell = STANDARD.encode(Boc::encode(Cell::default()));
        assert!(apply_edit(&address, None, &AccountChange::Data { boc: cell }, 1).is_err());
        assert!(apply_edit(&address, None, &AccountChange::Freeze, 1).is_err());
    }
}
