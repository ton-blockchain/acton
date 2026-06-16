use anyhow::Context;
use ton_emulator::{
    AccountsState, LocalAccountsState, WorldState, WorldStateAccountSnapshot, WorldStateSnapshot,
};
use tycho_types::cell::Lazy;
use tycho_types::cell::{Cell, CellBuilder};
use tycho_types::models::{
    Account, AccountState, CurrencyCollection, IntAddr, OptionalAccount, ShardAccount, StateInit,
    StdAddr, StorageInfo,
};
use tycho_types::prelude::HashBytes;

fn new_world_state() -> anyhow::Result<WorldState> {
    WorldState::new(AccountsState::Local(LocalAccountsState::new()), None)
}

fn body_with_u32(value: u32) -> anyhow::Result<Cell> {
    let mut builder = CellBuilder::new();
    builder.store_u32(value)?;
    Ok(builder.build()?)
}

const fn std_addr(workchain: i8, byte: u8) -> StdAddr {
    StdAddr::new(workchain, HashBytes([byte; 32]))
}

fn shard_account(
    address: StdAddr,
    balance: u128,
    code: Option<Cell>,
    data: Option<Cell>,
) -> anyhow::Result<ShardAccount> {
    Ok(ShardAccount {
        account: Lazy::new(&OptionalAccount(Some(Account {
            address: IntAddr::Std(address),
            balance: CurrencyCollection::new(balance),
            last_trans_lt: 4242,
            storage_stat: StorageInfo::default(),
            state: match (code, data) {
                (Some(code), data) => AccountState::Active(StateInit {
                    code: Some(code),
                    data,
                    ..Default::default()
                }),
                (None, _) => AccountState::Uninit,
            },
        })))?,
        last_trans_hash: HashBytes([0x99; 32]),
        last_trans_lt: 7_000_000,
    })
}

#[test]
fn public_world_state_snapshot_round_trip_preserves_state() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    state.set_now(1_700_000_123);
    state.set_random_seed(Some([0x42; 32]));
    state.get_lt();
    state.get_lt();
    state.register_lib(body_with_u32(0xcafe_babe)?);
    state.register_lib(body_with_u32(0xface_feed)?);

    let first_addr = std_addr(0, 0x11);
    let second_addr = std_addr(-1, 0x22);

    let first_account = shard_account(
        first_addr.clone(),
        10_000,
        Some(body_with_u32(0x1234_5678)?),
        Some(body_with_u32(0xfeed_beef)?),
    )?;
    let second_account = shard_account(second_addr.clone(), 20_000, None, None)?;

    state.update_account(&first_addr, &first_account);
    state.update_account(&second_addr, &second_account);

    let snapshot = state.snapshot()?;
    assert_eq!(snapshot.random_seed, Some(hex::encode([0x42; 32])));
    let json = serde_json::to_string_pretty(&snapshot)?;
    let decoded: WorldStateSnapshot = serde_json::from_str(&json)?;

    let restored = WorldState::from_snapshot(decoded)?;

    assert!(matches!(restored.state(), AccountsState::Local(_)));
    assert_eq!(restored.snapshot()?, snapshot);

    Ok(())
}

#[test]
fn public_world_state_load_snapshot_replaces_existing_state() -> anyhow::Result<()> {
    let mut source = new_world_state()?;
    source.set_now(88);
    source.get_lt();

    let source_addr = std_addr(0, 0x41);
    let source_account = shard_account(
        source_addr.clone(),
        900,
        Some(body_with_u32(0xabc_def0)?),
        Some(body_with_u32(0x1111_2222)?),
    )?;
    source.update_account(&source_addr, &source_account);
    source.register_lib(body_with_u32(0x3333_4444)?);

    let snapshot = source.snapshot()?;

    let mut target = new_world_state()?;
    let stale_addr = std_addr(0, 0x77);
    let stale_account = shard_account(
        stale_addr.clone(),
        123,
        Some(body_with_u32(1)?),
        Some(body_with_u32(2)?),
    )?;
    target.update_account(&stale_addr, &stale_account);
    target.register_lib(body_with_u32(3)?);
    target.set_now(999);
    target.get_lt();
    target.get_lt();

    target.load_snapshot(snapshot.clone())?;

    assert!(matches!(target.state(), AccountsState::Local(_)));
    assert_eq!(target.snapshot()?, snapshot);
    assert_eq!(target.get_accounts().len(), 1);
    assert!(
        !target.get_accounts().contains_key(&stale_addr),
        "stale accounts should be replaced by snapshot contents"
    );

    Ok(())
}

