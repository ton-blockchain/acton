use std::path::Path;

use expect_test::expect;
use rston::dict::AugDict;
use rston::merkle::MerkleUpdate;
use rston::models::{
    AccountBlock, AccountBlocks, AccountStatus, AccountStatusChange, Block, BlockExtra, BlockId,
    BlockInfo, ComputePhase, ComputePhaseSkipReason, CurrencyCollection, HashUpdate, IntMsgInfo,
    OrdinaryTxInfo, OwnedMessage, ShardIdent, SkippedComputePhase, StoragePhase, ValueFlow,
};
use rston::num::{Tokens, Uint15};

use super::*;

fn transaction(lt: u64, previous: Option<&Lazy<Transaction>>) -> Result<Lazy<Transaction>> {
    let mut out_msgs = rston::dict::Dict::new();
    out_msgs.set(
        Uint15::new(0),
        CellBuilder::build_from(OwnedMessage {
            info: MsgInfo::Int(IntMsgInfo {
                src: StdAddr::new(0, HashBytes([7; 32])).into(),
                dst: StdAddr::new(0, HashBytes([8; 32])).into(),
                value: CurrencyCollection::new(1_000_000_000),
                fwd_fee: Tokens::new(13),
                extra_flags: rston::models::MessageExtraFlags::all(),
                created_lt: lt + 1,
                ..Default::default()
            }),
            init: None,
            body: CellBuilder::build_from(0x1234_5678_u32)?.into(),
            layout: None,
        })?,
    )?;

    Ok(Lazy::new(&Transaction {
        account: HashBytes([7; 32]),
        lt,
        prev_trans_lt: previous
            .map(|tx| tx.load().map(|tx| tx.lt))
            .transpose()?
            .unwrap_or(0),
        prev_trans_hash: previous.map_or(HashBytes::ZERO, |tx| *tx.inner().repr_hash()),
        now: 1_700_000_000,
        out_msg_count: Uint15::new(1),
        orig_status: AccountStatus::Active,
        end_status: AccountStatus::Active,
        in_msg: None,
        out_msgs,
        total_fees: CurrencyCollection::new(123),
        state_update: Lazy::new(&HashUpdate {
            old: HashBytes([1; 32]),
            new: HashBytes([2; 32]),
        })?,
        info: Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: true,
            storage_phase: Some(StoragePhase {
                storage_fees_collected: Tokens::new(17),
                storage_fees_due: None,
                status_change: AccountStatusChange::Unchanged,
            }),
            credit_phase: None,
            compute_phase: ComputePhase::Skipped(SkippedComputePhase {
                reason: ComputePhaseSkipReason::NoGas,
            }),
            action_phase: None,
            aborted: true,
            bounce_phase: None,
            destroyed: false,
        }))?,
    })?)
}

fn block(
    path: &Path,
    shard: ShardIdent,
    seqno: u32,
    transactions: &[Lazy<Transaction>],
) -> Result<(BlockId, Cell)> {
    let mut dictionary = AugDict::new();
    for cell in transactions {
        let tx = cell.load()?;
        dictionary.set(tx.lt, tx.total_fees, cell.clone())?;
    }
    let first = transactions[0].load()?;
    let mut accounts = AccountBlocks::new();
    accounts.set(
        first.account,
        CurrencyCollection::ZERO,
        AccountBlock {
            account: first.account,
            transactions: dictionary,
            state_update: first.state_update,
        },
    )?;
    let root = CellBuilder::build_from(Block {
        global_id: -239,
        info: Lazy::new(&BlockInfo {
            seqno,
            shard,
            start_lt: first.lt,
            end_lt: transactions.last().unwrap().load()?.lt + 2,
            ..Default::default()
        })?,
        value_flow: Lazy::new(&ValueFlow::default())?,
        state_update: Lazy::new(&MerkleUpdate::default())?,
        extra: Lazy::new(&BlockExtra {
            account_blocks: Lazy::new(&accounts)?,
            ..Default::default()
        })?,
    })?;
    let bytes = Boc::encode(&root);
    std::fs::write(path, &bytes)?;
    Ok((
        BlockId {
            shard,
            seqno,
            root_hash: *root.repr_hash(),
            file_hash: Boc::file_hash(&bytes),
        },
        root,
    ))
}

