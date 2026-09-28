mod accounts;
mod storage;

use anyhow::{Context, Result};
use axum::body::Body;
use axum::http::Request;
use expect_test::{expect, expect_file};
use http_body_util::BodyExt;
use rston::boc::Boc;
use rston::cell::{CellBuilder, HashBytes, Lazy};
use rston::dict::AugDict;
use rston::merkle::MerkleUpdate;
use rston::models::{
    AccountBlock, AccountBlocks, AccountStatus, AccountStatusChange, ActionPhase, Block,
    BlockExtra, BlockInfo, BouncePhase, ComputePhase, ComputePhaseSkipReason, CreditPhase,
    CurrencyCollection, ExecutedComputePhase, ExtInMsgInfo, ExtOutMsgInfo, HashUpdate, IntMsgInfo,
    MsgInfo, NoFundsBouncePhase, OrdinaryTxInfo, OwnedMessage, ShardIdent, SkippedComputePhase,
    StateInit, StdAddr, StoragePhase, StorageUsedShort, TickTock, TickTockTxInfo, Transaction,
    TxInfo, ValueFlow,
};
use rston::num::{Tokens, VarUint24, VarUint56, VarUint248};
use serde_json::Value;
use tower::ServiceExt;

use super::*;

fn no_account_reads(_: &StdAddr) -> Result<AccountSnapshot> {
    anyhow::bail!("transaction-only subscriptions must not read account state")
}

fn account(index: u8, workchain: i8) -> StdAddr {
    StdAddr::new(workchain, HashBytes([index; 32]))
}

fn storage() -> StoragePhase {
    StoragePhase {
        storage_fees_collected: Tokens::new(17),
        storage_fees_due: Some(Tokens::new(3)),
        status_change: AccountStatusChange::Unchanged,
    }
}

