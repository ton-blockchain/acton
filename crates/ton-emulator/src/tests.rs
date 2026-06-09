#![cfg(test)]
use crate::emulator::Emulator;
use crate::world_state::{RemoteLibraryCache, RemoteSnapshotCache};
use crate::{
    AccountsState, LocalAccountsState, RemoteAccountState, WorldState, WorldStateSnapshot,
};
use anyhow::Context;
use std::sync::Arc;
use ton_networks::Network;
use tycho_types::boc::Boc;
use tycho_types::cell::Lazy;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, Store};
use tycho_types::dict::Dict;
use tycho_types::models::LibDescr;
use tycho_types::models::config::{BlockchainConfigParams, MsgForwardPrices};
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, OwnedRelaxedMessage,
    RelaxedIntMsgInfo, RelaxedMessage, RelaxedMsgInfo, ShardAccount, StateInit, StdAddr,
    StorageInfo,
};
use tycho_types::num::Tokens;
use tycho_types::prelude::HashBytes;

fn new_world_state() -> anyhow::Result<WorldState> {
    WorldState::new(AccountsState::Local(LocalAccountsState::new()), None)
}

fn to_cell<T: Store + ?Sized>(obj: &T) -> anyhow::Result<Cell> {
    let mut builder = CellBuilder::new();
    obj.store_into(&mut builder, Cell::empty_context())?;
    Ok(builder.build()?)
}

fn int_addr(workchain: i8, byte: u8) -> IntAddr {
    IntAddr::from((workchain, HashBytes([byte; 32])))
}

fn body_with_u32(value: u32) -> anyhow::Result<Cell> {
    let mut builder = CellBuilder::new();
    builder.store_u32(value)?;
    Ok(builder.build()?)
}

fn std_addr(workchain: i8, byte: u8) -> StdAddr {
    StdAddr::new(workchain, HashBytes([byte; 32]))
}

fn shard_account(
    address: StdAddr,
    balance: u128,
    code: Option<Cell>,
) -> anyhow::Result<ShardAccount> {
    Ok(ShardAccount {
        account: Lazy::new(&OptionalAccount(Some(Account {
            address: IntAddr::Std(address),
            balance: CurrencyCollection::new(balance),
            last_trans_lt: 777,
            storage_stat: StorageInfo::default(),
            state: match code {
                Some(code) => AccountState::Active(StateInit {
                    code: Some(code),
                    data: Some(body_with_u32(0xfeed_beef)?),
                    ..Default::default()
                }),
                None => AccountState::Uninit,
            },
        })))?,
        last_trans_hash: HashBytes([0x42; 32]),
        last_trans_lt: 1_234_567,
    })
}

fn decode_libs_arg(libs_boc64: &str) -> anyhow::Result<Dict<HashBytes, LibDescr>> {
    let libs_root = Boc::decode_base64(libs_boc64)?;
    let mut libs_slice = libs_root.as_slice_allow_exotic();
    Ok(Dict::<HashBytes, LibDescr>::load_from_root_ext(
        &mut libs_slice,
        Cell::empty_context(),
    )?)
}

#[test]
fn remote_account_retrieve_error_uses_current_lt_marker() -> anyhow::Result<()> {
    let remote = RemoteAccountState::new(
        Network::Custom(Arc::from("unit-missing-remote-network")),
        None,
        RemoteSnapshotCache::default(),
        RemoteLibraryCache::default(),
        false,
    );
    let mut state = WorldState::new(AccountsState::Remote(remote), None)?;
    let address = std_addr(0, 0xaa);

    let current_lt = state.get_lt();
    let account = state.get_account(&address);

    assert!(account.account.load()?.0.is_none());
    assert_eq!(account.last_trans_hash, HashBytes::ZERO);
    assert_eq!(account.last_trans_lt, current_lt);

    Ok(())
}