#[test]
fn public_world_state_from_snapshot_rejects_unknown_version() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    let addr = std_addr(0, 0x5a);
    let account = shard_account(addr.clone(), 77, Some(body_with_u32(9)?), None)?;
    state.update_account(&addr, &account);

    let mut snapshot = state.snapshot()?;
    snapshot.version = 999;

    let error = WorldState::from_snapshot(snapshot)
        .err()
        .expect("snapshot with unknown version should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Unsupported world state snapshot version"),
        "unexpected error: {message}"
    );

    Ok(())
}

#[test]
fn public_world_state_snapshot_contains_parseable_account_bocs() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    let addr = std_addr(0, 0x2b);
    let account = shard_account(
        addr.clone(),
        555,
        Some(body_with_u32(0x4444_5555)?),
        Some(body_with_u32(0x6666_7777)?),
    )?;
    state.update_account(&addr, &account);

    let snapshot = state.snapshot()?;
    let entry = snapshot
        .accounts
        .iter()
        .find(|entry| entry.address == addr.display_base64_url(false).to_string())
        .context("snapshot should contain the inserted account")?;
    let parsed = tycho_types::boc::Boc::decode_base64(&entry.shard_account_boc64)?
        .parse::<ShardAccount>()?;

    let original = account
        .account
        .load()
        .context("failed to load original account")?;
    let restored = parsed
        .account
        .load()
        .context("failed to load parsed account")?;

    assert_eq!(parsed.last_trans_lt, account.last_trans_lt);
    assert_eq!(parsed.last_trans_hash, account.last_trans_hash);
    assert_eq!(original.0, restored.0);

    Ok(())
}

#[test]
fn public_world_state_from_snapshot_rejects_duplicate_account_addresses() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    let addr = std_addr(0, 0x61);
    let account = shard_account(addr.clone(), 700, Some(body_with_u32(0x1010_2020)?), None)?;
    state.update_account(&addr, &account);

    let mut snapshot = state.snapshot()?;
    let duplicate = snapshot
        .accounts
        .first()
        .cloned()
        .context("snapshot should contain the inserted account")?;
    snapshot.accounts.push(duplicate);

    let error = WorldState::from_snapshot(snapshot)
        .err()
        .expect("snapshot with duplicate accounts should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Duplicate account address in snapshot"),
        "unexpected error: {message}"
    );

    Ok(())
}

#[test]
fn public_world_state_from_snapshot_rejects_invalid_account_address() -> anyhow::Result<()> {
    let snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 0,
        current_now: 0,
        random_seed: None,
        ignore_chksig: false,
        config_boc64: ton_executor::DEFAULT_CONFIG.to_owned(),
        libraries_boc64: vec![],
        accounts: vec![WorldStateAccountSnapshot {
            address: "not-an-address".to_owned(),
            shard_account_boc64: body_with_u32(0x9999_8888)
                .map(tycho_types::boc::Boc::encode_base64)?,
        }],
    };

    let error = WorldState::from_snapshot(snapshot)
        .err()
        .expect("snapshot with invalid address should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Invalid account address in snapshot"),
        "unexpected error: {message}"
    );

    Ok(())
}

#[test]
fn public_world_state_from_snapshot_rejects_invalid_config_boc() {
    let snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 0,
        current_now: 0,
        random_seed: None,
        ignore_chksig: false,
        config_boc64: "not-base64".to_owned(),
        libraries_boc64: vec![],
        accounts: vec![],
    };

    let error = WorldState::from_snapshot(snapshot)
        .err()
        .expect("snapshot with invalid config should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Corrupted blockchain config for world state"),
        "unexpected error: {message}"
    );
}

