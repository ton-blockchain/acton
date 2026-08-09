//! Parses account snapshots that will be imported into a new zerostate.
//!
//! Each `--add-account` value is a hex-encoded `ShardAccount` BoC. This module
//! validates the snapshot, extracts the data required by the TON state-building
//! tools, and records a stable descriptor in the network manifest.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail, ensure};
use tycho_types::{
    boc::{Boc, BocTag, de},
    cell::{Cell, HashBytes, Load},
    models::{AccountState, IntAddr, OptionalAccount, ShardAccount, SimpleLib, StateInit},
};

use crate::storage::ImportedAccountDescriptor;

#[derive(Debug, Clone)]
pub struct ImportedAccount {
    pub descriptor: ImportedAccountDescriptor,
    pub account_id_hex: String,
    pub balance_nano: u128,
    pub fixed_prefix_length: u8,
    pub shard_account_boc: Vec<u8>,
    pub code_boc: Option<Vec<u8>>,
    pub data_boc: Option<Vec<u8>>,
    pub libraries_boc: Option<Vec<u8>>,
}

pub fn parse_imported_accounts(values: &[String]) -> Result<Vec<ImportedAccount>> {
    let mut accounts = BTreeMap::new();
    for value in values {
        let account = parse_imported_account(value)?;
        let address = account.descriptor.address.clone();
        if accounts.insert(address.clone(), account).is_some() {
            bail!("duplicate --add-account address `{address}`");
        }
    }
    Ok(accounts.into_values().collect())
}

fn parse_imported_account(value: &str) -> Result<ImportedAccount> {
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    ensure!(!value.is_empty(), "--add-account ShardAccount hex is empty");

    let bytes = hex::decode(value).context("--add-account is not valid hexadecimal")?;
    ensure_boc_consumes_all(&bytes)?;
    let root = Boc::decode(&bytes).context("--add-account is not a valid single-root BoC")?;
    let shard_account = load_cell_exact::<ShardAccount>(
        &root,
        "--add-account BoC root is not a valid ShardAccount",
    )?;
    let OptionalAccount(account) = load_cell_exact::<OptionalAccount>(
        shard_account.account.inner(),
        "--add-account contains an invalid Account",
    )?;
    let account = account.context("--add-account contains Account::None")?;

    let address = match &account.address {
        IntAddr::Std(address) => address,
        IntAddr::Var(_) => bail!("--add-account contains a variable-length account address"),
    };
    ensure!(
        address.workchain == 0,
        "--add-account address `{address}` is not in basechain workchain 0"
    );
    ensure!(
        address.anycast.is_none(),
        "--add-account address `{address}` uses anycast, which zerostate import does not support"
    );
    ensure!(
        account.balance.other.is_empty(),
        "--add-account `{address}` has extra currencies, which create-state cannot preserve"
    );

    let state_init = match &account.state {
        AccountState::Active(state_init) => state_init,
        AccountState::Uninit => bail!("--add-account `{address}` is uninitialized"),
        AccountState::Frozen(_) => bail!("--add-account `{address}` is frozen"),
    };
    ensure!(
        state_init.special.is_none(),
        "--add-account `{address}` has tick/tock flags, which are invalid in basechain"
    );
    if let Some(code) = &state_init.code {
        ensure!(
            !is_empty_cell(code),
            "--add-account `{address}` has an explicitly empty code cell, \
             which create-state cannot preserve"
        );
    }
    if let Some(data) = &state_init.data {
        ensure!(
            !is_empty_cell(data),
            "--add-account `{address}` has an explicitly empty data cell, \
            which create-state cannot preserve"
        );
    }
    let canonical_address = address.to_string();
    validate_libraries(&canonical_address, state_init)?;
    let fixed_prefix_length = state_init
        .split_depth
        .map(|depth| depth.into_bit_len() as u8)
        .unwrap_or(0);
    ensure!(
        fixed_prefix_length == 0,
        "--add-account `{address}` uses split depth, which zerostate import does not support"
    );

    let balance_nano = account.balance.tokens.into_inner();
    let canonical_shard_account_boc = Boc::encode(&root);
    let descriptor = ImportedAccountDescriptor {
        address: canonical_address,
        shard_account_hash: root.repr_hash().to_string(),
        balance_nano: balance_nano.to_string(),
    };

    Ok(ImportedAccount {
        account_id_hex: address.address.to_string(),
        descriptor,
        balance_nano,
        fixed_prefix_length,
        shard_account_boc: canonical_shard_account_boc,
        code_boc: state_init.code.as_ref().map(encode_cell),
        data_boc: state_init.data.as_ref().map(encode_cell),
        libraries_boc: state_init.libraries.root().as_ref().map(encode_cell),
    })
}

