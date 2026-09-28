use rston::models::{CurrencyCollection, IntMsgInfo};
use rston::num::Uint15;

use super::*;

fn trace_request(boc: &str, timeout_ms: u64) -> Request<Body> {
    Request::post("/api/sendAndWaitTrace")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"boc": boc, "timeout_ms": timeout_ms}).to_string(),
        ))
        .unwrap()
}

fn internal(source: u8, destination: u8, lt: u64, bounced: bool) -> anyhow::Result<Cell> {
    Ok(CellBuilder::build_from(OwnedMessage {
        info: MsgInfo::Int(IntMsgInfo {
            src: StdAddr::new(0, HashBytes([source; 32])).into(),
            dst: StdAddr::new(0, HashBytes([destination; 32])).into(),
            value: CurrencyCollection::new(100_000_000),
            created_lt: lt,
            bounce: !bounced,
            bounced,
            ..Default::default()
        }),
        init: None,
        body: Cell::default().into(),
        layout: None,
    })?)
}

fn traced_transaction(
    account: u8,
    lt: u64,
    incoming: Cell,
    outgoing: &[Cell],
) -> anyhow::Result<Transaction> {
    let mut tx = transaction(account)?;
    tx.lt = lt;
    tx.prev_trans_lt = 0;
    tx.in_msg = Some(incoming);
    tx.out_msgs = Default::default();
    for (index, message) in outgoing.iter().enumerate() {
        tx.out_msgs
            .set(Uint15::new(u16::try_from(index)?), message)?;
    }
    tx.out_msg_count = Uint15::new(u16::try_from(outgoing.len())?);
    // All child fixtures use ordinary aborted transactions, including a bounce.
    if account != 2 {
        tx.info = transaction(3)?.info;
    }
    Ok(tx)
}

#[tokio::test]
async fn trace_waits_for_every_branch_and_bounce_while_transaction_wait_finishes_at_root()
-> anyhow::Result<()> {
    let confirmations = Confirmations::default();
    let started = Arc::new(Semaphore::new(0));
    let signal = started.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |_| {
            signal.add_permits(1);
            Box::pin(async { Ok(()) })
        }),
    );
    let capacity = app.capacity.clone();
    let router = app.router();
    let boc = Boc::encode_base64(message());
    let mut first = tokio::spawn(router.clone().oneshot(trace_request(&boc, 30_000)));
    let second = tokio::spawn(router.clone().oneshot(trace_request(&boc, 30_000)));
    let root_wait = tokio::spawn(router.oneshot(request(json!({"boc": boc}))));
    started.acquire_many(3).await?.forget();

    let left = internal(2, 3, 101, false)?;
    let right = internal(2, 4, 102, false)?;
    let bounce = internal(3, 2, 201, true)?;
    let root = traced_transaction(2, 100, message(), &[left.clone(), right.clone()])?;
    let root_hash = STANDARD.encode(CellBuilder::build_from(&root)?.repr_hash());
    let root_batch = committed(root, ShardIdent::BASECHAIN)?;
    confirmations.publish(&root_batch)?;
    let root_reply = response(root_wait.await??).await;
    let after_root = tokio::time::timeout(Duration::from_millis(10), &mut first)
        .await
        .is_err();

    // An unrelated receiver cannot close either of the pending message hashes.
    let unrelated = traced_transaction(5, 110, internal(6, 5, 103, false)?, &[])?;
    confirmations.publish(&committed(unrelated, ShardIdent::BASECHAIN)?)?;
    let after_unrelated = tokio::time::timeout(Duration::from_millis(10), &mut first)
        .await
        .is_err();

    let rejected = traced_transaction(3, 200, left, std::slice::from_ref(&bounce))?;
    confirmations.publish(&committed(rejected, ShardIdent::BASECHAIN)?)?;
    let after_bounce_emitted = tokio::time::timeout(Duration::from_millis(10), &mut first)
        .await
        .is_err();
    let other_branch = traced_transaction(4, 210, right, &[])?;
    confirmations.publish(&committed(other_branch, ShardIdent::BASECHAIN)?)?;
    let after_other_branch = tokio::time::timeout(Duration::from_millis(10), &mut first)
        .await
        .is_err();

    // Re-observing the root must not re-add already consumed outgoing messages.
    confirmations.publish(&root_batch)?;
    let bounced = traced_transaction(2, 300, bounce, &[])?;
    confirmations.publish(&committed(bounced, ShardIdent::BASECHAIN)?)?;
    let first = response(tokio::time::timeout(Duration::from_secs(3), first).await???).await;
    let second = response(second.await??).await;
    let summary = json!({
        "pending": [after_root, after_unrelated, after_bounce_emitted, after_other_branch],
        "transaction_wait_status": root_reply["status"],
        "response": first,
        "same_trace_for_both_waiters": first == second,
        "root_transaction_hash": root_hash,
        "submission_slots": capacity.available_permits(),
        "observation_slots": available_observations(&confirmations),
    });
    expect_file!["../snapshots/trace-branches.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&summary)?));
    Ok(())
}