#[test]
fn history_survives_restart_and_pages_across_shard_splits_and_merges() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let blocks = directory.path().join("blocks");
    std::fs::create_dir(&blocks)?;
    let account = StdAddr::new(0, HashBytes([7; 32]));
    let first = transaction(10, None)?;
    let second = transaction(21, Some(&first))?;
    let third = transaction(24, Some(&second))?;
    let last = transaction(31, Some(&third))?;
    let (left, _) = ShardIdent::BASECHAIN.split().unwrap();
    block(
        &blocks.join("parent.boc"),
        ShardIdent::BASECHAIN,
        1,
        &[first],
    )?;
    let child = block(&blocks.join("child.boc"), left, 2, &[second, third])?;
    let merged = block(
        &blocks.join("merged.boc"),
        ShardIdent::BASECHAIN,
        3,
        std::slice::from_ref(&last),
    )?;
    std::fs::write(blocks.join("ignored.proof.boc"), b"not a block")?;
    let path = directory.path().join("index");
    let index = BlockIndex::open(&path)?;
    index.import_directory(&blocks)?;
    drop(index);

    let index = BlockIndex::open(&path)?;
    index.import_directory(&blocks)?;
    let cursor = (31, *last.inner().repr_hash());
    let page = read_history(&index, &account, cursor, 2, 3)?;
    let next = (
        24,
        STANDARD
            .decode(&page[1].transaction_id.hash)?
            .try_into()
            .map(HashBytes)
            .unwrap(),
    );
    let continued = read_history(&index, &account, next, 10, 3)?;
    let all = read_history(&index, &account, cursor, 100, 3)?;
    let lt = |page: &[wire::Transaction]| {
        page.iter()
            .map(|tx| tx.transaction_id.lt.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    let roundtrip = all.iter().all(|tx| {
        Boc::decode_base64(&tx.data)
            .is_ok_and(|cell| STANDARD.encode(cell.repr_hash()) == tx.transaction_id.hash)
    });
    let decoded = Boc::decode_base64(&all[0].data)?.parse::<Transaction>()?;
    let outgoing = decoded
        .out_msgs
        .get(Uint15::new(0))?
        .unwrap()
        .parse::<OwnedMessage>()?;
    let MsgInfo::Int(outgoing) = outgoing.info else {
        anyhow::bail!("expected an internal message");
    };

    let partial = BlockIndex::open(&directory.path().join("partial"))?;
    partial.insert(child.0, &child.1, &blocks.join("child.boc"))?;
    partial.insert(merged.0, &merged.1, &blocks.join("merged.boc"))?;
    let partial_page = read_history(&partial, &account, cursor, 100, 3)?;
    let missing = read_history(&partial, &account, (10, HashBytes::ZERO), 10, 3).unwrap_err();
    let wrong_hash = read_history(&index, &account, (31, HashBytes::ZERO), 10, 3).unwrap_err();
    let future = read_history(&index, &account, cursor, 10, 2).unwrap_err();
    let empty = read_history(&index, &account, (0, HashBytes::ZERO), 10, 3)?;
    let other_workchain =
        read_history(&index, &StdAddr::new(-1, account.address), cursor, 10, 3).unwrap_err();

    expect![[r"
        first page: 31,24
        inclusive cursor: 24,21,10
        full chain: 31,24,21,10
        full BoCs match hashes: true
        fees: 136/17/119
        outgoing fees: 13/0
        outgoing BoC flags: 3
        partial history: 31,24,21
        missing: 404, wrong hash: 400, uncommitted: 409, other workchain: 404
        empty account: 0
    "]].assert_eq(&format!(
        "first page: {}\ninclusive cursor: {}\nfull chain: {}\nfull BoCs match hashes: {roundtrip}\nfees: {}/{}/{}\noutgoing fees: {}/{}\noutgoing BoC flags: {}\npartial history: {}\nmissing: {}, wrong hash: {}, uncommitted: {}, other workchain: {}\nempty account: {}\n",
        lt(&page),
        lt(&continued),
        lt(&all),
        all[0].fee,
        all[0].storage_fee,
        all[0].other_fee,
        all[0].out_msgs[0].fwd_fee,
        all[0].out_msgs[0].ihr_fee,
        outgoing.extra_flags.bits(),
        lt(&partial_page),
        missing.downcast_ref::<ApiError>().unwrap().status.as_u16(),
        wrong_hash.downcast_ref::<ApiError>().unwrap().status.as_u16(),
        future.downcast_ref::<ApiError>().unwrap().status.as_u16(),
        other_workchain.downcast_ref::<ApiError>().unwrap().status.as_u16(),
        empty.len(),
    ));

    std::fs::write(blocks.join("merged.boc"), b"corrupted")?;
    expect![["history block file hash mismatch"]].assert_eq(
        read_history(&index, &account, cursor, 10, 3)
            .unwrap_err()
            .to_string()
            .split(':')
            .next()
            .unwrap(),
    );
    Ok(())
}

#[test]
fn query_accepts_v2_defaults_and_rejects_unsupported_filters() -> Result<()> {
    let address = StdAddr::new(0, HashBytes([7; 32]))
        .display_base64(true)
        .to_string();
    let mut outcomes = Vec::new();
    for fields in [
        serde_json::json!({}),
        serde_json::json!({"limit":"100", "archival":"false", "to_lt":"0"}),
        serde_json::json!({"lt":"31", "hash": STANDARD.encode([1; 32])}),
        serde_json::json!({"lt":"31", "hash": HashBytes([1; 32]).to_string()}),
        serde_json::json!({"lt":"31"}),
        serde_json::json!({"limit":0}),
        serde_json::json!({"limit":101}),
        serde_json::json!({"archival":true}),
        serde_json::json!({"to_lt":"10"}),
    ] {
        let mut fields = fields;
        fields["address"] = address.clone().into();
        let query = HistoryQuery::parse(serde_json::from_value(fields)?);
        outcomes.push(match query {
            Ok(query) => format!(
                "ok: limit={}, cursor={}",
                query.limit,
                query.cursor.is_some()
            ),
            Err(error) => format!("{}: {}", error.status.as_u16(), error.message),
        });
    }
    expect![[r"
        ok: limit=10, cursor=false
        ok: limit=100, cursor=false
        ok: limit=10, cursor=true
        ok: limit=10, cursor=true
        400: lt and hash must be supplied together
        400: limit must be between 1 and 100
        400: limit must be between 1 and 100
        400: archival is not supported
        400: to_lt is not supported
    "]]
    .assert_eq(&format!("{}\n", outcomes.join("\n")));
    Ok(())
}