fn encode_cell(cell: &Cell) -> Vec<u8> {
    Boc::encode(cell)
}

fn is_empty_cell(cell: &Cell) -> bool {
    !cell.is_exotic() && cell.bit_len() == 0 && cell.reference_count() == 0
}

fn ensure_boc_consumes_all(bytes: &[u8]) -> Result<()> {
    let header = de::BocHeader::decode(bytes, &de::Options::exact(1))
        .context("--add-account is not a valid single-root BoC")?;
    let last_cell = header
        .cells()
        .last()
        .context("--add-account BoC contains no cells")?;
    let cells_end = (last_cell.as_ptr() as usize)
        .checked_sub(bytes.as_ptr() as usize)
        .and_then(|offset| offset.checked_add(last_cell.len()))
        .context("--add-account BoC has invalid cell offsets")?;
    let tag_bytes: [u8; 4] = bytes
        .get(..4)
        .and_then(|value| value.try_into().ok())
        .context("--add-account BoC has an invalid header")?;
    let has_crc = match BocTag::from_bytes(tag_bytes) {
        Some(BocTag::IndexedCrc32) => true,
        Some(BocTag::Generic) => bytes.get(4).is_some_and(|flags| flags & 0x40 != 0),
        Some(BocTag::Indexed) => false,
        None => bail!("--add-account BoC has an unknown tag"),
    };
    let consumed = cells_end
        .checked_add(usize::from(has_crc) * 4)
        .context("--add-account BoC length overflow")?;
    ensure!(
        consumed == bytes.len(),
        "--add-account BoC has {} trailing byte(s)",
        bytes.len().saturating_sub(consumed)
    );
    Ok(())
}

fn validate_libraries(address: &str, state_init: &StateInit) -> Result<()> {
    if let Some(root) = state_init.libraries.root() {
        ensure!(
            !is_empty_cell(root),
            "--add-account `{address}` has an explicitly empty libraries root, \
             which create-state cannot preserve"
        );
    }

    for entry in state_init.libraries.raw_iter() {
        let (key, mut value) = entry.with_context(|| {
            format!("--add-account `{address}` has an invalid libraries dictionary")
        })?;
        let mut key_slice = key.as_data_slice();
        let key = HashBytes::load_from(&mut key_slice)
            .with_context(|| format!("--add-account `{address}` has an invalid library key"))?;
        ensure!(
            key_slice.is_empty(),
            "--add-account `{address}` has trailing data in a library key"
        );
        let library = SimpleLib::load_from(&mut value)
            .with_context(|| format!("--add-account `{address}` has an invalid library value"))?;
        ensure!(
            value.is_empty(),
            "--add-account `{address}` has trailing data in a library value"
        );
        ensure!(
            &key == library.root.repr_hash(),
            "--add-account `{address}` has a library key that does not match its root hash"
        );
        ensure!(
            !library.public,
            "--add-account `{address}` publishes a public library; \
             basechain zerostates cannot contain public libraries"
        );
    }
    Ok(())
}

pub(crate) fn load_cell_exact<'a, T>(cell: &'a Cell, description: &str) -> Result<T>
where
    T: Load<'a>,
{
    let mut slice = cell.as_slice().with_context(|| description.to_owned())?;
    let value = T::load_from(&mut slice).with_context(|| description.to_owned())?;
    ensure!(
        slice.is_empty(),
        "{description}: trailing data remains ({} bits, {} refs)",
        slice.size_bits(),
        slice.size_refs()
    );
    Ok(value)
}