fn make_internal_relaxed_message(
    src: Option<IntAddr>,
    dst: IntAddr,
    body: Cell,
) -> OwnedRelaxedMessage {
    OwnedRelaxedMessage {
        info: RelaxedMsgInfo::Int(RelaxedIntMsgInfo {
            src,
            dst,
            value: CurrencyCollection::ZERO,
            ..Default::default()
        }),
        init: None,
        body: body.into(),
        layout: None,
    }
}

fn expected_in_msg_fwd_fee(
    message: &RelaxedMessage<'_>,
    prices: &MsgForwardPrices,
) -> anyhow::Result<Tokens> {
    let message_cell = to_cell(message)?;
    let root_bits = u64::from(message_cell.bit_len());
    let mut stats = message_cell
        .as_slice()
        .context("Failed to parse message cell in test")?
        .compute_unique_stats(usize::MAX)
        .context("Failed to compute message stats in test")?;
    stats.bit_count = stats.bit_count.saturating_sub(root_bits);

    let total = prices.compute_fwd_fee(stats);
    Ok(total.saturating_sub(prices.get_first_part(total)))
}

#[test]
fn test_get_config() -> anyhow::Result<()> {
    let state = new_world_state()?;

    let config = state.get_config();
    let version = config.get(8).expect("No version").expect("Has value");
    assert!(
        version
            .as_slice()
            .expect("Version cell corrupted")
            .load_u32()?
            >= 12
    );

    let root = config.root().clone().expect("Config has no root");
    assert!(!root.repr_hash().is_zero());

    Ok(())
}

#[test]
fn compute_in_msg_fwd_fee_matches_forward_prices_formula() -> anyhow::Result<()> {
    let state = new_world_state()?;
    let message = make_internal_relaxed_message(
        Some(int_addr(0, 0x11)),
        int_addr(0, 0x22),
        body_with_u32(0xdead_beef)?,
    );
    let message_cell = to_cell(&message)?;
    let parsed = message_cell.parse::<RelaxedMessage<'_>>()?;

    let actual = Emulator::compute_in_msg_fwd_fee(state.get_config(), &parsed, false)?;

    let config_root = state
        .get_config()
        .root()
        .clone()
        .context("Config must have root")?;
    let prices = BlockchainConfigParams::from_raw(config_root).get_msg_forward_prices(false)?;
    let expected = expected_in_msg_fwd_fee(&parsed, &prices)?;

    assert_eq!(actual, expected);
    Ok(())
}

#[test]
fn compute_in_msg_fwd_fee_excludes_root_bits() -> anyhow::Result<()> {
    let state = new_world_state()?;

    let msg_small = make_internal_relaxed_message(
        Some(int_addr(0, 0x01)),
        int_addr(0, 0x02),
        Cell::empty_cell(),
    );
    let msg_large = make_internal_relaxed_message(
        Some(int_addr(0, 0x01)),
        int_addr(0, 0x02),
        body_with_u32(0x1234_5678)?,
    );

    let msg_small_cell = to_cell(&msg_small)?;
    let msg_large_cell = to_cell(&msg_large)?;
    assert_ne!(msg_small_cell.bit_len(), msg_large_cell.bit_len());

    let small = msg_small_cell.parse::<RelaxedMessage<'_>>()?;
    let large = msg_large_cell.parse::<RelaxedMessage<'_>>()?;
    let small_fee = Emulator::compute_in_msg_fwd_fee(state.get_config(), &small, false)?;
    let large_fee = Emulator::compute_in_msg_fwd_fee(state.get_config(), &large, false)?;

    assert_eq!(small_fee, large_fee);
    Ok(())
}