#[tokio::test]
async fn terminal_outputs_and_children_in_the_same_batch_complete_the_trace() -> anyhow::Result<()>
{
    let mut outcomes = Vec::new();
    for mode in [
        "root_only",
        "external_output",
        "reversed_accounts",
        "masterchain_child",
    ] {
        let confirmations = Confirmations::default();
        let mut outputs = Vec::new();
        if mode == "external_output" {
            outputs.push(transaction(2)?.out_msgs.get(Uint15::new(1))?.unwrap());
        } else if mode == "reversed_accounts" || mode == "masterchain_child" {
            outputs.push(internal(2, 1, 101, false)?);
        }
        let mut child = None;
        if mode == "masterchain_child" {
            let mut incoming = outputs[0].parse::<OwnedMessage>()?;
            let MsgInfo::Int(info) = &mut incoming.info else {
                unreachable!()
            };
            info.dst = StdAddr::new(-1, HashBytes([1; 32])).into();
            outputs[0] = CellBuilder::build_from(incoming)?;
        }
        if mode == "reversed_accounts" || mode == "masterchain_child" {
            child = Some(traced_transaction(1, 200, outputs[0].clone(), &[])?);
        }
        let root = traced_transaction(2, 100, message(), &outputs)?;
        let expected_hash = STANDARD.encode(CellBuilder::build_from(&root)?.repr_hash());
        // Account dictionaries and masterchain-first batches expose the child first.
        let (masterchain, shard) = if mode == "masterchain_child" {
            (vec![child.unwrap()], vec![root])
        } else {
            (vec![], std::iter::once(root).chain(child).collect())
        };
        let batch = Batch::try_new(
            block(ShardIdent::MASTERCHAIN, 42, masterchain)?,
            vec![block(ShardIdent::BASECHAIN, 17, shard)?],
        )?;
        let observer = confirmations.clone();
        let app = service(
            confirmations.clone(),
            Arc::new(move |_| {
                observer.publish(&batch).unwrap();
                // A confirmed trace wins over a later transport failure too.
                Box::pin(async { anyhow::bail!("later peer failed") })
            }),
        )
        .router();
        let reply = response(
            tokio::time::timeout(
                Duration::from_secs(3),
                app.oneshot(trace_request(&Boc::encode_base64(message()), 30_000)),
            )
            .await??,
        )
        .await;
        outcomes.push(json!({
            "mode": mode,
            "response": reply,
            "expected_root_hash": expected_hash,
            "observation_slots": available_observations(&confirmations),
        }));
    }
    expect_file!["../snapshots/trace-terminal.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&outcomes)?));
    Ok(())
}

#[tokio::test]
async fn incomplete_traces_timeout_and_release_state_on_shutdown_gap_or_cancellation()
-> anyhow::Result<()> {
    let mut outcomes = Vec::new();
    for mode in ["timeout", "shutdown", "publication_gap", "cancel"] {
        let confirmations = Confirmations::default();
        let root = committed(transaction(2)?, ShardIdent::BASECHAIN)?;
        let started = Arc::new(Notify::new());
        let signal = started.clone();
        let observer = confirmations.clone();
        let app = service(
            confirmations.clone(),
            Arc::new(move |_| {
                observer.publish(&root).unwrap();
                signal.notify_one();
                Box::pin(async { Ok(()) })
            }),
        )
        .router();
        let task = tokio::spawn(app.oneshot(trace_request(&Boc::encode_base64(message()), 1000)));
        started.notified().await;
        match mode {
            "timeout" => {
                tokio::time::pause();
                tokio::time::advance(Duration::from_secs(1)).await;
            }
            "shutdown" => confirmations.close(),
            "publication_gap" => confirmations.fail(),
            "cancel" => task.abort(),
            _ => unreachable!(),
        }
        let reply = if mode == "cancel" {
            json!({"cancelled": task.await.unwrap_err().is_cancelled()})
        } else {
            response(task.await??).await
        };
        if mode == "timeout" {
            tokio::time::resume();
        }
        outcomes.push(json!({
            "mode": mode,
            "response": reply,
            "observation_slots": available_observations(&confirmations),
        }));
    }
    expect_file!["../snapshots/trace-failures.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&outcomes)?));
    Ok(())
}

#[tokio::test]
async fn trace_timeout_defaults_to_two_minutes_and_accepts_up_to_ten_minutes() -> anyhow::Result<()>
{
    let confirmations = Confirmations::default();
    let started = Arc::new(Notify::new());
    let signal = started.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |_| {
            signal.notify_one();
            Box::pin(async { Ok(()) })
        }),
    )
    .router();
    let boc = Boc::encode_base64(message());
    let default_request = Request::post("/api/sendAndWaitTrace")
        .header("content-type", "application/json")
        .body(Body::from(json!({"boc": boc}).to_string()))?;
    let mut task = tokio::spawn(app.clone().oneshot(default_request));
    started.notified().await;
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(30)).await;
    let pending_after_thirty_seconds = tokio::time::timeout(Duration::from_millis(1), &mut task)
        .await
        .is_err();
    tokio::time::advance(Duration::from_secs(90)).await;
    let default_reply = response(task.await??).await;
    tokio::time::resume();

    let mut rejected = Vec::new();
    for timeout in [999, 600_001] {
        rejected.push(response(app.clone().oneshot(trace_request(&boc, timeout)).await?).await);
    }
    let task = tokio::spawn(app.oneshot(trace_request(&boc, 600_000)));
    started.notified().await;
    confirmations.publish(&committed(
        traced_transaction(2, 100, message(), &[])?,
        ShardIdent::BASECHAIN,
    )?)?;
    let maximum_reply = response(task.await??).await;
    let summary = json!({
        "pending_after_thirty_seconds": pending_after_thirty_seconds,
        "default_timeout_response": default_reply,
        "rejected_timeouts": rejected,
        "maximum_timeout_status": maximum_reply["status"],
        "observation_slots": available_observations(&confirmations),
    });
    expect_file!["../snapshots/trace-timeouts.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&summary)?));
    Ok(())
}

#[tokio::test]
async fn oversized_pending_trace_fails_without_claiming_completion() -> anyhow::Result<()> {
    let outputs = (0..16_385)
        .map(|index| internal(2, 3, 101 + index, false))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let batch = committed(
        traced_transaction(2, 100, message(), &outputs)?,
        ShardIdent::BASECHAIN,
    )?;
    let confirmations = Confirmations::default();
    let observer = confirmations.clone();
    let app = service(
        confirmations.clone(),
        Arc::new(move |_| {
            observer.publish(&batch).unwrap();
            Box::pin(async { Ok(()) })
        }),
    )
    .router();
    let reply = response(
        app.oneshot(trace_request(&Boc::encode_base64(message()), 30_000))
            .await?,
    )
    .await;
    let summary = json!({
        "response": reply,
        "observation_slots": available_observations(&confirmations),
    });
    expect_file!["../snapshots/trace-limit.json"]
        .assert_eq(&format!("{}\n", serde_json::to_string_pretty(&summary)?));
    Ok(())
}