#[test]
fn public_world_state_from_snapshot_rejects_invalid_library_boc() {
    let snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 0,
        current_now: 0,
        random_seed: None,
        ignore_chksig: false,
        config_boc64: ton_executor::DEFAULT_CONFIG.to_owned(),
        libraries_boc64: vec!["not-base64".to_owned()],
        accounts: vec![],
    };

    assert!(
        WorldState::from_snapshot(snapshot).is_err(),
        "snapshot with invalid library should be rejected"
    );
}

#[test]
fn public_world_state_from_snapshot_rejects_invalid_random_seed_hex() {
    let snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 0,
        current_now: 0,
        random_seed: Some("not-hex".to_owned()),
        ignore_chksig: false,
        config_boc64: ton_executor::DEFAULT_CONFIG.to_owned(),
        libraries_boc64: vec![],
        accounts: vec![],
    };

    let error = WorldState::from_snapshot(snapshot)
        .err()
        .expect("snapshot with invalid random seed should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Invalid random seed in snapshot"),
        "unexpected error: {message}"
    );
}

#[test]
fn public_world_state_from_snapshot_rejects_invalid_random_seed_length() {
    let snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 0,
        current_now: 0,
        random_seed: Some("42".to_owned()),
        ignore_chksig: false,
        config_boc64: ton_executor::DEFAULT_CONFIG.to_owned(),
        libraries_boc64: vec![],
        accounts: vec![],
    };

    let error = WorldState::from_snapshot(snapshot)
        .err()
        .expect("snapshot with short random seed should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("expected 32 bytes, got 1"),
        "unexpected error: {message}"
    );
}

#[test]
fn public_world_state_load_snapshot_failure_keeps_existing_state() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    state.set_now(321);
    state.get_lt();
    let addr = std_addr(0, 0x73);
    let account = shard_account(
        addr.clone(),
        1_234,
        Some(body_with_u32(0x2222_1111)?),
        Some(body_with_u32(0x5555_4444)?),
    )?;
    state.update_account(&addr, &account);
    state.register_lib(body_with_u32(0xaaaa_bbbb)?);

    let before = state.snapshot()?;
    let invalid_snapshot = WorldStateSnapshot {
        version: 1,
        current_lt: 9,
        current_now: 9,
        random_seed: None,
        ignore_chksig: false,
        config_boc64: ton_executor::DEFAULT_CONFIG.to_owned(),
        libraries_boc64: vec![],
        accounts: vec![WorldStateAccountSnapshot {
            address: "not-an-address".to_owned(),
            shard_account_boc64: body_with_u32(1).map(tycho_types::boc::Boc::encode_base64)?,
        }],
    };

    let error = state
        .load_snapshot(invalid_snapshot)
        .expect_err("invalid snapshot load should fail");
    let message = error.to_string();
    assert!(
        message.contains("Invalid account address in snapshot"),
        "unexpected error: {message}"
    );
    assert_eq!(state.snapshot()?, before);

    Ok(())
}

#[test]
fn public_world_state_snapshot_skips_cached_non_existing_accounts() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    let addr = std_addr(0, 0x81);

    let cached = state.get_account(&addr);
    let loaded = cached
        .account
        .load()
        .context("failed to load cached account")?;
    assert!(
        loaded.0.is_none(),
        "fresh cached account should be non-existing"
    );

    let snapshot = state.snapshot()?;
    assert!(
        snapshot.accounts.is_empty(),
        "snapshot should not serialize cached non-existing accounts"
    );
    assert!(
        !state.check_deployed(&addr),
        "cached non-existing account should not flip deployed status"
    );

    Ok(())
}

#[test]
fn public_world_state_snapshot_rejects_rootless_config_without_panicking() -> anyhow::Result<()> {
    let mut state = new_world_state()?;
    state.set_config(tycho_types::dict::Dict::new());

    let error = state
        .snapshot()
        .expect_err("rootless config should fail snapshot");
    let message = error.to_string();
    assert!(
        message.contains("Config has no root"),
        "unexpected error: {message}"
    );

    Ok(())
}