#[test]
fn compute_in_msg_fwd_fee_uses_workchain_specific_prices() -> anyhow::Result<()> {
    let state = new_world_state()?;
    let message = make_internal_relaxed_message(
        Some(int_addr(0, 0xaa)),
        int_addr(-1, 0xbb),
        body_with_u32(0xabcd_ef01)?,
    );
    let message_cell = to_cell(&message)?;
    let parsed = message_cell.parse::<RelaxedMessage<'_>>()?;

    let sc_fee = Emulator::compute_in_msg_fwd_fee(state.get_config(), &parsed, false)?;
    let mc_fee = Emulator::compute_in_msg_fwd_fee(state.get_config(), &parsed, true)?;

    let config_root = state
        .get_config()
        .root()
        .clone()
        .context("Config must have root")?;
    let config = BlockchainConfigParams::from_raw(config_root);

    let sc_prices = config.get_msg_forward_prices(false)?;
    let mc_prices = config.get_msg_forward_prices(true)?;
    assert_eq!(sc_fee, expected_in_msg_fwd_fee(&parsed, &sc_prices)?);
    assert_eq!(mc_fee, expected_in_msg_fwd_fee(&parsed, &mc_prices)?);

    if sc_prices != mc_prices {
        assert_ne!(sc_fee, mc_fee);
    }

    Ok(())
}

#[test]
fn prepare_send_transaction_preserves_valid_remote_previous_lts() -> anyhow::Result<()> {
    let account_addr = std_addr(0, 0x45);
    let mut account = shard_account(
        account_addr.clone(),
        123_456_789,
        Some(body_with_u32(0x1234_5678)?),
    )?;
    let mut optional_account = account.account.load()?;
    let large_lt = 74_118_931_000_008;
    optional_account
        .0
        .as_mut()
        .expect("test account must exist")
        .last_trans_lt = large_lt + 1;
    account.account = Lazy::new(&optional_account)?;
    let last_trans_hash = HashBytes([0x99; 32]);
    account.last_trans_hash = last_trans_hash;
    account.last_trans_lt = large_lt;

    let mut state = new_world_state()?;
    state.update_account(&account_addr, &account);
    let message = make_internal_relaxed_message(
        Some(int_addr(0, 0x11)),
        IntAddr::Std(account_addr),
        body_with_u32(0xabcd_ef01)?,
    );
    let libs = Default::default();

    let prepared = Emulator::prepare_send_transaction(&mut state, to_cell(&message)?, &libs, None)?;
    let executor_account =
        Boc::decode_base64(&prepared.run_args.shard_account)?.parse::<ShardAccount>()?;
    let account = executor_account
        .account
        .load()?
        .0
        .expect("executor account must exist");

    assert_eq!(prepared.run_args.lt, large_lt + 1_000_000);
    assert_eq!(account.last_trans_lt, large_lt + 1);
    assert_eq!(executor_account.last_trans_lt, large_lt);
    assert_eq!(executor_account.last_trans_hash, last_trans_hash);
    Ok(())
}

#[test]
fn prepare_send_transaction_merges_fresh_world_state_libraries() -> anyhow::Result<()> {
    let account_addr = std_addr(0, 0x46);
    let account = shard_account(
        account_addr.clone(),
        123_456_789,
        Some(body_with_u32(0x1234_5678)?),
    )?;

    let mut state = new_world_state()?;
    state.update_account(&account_addr, &account);

    let stale_libs = Dict::<HashBytes, LibDescr>::new();
    let library = body_with_u32(0xcafe_babe)?;
    let library_hash = *library.repr_hash();
    state.register_lib(library.clone());

    let message = make_internal_relaxed_message(
        Some(int_addr(0, 0x11)),
        IntAddr::Std(account_addr),
        body_with_u32(0xabcd_ef01)?,
    );

    let prepared =
        Emulator::prepare_send_transaction(&mut state, to_cell(&message)?, &stale_libs, None)?;
    let libs_boc64 = prepared
        .run_args
        .libs
        .context("prepared transaction must include world state libraries")?;
    let libs = decode_libs_arg(&libs_boc64)?;

    let loaded = libs
        .get(library_hash)?
        .context("prepared transaction must include registered library")?;
    assert_eq!(loaded.lib, library);
    Ok(())
}