pub(crate) fn transaction(index: u8) -> Result<Transaction> {
    let skipped = ComputePhase::Skipped(SkippedComputePhase {
        reason: ComputePhaseSkipReason::NoGas,
    });
    let info = if index == 1 {
        TxInfo::TickTock(TickTockTxInfo {
            kind: TickTock::Tock,
            storage_phase: storage(),
            compute_phase: skipped,
            action_phase: None,
            aborted: true,
            destroyed: false,
        })
    } else {
        TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: false,
            storage_phase: Some(storage()),
            credit_phase: Some(CreditPhase {
                due_fees_collected: Some(Tokens::new(3)),
                credit: CurrencyCollection::new(1_000_000_000),
            }),
            compute_phase: skipped,
            action_phase: None,
            aborted: true,
            bounce_phase: Some(BouncePhase::NoFunds(NoFundsBouncePhase {
                msg_size: StorageUsedShort {
                    cells: VarUint56::new(1),
                    bits: VarUint56::new(32),
                },
                req_fwd_fees: Tokens::new(120),
            })),
            destroyed: false,
        })
    };
    let mut tx = Transaction {
        account: HashBytes([index; 32]),
        lt: 9_007_199_254_740_999 + u64::from(index),
        prev_trans_hash: HashBytes([4; 32]),
        prev_trans_lt: 9_007_199_254_740_993,
        now: 1_700_000_000,
        out_msg_count: Default::default(),
        orig_status: AccountStatus::Active,
        end_status: AccountStatus::Active,
        in_msg: None,
        out_msgs: Default::default(),
        total_fees: CurrencyCollection::new(12345),
        state_update: Lazy::new(&HashUpdate {
            old: HashBytes([5; 32]),
            new: HashBytes([6; 32]),
        })?,
        info: Lazy::new(&info)?,
    };
    tx.total_fees
        .other
        .as_dict_mut()
        .set(7, VarUint248::from(99_u32))?;

    if index == 2 {
        let body = CellBuilder::build_from(0x8000_0001_u32)?;
        let message = OwnedMessage {
            info: MsgInfo::ExtIn(ExtInMsgInfo {
                dst: account(2, 0).into(),
                ..Default::default()
            }),
            init: Some(StateInit {
                data: Some(body.clone()),
                ..Default::default()
            }),
            body: body.clone().into(),
            layout: None,
        };
        tx.in_msg = Some(CellBuilder::build_from(&message)?);
        let mut value = CurrencyCollection::new(1_234_567_890);
        value.other.as_dict_mut().set(7, VarUint248::from(44_u32))?;
        for (position, info) in [
            MsgInfo::Int(IntMsgInfo {
                src: account(2, 0).into(),
                dst: account(3, 0).into(),
                value,
                bounce: true,
                created_lt: tx.lt + 1,
                created_at: tx.now,
                fwd_fee: Tokens::new(11),
                ..Default::default()
            }),
            MsgInfo::ExtOut(ExtOutMsgInfo {
                src: account(2, 0).into(),
                dst: None,
                created_lt: tx.lt + 2,
                created_at: tx.now,
            }),
        ]
        .into_iter()
        .enumerate()
        {
            let message = OwnedMessage {
                info,
                init: None,
                body: body.clone().into(),
                layout: None,
            };
            tx.out_msgs.set(
                rston::num::Uint15::new(position as u16),
                CellBuilder::build_from(&message)?,
            )?;
        }
        tx.out_msg_count = rston::num::Uint15::new(2);
        tx.info = Lazy::new(&TxInfo::Ordinary(OrdinaryTxInfo {
            credit_first: true,
            storage_phase: Some(storage()),
            credit_phase: None,
            compute_phase: ComputePhase::Executed(ExecutedComputePhase {
                success: true,
                msg_state_used: true,
                account_activated: false,
                gas_fees: Tokens::new(567),
                gas_used: VarUint56::new(678),
                gas_limit: VarUint56::new(1000),
                gas_credit: Some(VarUint24::new(12)),
                mode: -1,
                exit_code: 1,
                exit_arg: Some(-2),
                vm_steps: 37,
                vm_init_state_hash: HashBytes([8; 32]),
                vm_final_state_hash: HashBytes([9; 32]),
            }),
            action_phase: Some(ActionPhase {
                success: true,
                valid: true,
                no_funds: false,
                status_change: AccountStatusChange::Unchanged,
                total_fwd_fees: Some(Tokens::new(42)),
                total_action_fees: Some(Tokens::new(21)),
                result_code: 0,
                result_arg: None,
                total_actions: 2,
                special_actions: 0,
                skipped_actions: 0,
                messages_created: 2,
                action_list_hash: HashBytes([10; 32]),
                total_message_size: StorageUsedShort {
                    cells: VarUint56::new(2),
                    bits: VarUint56::new(64),
                },
            }),
            aborted: false,
            bounce_phase: None,
            destroyed: false,
        }))?;
    }
    Ok(tx)
}

pub(crate) fn block(
    shard: ShardIdent,
    seqno: u32,
    transactions: Vec<Transaction>,
) -> Result<ton_indexer_core::BlockData> {
    let mut accounts = AccountBlocks::new();
    for tx in transactions {
        let mut transactions = AugDict::new();
        transactions.set(tx.lt, tx.total_fees.clone(), Lazy::new(&tx)?)?;
        let account = AccountBlock {
            account: tx.account,
            transactions,
            state_update: tx.state_update,
        };
        accounts.set(tx.account, tx.total_fees, account)?;
    }
    let block = Block {
        global_id: -239,
        info: Lazy::new(&BlockInfo {
            seqno,
            shard,
            ..Default::default()
        })?,
        value_flow: Lazy::new(&ValueFlow::default())?,
        state_update: Lazy::new(&MerkleUpdate::default())?,
        extra: Lazy::new(&BlockExtra {
            account_blocks: Lazy::new(&accounts)?,
            ..Default::default()
        })?,
    };
    let root = CellBuilder::build_from(&block)?;
    let boc = Boc::encode(&root);
    Ok(ton_indexer_core::BlockData::decode(
        ton_indexer_core::BlockId {
            workchain: shard.workchain(),
            shard: shard.prefix(),
            seqno,
            root_hash: ton_indexer_core::Hash256::new(root.repr_hash().0),
            file_hash: ton_indexer_core::Hash256::new(Boc::file_hash(&boc).0),
        },
        &boc,
    )?)
}

