use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use base64::{Engine, engine::general_purpose::STANDARD};
use expect_test::expect;
use rocksdb::DB;
use rston::boc::Boc;
use rston::cell::{Cell, CellBuilder, HashBytes, Lazy, LazyExotic};
use rston::merkle::MerkleUpdate;
use rston::models::{
    Account, AccountState, Block, BlockExtra, BlockId, BlockInfo, BlockRef, BlockchainConfig,
    CurrencyCollection, DepthBalanceInfo, IntAddr, McStateExtra, OptionalAccount, PrevBlockRef,
    ShardAccount, ShardAccounts, ShardDescription, ShardHashes, ShardIdent, ShardStateSplit,
    ShardStateUnsplit, StdAddr, ValidatorInfo, ValueFlow,
};
use sha2::{Digest, Sha256};
use ton_node_db::{NodeDb, StateStore};

#[test]
fn account_snapshots_report_their_shard_time() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let snapshot = directory.path().join("snapshot");
    let address = StdAddr::new(0, HashBytes([7; 32]));
    let mut shard = shard_state(&address, 0, 100)?.parse::<ShardStateUnsplit>()?;
    shard.gen_utime = 1_700_000_000;
    let shard = CellBuilder::build_from(shard)?;
    let shard_id = state_id(&shard)?;
    let mut master = master_state(0, &[shard_id])?.parse::<ShardStateUnsplit>()?;
    master.gen_utime = 1_700_000_001;
    let master = CellBuilder::build_from(master)?;
    let master_id = state_id(&master)?;
    write_snapshot(&snapshot, &[(master_id, master), (shard_id, shard)])?;

    let store = StateStore::open(&snapshot, &directory.path().join("updates"), 1000)?;
    let database = NodeDb::open(&snapshot)?;
    let mut rows = Vec::new();
    for address in [
        address,
        StdAddr::new(-1, HashBytes([7; 32])),
        StdAddr::new(0, HashBytes([8; 32])),
    ] {
        let persisted = store.get_account(&address)?;
        let original = database.get_account(&master_id, &address, 1000)?;
        rows.push((
            address.workchain,
            persisted.account.is_some(),
            persisted.gen_utime,
            original.gen_utime,
        ));
    }

    expect![[r"
        [
            (0, true, 1700000000, 1700000000),
            (-1, false, 1700000001, 1700000001),
            (0, false, 1700000000, 1700000000),
        ]
    "]]
    .assert_eq(&format!(
        "[\n{}]\n",
        rows.iter()
            .map(|row| format!("    {row:?},\n"))
            .collect::<String>()
    ));

    Ok(())
}

#[test]
fn commits_complete_batches_and_resumes_account_state() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let snapshot = directory.path().join("snapshot");
    let updates = directory.path().join("updates");
    let address = StdAddr::new(0, HashBytes([7; 32]));
    let shard0 = shard_state(&address, 0, 100)?;
    let shard_id0 = state_id(&shard0)?;
    let master0 = master_state(0, &[shard_id0])?;
    let master_id0 = state_id(&master0)?;
    write_snapshot(
        &snapshot,
        &[(master_id0, master0.clone()), (shard_id0, shard0.clone())],
    )?;

    let shard1 = shard_state(&address, 1, 200)?;
    let (shard_id1, shard_boc1) = block(&shard_id0, &shard0, &shard1)?;
    let master1 = master_state(1, &[shard_id1])?;
    let (master_id1, master_boc1) = block(&master_id0, &master0, &master1)?;
    let mut store = StateStore::open(&snapshot, &updates, 1000)?;
    let before = balance(&store, &address)?;
    let pinned = store.masterchain_state()?;
    let missing = store.apply_batch((master_id1, &master_boc1), []).is_err();
    let unchanged = store.head() == master_id0;
    let master_root1 = Boc::decode(&master_boc1)?;
    let shard_root1 = Boc::decode(&shard_boc1)?;
    let wrong_root = store
        .apply_roots((master_id1, &shard_root1), [(shard_id1, &shard_root1)])
        .is_err();
    let missing_root = store.apply_roots((master_id1, &master_root1), []).is_err();
    let roots_atomic = store.head() == master_id0;

    store.apply_batch(
        (master_id1, &master_boc1),
        [(shard_id1, shard_boc1.as_slice())],
    )?;
    let after = balance(&store, &address)?;
    let pinned_unchanged = pinned.block_id() == master_id0;
    drop(pinned);
    drop(store);

    let mut store = StateStore::open(&snapshot, &updates, 1000)?;
    let resumed = store.head() == master_id1;
    let resumed_balance = balance(&store, &address)?;
    store.apply_batch((master_id1, &master_boc1), [])?;
    let replay_noop = store.head() == master_id1;

    let shard2 = shard_state(&address, 2, 300)?;
    let (shard_id2, shard_boc2) = block(&shard_id1, &shard1, &shard2)?;
    let master2 = master_state(2, &[shard_id2])?;
    let (master_id2, master_boc2) = block(&master_id1, &master1, &master2)?;
    let invalid = store
        .apply_batch((master_id2, &master_boc2), [(shard_id2, &b"broken"[..])])
        .is_err();
    drop(store);

    let mut store = StateStore::open(&snapshot, &updates, 1000)?;
    let failed_batch_not_committed = store.head() == master_id1;
    store.apply_roots(
        (master_id2, &Boc::decode(&master_boc2)?),
        [(shard_id2, &Boc::decode(&shard_boc2)?)],
    )?;
    store.apply_roots((master_id2, &Boc::decode(&master_boc2)?), [])?;
    let final_balance = balance(&store, &address)?;
    drop(store);
    let store = StateStore::open(&snapshot, &updates, 1000)?;
    let final_resume = store.head() == master_id2;
    let original = StateStore::open(&snapshot, &directory.path().join("fresh"), 1000)?;

    expect![[r"
        balances: 100 -> 200 -> 200 -> 300
        missing shard rejected: true, checkpoint unchanged: true
        wrong root rejected: true, missing root rejected: true, checkpoint unchanged: true
        pinned view unchanged: true, resumed: true, replay: true
        invalid shard rejected: true, restart unchanged: true
        final resume: true, snapshot balance: 100
    "]]
    .assert_eq(&format!(
        "balances: {before} -> {after} -> {resumed_balance} -> {final_balance}\n\
         missing shard rejected: {missing}, checkpoint unchanged: {unchanged}\n\
         wrong root rejected: {wrong_root}, missing root rejected: {missing_root}, checkpoint unchanged: {roots_atomic}\n\
         pinned view unchanged: {pinned_unchanged}, resumed: {resumed}, replay: {replay_noop}\n\
         invalid shard rejected: {invalid}, restart unchanged: {failed_batch_not_committed}\n\
         final resume: {final_resume}, snapshot balance: {}\n",
        balance(&original, &address)?,
    ));

    let other_snapshot = directory.path().join("other-snapshot");
    write_snapshot(
        &other_snapshot,
        &[(master_id1, master1), (shard_id1, shard1)],
    )?;
    drop(store);
    expect![["state updates belong to a different snapshot"]].assert_eq(
        &StateStore::open(&other_snapshot, &updates, 1000)
            .err()
            .context("accepted a different snapshot")?
            .to_string(),
    );

    Ok(())
}

#[test]
fn snapshots_read_a_fixed_frontier_while_the_writer_advances() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let snapshot_path = directory.path().join("snapshot");
    let updates = directory.path().join("updates");
    let address = StdAddr::new(0, HashBytes([7; 32]));
    let shard0 = shard_state(&address, 0, 100)?;
    let shard_id0 = state_id(&shard0)?;
    let master0 = master_state(0, &[shard_id0])?;
    let master_id0 = state_id(&master0)?;
    write_snapshot(
        &snapshot_path,
        &[(master_id0, master0.clone()), (shard_id0, shard0.clone())],
    )?;

    let shard1 = shard_state(&address, 1, 200)?;
    let (shard_id1, shard_boc1) = block(&shard_id0, &shard0, &shard1)?;
    let master1 = master_state(1, &[shard_id1])?;
    let (master_id1, master_boc1) = block(&master_id0, &master0, &master1)?;
    let mut store = StateStore::open(&snapshot_path, &updates, 1000)?;
    let initial = store.snapshot();
    store.apply_batch(
        (master_id1, &master_boc1),
        [(shard_id1, shard_boc1.as_slice())],
    )?;
    let committed = store.snapshot();

    let shard2 = shard_state(&address, 2, 300)?;
    let (shard_id2, shard_boc2) = block(&shard_id1, &shard1, &shard2)?;
    let master2 = master_state(2, &[shard_id2])?;
    let (master_id2, master_boc2) = block(&master_id1, &master1, &master2)?;
    let (entered, applying) = mpsc::channel();
    let (resume, paused) = mpsc::channel();

    // Stop inside application, after changing the working masterchain root but
    // before applying its shard. Readers must still see the complete old frontier.
    let writer = std::thread::spawn(move || {
        let shards = std::iter::once((shard_id2, shard_boc2.as_slice())).inspect(|_| {
            entered.send(()).expect("reader disconnected");
            paused
                .recv_timeout(Duration::from_secs(10))
                .expect("reader did not release the writer");
        });
        store.apply_batch((master_id2, &master_boc2), shards)?;
        anyhow::Ok(store)
    });
    applying.recv_timeout(Duration::from_secs(10))?;
    let during = committed.get_account(&address)?;
    resume.send(())?;
    let store = writer.join().expect("writer panicked")?;
    let latest = store.snapshot();
    drop(store);
    let _ = committed.get_account(&address)?;
    let warm = committed.get_account(&address)?;
    let cached = warm.reads.records > 0 && warm.reads.cache_hits == warm.reads.records;

    let mut rows = Vec::new();
    for state in [&initial, &committed, &latest] {
        let account = state.get_account(&address)?;
        let balance = account
            .account
            .context("missing account")?
            .load_account()?
            .context("empty account")?
            .balance
            .tokens
            .into_inner();
        rows.push((
            state.head().seqno,
            state.masterchain_state()?.block_id().seqno,
            account.masterchain_block.seqno,
            account.shard_block.seqno,
            balance,
        ));
    }

    expect![[r"
        read during application: masterchain 1, shard 1
        repeated query uses committed record cache: true
        after writer closed (head, masterchain state, account masterchain, shard, balance):
        [(0, 0, 0, 0, 100), (1, 1, 1, 1, 200), (2, 2, 2, 2, 300)]
    "]]
    .assert_eq(&format!(
        "read during application: masterchain {}, shard {}\n\
         repeated query uses committed record cache: {cached}\n\
         after writer closed (head, masterchain state, account masterchain, shard, balance):\n\
         {rows:?}\n",
        during.masterchain_block.seqno, during.shard_block.seqno,
    ));

    drop((initial, committed, latest));
    let reopened = StateStore::open(&snapshot_path, &updates, 1000)?;
    expect![["(2, 300)"]].assert_eq(&format!(
        "{:?}",
        (reopened.head().seqno, balance(&reopened, &address)?)
    ));

    Ok(())
}

#[test]
fn split_merge_and_restart_preserve_both_account_branches() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let snapshot = directory.path().join("snapshot");
    let updates = directory.path().join("updates");
    let left_address = StdAddr::new(0, HashBytes([7; 32]));
    let right_address = StdAddr::new(0, HashBytes([135; 32]));
    let (left_shard, right_shard) = ShardIdent::BASECHAIN
        .split()
        .context("cannot split basechain")?;
    let parent = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            ..Default::default()
        },
        &[(&left_address, 100), (&right_address, 200)],
    )?;
    let parent_id = state_id(&parent)?;
    let master0 = master_state(0, &[parent_id])?;
    let master_id0 = state_id(&master0)?;
    write_snapshot(
        &snapshot,
        &[(master_id0, master0.clone()), (parent_id, parent.clone())],
    )?;

    // Persist the before-split parent first. Both children must still be able
    // to use the same parent root after restarting the writer.
    let before_split = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            seqno: 1,
            before_split: true,
            ..Default::default()
        },
        &[(&left_address, 110), (&right_address, 210)],
    )?;
    let (split_id, split_boc) = block(&parent_id, &parent, &before_split)?;
    let master1 = master_state(1, &[split_id])?;
    let (master_id1, master_boc1) = block(&master_id0, &master0, &master1)?;
    let mut store = StateStore::open(&snapshot, &updates, 10_000)?;
    store.apply_batch(
        (master_id1, &master_boc1),
        [(split_id, split_boc.as_slice())],
    )?;
    drop(store);

    let left = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: left_shard,
            seqno: 2,
            ..Default::default()
        },
        &[(&left_address, 120)],
    )?;
    let right = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: right_shard,
            seqno: 2,
            ..Default::default()
        },
        &[(&right_address, 220)],
    )?;
    let (left_id, left_boc) = block(&split_id, &before_split, &left)?;
    let (right_id, right_boc) = block(&split_id, &before_split, &right)?;
    let left_next = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: left_shard,
            seqno: 3,
            ..Default::default()
        },
        &[(&left_address, 130)],
    )?;
    let (left_next_id, left_next_boc) = block(&left_id, &left, &left_next)?;
    let master2 = master_state(2, &[left_next_id, right_id])?;
    let (master_id2, master_boc2) = block(&master_id1, &master1, &master2)?;
    let mut store = StateStore::open(&snapshot, &updates, 10_000)?;
    let incomplete_split = store
        .apply_batch(
            (master_id2, &master_boc2),
            [(right_id, right_boc.as_slice())],
        )
        .is_err();
    let split_atomic = store.head() == master_id1 && balance(&store, &left_address)? == 110;
    store.apply_batch(
        (master_id2, &master_boc2),
        [
            (right_id, right_boc.as_slice()),
            (left_id, left_boc.as_slice()),
            (left_next_id, left_next_boc.as_slice()),
        ],
    )?;
    drop(store);

    let mut store = StateStore::open(&snapshot, &updates, 10_000)?;
    let split_resumed = store.head() == master_id2;
    let split_balances = (
        balance(&store, &left_address)?,
        balance(&store, &right_address)?,
    );
    let routed = store.get_account(&left_address)?.shard_block == left_next_id
        && store.get_account(&right_address)?.shard_block == right_id;

    // The children deliberately have different seqnos: a merge follows the
    // larger one, and its old root is the ordered pair, not either child alone.
    let pair = CellBuilder::build_from(ShardStateSplit {
        left: Lazy::from_raw(left_next.clone())?,
        right: Lazy::from_raw(right.clone())?,
    })?;
    let merged = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            seqno: 4,
            ..Default::default()
        },
        &[(&left_address, 140), (&right_address, 240)],
    )?;
    let previous = PrevBlockRef::AfterMerge {
        left: block_ref(&left_next_id),
        right: block_ref(&right_id),
    };
    let (merge_id, merge_boc) = transition(previous.clone(), &pair, &merged)?;
    let master3 = master_state(3, &[merge_id])?;
    let (master_id3, master_boc3) = block(&master_id2, &master2, &master3)?;
    let reversed = CellBuilder::build_from(ShardStateSplit {
        left: Lazy::from_raw(right)?,
        right: Lazy::from_raw(left_next)?,
    })?;
    let (bad_id, bad_boc) = transition(previous, &reversed, &merged)?;
    let reversed_rejected = store
        .apply_batch((master_id3, &master_boc3), [(bad_id, bad_boc.as_slice())])
        .is_err();

    let (missing_id, missing_boc) = transition(
        PrevBlockRef::AfterMerge {
            left: block_ref(&left_next_id),
            right: BlockRef {
                file_hash: HashBytes::ZERO,
                ..block_ref(&right_id)
            },
        },
        &pair,
        &merged,
    )?;
    let missing_child = store
        .apply_batch(
            (master_id3, &master_boc3),
            [(missing_id, missing_boc.as_slice())],
        )
        .is_err();
    drop(store);

    let mut store = StateStore::open(&snapshot, &updates, 10_000)?;
    let merge_atomic = store.head() == master_id2;
    let split_snapshot = store.snapshot();
    store.apply_batch(
        (master_id3, &master_boc3),
        [(merge_id, merge_boc.as_slice())],
    )?;
    drop(store);

    let pinned_shards = (
        split_snapshot.get_account(&left_address)?.shard_block == left_next_id,
        split_snapshot.get_account(&right_address)?.shard_block == right_id,
        split_snapshot.head() == master_id2,
    );
    drop(split_snapshot);

    let mut store = StateStore::open(&snapshot, &updates, 10_000)?;
    let merge_resumed = store.head() == master_id3;
    let merge_balances = (
        balance(&store, &left_address)?,
        balance(&store, &right_address)?,
    );
    let merge_routed = store.get_account(&left_address)?.shard_block == merge_id
        && store.get_account(&right_address)?.shard_block == merge_id;
    let next = state_with_accounts(
        ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            seqno: 5,
            ..Default::default()
        },
        &[(&left_address, 150), (&right_address, 250)],
    )?;
    let (next_id, next_boc) = block(&merge_id, &merged, &next)?;
    let master4 = master_state(4, &[next_id])?;
    let (master_id4, master_boc4) = block(&master_id3, &master3, &master4)?;
    store.apply_batch((master_id4, &master_boc4), [(next_id, next_boc.as_slice())])?;

    // A single masterchain step can include several shard transitions. Keep
    // intermediate roots available until their children have consumed them.
    let mut combined = StateStore::open(&snapshot, &directory.path().join("combined"), 10_000)?;
    let combined_master = master_state(1, &[merge_id])?;
    let (combined_id, combined_boc) = block(&master_id0, &master0, &combined_master)?;
    combined.apply_roots(
        (combined_id, &Boc::decode(&combined_boc)?),
        [
            (split_id, &Boc::decode(&split_boc)?),
            (left_id, &Boc::decode(&left_boc)?),
            (left_next_id, &Boc::decode(&left_next_boc)?),
            (right_id, &Boc::decode(&right_boc)?),
            (merge_id, &Boc::decode(&merge_boc)?),
        ],
    )?;

    expect![[r"
        incomplete split rejected: true, checkpoint intact: true
        split resumed: true, balances: (130, 220), routed to children: true
        reversed merge rejected: true, checkpoint intact after restart: true
        missing merge child rejected: true
        merge resumed: true, balances: (140, 240), routed to parent: true
        snapshot retained split frontier after merge: (true, true, true)
        continued after merge: (150, 250)
        split and merge in one batch: (140, 240)
    "]]
    .assert_eq(&format!(
        "incomplete split rejected: {incomplete_split}, checkpoint intact: {split_atomic}\n\
         split resumed: {split_resumed}, balances: {split_balances:?}, routed to children: {routed}\n\
         reversed merge rejected: {reversed_rejected}, checkpoint intact after restart: {merge_atomic}\n\
         missing merge child rejected: {missing_child}\n\
         merge resumed: {merge_resumed}, balances: {merge_balances:?}, routed to parent: {merge_routed}\n\
         snapshot retained split frontier after merge: {pinned_shards:?}\n\
         continued after merge: {:?}\n\
         split and merge in one batch: {:?}\n",
        (balance(&store, &left_address)?, balance(&store, &right_address)?),
        (balance(&combined, &left_address)?, balance(&combined, &right_address)?),
    ));

    Ok(())
}

fn balance(store: &StateStore, address: &StdAddr) -> Result<u128> {
    let account = store
        .get_account(address)?
        .account
        .context("missing account in shard state")?
        .load_account()?
        .context("account is empty")?;

    Ok(account.balance.tokens.into_inner())
}

fn shard_state(address: &StdAddr, seqno: u32, amount: u128) -> Result<Cell> {
    state_with_accounts(
        ShardStateUnsplit {
            shard_ident: ShardIdent::BASECHAIN,
            seqno,
            ..Default::default()
        },
        &[(address, amount)],
    )
}

fn state_with_accounts(
    mut state: ShardStateUnsplit,
    balances: &[(&StdAddr, u128)],
) -> Result<Cell> {
    let mut accounts = ShardAccounts::new();
    for &(address, amount) in balances {
        let balance = CurrencyCollection::new(amount);
        let account = ShardAccount {
            account: Lazy::new(&OptionalAccount(Some(Account {
                address: IntAddr::Std(address.clone()),
                storage_stat: Default::default(),
                last_trans_lt: u64::from(state.seqno),
                balance: balance.clone(),
                state: AccountState::Uninit,
            })))?,
            last_trans_hash: HashBytes::ZERO,
            last_trans_lt: u64::from(state.seqno),
        };
        accounts.set(
            address.address,
            DepthBalanceInfo {
                split_depth: 0,
                balance,
            },
            account,
        )?;
    }

    state.accounts = Lazy::new(&accounts)?;
    Ok(CellBuilder::build_from(state)?)
}

fn master_state(seqno: u32, shards: &[BlockId]) -> Result<Cell> {
    let descriptions = shards
        .iter()
        .map(|shard| ShardDescription {
            seqno: shard.seqno,
            reg_mc_seqno: seqno,
            start_lt: 0,
            end_lt: 0,
            root_hash: shard.root_hash,
            file_hash: shard.file_hash,
            before_split: false,
            before_merge: false,
            want_split: false,
            want_merge: false,
            nx_cc_updated: false,
            next_catchain_seqno: 0,
            next_validator_shard: shard.shard.prefix(),
            min_ref_mc_seqno: 0,
            gen_utime: 0,
            split_merge_at: None,
            fees_collected: CurrencyCollection::ZERO,
            funds_created: CurrencyCollection::ZERO,
        })
        .collect::<Vec<_>>();
    let extra = McStateExtra {
        shards: ShardHashes::from_shards(shards.iter().map(|id| &id.shard).zip(&descriptions))?,
        config: BlockchainConfig::new_empty(HashBytes::ZERO),
        validator_info: ValidatorInfo {
            validator_list_hash_short: 0,
            catchain_seqno: 0,
            nx_cc_updated: false,
        },
        prev_blocks: Default::default(),
        after_key_block: false,
        last_key_block: None,
        block_create_stats: None,
        global_balance: CurrencyCollection::ZERO,
    };

    Ok(CellBuilder::build_from(ShardStateUnsplit {
        seqno,
        custom: Some(Lazy::new(&extra)?),
        ..Default::default()
    })?)
}

fn state_id(root: &Cell) -> Result<BlockId> {
    let state = root.parse::<ShardStateUnsplit>()?;
    Ok(BlockId {
        shard: state.shard_ident,
        seqno: state.seqno,
        root_hash: *root.repr_hash(),
        file_hash: Boc::file_hash(Boc::encode(root)),
    })
}

fn block(previous: &BlockId, old: &Cell, new: &Cell) -> Result<(BlockId, Vec<u8>)> {
    transition(PrevBlockRef::Single(block_ref(previous)), old, new)
}

const fn block_ref(id: &BlockId) -> BlockRef {
    BlockRef {
        end_lt: 0,
        seqno: id.seqno,
        root_hash: id.root_hash,
        file_hash: id.file_hash,
    }
}

fn transition(previous: PrevBlockRef, old: &Cell, new: &Cell) -> Result<(BlockId, Vec<u8>)> {
    let state = new.parse::<ShardStateUnsplit>()?;
    let after_merge = matches!(&previous, PrevBlockRef::AfterMerge { .. });
    let mut info = BlockInfo {
        seqno: state.seqno,
        shard: state.shard_ident,
        before_split: state.before_split,
        after_split: !after_merge
            && old.parse::<ShardStateUnsplit>()?.shard_ident != state.shard_ident,
        after_merge,
        ..Default::default()
    };
    info.set_prev_ref(&previous);

    let mut old_hashes = HashSet::new();
    let mut pending = vec![old.clone()];
    while let Some(cell) = pending.pop() {
        if old_hashes.insert(*cell.repr_hash()) {
            pending.extend(cell.references().cloned());
        }
    }
    let update = MerkleUpdate::create(old.as_ref(), new.as_ref(), old_hashes).build()?;
    let root = CellBuilder::build_from(Block {
        global_id: 0,
        info: Lazy::new(&info)?,
        value_flow: Lazy::new(&ValueFlow::default())?,
        state_update: LazyExotic::new(&update)?,
        extra: Lazy::new(&BlockExtra::default())?,
    })?;
    let boc = Boc::encode(&root);
    Ok((
        BlockId {
            shard: info.shard,
            seqno: info.seqno,
            root_hash: *root.repr_hash(),
            file_hash: Boc::file_hash(&boc),
        },
        boc,
    ))
}

/// Only roots have database records; descendants exist inside their embedded
/// `BoCs`. Updates must persist any reused descendants that gain a direct reference.
fn write_snapshot(path: &Path, roots: &[(BlockId, Cell)]) -> Result<()> {
    std::fs::create_dir_all(path)?;
    let cells = DB::open_default(path.join("celldb"))?;
    let state = DB::open_default(path.join("state"))?;
    for (id, root) in roots {
        let bare = bare_id(id);
        let constructor: u32 = tl_proto::id!("tonNode.blockIdExt", scheme = "../src/db.tl");
        let mut boxed = constructor.to_le_bytes().to_vec();
        boxed.extend_from_slice(&bare);
        let key = format!("desc{}", STANDARD.encode(Sha256::digest(boxed)));
        let constructor: u32 = tl_proto::id!("db.celldb.value", scheme = "../src/db.tl");
        let mut value = constructor.to_le_bytes().to_vec();
        value.extend_from_slice(&bare);
        value.extend_from_slice(&[0; 64]);
        value.extend_from_slice(root.repr_hash().as_slice());
        cells.put(key, value)?;

        let mut value = (-1_i32).to_le_bytes().to_vec();
        value.extend_from_slice(&1_i32.to_le_bytes());
        value.extend_from_slice(&Boc::encode(root));
        cells.put(root.repr_hash().as_slice(), value)?;
    }

    let constructor: u32 = tl_proto::id!("db.state.key.shardClient", scheme = "../src/db.tl");
    let key = constructor.to_le_bytes();
    let constructor: u32 = tl_proto::id!("db.state.shardClient", scheme = "../src/db.tl");
    let mut value = constructor.to_le_bytes().to_vec();
    value.extend_from_slice(&bare_id(&roots[0].0));
    state.put(Sha256::digest(key), value)?;
    Ok(())
}

fn bare_id(id: &BlockId) -> Vec<u8> {
    let mut value = id.shard.workchain().to_le_bytes().to_vec();
    value.extend_from_slice(&id.shard.prefix().to_le_bytes());
    value.extend_from_slice(&id.seqno.to_le_bytes());
    value.extend_from_slice(id.root_hash.as_slice());
    value.extend_from_slice(id.file_hash.as_slice());
    value
}