#[test]
fn prepare_send_transaction_uses_preloaded_remote_account_library() -> anyhow::Result<()> {
    let account_addr = std_addr(0, 0x47);
    let library = body_with_u32(0xd00d_f00d)?;
    let library_hash = *library.repr_hash();
    let code_ref = CellBuilder::build_library(&library_hash);
    let account = shard_account(account_addr.clone(), 123_456_789, Some(code_ref))?;

    let library_cache = RemoteLibraryCache::new();
    library_cache.insert(library_hash, library.clone());
    let mut remote = RemoteAccountState::new(
        Network::Custom(Arc::from("unit-cached-library-network")),
        None,
        RemoteSnapshotCache::default(),
        library_cache,
        false,
    );
    remote.accounts.insert(account_addr.clone(), account);
    let mut state = WorldState::new(AccountsState::Remote(remote), None)?;

    let stale_libs = Dict::<HashBytes, LibDescr>::new();
    let message = make_internal_relaxed_message(
        Some(int_addr(0, 0x11)),
        IntAddr::Std(account_addr),
        body_with_u32(0xabcd_ef01)?,
    );

    assert!(state.libs().is_empty());
    let prepared =
        Emulator::prepare_send_transaction(&mut state, to_cell(&message)?, &stale_libs, None)?;
    assert_eq!(state.find_lib_by_hash(&library_hash), Some(library.clone()));

    let libs_boc64 = prepared
        .run_args
        .libs
        .context("prepared transaction must include preloaded remote libraries")?;
    let libs = decode_libs_arg(&libs_boc64)?;
    let loaded = libs
        .get(library_hash)?
        .context("prepared transaction must include preloaded remote library")?;
    assert_eq!(loaded.lib, library);
    Ok(())
}

#[test]
fn world_state_snapshot_round_trip_preserves_state() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    state.set_now(1_717_171_717);
    let lt = state.get_lt();
    assert_eq!(lt, 1_000_000);

    let library = body_with_u32(0xcafe_babe)?;
    state.register_lib(library);

    let account_addr = std_addr(0, 0x55);
    let code = body_with_u32(0x1234_5678)?;
    let account = shard_account(account_addr.clone(), 123_456_789, Some(code))?;
    state.update_account(&account_addr, &account);

    let snapshot = state.snapshot()?;
    let json = serde_json::to_string(&snapshot)?;
    let decoded_snapshot: WorldStateSnapshot = serde_json::from_str(&json)?;

    let restored = WorldState::from_snapshot(decoded_snapshot)?;
    let restored_snapshot = restored.snapshot()?;

    assert_eq!(restored_snapshot, snapshot);
    Ok(())
}

#[test]
fn world_state_find_lib_by_hash_returns_registered_library() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    let first = body_with_u32(0xcafe_babe)?;
    let second = body_with_u32(0xface_feed)?;
    let second_hash = *second.repr_hash();

    state.register_lib(first);
    state.register_lib(second.clone());
    state.register_lib(second.clone());

    assert_eq!(state.find_lib_by_hash(&second_hash), Some(second));
    assert!(state.find_lib_by_hash(&HashBytes([0xff; 32])).is_none());
    assert_eq!(state.libs().len(), 2);

    Ok(())
}

#[test]
fn world_state_load_snapshot_replaces_existing_state() -> anyhow::Result<()> {
    let mut source = new_world_state()?;
    source.set_now(77);
    source.get_lt();

    let source_addr = std_addr(0, 0x21);
    let source_account = shard_account(source_addr.clone(), 900, None)?;
    source.update_account(&source_addr, &source_account);
    let snapshot = source.snapshot()?;

    let mut target = new_world_state()?;
    let target_addr = std_addr(0, 0x99);
    let target_account = shard_account(target_addr.clone(), 100, Some(body_with_u32(1)?))?;
    target.update_account(&target_addr, &target_account);

    target.load_snapshot(snapshot.clone())?;

    assert_eq!(target.snapshot()?, snapshot);
    Ok(())
}