#[cfg(test)]
mod tests {
    use tycho_types::{
        boc::BocRepr,
        cell::{CellBuilder, HashBytes, Lazy},
        dict::Dict,
        models::{
            Account, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount, SimpleLib,
            StateInit, StdAddr, StorageInfo,
        },
    };

    use super::*;

    fn cell_with_u32(value: u32) -> Cell {
        let mut builder = CellBuilder::new();
        builder.store_u32(value).unwrap();
        builder.build().unwrap()
    }

    fn account_hex_with_state(address_byte: u8, balance: u128, state: StateInit) -> String {
        BocRepr::encode_hex(shard_account(address_byte, balance, state)).unwrap()
    }

    fn shard_account(address_byte: u8, balance: u128, state: StateInit) -> ShardAccount {
        ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                address: IntAddr::Std(StdAddr::new(0, HashBytes([address_byte; 32]))),
                storage_stat: StorageInfo::default(),
                last_trans_lt: 123,
                balance: CurrencyCollection::new(balance),
                state: AccountState::Active(state),
            })))
            .unwrap(),
            last_trans_hash: HashBytes([0x55; 32]),
            last_trans_lt: 456,
        }
    }

    fn account_hex(address_byte: u8, balance: u128) -> String {
        account_hex_with_state(
            address_byte,
            balance,
            StateInit {
                code: Some(cell_with_u32(0x1234_5678)),
                data: Some(cell_with_u32(0x90ab_cdef)),
                ..Default::default()
            },
        )
    }

    fn state_with_library(public: bool, matching_key: bool) -> StateInit {
        let root = cell_with_u32(0xfeed_beef);
        let key = if matching_key {
            *root.repr_hash()
        } else {
            HashBytes([0x77; 32])
        };
        let mut libraries = Dict::new();
        libraries.set(key, SimpleLib { public, root }).unwrap();
        StateInit {
            code: Some(cell_with_u32(0x1234_5678)),
            libraries,
            ..Default::default()
        }
    }

    #[test]
    fn parses_shard_account_and_extracts_import_fields() {
        let fixture = account_hex(0x11, 123_456_789);
        let parsed = parse_imported_accounts(&[fixture]).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].descriptor.address,
            format!("0:{}", "11".repeat(32))
        );
        assert_eq!(parsed[0].balance_nano, 123_456_789);
        assert_eq!(parsed[0].descriptor.balance_nano, "123456789");
        assert!(parsed[0].code_boc.is_some());
        assert!(parsed[0].data_boc.is_some());
    }

    #[test]
    fn accepts_repeated_accounts_in_canonical_address_order() {
        let parsed =
            parse_imported_accounts(&[account_hex(0x22, 2), account_hex(0x11, 1)]).unwrap();
        assert!(parsed[0].descriptor.address.ends_with(&"11".repeat(32)));
        assert!(parsed[1].descriptor.address.ends_with(&"22".repeat(32)));
    }

    #[test]
    fn rejects_duplicate_account_addresses() {
        let error =
            parse_imported_accounts(&[account_hex(0x11, 1), account_hex(0x11, 2)]).unwrap_err();
        assert!(error.to_string().contains("duplicate --add-account"));
    }

    #[test]
    fn rejects_malformed_hex_and_boc() {
        assert!(
            parse_imported_accounts(&["not-hex".to_owned()])
                .unwrap_err()
                .to_string()
                .contains("valid hexadecimal")
        );
        assert!(
            parse_imported_accounts(&["00".to_owned()])
                .unwrap_err()
                .to_string()
                .contains("single-root BoC")
        );

        let mut boc_with_tail = hex::decode(account_hex(0x11, 1)).unwrap();
        boc_with_tail.extend_from_slice(&[0xaa, 0xbb]);
        assert!(
            parse_imported_accounts(&[hex::encode(boc_with_tail)])
                .unwrap_err()
                .to_string()
                .contains("trailing byte")
        );
    }

    #[test]
    fn rejects_cells_that_create_state_would_silently_drop() {
        let empty = CellBuilder::new().build().unwrap();
        let empty_code = account_hex_with_state(
            0x11,
            1,
            StateInit {
                code: Some(empty.clone()),
                ..Default::default()
            },
        );
        assert!(
            parse_imported_accounts(&[empty_code])
                .unwrap_err()
                .to_string()
                .contains("explicitly empty code cell")
        );

        let empty_data = account_hex_with_state(
            0x11,
            1,
            StateInit {
                code: Some(cell_with_u32(0x1234_5678)),
                data: Some(empty),
                ..Default::default()
            },
        );
        assert!(
            parse_imported_accounts(&[empty_data])
                .unwrap_err()
                .to_string()
                .contains("explicitly empty data cell")
        );
    }

    #[test]
    fn accepts_active_account_without_code() {
        let fixture = account_hex_with_state(
            0x11,
            1,
            StateInit {
                data: Some(cell_with_u32(0x90ab_cdef)),
                ..Default::default()
            },
        );
        let parsed = parse_imported_accounts(&[fixture]).unwrap();
        assert!(parsed[0].code_boc.is_none());
        assert!(parsed[0].data_boc.is_some());
    }

    #[test]
    fn rejects_trailing_data_in_shard_account_and_account_cells() {
        let state = StateInit {
            code: Some(cell_with_u32(0x1234_5678)),
            ..Default::default()
        };
        let original = shard_account(0x11, 1, state.clone());

        let root = Boc::decode(BocRepr::encode(&original).unwrap()).unwrap();
        let mut root_builder = CellBuilder::new();
        root_builder.store_slice(root.as_slice().unwrap()).unwrap();
        root_builder.store_bit(true).unwrap();
        let root_with_tail = root_builder.build().unwrap();
        assert!(
            parse_imported_accounts(&[hex::encode(Boc::encode(root_with_tail))])
                .unwrap_err()
                .to_string()
                .contains("trailing data remains")
        );

        let account = original.account.inner();
        let mut account_builder = CellBuilder::new();
        account_builder
            .store_slice(account.as_slice().unwrap())
            .unwrap();
        account_builder.store_bit(true).unwrap();
        let account_with_tail = account_builder.build().unwrap();
        let nested_tail = ShardAccount {
            account: Lazy::from_raw(account_with_tail).unwrap(),
            last_trans_hash: original.last_trans_hash,
            last_trans_lt: original.last_trans_lt,
        };
        assert!(
            parse_imported_accounts(&[BocRepr::encode_hex(nested_tail).unwrap()])
                .unwrap_err()
                .to_string()
                .contains("trailing data remains")
        );
    }

    #[test]
    fn validates_private_library_dictionary() {
        let valid = account_hex_with_state(0x11, 1, state_with_library(false, true));
        let parsed = parse_imported_accounts(&[valid]).unwrap();
        assert!(parsed[0].libraries_boc.is_some());

        let wrong_hash = account_hex_with_state(0x11, 1, state_with_library(false, false));
        assert!(
            parse_imported_accounts(&[wrong_hash])
                .unwrap_err()
                .to_string()
                .contains("does not match its root hash")
        );

        let public = account_hex_with_state(0x11, 1, state_with_library(true, true));
        assert!(
            parse_imported_accounts(&[public])
                .unwrap_err()
                .to_string()
                .contains("publishes a public library")
        );
    }

    #[test]
    fn rejects_empty_and_trailing_library_dictionary_cells() {
        let empty_root = StateInit {
            code: Some(cell_with_u32(0x1234_5678)),
            libraries: Dict::from_raw(Some(CellBuilder::new().build().unwrap())),
            ..Default::default()
        };
        assert!(
            parse_imported_accounts(&[account_hex_with_state(0x11, 1, empty_root)])
                .unwrap_err()
                .to_string()
                .contains("explicitly empty libraries root")
        );

        let valid = state_with_library(false, true);
        let mut builder = CellBuilder::new();
        builder
            .store_slice(valid.libraries.root().as_ref().unwrap().as_slice().unwrap())
            .unwrap();
        builder.store_bit(true).unwrap();
        let trailing_root = StateInit {
            code: valid.code,
            libraries: Dict::from_raw(Some(builder.build().unwrap())),
            ..Default::default()
        };
        assert!(
            parse_imported_accounts(&[account_hex_with_state(0x11, 1, trailing_root)])
                .unwrap_err()
                .to_string()
                .contains("trailing data in a library value")
        );
    }
}