fn batch() -> Result<Batch> {
    Ok(Batch::try_new(
        block(ShardIdent::MASTERCHAIN, 42, vec![transaction(1)?])?,
        vec![block(
            ShardIdent::BASECHAIN,
            17,
            vec![transaction(2)?, transaction(3)?],
        )?],
    )?)
}

async fn subscribe_to(hub: &Subscriptions, body: Value) -> Result<Response> {
    Ok(hub
        .clone()
        .router()
        .oneshot(
            Request::post("/api/streaming/sse")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))?,
        )
        .await?)
}

async fn frame(body: &mut Body) -> Result<String> {
    let frame = tokio::time::timeout(Duration::from_secs(1), body.frame())
        .await?
        .context("unexpected stream end")??;
    let bytes = frame
        .into_data()
        .map_err(|_| anyhow::anyhow!("unexpected trailers"))?;
    Ok(String::from_utf8(bytes.to_vec())?)
}

#[test]
fn transaction_messages_preserve_extra_flags_and_forwarding_fees() -> Result<()> {
    let mut tx = transaction(2)?;
    let key = rston::num::Uint15::new(0);
    let mut message = tx.out_msgs.get(key)?.unwrap().parse::<OwnedMessage>()?;
    let MsgInfo::Int(info) = &mut message.info else {
        anyhow::bail!("expected an internal message");
    };
    info.extra_flags = rston::models::MessageExtraFlags::all();
    tx.out_msgs.set(key, CellBuilder::build_from(message)?)?;

    let converted = transaction::convert_cell(0, &Lazy::new(&tx)?, &tx)?;
    let incoming = converted.in_msg.unwrap();
    let internal = &converted.out_msgs[0];
    let external = &converted.out_msgs[1];
    expect![[r#"
        external-in: None/None/None
        internal: Some("3")/Some("0")/Some("11")
        external-out: None/None/None
    "#]]
    .assert_eq(&format!(
        "external-in: {:?}/{:?}/{:?}\ninternal: {:?}/{:?}/{:?}\nexternal-out: {:?}/{:?}/{:?}\n",
        incoming.extra_flags,
        incoming.ihr_fee,
        incoming.fwd_fee,
        internal.extra_flags,
        internal.ihr_fee,
        internal.fwd_fee,
        external.extra_flags,
        external.ihr_fee,
        external.fwd_fee,
    ));
    Ok(())
}

#[tokio::test]
async fn finalized_batches_emit_single_transactions_and_filter_account_addresses() -> Result<()> {
    let hub = Subscriptions::default();
    let batch = batch()?;
    hub.publish(&batch, no_account_reads)?;
    let friendly = account(2, 0).display_base64(true).to_string();
    let response = subscribe_to(
        &hub,
        json!({"addresses": [friendly, account(2, 0).to_string()]}),
    )
    .await?;
    expect![["200 OK text/event-stream no-cache no"]].assert_eq(&format!(
        "{} {} {} {}",
        response.status(),
        response.headers()["content-type"].to_str()?,
        response.headers()["cache-control"].to_str()?,
        response.headers()["x-accel-buffering"].to_str()?
    ));
    let mut body = response.into_body();
    expect![["data: {\"status\":\"subscribed\"}\n\n"]].assert_eq(&frame(&mut body).await?);
    expect![["true"]].assert_eq(
        &tokio::time::timeout(Duration::from_millis(10), body.frame())
            .await
            .is_err()
            .to_string(),
    );

    hub.publish(&batch, no_account_reads)?;
    let text = frame(&mut body).await?;
    let event: Value = serde_json::from_str(
        text.trim()
            .strip_prefix("data: ")
            .context("missing SSE data")?,
    )?;
    let _: toncenter::v3::responses::Transaction =
        serde_json::from_value(event["transaction"].clone())?;
    expect_file!["snapshots/transaction.json"].assert_eq(&serde_json::to_string_pretty(&event)?);
    // Outgoing destinations do not match the subscription; repeated address
    // forms do not duplicate an event. There is no replay on a new connection.
    expect![["true"]].assert_eq(
        &tokio::time::timeout(Duration::from_millis(10), body.frame())
            .await
            .is_err()
            .to_string(),
    );
    let mut late = subscribe_to(&hub, json!({"addresses": [account(2, 0).to_string()]}))
        .await?
        .into_body();
    frame(&mut late).await?;
    expect![["true"]].assert_eq(
        &tokio::time::timeout(Duration::from_millis(10), late.frame())
            .await
            .is_err()
            .to_string(),
    );
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn masterchain_and_aborted_transactions_preserve_their_own_block_coordinates() -> Result<()> {
    let hub = Subscriptions::default();
    let mut body = subscribe_to(
        &hub,
        json!({
            "types": ["transactions"], "min_finality": "finalized",
            "addresses": [account(1, -1).to_string(), account(3, 0).to_string()],
        }),
    )
    .await?
    .into_body();
    frame(&mut body).await?;
    hub.publish(&batch()?, no_account_reads)?;
    let mut events = Vec::new();
    for _ in 0..2 {
        let text = frame(&mut body).await?;
        events.push(serde_json::from_str::<Value>(
            text.trim().strip_prefix("data: ").context("missing data")?,
        )?);
    }
    expect_file!["snapshots/masterchain-and-aborted.json"]
        .assert_eq(&serde_json::to_string_pretty(&events)?);
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn rejects_unsupported_subscriptions() -> Result<()> {
    let hub = Subscriptions::default();
    let address = account(2, 0).to_string();
    let mut rows = Vec::new();
    for body in [
        json!({"addresses": []}),
        json!({"addresses": ["bad"]}),
        json!({"addresses": vec![address.clone(); MAX_ADDRESSES + 1]}),
        json!({"addresses": [address], "types": ["actions"]}),
        json!({"addresses": [address], "types": []}),
        json!({"addresses": [address], "min_finality": "confirmed"}),
        json!({"addresses": [address], "include_metadata": true}),
        json!({"addresses": [address], "include_code_data": "false"}),
    ] {
        let response = subscribe_to(&hub, body).await?;
        rows.push(format!(
            "{} {}",
            response.status(),
            String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?
        ));
    }
    let response = hub
        .clone()
        .router()
        .oneshot(
            Request::post("/api/streaming/sse")
                .header("content-type", "application/json")
                .header("last-event-id", "42")
                .body(Body::from(json!({"addresses": [address]}).to_string()))?,
        )
        .await?;
    rows.push(format!(
        "{} {}",
        response.status(),
        String::from_utf8(response.into_body().collect().await?.to_bytes().to_vec())?
    ));
    expect![[r#"
        400 Bad Request {"error":"expected_1_to_100_addresses"}
        400 Bad Request {"error":"invalid_address"}
        400 Bad Request {"error":"expected_1_to_100_addresses"}
        400 Bad Request {"error":"invalid_subscription"}
        400 Bad Request {"error":"expected_nonempty_types"}
        400 Bad Request {"error":"only_finalized_events_supported"}
        400 Bad Request {"error":"invalid_subscription"}
        400 Bad Request {"error":"invalid_subscription"}
        400 Bad Request {"error":"replay_not_supported"}"#]]
    .assert_eq(&rows.join("\n"));
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn overflow_closes_only_the_slow_subscriber() -> Result<()> {
    let hub = Subscriptions::default();
    let request = json!({"addresses": [account(2, 0).to_string()]});
    let mut slow = subscribe_to(&hub, request.clone()).await?.into_body();
    let mut fast = subscribe_to(&hub, request).await?.into_body();
    frame(&mut slow).await?;
    frame(&mut fast).await?;
    let batch = batch()?;
    for _ in 0..=QUEUE_EVENTS {
        hub.publish(&batch, no_account_reads)?;
        frame(&mut fast).await?;
    }
    expect![["data: {\"error\":\"slow_consumer\",\"type\":\"error\"}\n\n"]]
        .assert_eq(&frame(&mut slow).await?);
    expect![["true"]].assert_eq(&slow.frame().await.is_none().to_string());
    hub.publish(&batch, no_account_reads)?;
    frame(&mut fast).await?;
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn large_messages_hit_the_byte_budget_and_consumption_releases_it() -> Result<()> {
    let hub = Subscriptions::default();
    let request = json!({"addresses": [account(3, 0).to_string()]});
    let mut slow = subscribe_to(&hub, request.clone()).await?.into_body();
    let mut fast = subscribe_to(&hub, request).await?.into_body();
    frame(&mut slow).await?;
    frame(&mut fast).await?;

    let mut body = CellBuilder::build_from(0_u32)?;
    for byte in 0..700_u32 {
        let mut cell = CellBuilder::new();
        cell.store_raw(&[byte as u8; 127], 1016)?;
        cell.store_reference(body)?;
        body = cell.build()?;
    }
    let mut tx = transaction(3)?;
    tx.in_msg = Some(CellBuilder::build_from(OwnedMessage {
        info: MsgInfo::Int(IntMsgInfo {
            src: account(2, 0).into(),
            dst: account(3, 0).into(),
            value: CurrencyCollection::new(1_000_000_000),
            ..Default::default()
        }),
        init: None,
        body: body.into(),
        layout: None,
    })?);
    let batch = Batch::try_new(
        block(ShardIdent::MASTERCHAIN, 42, Vec::new())?,
        vec![block(ShardIdent::BASECHAIN, 17, vec![tx])?],
    )?;

    // Fewer events than the count limit fill the byte budget. A reader that
    // drains its queue continues across several full byte budgets.
    for _ in 0..QUEUE_EVENTS - 1 {
        hub.publish(&batch, no_account_reads)?;
        frame(&mut fast).await?;
    }
    expect![["data: {\"error\":\"slow_consumer\",\"type\":\"error\"}\n\n"]]
        .assert_eq(&frame(&mut slow).await?);
    expect![["true"]].assert_eq(&slow.frame().await.is_none().to_string());
    drop(hub);
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn keepalive_failure_and_shutdown_end_idle_streams() -> Result<()> {
    let hub = Subscriptions::default();
    let request = json!({"addresses": [account(2, 0).to_string()]});
    let mut body = subscribe_to(&hub, request.clone()).await?.into_body();
    frame(&mut body).await?;
    tokio::time::advance(Duration::from_secs(15)).await;
    expect![[": keepalive\n\n"]].assert_eq(&frame(&mut body).await?);
    hub.fail();
    expect![["data: {\"error\":\"stream_failed\",\"type\":\"error\"}\n\n"]]
        .assert_eq(&frame(&mut body).await?);
    expect![["true"]].assert_eq(&body.frame().await.is_none().to_string());
    let mut body = subscribe_to(&hub, request).await?.into_body();
    frame(&mut body).await?;
    hub.close();
    expect![["true"]].assert_eq(&body.frame().await.is_none().to_string());
    drop(hub);
    Ok(())
}

#[tokio::test]
async fn disconnected_clients_release_connection_slots() -> Result<()> {
    let hub = Subscriptions::default();
    let request = json!({"addresses": [account(2, 0).to_string()]});
    let mut responses = Vec::new();
    for _ in 0..MAX_SUBSCRIBERS {
        responses.push(subscribe_to(&hub, request.clone()).await?);
    }
    let full = subscribe_to(&hub, request.clone()).await?;
    let disconnected = responses.pop().context("missing open connection")?;
    expect![["200 OK"]].assert_eq(&disconnected.status().to_string());
    drop(disconnected);
    let available = subscribe_to(&hub, request).await?;
    expect![["503 Service Unavailable\n200 OK"]].assert_eq(&format!(
        "{}\n{}",
        full.status(),
        available.status()
    ));
    drop(hub);
    Ok(())
}
